// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared Starlark parsing helpers for the Bazel and Buck parsers.
//!
//! Both build systems use the same Starlark (Python-like) dialect, so they share a
//! single parse entry point. The notable behavior here is [`parse_with_repair`]: a
//! bounded, post-failure recovery pass for the common real-world malformation of a
//! missing comma between arguments on separate lines. When recovery succeeds, callers
//! record a [`PARSE_RECOVERY_KEY`] breadcrumb on the produced packages so the result
//! stays auditable without masquerading as a scan error.

use serde_json::Value as JsonValue;
use starlark_syntax::syntax::{AstModule, Dialect};

use crate::models::PackageData;

/// `extra_data` key recording that a package was extracted from input recovered by a
/// fallback parse repair (rather than a clean parse).
pub(crate) const PARSE_RECOVERY_KEY: &str = "provenant_parse_recovery";

/// Recovery note used when a missing argument/element separator was reinserted.
pub(crate) const RECOVERY_MISSING_SEPARATOR: &str = "inserted-missing-argument-separator";

/// Parse Starlark `content`, falling back to a conservative comma repair if the first
/// parse fails. Returns the module and whether the repair pass was the one that
/// succeeded, so callers can annotate recovered packages.
pub(crate) fn parse_with_repair(
    filename: &str,
    content: String,
) -> Result<(AstModule, bool), String> {
    let dialect = Dialect {
        enable_top_level_stmt: true,
        ..Dialect::Standard
    };
    match AstModule::parse(filename, content.clone(), &dialect) {
        Ok(module) => Ok((module, false)),
        Err(first_error) => {
            // Real-world vendored BUILD/BUCK files occasionally omit a comma between
            // arguments on separate lines. Retry once with a conservative repair so a
            // single upstream typo does not cost the whole file's package extraction.
            // The repair only ever runs on content that already failed to parse, so it
            // cannot alter the result for any well-formed file.
            let repaired = repair_missing_argument_commas(&content);
            if repaired != content
                && let Ok(module) = AstModule::parse(filename, repaired, &dialect)
            {
                return Ok((module, true));
            }
            Err(first_error.to_string())
        }
    }
}

/// Record an auditable breadcrumb on a package extracted from repaired input.
pub(crate) fn mark_parse_recovery(package: &mut PackageData, note: &str) {
    package
        .extra_data
        .get_or_insert_with(Default::default)
        .insert(
            PARSE_RECOVERY_KEY.to_string(),
            JsonValue::String(note.to_string()),
        );
}

