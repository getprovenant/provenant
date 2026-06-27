// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Static parser for Elixir `mix.exs` project manifests.
//!
//! `mix.exs` is arbitrary Elixir source code, not a data file. Provenant never
//! executes it (ADR 0004). Instead this parser scans a **bounded literal
//! subset** that the overwhelming majority of real projects use:
//!
//! - project identity from the `def project do [...]` keyword list:
//!   - `app:` (an atom → the package name)
//!   - `version:` (a string literal, or a simple top-level `@version "x.y.z"`
//!     module attribute when `version: @version` is used)
//! - direct dependencies from the `defp deps do [...]` (or `def deps`) list of
//!   tuples such as `{:phoenix, "~> 1.7"}`, `{:ecto, ">= 3.0", only: :test}`,
//!   or `{:foo, github: "..."}`.
//!
//! Anything dynamic (computed versions, helper-function deps, `System.get_env`,
//! `if Mix.env()`, interpolated strings, …) is skipped rather than guessed: a
//! tuple whose first element is not a literal atom, or whose version slot is not
//! a string literal, simply contributes name-only (or is dropped) instead of
//! producing a wrong requirement.

use std::collections::HashMap;
use std::path::Path;

use packageurl::PackageUrl;
use serde_json::Value as JsonValue;

use crate::models::{DatasourceId, Dependency, PackageData, PackageType};
use crate::parser_warn as warn;
use crate::parsers::utils::{MAX_ITERATION_COUNT, read_file_to_string, truncate_field};

use super::PackageParser;
use super::metadata::ParserMetadata;

pub struct MixExsParser;

impl PackageParser for MixExsParser {
    const PACKAGE_TYPE: PackageType = PackageType::Hex;

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "Elixir mix.exs project manifest",
            file_patterns: &["**/mix.exs"],
            package_type: "hex",
            primary_language: "Elixir",
            documentation_url: Some("https://hexdocs.pm/mix/Mix.Project.html"),
        }]
    }

    fn is_match(path: &Path) -> bool {
        path.file_name().and_then(|name| name.to_str()) == Some("mix.exs")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(content) => content,
            Err(e) => {
                warn!("Failed to read mix.exs at {:?}: {}", path, e);
                return vec![default_package_data()];
            }
        };

        vec![parse_mix_exs(&content)]
    }
}

#[cfg(test)]
pub(super) fn parse_mix_exs_for_test(content: &str) -> PackageData {
    parse_mix_exs(content)
}

fn default_package_data() -> PackageData {
    PackageData {
        package_type: Some(PackageType::Hex),
        primary_language: Some("Elixir".to_string()),
        datasource_id: Some(DatasourceId::HexMixExs),
        ..Default::default()
    }
}

fn parse_mix_exs(content: &str) -> PackageData {
    let mut package = default_package_data();

    let module_version = extract_module_version(content);

    if let Some(project_body) = extract_block_body(content, "def", "project") {
        let entries = parse_keyword_list(&project_body);
        if let Some(app) = entries
            .iter()
            .find(|(k, _)| k == "app")
            .and_then(|(_, v)| atom_value(v))
        {
            package.name = Some(truncate_field(app));
        }
        if let Some((_, version_value)) = entries.iter().find(|(k, _)| k == "version") {
            if let Some(version) = string_value(version_value) {
                package.version = Some(truncate_field(version));
            } else if is_module_version_attribute(version_value)
                && let Some(version) = module_version.clone()
            {
                package.version = Some(truncate_field(version));
            }
        }
    }

    package.purl = build_hex_purl(package.name.as_deref(), package.version.as_deref());

    package.dependencies = extract_deps(content);

    package
}

/// Resolve a top-level `@version "x.y.z"` module attribute. Only a single
/// string-literal assignment is recognized; anything dynamic is ignored.
fn extract_module_version(content: &str) -> Option<String> {
    for line in content.lines().take(MAX_ITERATION_COUNT) {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("@version") {
            let rest = rest.trim_start();
            if let Some(value) = parse_leading_string(rest) {
                return Some(value);
            }
        }
    }
    None
}

