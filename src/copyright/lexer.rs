// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Lexer (tokenizer + POS tagger) for copyright detection.
//!
//! Splits prepared text lines into tokens, then assigns each token a
//! part-of-speech (POS) tag using the compiled regex patterns. This is
//! the bridge between candidate line selection and grammar parsing.
//!
//! Pipeline: numbered lines → tokenize → POS tag → tagged tokens

use regex::Regex;

use super::patterns::match_token_thread_local;
use super::types::{PosTag, Token};
use crate::models::LineNumber;

// Splitter regex: splits on tabs, spaces, equals signs, and semicolons.
// Matches Python's `re.compile(r'[\t =;]+').split`.
thread_local! {
    static SPLITTER: Regex = Regex::new(r"[\t =;]+").unwrap();
}

/// Tokenize and POS-tag a group of numbered lines.
///
/// Takes an iterable of `(line_number, prepared_text)` tuples (output of
/// `collect_candidate_lines`) and returns a flat list of POS-tagged tokens.
///
/// Empty lines are handled specially: if the previous line starts with
/// "copyright" or ends with continuation markers ("by", "copyright", or
/// a digit), the empty line is skipped (continuation). Otherwise an
/// `EMPTY_LINE` token is emitted.
pub fn get_tokens(numbered_lines: &[(usize, String)]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut last_line = String::new();

    for (start_line, line) in numbered_lines {
        if line.trim().is_empty() {
            let stripped = last_line
                .to_lowercase()
                .trim_matches(|c: char| c.is_ascii_punctuation())
                .to_string();

            if stripped.starts_with("copyright")
                || stripped.ends_with("by")
                || stripped.ends_with("copyright")
                || stripped.chars().last().is_some_and(|c| c.is_ascii_digit())
            {
                continue;
            } else {
                tokens.push(Token {
                    value: "\n".to_string(),
                    tag: PosTag::EmptyLine,
                    start_line: LineNumber::new(*start_line).expect("invalid line number"),
                });
                last_line.clear();
                continue;
            }
        }

        last_line.clone_from(line);

        SPLITTER.with(|splitter| {
            for tok_str in splitter.split(line) {
                let quoted_structured_key = is_quoted_structured_key(tok_str);
                let mut tok = tok_str.to_string();

                if tok.ends_with("',") {
                    tok = tok.trim_end_matches(&[',', '\''][..]).to_string();
                }

                tok = tok.trim_matches(&['\'', ' '][..]).to_string();
                tok = tok.trim_end_matches(':').to_string();
                tok = tok.trim_end_matches('"').trim_end_matches('\'').to_string();
                tok = tok.trim().to_string();

                if tok.is_empty() || tok == ":" || tok == "." {
                    continue;
                }

                if tok.ends_with(',') {
                    let base = tok.trim_end_matches(',').trim();
                    if !base.is_empty() {
                        let tag = match_token_thread_local(base);
                        tokens.push(Token {
                            value: base.to_string(),
                            tag,
                            start_line: LineNumber::new(*start_line).expect("invalid line number"),
                        });
                        tokens.push(Token {
                            value: ",".to_string(),
                            tag: PosTag::Cc,
                            start_line: LineNumber::new(*start_line).expect("invalid line number"),
                        });
                        continue;
                    }
                }

                let tag = if quoted_structured_key {
                    PosTag::Junk
                } else {
                    match_token_thread_local(&tok)
                };

                tokens.push(Token {
                    value: tok,
                    tag,
                    start_line: LineNumber::new(*start_line).expect("invalid line number"),
                });
            }
        });
    }

    retag_camel_case_junk_before_company_suffix_in_copyright_context(&mut tokens);
    compose_obfuscated_emails(&mut tokens);

    tokens
}