/// Quote/comment-aware repair that inserts a missing comma between two arguments or
/// collection elements that sit on separate lines inside `()`, `[]`, or `{}`.
///
/// Deliberately conservative: it only acts at a line boundary where a completed value
/// is followed by the start of a new element, never inside strings, and never when the
/// next line continues an expression (operator, closer, attribute access, or a
/// comprehension/conditional keyword). Because it runs only as a post-failure fallback,
/// any false positive is bounded to input that was already unparseable.
pub(crate) fn repair_missing_argument_commas(content: &str) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum StrKind {
        Single,
        Double,
        TripleSingle,
        TripleDouble,
    }

    struct LineMeta {
        depth_before: i32,
        starts_in_string: bool,
        ends_in_string: bool,
        last_sig: Option<char>,
        last_sig_idx: Option<usize>,
        first_sig: Option<char>,
        first_word: String,
    }

    fn leading_word(line: &str) -> String {
        line.trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    }

    fn is_value_end(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || matches!(c, '\'' | '"' | ']' | ')' | '}')
    }

    fn is_element_start(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || matches!(c, '\'' | '"' | '[' | '(' | '{')
    }

    fn is_continuation_keyword(word: &str) -> bool {
        matches!(
            word,
            "if" | "else" | "elif" | "for" | "and" | "or" | "not" | "in"
        )
    }

    let raw_lines: Vec<&str> = content.split('\n').collect();
    let mut metas: Vec<LineMeta> = Vec::with_capacity(raw_lines.len());
    let mut depth: i32 = 0;
    let mut string: Option<StrKind> = None;

    for line in &raw_lines {
        let depth_before = depth;
        let starts_in_string = string.is_some();
        let mut last_sig: Option<char> = None;
        let mut last_sig_idx: Option<usize> = None;
        let mut first_sig: Option<char> = None;
        let mut escaped = false;
        let mut in_comment = false;
        let chars: Vec<char> = line.chars().collect();
        let mut k = 0;
        while k < chars.len() {
            let c = chars[k];
            if let Some(kind) = string {
                if matches!(kind, StrKind::Single | StrKind::Double) {
                    if escaped {
                        escaped = false;
                        k += 1;
                        continue;
                    }
                    if c == '\\' {
                        escaped = true;
                        k += 1;
                        continue;
                    }
                }
                let triple = matches!(kind, StrKind::TripleSingle | StrKind::TripleDouble);
                let closes = match kind {
                    StrKind::Single => c == '\'',
                    StrKind::Double => c == '"',
                    StrKind::TripleSingle => {
                        c == '\''
                            && chars.get(k + 1) == Some(&'\'')
                            && chars.get(k + 2) == Some(&'\'')
                    }
                    StrKind::TripleDouble => {
                        c == '"' && chars.get(k + 1) == Some(&'"') && chars.get(k + 2) == Some(&'"')
                    }
                };
                if closes {
                    let adv = if triple { 3 } else { 1 };
                    string = None;
                    first_sig.get_or_insert(c);
                    last_sig = Some(c);
                    last_sig_idx = Some(k + adv - 1);
                    k += adv;
                    continue;
                }
                k += 1;
                continue;
            }
            if in_comment {
                k += 1;
                continue;
            }
            if c == '#' {
                in_comment = true;
                k += 1;
                continue;
            }
            if c == '"' || c == '\'' {
                let triple = chars.get(k + 1) == Some(&c) && chars.get(k + 2) == Some(&c);
                string = Some(match (c, triple) {
                    ('"', true) => StrKind::TripleDouble,
                    ('"', false) => StrKind::Double,
                    ('\'', true) => StrKind::TripleSingle,
                    _ => StrKind::Single,
                });
                first_sig.get_or_insert(c);
                last_sig = Some(c);
                last_sig_idx = Some(k);
                k += if triple { 3 } else { 1 };
                continue;
            }
            if c.is_whitespace() {
                k += 1;
                continue;
            }
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = (depth - 1).max(0),
                _ => {}
            }
            first_sig.get_or_insert(c);
            last_sig = Some(c);
            last_sig_idx = Some(k);
            k += 1;
        }
        metas.push(LineMeta {
            depth_before,
            starts_in_string,
            ends_in_string: string.is_some(),
            last_sig,
            last_sig_idx,
            first_sig,
            first_word: leading_word(line),
        });
    }

    let mut out_lines: Vec<String> = raw_lines.iter().map(|s| s.to_string()).collect();
    for i in 0..metas.len() {
        if metas[i].last_sig.is_none() {
            continue;
        }
        let Some(j) = ((i + 1)..metas.len()).find(|&x| metas[x].first_sig.is_some()) else {
            continue;
        };
        let (a, b) = (&metas[i], &metas[j]);
        if a.ends_in_string || b.starts_in_string || b.depth_before <= 0 {
            continue;
        }
        let (Some(last), Some(first)) = (a.last_sig, b.first_sig) else {
            continue;
        };
        if !is_value_end(last) || !is_element_start(first) || is_continuation_keyword(&b.first_word)
        {
            continue;
        }
        let Some(idx) = a.last_sig_idx else { continue };
        let mut rebuilt = String::with_capacity(out_lines[i].len() + 1);
        for (ci, ch) in out_lines[i].chars().enumerate() {
            rebuilt.push(ch);
            if ci == idx {
                rebuilt.push(',');
            }
        }
        out_lines[i] = rebuilt;
    }
    out_lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repair_recovers_missing_argument_comma() {
        // Mirrors a real vendored zstd BUCK file: a missing comma after the
        // `exported_headers=[...]` list leaves `srcs=` with no separator.
        let content = "cxx_library(\n    name='errors',\n    exported_headers=[\n        'a.h',\n    ]\n    srcs=['a.c'],\n)\n";
        let (_, repaired) = parse_with_repair("<BUCK>", content.to_string())
            .expect("content should parse after repair");
        assert!(repaired, "the repair pass should be what succeeds");
    }

    #[test]
    fn test_repair_leaves_well_formed_content_unchanged() {
        let content = "cxx_library(\n    name='ok',\n    srcs=['a.c'],\n)\n";
        assert_eq!(repair_missing_argument_commas(content), content);
        let (_, repaired) =
            parse_with_repair("<BUCK>", content.to_string()).expect("well-formed parses");
        assert!(!repaired, "well-formed input is never repaired");
    }

    #[test]
    fn test_repair_preserves_multiline_expressions() {
        // A multiline binary expression and a comprehension must NOT gain commas.
        let binary = "x = (\n    a\n    + b\n)\n";
        assert_eq!(repair_missing_argument_commas(binary), binary);

        let comprehension = "y = [\n    item\n    for item in source\n]\n";
        assert_eq!(repair_missing_argument_commas(comprehension), comprehension);
    }

    #[test]
    fn test_repair_ignores_brackets_inside_strings() {
        // A bracket character inside a string must not be treated as structure.
        let content = "name = \"value with ] bracket\"\nother = 1\n";
        assert_eq!(repair_missing_argument_commas(content), content);
    }

    #[test]
    fn test_mark_parse_recovery_sets_breadcrumb() {
        let mut package = PackageData::default();
        mark_parse_recovery(&mut package, RECOVERY_MISSING_SEPARATOR);
        let value = package
            .extra_data
            .as_ref()
            .and_then(|map| map.get(PARSE_RECOVERY_KEY));
        assert_eq!(
            value,
            Some(&JsonValue::String(RECOVERY_MISSING_SEPARATOR.to_string()))
        );
    }
}
