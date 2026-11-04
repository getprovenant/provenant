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
//!   or `{:foo, github: "..."}`. Only the list the function body provably
//!   returns is used: a literal, a `var = [...]` binding returned as `var`,
//!   `++` concatenations of those, or `if` branches that all return them.
//!   Earlier statements are never mined for tuples.
//!
//! Anything dynamic (computed versions, helper-function deps, `System.get_env`,
//! `if Mix.env()`, interpolated strings, …) is skipped rather than guessed: a
//! tuple whose first element is not a literal atom, or whose version slot is not
//! a string literal, simply contributes name-only (or is dropped) instead of
//! producing a wrong requirement.
//!
//! # Umbrella projects
//!
//! A root `mix.exs` whose `project` keyword list carries a literal `apps_path:`
//! string marks an [umbrella project](https://hexdocs.pm/mix/Mix.Project.html#module-umbrella-projects):
//! its child applications live under that directory (one `mix.exs` per app) and
//! typically share a single root `mix.lock`. This parser records `apps_path`
//! (and the optional `apps:` allow-list) into `extra_data` so
//! `src/assembly/mix_umbrella_merge.rs` can plan the umbrella topology; the
//! parser itself stays file-local and does not look at sibling files.
//!
//! A dependency declared as `{:sibling, in_umbrella: true}` refers to another
//! app in the same umbrella, not a real Hex package. Such deps are emitted with
//! `purl: None` and `extra_data["in_umbrella"] = true` so they are never
//! mistaken for (nonexistent, unversioned) Hex registry packages; umbrella
//! assembly resolves them to the sibling app's actual package identity.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

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
    let mut extra_data: HashMap<String, JsonValue> = HashMap::new();

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

        // `apps_path:` marks this manifest as an umbrella root; the optional
        // `apps:` allow-list restricts which child apps under that directory
        // are actually part of the umbrella (Mix defaults to every app found
        // there when `apps:` is absent).
        if let Some((_, apps_path_value)) = entries.iter().find(|(k, _)| k == "apps_path")
            && let Some(apps_path) = string_value(apps_path_value)
        {
            extra_data.insert(
                "apps_path".to_string(),
                JsonValue::String(truncate_field(apps_path)),
            );
        }
        if let Some((_, apps_value)) = entries.iter().find(|(k, _)| k == "apps")
            && let Some(apps) = literal_env_list(apps_value)
        {
            extra_data.insert(
                "apps".to_string(),
                JsonValue::Array(
                    apps.into_iter()
                        .map(|app| JsonValue::String(truncate_field(app)))
                        .collect(),
                ),
            );
        }
    }

    package.purl = build_hex_purl(package.name.as_deref(), package.version.as_deref());

    package.dependencies = extract_deps(content);

    if !extra_data.is_empty() {
        package.extra_data = Some(extra_data);
    }

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
    // The keyword form `def deps, do: [...]` has no `end`-delimited body.
    if chars.get(do_pos + 2) == Some(&':') {
        return None;
    }
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
            if is_block_delimiter(&chars, idx, &word) {
                if word == "end" {
                    depth -= 1;
                    if depth == 0 {
                        let body: String = chars[body_start..idx].iter().collect();
                        return Some(body);
                    }
                } else {
                    depth += 1;
                }
            }
            idx += word.chars().count();
            continue;
        }
        idx += 1;
    }
    None
}