/// Collapse a space-separated obfuscated email such as
/// `matt at genges dot com` into a single [`PosTag::Email`] token.
///
/// Mirrors the ScanCode `EMAIL` grammar rules for obfuscated forms
/// (`copyrights.py` rules `# foo at bat dot com` and `#350.3`): a word, the
/// literal connector `at`, a word, the literal `dot`, and a final word. The
/// connector tokenizes as either [`PosTag::At`] (from `AT`) or [`PosTag::Cc`]
/// (from lowercase `at`); the separator tokenizes as [`PosTag::Dot`]. Composing
/// in the token stream — rather than as grammar rules — keeps `Email` a leaf
/// tag, so the existing `NAME`/`NAME-EMAIL` rules chain holders across the
/// obfuscated email and the trailing comma instead of stopping at it.
fn compose_obfuscated_emails(tokens: &mut Vec<Token>) {
    if tokens.len() < 5 {
        return;
    }

    // A local/domain word of the obfuscated-email pattern. `PosTag::Pn` (dotted
    // initials such as `j.` or `DMTF.`) is intentionally excluded: a dotted
    // local-part like `j.doe at example dot com` is vanishingly rare in real
    // copyright headers and not worth the over-composition risk.
    let is_word = |t: &Token| {
        matches!(
            t.tag,
            PosTag::Nn | PosTag::Nnp | PosTag::Caps | PosTag::MixedCap
        )
    };
    let is_at_connector = |t: &Token| {
        t.tag == PosTag::At || (t.tag == PosTag::Cc && t.value.eq_ignore_ascii_case("at"))
    };

    let mut i = 0;
    while i + 5 <= tokens.len() {
        // A parenthesis-wrapped obfuscated email such as `(pdimov at gmail dot
        // com)` is left to the existing single-token bracket pattern and stays
        // inline in the holder, matching ScanCode. Only the angle-bracketed and
        // bare forms (whose brackets were already stripped upstream) compose
        // here, where ScanCode also lifts the email out of the holder.
        // Checking only the first and last token's line is sufficient: a blank
        // line between tokens injects an `EmptyLine` token (which fails `is_word`
        // /`is_at_connector` below), and a gapless continuation is treated as one
        // logical line — so the three inner tokens cannot straddle lines here.
        let has_paren_boundary =
            tokens[i].value.starts_with('(') || tokens[i + 4].value.ends_with(')');
        if !has_paren_boundary
            && tokens[i].start_line == tokens[i + 4].start_line
            && is_word(&tokens[i])
            && is_at_connector(&tokens[i + 1])
            && is_word(&tokens[i + 2])
            && tokens[i + 3].tag == PosTag::Dot
            && is_word(&tokens[i + 4])
        {
            let value = tokens[i..i + 5]
                .iter()
                .map(|t| t.value.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let start_line = tokens[i].start_line;
            tokens.splice(
                i..i + 5,
                std::iter::once(Token {
                    value,
                    tag: PosTag::Email,
                    start_line,
                }),
            );
        }
        i += 1;
    }
}

fn retag_camel_case_junk_before_company_suffix_in_copyright_context(tokens: &mut [Token]) {
    if tokens.len() < 2 {
        return;
    }

    for i in 0..tokens.len().saturating_sub(1) {
        if tokens[i].tag != PosTag::Junk {
            continue;
        }
        if tokens[i + 1].tag != PosTag::Comp {
            continue;
        }
        if tokens[i].start_line != tokens[i + 1].start_line {
            continue;
        }
        if !is_camel_case_identifier_candidate(&tokens[i].value) {
            continue;
        }

        let mut has_copy_prefix = false;
        let mut j = i;
        while j > 0 {
            j -= 1;
            if tokens[j].start_line != tokens[i].start_line || tokens[j].tag == PosTag::EmptyLine {
                break;
            }
            if tokens[j].tag == PosTag::Copy {
                has_copy_prefix = true;
                break;
            }
        }

        if has_copy_prefix {
            tokens[i].tag = PosTag::Nnp;
        }
    }
}

fn is_camel_case_identifier_candidate(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_uppercase() {
        return false;
    }

    let mut has_lower = false;
    let mut has_inner_upper = false;
    for c in chars {
        if !c.is_ascii_alphanumeric() {
            return false;
        }
        if c.is_ascii_lowercase() {
            has_lower = true;
        } else if c.is_ascii_uppercase() {
            has_inner_upper = true;
        }
    }

    has_lower && has_inner_upper
}

fn is_quoted_structured_key(raw: &str) -> bool {
    let trimmed = raw.trim();
    if !(trimmed.starts_with('\'') || trimmed.starts_with('"')) {
        return false;
    }

    let without_trailing_comma = trimmed.trim_end_matches(',').trim_end();
    without_trailing_comma.ends_with(':')
}

#[cfg(test)]
#[path = "lexer_test.rs"]
mod tests;