/// Extract the body of a `<keyword> <name> do ... end` block (for example
/// `def project do ... end`), balancing nested `do`/`fn`/block keywords against
/// `end`. Returns the source slice between `do` and its matching `end`.
fn extract_block_body(content: &str, keyword: &str, name: &str) -> Option<String> {
    let chars: Vec<char> = content.chars().collect();
    let header = find_block_header(content, keyword, name)?;

    // Find the `do` that opens the block, starting at the header.
    let do_pos = find_do_after(&chars, header)?;
    let body_start = do_pos + 2;

    let mut depth = 1usize;
    let mut idx = body_start;
    let mut iterations = 0usize;
    while idx < chars.len() {
        iterations += 1;
        if iterations > MAX_ITERATION_COUNT {
            warn!("mix.exs block scan exceeded MAX_ITERATION_COUNT");
            return None;
        }
        match chars[idx] {
            '"' => {
                idx = skip_string(&chars, idx);
                continue;
            }
            '#' => {
                idx = skip_line_comment(&chars, idx);
                continue;
            }
            _ => {}
        }
        if let Some(word) = word_at(&chars, idx) {
            match word.as_str() {
                "do" | "fn" => depth += 1,
                "end" => {
                    depth -= 1;
                    if depth == 0 {
                        let body: String = chars[body_start..idx].iter().collect();
                        return Some(body);
                    }
                }
                _ => {}
            }
            idx += word.chars().count();
            continue;
        }
        idx += 1;
    }
    None
}

/// Locate the byte-independent char index just past a `<keyword> <name>` header
/// (e.g. `def project`), allowing `defp`, parentheses, and arbitrary spacing.
fn find_block_header(content: &str, keyword: &str, name: &str) -> Option<usize> {
    let chars: Vec<char> = content.chars().collect();
    let mut idx = 0usize;
    while idx < chars.len() {
        if let Some(word) = word_at(&chars, idx) {
            if word == keyword {
                let after_kw = idx + word.chars().count();
                let mut cursor = skip_inline_ws(&chars, after_kw);
                if let Some(next) = word_at(&chars, cursor)
                    && next == name
                {
                    cursor += next.chars().count();
                    return Some(cursor);
                }
            }
            idx += word.chars().count().max(1);
            continue;
        }
        idx += 1;
    }
    None
}

fn find_do_after(chars: &[char], mut idx: usize) -> Option<usize> {
    let mut iterations = 0usize;
    while idx < chars.len() {
        iterations += 1;
        if iterations > MAX_ITERATION_COUNT {
            return None;
        }
        match chars[idx] {
            '"' => {
                idx = skip_string(chars, idx);
                continue;
            }
            '#' => {
                idx = skip_line_comment(chars, idx);
                continue;
            }
            _ => {}
        }
        if let Some(word) = word_at(chars, idx) {
            if word == "do" {
                return Some(idx);
            }
            idx += word.chars().count();
            continue;
        }
        idx += 1;
    }
    None
}

/// Extract direct dependencies from the `deps` block. Supports `defp deps` and
/// `def deps`. The body is expected to be a list literal `[ {..}, {..} ]`.
fn extract_deps(content: &str) -> Vec<Dependency> {
    let body = extract_block_body(content, "defp", "deps")
        .or_else(|| extract_block_body(content, "def", "deps"));
    let Some(body) = body else {
        return Vec::new();
    };

    let Some(list_inner) = slice_outer_list(&body) else {
        return Vec::new();
    };

    let tuples = split_top_level_items(&list_inner);
    let mut dependencies = Vec::new();
    for tuple_src in tuples.into_iter().take(MAX_ITERATION_COUNT) {
        if let Some(dep) = parse_dep_tuple(&tuple_src) {
            dependencies.push(dep);
        }
    }
    dependencies
}

fn parse_dep_tuple(tuple_src: &str) -> Option<Dependency> {
    let trimmed = tuple_src.trim();
    let inner = trimmed.strip_prefix('{')?.strip_suffix('}')?;
    let items = split_top_level_items(inner);
    if items.is_empty() {
        return None;
    }

    let name = atom_literal(items[0].trim())?;

    // The version requirement is the second positional element, but only when
    // it is a string literal. `{:foo, github: "..."}` has options (not a
    // version) in slot two, so guard on the literal-string shape.
    let mut requirement: Option<String> = None;
    let mut options_start = 1usize;
    if items.len() >= 2
        && let Some(version) = parse_leading_string(items[1].trim())
    {
        requirement = Some(version);
        options_start = 2;
    }

    // Remaining items may be `key: value` options; collect literal `only:` and
    // `optional:` only.
    let mut scope: Option<String> = None;
    let mut is_optional: Option<bool> = None;
    for item in items.iter().skip(options_start).take(MAX_ITERATION_COUNT) {
        for (key, value) in parse_keyword_list(item) {
            match key.as_str() {
                "only" => {
                    if let Some(envs) = literal_env_list(&value) {
                        scope = Some(envs.join(","));
                    }
                }
                "optional" => {
                    if let Some(flag) = bool_value(&value) {
                        is_optional = Some(flag);
                    }
                }
                _ => {}
            }
        }
    }

    Some(Dependency {
        purl: build_hex_purl(Some(&name), None).map(truncate_field),
        extracted_requirement: requirement.map(truncate_field),
        scope: scope.map(truncate_field),
        is_runtime: None,
        is_optional,
        is_pinned: None,
        is_direct: Some(true),
        resolved_package: None,
        extra_data: Some(HashMap::from([(
            "app".to_string(),
            JsonValue::String(truncate_field(name)),
        )])),
    })
}