/// `do`/`fn`/`end` used as a block delimiter, not as a `do:` keyword key or a `:end` atom.
fn is_block_delimiter(chars: &[char], idx: usize, word: &str) -> bool {
    matches!(word, "do" | "fn" | "end")
        && (idx == 0 || chars[idx - 1] != ':')
        && chars.get(idx + word.chars().count()) != Some(&':')
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
/// `def deps`; see [`resolve_expr`] for which return shapes are traced.
fn extract_deps(content: &str) -> Vec<Dependency> {
    let body = extract_block_body(content, "defp", "deps")
        .or_else(|| extract_block_body(content, "def", "deps"));
    let Some(body) = body else {
        return Vec::new();
    };

    let items = returned_items(&body, &HashMap::new(), 0).unwrap_or_default();
    if items.len() >= MAX_DEPENDENCIES {
        warn!(
            "mix.exs deps list truncated at {} entries",
            MAX_DEPENDENCIES
        );
    }
    items.into_iter().map(Rc::unwrap_or_clone).collect()
}

const MAX_RETURN_NESTING: usize = 8;
pub(super) const MAX_DEPENDENCIES: usize = 5_000;
const MAX_BODY_STATEMENTS: usize = 1_000;

type Items = Vec<Rc<Dependency>>;
type Bindings = HashMap<String, Rc<Items>>;

/// Known-returned dependencies of a function or branch body: `var = expr` statements
/// update bindings, and the final statement is resolved with [`resolve_expr`].
fn returned_items(body: &str, outer: &Bindings, depth: usize) -> Option<Items> {
    if depth > MAX_RETURN_NESTING {
        return None;
    }
    let statements = split_top_level_statements(body);
    let (last, earlier) = statements.split_last()?;
    if earlier.len() > MAX_BODY_STATEMENTS {
        warn!(
            "mix.exs deps body exceeded {} statements",
            MAX_BODY_STATEMENTS
        );
        return None;
    }
    let mut bindings = outer.clone();
    for statement in earlier {
        if let Some((var, rhs)) = split_assignment(statement) {
            match resolve_expr(&rhs, &bindings, depth) {
                Some(items) => bindings.insert(var, Rc::new(items)),
                None => bindings.remove(&var),
            };
        }
    }
    resolve_expr(last, &bindings, depth)
}

/// Dependencies an expression is known to return (at most [`MAX_DEPENDENCIES`]): list
/// literals and bound variables, `++` chains of those (calls in a chain only add unknown
/// items), and `if` branches (deps common to all branches). Other shapes are `None`.
fn resolve_expr(expr: &str, bindings: &Bindings, depth: usize) -> Option<Items> {
    let expr = expr.trim();
    if let Some(rest) = strip_if_keyword(expr) {
        return resolve_if(rest, bindings, depth + 1);
    }
    let mut items = Items::new();
    for part in split_top_level_concat(expr) {
        let part = part.trim();
        let room = MAX_DEPENDENCIES.saturating_sub(items.len());
        if let Some(inner) = exact_list_literal(part) {
            let parsed = split_top_level_items(&inner)
                .into_iter()
                .filter_map(|tuple_src| parse_dep_tuple(&tuple_src))
                .take(room)
                .map(Rc::new);
            items.extend(parsed);
        } else if is_variable(part) {
            if let Some(bound) = bindings.get(part) {
                items.extend(bound.iter().take(room).cloned());
            }
        } else if !is_call(part) {
            return None;
        }
    }
    Some(items)
}

fn resolve_if(rest: &str, bindings: &Bindings, depth: usize) -> Option<Items> {
    let (then_src, else_src) = split_if_block(rest).or_else(|| split_if_keywords(rest))?;
    let then_items = returned_items(&then_src, bindings, depth)?;
    let else_items = returned_items(&else_src?, bindings, depth)?;
    let else_keys: HashSet<_> = else_items.iter().map(|dep| dependency_key(dep)).collect();
    Some(
        then_items
            .into_iter()
            .filter(|dep| else_keys.contains(&dependency_key(dep)))
            .collect(),
    )
}

type DependencyKey<'a> = (
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
    Option<bool>,
    Option<&'a str>,
);

/// Identity of a parsed dependency, independent of the tuple's source formatting.
fn dependency_key(dep: &Dependency) -> DependencyKey<'_> {
    let extra = dep.extra_data.as_ref();
    (
        dep.purl.as_deref(),
        dep.extracted_requirement.as_deref(),
        dep.scope.as_deref(),
        dep.is_optional,
        extra
            .and_then(|extra| extra.get("app"))
            .and_then(JsonValue::as_str),
    )
}

/// The source after a leading `if` keyword, if `expr` is an `if` expression.
fn strip_if_keyword(expr: &str) -> Option<&str> {
    let rest = expr.strip_prefix("if")?;
    rest.starts_with(|c: char| c.is_whitespace() || c == '(')
        .then_some(rest)
}

/// Branches of `if cond do X else Y end`; the else branch is `None` when absent.
fn split_if_block(rest: &str) -> Option<(String, Option<String>)> {
    let chars: Vec<char> = rest.chars().collect();
    let mut nesting = 0i32;
    let mut blocks = 0i32;
    let mut then_start = None;
    let mut else_at = None;
    let mut idx = 0usize;
    while idx < chars.len() {
        match chars[idx] {
            '"' => {
                idx = skip_string(&chars, idx);
                continue;
            }
            '(' | '[' | '{' => nesting += 1,
            ')' | ']' | '}' => nesting -= 1,
            _ => {}
        }
        if let Some(word) = word_at(&chars, idx) {
            let len = word.chars().count();
            if is_block_delimiter(&chars, idx, &word) {
                if word == "end" {
                    blocks -= 1;
                    if blocks == 0 {
                        let start = then_start?;
                        if chars[idx + len..].iter().any(|c| !c.is_whitespace()) {
                            return None;
                        }
                        let collect =
                            |from: usize, to: usize| -> String { chars[from..to].iter().collect() };
                        return Some(match else_at {
                            Some(at) => (collect(start, at), Some(collect(at + 4, idx))),
                            None => (collect(start, idx), None),
                        });
                    }
                } else {
                    if word == "do" && blocks == 0 && nesting == 0 {
                        then_start = Some(idx + len);
                    }
                    blocks += 1;
                }
            } else if word == "else"
                && blocks == 1
                && nesting == 0
                && chars.get(idx + len) != Some(&':')
            {
                else_at = Some(idx);
            }
            idx += len;
            continue;
        }
        idx += 1;
    }
    None
}

/// Branches of `if cond, do: X, else: Y` (optionally parenthesized).
fn split_if_keywords(rest: &str) -> Option<(String, Option<String>)> {
    let rest = rest.trim();
    let inner = exact_parenthesized(rest).unwrap_or(rest);
    let items = split_top_level_items(inner);
    let mut then_src = None;
    let mut else_src = None;
    for item in items.iter().skip(1) {
        match split_keyword_entry(item)? {
            (key, value) if key == "do" => then_src = Some(value),
            (key, value) if key == "else" => else_src = Some(value),
            _ => return None,
        }
    }
    Some((then_src?, else_src))
}

/// Split `var = rhs` (a plain variable binding) into the variable and the right-hand side.
fn split_assignment(statement: &str) -> Option<(String, String)> {
    let chars: Vec<char> = statement.chars().collect();
    let var = word_at(&chars, 0)?;
    if !is_variable(&var) {
        return None;
    }
    let eq = skip_inline_ws(&chars, var.chars().count());
    if chars.get(eq) != Some(&'=') || matches!(chars.get(eq + 1), Some('=' | '~' | '>')) {
        return None;
    }
    Some((var, chars[eq + 1..].iter().collect()))
}

/// Split an expression on top-level `++` operators.
fn split_top_level_concat(expr: &str) -> Vec<String> {
    let chars: Vec<char> = expr.chars().collect();
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut nesting = 0i32;
    let mut blocks = 0i32;
    let mut idx = 0usize;
    while idx < chars.len() {
        let c = chars[idx];
        match c {
            '"' => {
                let end = skip_string(&chars, idx);
                current.extend(&chars[idx..end.min(chars.len())]);
                idx = end;
                continue;
            }
            '(' | '[' | '{' => nesting += 1,
            ')' | ']' | '}' => nesting -= 1,
            '+' if nesting == 0 && blocks == 0 && chars.get(idx + 1) == Some(&'+') => {
                parts.push(std::mem::take(&mut current));
                idx += 2;
                continue;
            }
            _ => {}
        }
        if let Some(word) = word_at(&chars, idx) {
            if is_block_delimiter(&chars, idx, &word) {
                blocks += if word == "end" { -1 } else { 1 };
            }
            idx += word.chars().count();
            current.push_str(&word);
            continue;
        }
        current.push(c);
        idx += 1;
    }
    parts.push(current);
    parts
}

/// Inner source of `src` when the whole of it is one `[ ... ]` list literal.
fn exact_list_literal(src: &str) -> Option<String> {
    let chars: Vec<char> = src.chars().collect();
    if chars.first() != Some(&'[') || matching_close(&chars, 0)? + 1 != chars.len() {
        return None;
    }
    Some(chars[1..chars.len() - 1].iter().collect())
}

/// Inner source of `src` when the whole of it is one `( ... )` group.
fn exact_parenthesized(src: &str) -> Option<&str> {
    let chars: Vec<char> = src.chars().collect();
    if chars.first() != Some(&'(') || matching_close(&chars, 0)? + 1 != chars.len() {
        return None;
    }
    src.strip_prefix('(')?.strip_suffix(')')
}

/// A local variable name (lowercase or `_` start), not a module or a call.
fn is_variable(src: &str) -> bool {
    let chars: Vec<char> = src.chars().collect();
    word_at(&chars, 0).is_some_and(|word| word == src)
        && src.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
}

/// A function call such as `extra_deps()` or `Mod.fun(arg)` spanning all of `src`.
fn is_call(src: &str) -> bool {
    let chars: Vec<char> = src.chars().collect();
    let name_len = chars
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '?' | '!'))
        .count();
    name_len > 0
        && chars.get(name_len) == Some(&'(')
        && matching_close(&chars, name_len).is_some_and(|close| close + 1 == chars.len())
}