/// Parse a `:atom` literal, returning the atom name without the leading colon.
fn atom_literal(src: &str) -> Option<String> {
    let rest = src.strip_prefix(':')?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '?' || *c == '!')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

/// `only:` may be a single atom (`:test`) or a list (`[:dev, :test]`). Return the
/// environment names when every element is a literal atom; otherwise `None`.
fn literal_env_list(value: &str) -> Option<Vec<String>> {
    let trimmed = value.trim();
    if let Some(atom) = atom_literal(trimmed) {
        return Some(vec![atom]);
    }
    let inner = trimmed.strip_prefix('[')?.strip_suffix(']')?;
    let mut envs = Vec::new();
    for item in split_top_level_items(inner)
        .into_iter()
        .take(MAX_ITERATION_COUNT)
    {
        let atom = atom_literal(item.trim())?;
        envs.push(atom);
    }
    if envs.is_empty() { None } else { Some(envs) }
}

fn bool_value(value: &str) -> Option<bool> {
    match value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// A `keyword:`-style value that is a literal string.
fn string_value(value: &str) -> Option<String> {
    parse_leading_string(value.trim())
}

fn atom_value(value: &str) -> Option<String> {
    atom_literal(value.trim())
}

fn is_module_version_attribute(value: &str) -> bool {
    value.trim() == "@version"
}

/// Parse a leading double-quoted string literal at the start of `src`. Returns
/// `None` for interpolated strings (`"#{...}"`) so dynamic values are skipped.
fn parse_leading_string(src: &str) -> Option<String> {
    let chars: Vec<char> = src.chars().collect();
    if chars.first() != Some(&'"') {
        return None;
    }
    let mut out = String::new();
    let mut idx = 1usize;
    while idx < chars.len() {
        match chars[idx] {
            '"' => {
                // The string closes here. Accept it only if nothing (other than a
                // comment) follows: a value like `"1." <> patch` is a computed
                // expression whose literal prefix must not be emitted as a partial
                // version/requirement.
                let rest: String = chars[idx + 1..].iter().collect();
                let rest = rest.trim_start();
                if rest.is_empty() || rest.starts_with('#') {
                    return Some(out);
                }
                return None;
            }
            '\\' => {
                idx += 1;
                if idx >= chars.len() {
                    return None;
                }
                out.push(match chars[idx] {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    other => other,
                });
            }
            '#' if chars.get(idx + 1) == Some(&'{') => {
                // String interpolation → dynamic, not a literal.
                return None;
            }
            other => out.push(other),
        }
        idx += 1;
    }
    None
}

/// Parse a keyword list source fragment into `(key, value)` pairs, where keys
/// are `key:` identifiers and values are the raw source up to the next
/// top-level comma. Leading list/tuple delimiters are tolerated.
fn parse_keyword_list(src: &str) -> Vec<(String, String)> {
    let trimmed = src.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);

    let mut pairs = Vec::new();
    for item in split_top_level_items(inner)
        .into_iter()
        .take(MAX_ITERATION_COUNT)
    {
        if let Some((key, value)) = split_keyword_entry(&item) {
            pairs.push((key, value));
        }
    }
    pairs
}

/// Split a single `key: value` entry. The key is a bare identifier followed by a
/// colon that is **not** part of an atom (`::`) and not a quoted string key.
fn split_keyword_entry(item: &str) -> Option<(String, String)> {
    let trimmed = item.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    let mut idx = 0usize;
    while idx < chars.len() {
        let c = chars[idx];
        if c.is_ascii_alphanumeric() || c == '_' || c == '?' || c == '!' {
            idx += 1;
        } else {
            break;
        }
    }
    if idx == 0 || chars.get(idx) != Some(&':') {
        return None;
    }
    let key: String = chars[..idx].iter().collect();
    let value: String = chars[idx + 1..].iter().collect();
    Some((key, value.trim().to_string()))
}