/// Index of the bracket closing the one opened at `open`, skipping string literals.
fn matching_close(chars: &[char], open: usize) -> Option<usize> {
    let mut nesting = 0i32;
    let mut idx = open;
    while idx < chars.len() {
        match chars[idx] {
            '"' => {
                idx = skip_string(chars, idx);
                continue;
            }
            '(' | '[' | '{' => nesting += 1,
            ')' | ']' | '}' => {
                nesting -= 1;
                if nesting == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

/// Split a block body into top-level statements on `;` or on a newline that ends an
/// expression outside brackets and `do`/`fn`...`end` blocks. Comments are dropped.
fn split_top_level_statements(body: &str) -> Vec<String> {
    let chars: Vec<char> = body.chars().collect();
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut nesting = 0i32;
    let mut blocks = 0i32;
    let mut last_significant: Option<char> = None;
    let mut idx = 0usize;
    let mut iterations = 0usize;
    while idx < chars.len() {
        iterations += 1;
        if iterations > MAX_ITERATION_COUNT {
            warn!("mix.exs statement scan exceeded MAX_ITERATION_COUNT");
            break;
        }
        let c = chars[idx];
        match c {
            '"' => {
                let end = skip_string(&chars, idx);
                current.extend(&chars[idx..end.min(chars.len())]);
                last_significant = Some('"');
                idx = end;
                continue;
            }
            '#' => {
                idx = skip_line_comment(&chars, idx);
                continue;
            }
            '(' | '[' | '{' => nesting += 1,
            ')' | ']' | '}' => nesting -= 1,
            ';' | '\n' if nesting <= 0 && blocks <= 0 => {
                let next = next_significant_index(&chars, idx + 1);
                if c == '\n'
                    && (last_significant.is_some_and(is_continuation_char)
                        || starts_continuation_operator(&chars, next))
                {
                    current.push(' ');
                    idx = next;
                    continue;
                }
                push_statement(&mut statements, &mut current);
                last_significant = None;
                idx += 1;
                continue;
            }
            _ => {}
        }
        if let Some(word) = word_at(&chars, idx) {
            if is_block_delimiter(&chars, idx, &word) {
                if word == "end" {
                    blocks -= 1;
                } else {
                    blocks += 1;
                }
            }
            last_significant = word.chars().last();
            idx += word.chars().count();
            current.push_str(&word);
            continue;
        }
        current.push(c);
        if !c.is_whitespace() {
            last_significant = Some(c);
        }
        idx += 1;
    }
    push_statement(&mut statements, &mut current);
    statements
}

fn push_statement(statements: &mut Vec<String>, current: &mut String) {
    let statement = current.trim();
    if !statement.is_empty() {
        statements.push(statement.to_string());
    }
    current.clear();
}

/// Index of the next char that is neither whitespace nor inside a `#` comment.
fn next_significant_index(chars: &[char], mut idx: usize) -> usize {
    while idx < chars.len() {
        match chars[idx] {
            '#' => idx = skip_line_comment(chars, idx),
            c if c.is_whitespace() => idx += 1,
            _ => break,
        }
    }
    idx
}

/// A trailing char that leaves an expression open across a newline (`=`, `++`, `,`, `do:`).
fn is_continuation_char(c: char) -> bool {
    matches!(
        c,
        '=' | '+' | '-' | '*' | '/' | '|' | '>' | '<' | ',' | ':' | '\\' | '&' | '.'
    )
}

/// A binary operator at the start of a line that continues the previous expression.
fn starts_continuation_operator(chars: &[char], idx: usize) -> bool {
    let pair: String = chars.iter().skip(idx).take(2).collect();
    matches!(pair.as_str(), "|>" | "++" | "--" | "<>" | "||" | "&&")
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

    // Remaining items may be `key: value` options; collect literal `only:`,
    // `optional:`, and `in_umbrella:` only.
    let mut scope: Option<String> = None;
    let mut is_optional: Option<bool> = None;
    let mut is_in_umbrella = false;
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
                "in_umbrella" if bool_value(&value) == Some(true) => {
                    is_in_umbrella = true;
                }
                _ => {}
            }
        }
    }

    // `{:sibling, in_umbrella: true}` names another app in the same umbrella,
    // not a real Hex package: it has no registry version and building a
    // `pkg:hex/<name>` purl for it would fabricate a nonexistent, unversioned
    // package. Leave the purl unset and flag it so umbrella assembly can
    // resolve it to the sibling app's real (versioned) package identity.
    let mut extra_data = HashMap::from([(
        "app".to_string(),
        JsonValue::String(truncate_field(name.clone())),
    )]);
    let purl = if is_in_umbrella {
        extra_data.insert("in_umbrella".to_string(), JsonValue::Bool(true));
        None
    } else {
        build_hex_purl(Some(&name), None).map(truncate_field)
    };

    Some(Dependency {
        purl,
        extracted_requirement: if is_in_umbrella {
            None
        } else {
            requirement.map(truncate_field)
        },
        scope: scope.map(truncate_field),
        is_runtime: None,
        is_optional,
        is_pinned: None,
        is_direct: Some(true),
        resolved_package: None,
        extra_data: Some(extra_data),
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