/// Given a block body, return the inner content of its outer `[ ... ]` list, if
/// the body is a list literal (ignoring surrounding whitespace/comments).
fn slice_outer_list(body: &str) -> Option<String> {
    let chars: Vec<char> = body.chars().collect();
    let mut start = 0usize;
    while start < chars.len() {
        match chars[start] {
            c if c.is_whitespace() => start += 1,
            '#' => start = skip_line_comment(&chars, start),
            '[' => break,
            _ => return None,
        }
    }
    if chars.get(start) != Some(&'[') {
        return None;
    }
    // Find the matching close bracket.
    let mut depth = 0usize;
    let mut idx = start;
    let mut iterations = 0usize;
    while idx < chars.len() {
        iterations += 1;
        if iterations > MAX_ITERATION_COUNT {
            return None;
        }
        match chars[idx] {
            '"' => {
                idx = skip_string(&chars, idx);
                continue;
            }
            '#' if chars.get(idx + 1) != Some(&'{') => {
                idx = skip_line_comment(&chars, idx);
                continue;
            }
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    let inner: String = chars[start + 1..idx].iter().collect();
                    return Some(inner);
                }
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

/// Split a comma-separated source fragment into top-level items, respecting
/// nesting of `()[]{}`, string literals, and line comments.
fn split_top_level_items(src: &str) -> Vec<String> {
    let chars: Vec<char> = src.chars().collect();
    let mut items = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut idx = 0usize;
    let mut iterations = 0usize;
    while idx < chars.len() {
        iterations += 1;
        if iterations > MAX_ITERATION_COUNT {
            break;
        }
        let c = chars[idx];
        match c {
            '"' => {
                let end = skip_string(&chars, idx);
                current.extend(&chars[idx..end.min(chars.len())]);
                idx = end;
                continue;
            }
            '#' if chars.get(idx + 1) != Some(&'{') => {
                idx = skip_line_comment(&chars, idx);
                continue;
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                let item = current.trim().to_string();
                if !item.is_empty() {
                    items.push(item);
                }
                current.clear();
            }
            _ => current.push(c),
        }
        idx += 1;
    }
    let item = current.trim().to_string();
    if !item.is_empty() {
        items.push(item);
    }
    items
}

/// Advance past a double-quoted string starting at `idx` (which must be `"`).
/// Returns the index just after the closing quote.
fn skip_string(chars: &[char], idx: usize) -> usize {
    let mut idx = idx + 1;
    while idx < chars.len() {
        match chars[idx] {
            '\\' => idx += 2,
            '"' => return idx + 1,
            _ => idx += 1,
        }
    }
    idx
}

/// Advance to the start of the next line after a `#` comment.
fn skip_line_comment(chars: &[char], idx: usize) -> usize {
    let mut idx = idx;
    while idx < chars.len() && chars[idx] != '\n' {
        idx += 1;
    }
    idx
}

fn skip_inline_ws(chars: &[char], mut idx: usize) -> usize {
    while idx < chars.len() && (chars[idx] == ' ' || chars[idx] == '\t') {
        idx += 1;
    }
    idx
}

/// Return the identifier word at `idx` only when it begins on a word boundary
/// (the previous char is not part of an identifier), so `do`, `end`, and `fn`
/// are matched as whole keywords rather than substrings of other identifiers.
fn word_at(chars: &[char], idx: usize) -> Option<String> {
    let c = *chars.get(idx)?;
    if !(c.is_ascii_alphabetic() || c == '_') {
        return None;
    }
    if idx > 0 {
        let prev = chars[idx - 1];
        if prev.is_ascii_alphanumeric() || prev == '_' || prev == '?' || prev == '!' {
            return None;
        }
    }
    let word: String = chars[idx..]
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || **c == '_' || **c == '?' || **c == '!')
        .collect();
    Some(word)
}

fn build_hex_purl(name: Option<&str>, version: Option<&str>) -> Option<String> {
    let name = name?;
    let mut purl = PackageUrl::new("hex", name).ok()?;
    if let Some(version) = version {
        purl.with_version(version).ok()?;
    }
    Some(purl.to_string())
}
