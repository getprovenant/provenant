// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Parser for Gradle build files (Groovy and Kotlin DSL).
//!
//! Extracts dependencies from Gradle build scripts using a custom token-based
//! lexer and recursive descent parser supporting both Groovy and Kotlin syntax.
//!
//! # Supported Formats
//! - build.gradle (Groovy DSL)
//! - build.gradle.kts (Kotlin DSL)
//!
//! # Key Features
//! - Token-based lexer for Gradle syntax parsing (not full language parser)
//! - Support for multiple dependency declaration styles
//! - Dependency scope tracking (implementation, testImplementation, etc.)
//! - Project dependency references and platform dependencies
//! - Version interpolation and constraint parsing
//! - Package URL (purl) generation for Maven packages
//!
//! # Implementation Notes
//! - Custom 870-line lexer instead of external parser (smaller binary, easier maintenance)
//! - Supports Groovy and Kotlin syntax variations
//! - Graceful error handling with `warn!()` logs
//! - Direct dependency tracking (all in build file are direct)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::parser_warn as warn;
use crate::parsers::utils::{
    CappedIterExt, MAX_ITERATION_COUNT, capped_iteration_limit, read_file_to_string, truncate_field,
};

const MAX_RECURSION_DEPTH: usize = 50;
use packageurl::PackageUrl;
use serde_json::json;

use super::metadata::ParserMetadata;
use crate::models::{DatasourceId, Dependency, PackageData, PackageType};
use crate::parsers::PackageParser;

use super::license_normalization::{
    DeclaredLicenseMatchMetadata, build_declared_license_data, normalize_declared_name_and_url,
};

/// Parses Gradle build files (build.gradle, build.gradle.kts).
///
/// Extracts dependencies from Gradle build scripts using a custom
/// token-based lexer and recursive descent parser. Supports both
/// Groovy and Kotlin DSL syntax.
///
/// # Supported Patterns
/// - String notation: `implementation 'group:name:version'`
/// - Named parameters: `implementation group: 'x', name: 'y', version: 'z'`
/// - Map format: `implementation([group: 'x', name: 'y'])`
/// - Nested functions: `implementation(enforcedPlatform("..."))`
/// - Project references: `implementation(project(":module"))`
/// - String interpolation: `implementation("group:name:${version}")`
///
/// # Implementation
/// Uses a custom token-based lexer (870 lines) instead of tree-sitter for:
/// - Lighter binary size (no external parser dependencies)
/// - Easier maintenance for DSL-specific quirks
/// - Better error messages for malformed input
///
/// Typical usage is calling `GradleParser::extract_first_package()` on a
/// `build.gradle` or `build.gradle.kts` file and then inspecting the returned
/// dependency list.
pub struct GradleParser;

impl PackageParser for GradleParser {
    const PACKAGE_TYPE: PackageType = PackageType::Maven;

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "Gradle build script",
            file_patterns: &["**/build.gradle", "**/build.gradle.kts"],
            package_type: "maven",
            primary_language: "Java",
            documentation_url: Some("https://gradle.org/"),
        }]
    }

    fn is_match(path: &Path) -> bool {
        path.file_name().is_some_and(|name| {
            let name_str = name.to_string_lossy();
            name_str == "build.gradle" || name_str == "build.gradle.kts"
        })
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read {:?}: {}", path, e);
                return vec![default_package_data()];
            }
        };

        let tokens = lex(&content);
        let dependencies = extract_dependencies_with_context(path, &content, &tokens);
        let (
            extracted_license_statement,
            declared_license_expression,
            declared_license_expression_spdx,
            license_detections,
        ) = extract_gradle_license_metadata(&tokens);

        let extra_data = extract_gradle_project_coordinates(path, &content, &tokens);

        vec![PackageData {
            package_type: Some(Self::PACKAGE_TYPE),
            namespace: None,
            name: None,
            version: None,
            qualifiers: None,
            subpath: None,
            primary_language: None,
            description: None,
            release_date: None,
            parties: Vec::new(),
            keywords: Vec::new(),
            homepage_url: None,
            download_url: None,
            size: None,
            sha1: None,
            md5: None,
            sha256: None,
            sha512: None,
            bug_tracking_url: None,
            code_view_url: None,
            vcs_url: None,
            copyright: None,
            holder: None,
            declared_license_expression,
            declared_license_expression_spdx,
            license_detections,
            other_license_expression: None,
            other_license_expression_spdx: None,
            other_license_detections: Vec::new(),
            extracted_license_statement,
            notice_text: None,
            source_packages: Vec::new(),
            file_references: Vec::new(),
            extra_data,
            dependencies,
            repository_homepage_url: None,
            repository_download_url: None,
            api_data_url: None,
            datasource_id: Some(DatasourceId::BuildGradle),
            purl: None,
            is_private: false,
            is_virtual: false,
        }]
    }
}

fn default_package_data() -> PackageData {
    PackageData {
        package_type: Some(GradleParser::PACKAGE_TYPE),
        datasource_id: Some(DatasourceId::BuildGradle),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tok {
    Ident(String),
    Str(String),
    MalformedStr(String),
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    Colon,
    Comma,
    Equals,
}

pub(super) fn lex(input: &str) -> Vec<Tok> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    while i < len {
        if tokens.len() >= MAX_ITERATION_COUNT {
            warn!(
                "Lexer exceeded MAX_ITERATION_COUNT ({}) tokens, stopping",
                MAX_ITERATION_COUNT
            );
            break;
        }
        let c = chars[i];

        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        if c == '\'' {
            i += 1;
            let start = i;
            while i < len && chars[i] != '\'' && chars[i] != '\n' {
                i += 1;
            }
            let val: String = chars[start..i].iter().collect();
            let val = truncate_field(val);
            if i < len && chars[i] == '\'' {
                tokens.push(Tok::Str(val));
                i += 1;
            } else {
                tokens.push(Tok::MalformedStr(val));
            }
            continue;
        }

        if c == '"' {
            i += 1;
            let start = i;
            while i < len && chars[i] != '"' && chars[i] != '\n' {
                if chars[i] == '\\' && i + 1 < len {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            let val: String = chars[start..i].iter().collect();
            let val = truncate_field(val);
            if i < len && chars[i] == '"' {
                tokens.push(Tok::Str(val));
                i += 1;
            } else {
                tokens.push(Tok::MalformedStr(val));
            }
            continue;
        }

        match c {
            '(' => {
                tokens.push(Tok::OpenParen);
                i += 1;
            }
            ')' => {
                tokens.push(Tok::CloseParen);
                i += 1;
            }
            '[' => {
                tokens.push(Tok::OpenBracket);
                i += 1;
            }
            ']' => {
                tokens.push(Tok::CloseBracket);
                i += 1;
            }
            '{' => {
                tokens.push(Tok::OpenBrace);
                i += 1;
            }
            '}' => {
                tokens.push(Tok::CloseBrace);
                i += 1;
            }
            ':' => {
                tokens.push(Tok::Colon);
                i += 1;
            }
            ',' => {
                tokens.push(Tok::Comma);
                i += 1;
            }
            '=' => {
                tokens.push(Tok::Equals);
                i += 1;
            }
            _ if is_ident_start(c) => {
                let start = i;
                while i < len && is_ident_char(chars[i]) {
                    i += 1;
                }
                let val: String = chars[start..i].iter().collect();
                tokens.push(Tok::Ident(truncate_field(val)));
            }
            _ => {
                i += 1;
            }
        }
    }

    tokens
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' || c == '$'
}

// ---------------------------------------------------------------------------
// Dependency block extraction
// ---------------------------------------------------------------------------

fn find_dependency_blocks(tokens: &[Tok]) -> Vec<Vec<Tok>> {
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        if let Tok::Ident(ref name) = tokens[i]
            && name == "dependencies"
            && i + 1 < tokens.len()
            && tokens[i + 1] == Tok::OpenBrace
        {
            i += 2;
            let mut depth = 1;
            let start = i;
            while i < tokens.len() && depth > 0 {
                match &tokens[i] {
                    Tok::OpenBrace => {
                        depth += 1;
                        if depth > MAX_RECURSION_DEPTH {
                            warn!(
                                "Gradle parser: nesting depth exceeded {} in find_dependency_blocks",
                                MAX_RECURSION_DEPTH
                            );
                            break;
                        }
                    }
                    Tok::CloseBrace => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    i += 1;
                }
            }
            blocks.push(tokens[start..i].to_vec());
            if i < tokens.len() {
                i += 1;
            }
            continue;
        }
        i += 1;
    }

    blocks
}

// ---------------------------------------------------------------------------
// Dependency extraction from blocks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RawDep {
    namespace: String,
    name: String,
    version: String,
    scope: String,
    catalog_alias: Option<String>,
    symbolic_ref: Option<String>,
    project_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BuildSrcExpr {
    Literal(String),
    Ref(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildSrcConst {
    scope: String,
    expr: BuildSrcExpr,
}

type BuildSrcConstMap = HashMap<String, BuildSrcConst>;
type BuildSrcCache = HashMap<PathBuf, Option<BuildSrcConstMap>>;

static BUILD_SRC_CONSTANT_CACHE: OnceLock<Mutex<BuildSrcCache>> = OnceLock::new();

fn extract_dependencies_with_context(
    path: &Path,
    content: &str,
    tokens: &[Tok],
) -> Vec<Dependency> {
    let mut raw_dependencies = extract_raw_dependencies(tokens);
    resolve_gradle_script_interpolations(path, content, &mut raw_dependencies);
    resolve_gradle_buildsrc_symbolic_refs(path, &mut raw_dependencies);
    let mut dependencies = raw_dependencies
        .iter()
        .filter_map(create_dependency)
        .collect::<Vec<_>>();
    resolve_gradle_version_catalog_aliases(path, &mut dependencies);
    dependencies
}

#[cfg(test)]
fn extract_dependencies(tokens: &[Tok]) -> Vec<Dependency> {
    extract_raw_dependencies(tokens)
        .iter()
        .filter_map(create_dependency)
        .collect()
}

fn extract_raw_dependencies(tokens: &[Tok]) -> Vec<RawDep> {
    let blocks = find_dependency_blocks(tokens);
    let mut dependencies = Vec::new();

    for block in blocks {
        let parsed = parse_block(&block);
        let limit = capped_iteration_limit(parsed.len(), "gradle dependency block");
        for rd in parsed.into_iter().take(limit) {
            dependencies.push(rd);
        }
    }

    dependencies
}

fn parse_block(tokens: &[Tok]) -> Vec<RawDep> {
    let mut deps = Vec::new();
    let mut i = 0;
    let mut iterations = 0;

    while i < tokens.len() {
        iterations += 1;
        if iterations > MAX_ITERATION_COUNT {
            warn!(
                "parse_block exceeded MAX_ITERATION_COUNT ({}) iterations, stopping",
                MAX_ITERATION_COUNT
            );
            break;
        }

        if let Some(next_index) = parse_control_flow_block(tokens, i, &mut deps) {
            i = next_index;
            continue;
        }

        // Skip nested blocks (closures like `{ transitive = true }`)
        if tokens[i] == Tok::OpenBrace {
            let mut depth = 1;
            i += 1;
            while i < tokens.len() && depth > 0 {
                match &tokens[i] {
                    Tok::OpenBrace => {
                        depth += 1;
                        if depth > MAX_RECURSION_DEPTH {
                            warn!(
                                "Gradle parser: nesting depth exceeded {} in parse_block",
                                MAX_RECURSION_DEPTH
                            );
                            break;
                        }
                    }
                    Tok::CloseBrace => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            continue;
        }

        if let Tok::Str(scope_name) = &tokens[i]
            && i + 1 < tokens.len()
            && tokens[i + 1] == Tok::OpenParen
            && let Some(end) = find_matching_paren(tokens, i + 1)
        {
            let inner = &tokens[i + 2..end];
            parse_paren_content(scope_name, inner, &mut deps);
            i = end + 1;
            continue;
        }

        let scope_name = match &tokens[i] {
            Tok::Ident(name) => name.clone(),
            _ => {
                i += 1;
                continue;
            }
        };

        if is_skip_keyword(&scope_name) {
            i += 1;
            continue;
        }

        let next = i + 1;

        // PATTERN: scope ( ... )  — parenthesized dependency
        if next < tokens.len() && tokens[next] == Tok::OpenParen {
            let paren_end = find_matching_paren(tokens, next);
            if let Some(end) = paren_end {
                let inner = &tokens[next + 1..end];
                parse_paren_content(&scope_name, inner, &mut deps);
                i = end + 1;
                continue;
            }
        }

        // PATTERN: scope group: ..., name: ..., version: ... (named params without parens)
        if next < tokens.len()
            && let Tok::Ident(ref label) = tokens[next]
            && label == "group"
            && next + 1 < tokens.len()
            && tokens[next + 1] == Tok::Colon
            && let Some((rd, consumed)) = parse_named_params(&scope_name, &tokens[next..])
        {
            deps.push(rd);
            i = next + consumed;
            continue;
        }

        // PATTERN: scope 'string:notation' (string notation)
        if next < tokens.len()
            && matches!(
                tokens.get(next),
                Some(Tok::Str(_)) | Some(Tok::MalformedStr(_))
            )
        {
            let (val, is_malformed) = match &tokens[next] {
                Tok::Str(val) => (val.as_str(), false),
                Tok::MalformedStr(val) => (val.as_str(), true),
                _ => unreachable!(),
            };

            if !val.contains(':') {
                i = next + 1;
                continue;
            }

            if val.chars().next().is_some_and(|c| c.is_whitespace()) {
                break;
            }

            // `scope 'str', { closure }` → skip (unparenthesized call with trailing closure)
            if next + 1 < tokens.len()
                && tokens[next + 1] == Tok::Comma
                && next + 2 < tokens.len()
                && tokens[next + 2] == Tok::OpenBrace
            {
                i = next + 1;
                continue;
            }
            let is_multi = i + 2 < tokens.len()
                && tokens[next + 1] == Tok::Comma
                && matches!(tokens.get(next + 2), Some(Tok::Str(_)));
            let effective_scope = if is_multi { "" } else { &scope_name };
            let rd = parse_colon_string(val, effective_scope);
            deps.push(rd);
            if is_malformed {
                break;
            }
            i = next + 1;
            while i < tokens.len() && tokens[i] == Tok::Comma {
                i += 1;
                if i < tokens.len()
                    && let Tok::Str(ref v2) = tokens[i]
                    && v2.contains(':')
                {
                    deps.push(parse_colon_string(v2, ""));
                    i += 1;
                    continue;
                }
                break;
            }
            continue;
        }

        // PATTERN: scope libs.foo.bar (version catalog alias)
        // Keep TOML-backed `libs.*` aliases for later version-catalog resolution,
        // but ignore other unresolved dotted identifiers such as `dependencies.*`
        // or arbitrary constants like `Deps.AndroidX.core`.
        if next < tokens.len()
            && let Tok::Ident(ref val) = tokens[next]
            && val.starts_with("libs.")
            && let Some(last_seg) = val.rsplit('.').next()
            && !last_seg.is_empty()
        {
            deps.push(RawDep {
                namespace: String::new(),
                name: truncate_field(last_seg.to_string()),
                version: String::new(),
                scope: truncate_field(scope_name.clone()),
                catalog_alias: val
                    .strip_prefix("libs.")
                    .map(|alias| truncate_field(alias.to_string())),
                symbolic_ref: None,
                project_path: None,
            });
            i = next + 1;
            continue;
        }

        if next < tokens.len()
            && let Tok::Ident(ref val) = tokens[next]
            && val.contains('.')
        {
            deps.push(parse_symbolic_ref(&scope_name, val));
            i = next + 1;
            continue;
        }

        // PATTERN: scope project(':module') — project reference without parens
        if next < tokens.len()
            && let Tok::Ident(ref name) = tokens[next]
            && name == "project"
            && next + 1 < tokens.len()
            && tokens[next + 1] == Tok::OpenParen
            && let Some(end) = find_matching_paren(tokens, next + 1)
        {
            let inner = &tokens[next + 2..end];
            if let Some(rd) = parse_project_ref(inner, &scope_name) {
                deps.push(rd);
            }
            i = end + 1;
            continue;
        }

        i += 1;
    }

    deps
}

fn parse_control_flow_block(tokens: &[Tok], start: usize, deps: &mut Vec<RawDep>) -> Option<usize> {
    let Tok::Ident(keyword) = tokens.get(start)? else {
        return None;
    };

    if keyword != "if" && keyword != "else" {
        return None;
    }

    let mut block_start = start + 1;
    if keyword == "if" {
        if tokens.get(block_start) != Some(&Tok::OpenParen) {
            return None;
        }
        let cond_end = find_matching_paren(tokens, block_start)?;
        block_start = cond_end + 1;
    } else if let Some(Tok::Ident(next)) = tokens.get(block_start)
        && next == "if"
    {
        return parse_control_flow_block(tokens, block_start, deps);
    }

    if tokens.get(block_start) != Some(&Tok::OpenBrace) {
        return None;
    }

    let block_end = find_matching_brace(tokens, block_start)?;
    deps.extend(parse_block(&tokens[block_start + 1..block_end]));
    Some(block_end + 1)
}

fn is_skip_keyword(name: &str) -> bool {
    matches!(
        name,
        "plugins"
            | "apply"
            | "ext"
            | "configurations"
            | "repositories"
            | "subprojects"
            | "allprojects"
            | "buildscript"
            | "pluginManager"
            | "publishing"
            | "sourceSets"
            | "tasks"
            | "task"
    )
}

fn parse_paren_content(scope: &str, tokens: &[Tok], deps: &mut Vec<RawDep>) {
    if tokens.is_empty() {
        return;
    }

    // Check for bracket-enclosed maps: [group: ..., name: ..., version: ...]
    if tokens[0] == Tok::OpenBracket {
        parse_bracket_maps(tokens, deps);
        return;
    }

    // Check for named parameters: group: 'x' or group = "x"
    if let Some(Tok::Ident(label)) = tokens.first()
        && label == "group"
        && tokens.len() > 1
        && tokens[1] == Tok::Colon
    {
        if let Some((rd, _)) = parse_named_params("", tokens) {
            deps.push(rd);
        }
        return;
    }

    // Check for nested function call or project reference
    if let Some(Tok::Ident(inner_fn)) = tokens.first()
        && tokens.len() > 1
        && tokens[1] == Tok::OpenParen
    {
        if inner_fn == "project" {
            if let Some(end) = find_matching_paren(tokens, 1) {
                let inner = &tokens[2..end];
                if let Some(rd) = parse_project_ref(inner, scope) {
                    deps.push(rd);
                }
            }
            return;
        }

        if let Some(end) = find_matching_paren(tokens, 1) {
            let inner = &tokens[2..end];
            if let Some(Tok::Str(val)) = inner.first()
                && val.contains(':')
            {
                deps.push(parse_colon_string(val, inner_fn));
                return;
            }

            if let Some(Tok::Ident(val)) = inner.first()
                && val.contains('.')
            {
                deps.push(parse_symbolic_ref(inner_fn, val));
                return;
            }
        }
    }

    if let Some(Tok::Ident(val)) = tokens.first()
        && val.contains('.')
    {
        deps.push(parse_symbolic_ref(scope, val));
        return;
    }

    // Simple string: ("g:n:v")
    if let Some(Tok::Str(val)) = tokens.first()
        && val.contains(':')
    {
        deps.push(parse_colon_string(val, scope));
    }
}

fn parse_bracket_maps(tokens: &[Tok], deps: &mut Vec<RawDep>) {
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == Tok::OpenBracket
            && let Some(end) = find_matching_bracket(tokens, i)
        {
            let map_tokens = &tokens[i + 1..end];
            if let Some(rd) = parse_map_entries(map_tokens)
                && !contains_equivalent_map_dep(deps, &rd)
            {
                deps.push(rd);
            }
            i = end + 1;
            continue;
        }
        i += 1;
    }
}

fn contains_equivalent_map_dep(existing: &[RawDep], candidate: &RawDep) -> bool {
    existing.iter().any(|dep| {
        dep.name == candidate.name
            && dep.version == candidate.version
            && dep.scope == candidate.scope
            && (dep.namespace == candidate.namespace
                || dep.namespace.is_empty()
                || candidate.namespace.is_empty())
    })
}

fn parse_map_entries(tokens: &[Tok]) -> Option<RawDep> {
    let mut name = String::new();
    let mut version = String::new();
    let mut i = 0;

    while i < tokens.len() {
        if let Tok::Ident(ref label) = tokens[i]
            && i + 2 < tokens.len()
            && tokens[i + 1] == Tok::Colon
            && let Tok::Str(ref val) = tokens[i + 2]
        {
            match label.as_str() {
                "name" => name = truncate_field(val.clone()),
                "version" => version = truncate_field(val.clone()),
                _ => {}
            }
            i += 3;
            if i < tokens.len() && tokens[i] == Tok::Comma {
                i += 1;
            }
            continue;
        }
        i += 1;
    }

    if name.is_empty() {
        return None;
    }

    Some(RawDep {
        namespace: String::new(),
        name,
        version,
        scope: String::new(),
        catalog_alias: None,
        symbolic_ref: None,
        project_path: None,
    })
}

fn parse_named_params(scope: &str, tokens: &[Tok]) -> Option<(RawDep, usize)> {
    let mut group = String::new();
    let mut name = String::new();
    let mut version = String::new();
    let mut i = 0;

    while i < tokens.len() {
        if let Tok::Ident(ref label) = tokens[i]
            && i + 2 < tokens.len()
            && tokens[i + 1] == Tok::Colon
            && let Tok::Str(ref val) = tokens[i + 2]
        {
            match label.as_str() {
                "group" => group = truncate_field(val.clone()),
                "name" => name = truncate_field(val.clone()),
                "version" => version = truncate_field(val.clone()),
                _ => {}
            }
            i += 3;
            if i < tokens.len() && tokens[i] == Tok::Comma {
                i += 1;
            }
            continue;
        }
        break;
    }

    if name.is_empty() {
        return None;
    }

    Some((
        RawDep {
            namespace: group,
            name,
            version,
            scope: scope.to_string(),
            catalog_alias: None,
            symbolic_ref: None,
            project_path: None,
        },
        i,
    ))
}

fn parse_project_ref(tokens: &[Tok], scope: &str) -> Option<RawDep> {
    if let Some(Tok::Str(val)) = tokens.first() {
        let module_name = val.trim_start_matches(':');
        let mut segments = module_name
            .split(':')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        let name = segments.pop().unwrap_or(module_name);
        if name.is_empty() {
            return None;
        }
        return Some(RawDep {
            namespace: if segments.is_empty() {
                String::new()
            } else {
                truncate_field(segments.join("/"))
            },
            name: truncate_field(name.to_string()),
            version: String::new(),
            scope: truncate_field(scope.to_string()),
            catalog_alias: None,
            symbolic_ref: None,
            project_path: Some(truncate_field(module_name.to_string())),
        });
    }
    None
}

fn parse_symbolic_ref(scope: &str, value: &str) -> RawDep {
    RawDep {
        namespace: String::new(),
        name: String::new(),
        version: String::new(),
        scope: truncate_field(scope.to_string()),
        catalog_alias: None,
        symbolic_ref: Some(truncate_field(value.to_string())),
        project_path: None,
    }
}

fn parse_colon_string(val: &str, scope: &str) -> RawDep {
    let parts: Vec<&str> = val.split(':').collect();
    let (namespace, name, version) = match parts.len() {
        n if n >= 4 => (
            truncate_field(parts[0].to_string()),
            truncate_field(parts[1].to_string()),
            truncate_field(parts[2].to_string()),
        ),
        3 => (
            truncate_field(parts[0].to_string()),
            truncate_field(parts[1].to_string()),
            truncate_field(parts[2].to_string()),
        ),
        2 => (
            truncate_field(parts[0].to_string()),
            truncate_field(parts[1].to_string()),
            String::new(),
        ),
        _ => (
            String::new(),
            truncate_field(val.to_string()),
            String::new(),
        ),
    };

    RawDep {
        namespace,
        name,
        version,
        scope: truncate_field(scope.to_string()),
        catalog_alias: None,
        symbolic_ref: None,
        project_path: None,
    }
}

fn find_matching_paren(tokens: &[Tok], start: usize) -> Option<usize> {
    if tokens.get(start) != Some(&Tok::OpenParen) {
        return None;
    }
    let mut depth = 1;
    let mut i = start + 1;
    while i < tokens.len() && depth > 0 {
        match &tokens[i] {
            Tok::OpenParen => {
                depth += 1;
                if depth > MAX_RECURSION_DEPTH {
                    warn!(
                        "Gradle parser: nesting depth exceeded {} in find_matching_paren",
                        MAX_RECURSION_DEPTH
                    );
                    break;
                }
            }
            Tok::CloseParen => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_matching_bracket(tokens: &[Tok], start: usize) -> Option<usize> {
    if tokens.get(start) != Some(&Tok::OpenBracket) {
        return None;
    }
    let mut depth = 1;
    let mut i = start + 1;
    while i < tokens.len() && depth > 0 {
        match &tokens[i] {
            Tok::OpenBracket => {
                depth += 1;
                if depth > MAX_RECURSION_DEPTH {
                    warn!(
                        "Gradle parser: nesting depth exceeded {} in find_matching_bracket",
                        MAX_RECURSION_DEPTH
                    );
                    break;
                }
            }
            Tok::CloseBracket => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Some(i);
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Dependency construction
// ---------------------------------------------------------------------------

fn create_dependency(raw: &RawDep) -> Option<Dependency> {
    let namespace = raw.namespace.as_str();
    let name = raw.name.as_str();
    let version = raw.version.as_str();
    let scope = raw.scope.as_str();
    if name.is_empty() {
        return None;
    }

    let mut purl = PackageUrl::new("maven", name).ok()?;

    if !namespace.is_empty() {
        purl.with_namespace(namespace).ok()?;
    }

    if !version.is_empty() {
        purl.with_version(version).ok()?;
    }

    let (is_runtime, is_optional) = classify_scope(scope);
    let is_pinned = !version.is_empty();

    let purl_string = truncate_field(purl.to_string().replace("$", "%24").replace('\'', "%27"));
    let mut extra_data = std::collections::HashMap::new();
    if let Some(alias) = &raw.catalog_alias {
        extra_data.insert(
            "catalog_alias".to_string(),
            json!(truncate_field(alias.clone())),
        );
    }
    if let Some(project_path) = &raw.project_path {
        extra_data.insert(
            "project_path".to_string(),
            json!(truncate_field(project_path.clone())),
        );
    }
    if let Some(symbolic_ref) = &raw.symbolic_ref {
        extra_data.insert(
            "symbolic_ref".to_string(),
            json!(truncate_field(symbolic_ref.clone())),
        );
    }

    Some(Dependency {
        purl: Some(purl_string),
        extracted_requirement: Some(truncate_field(version.to_string())),
        scope: Some(truncate_field(scope.to_string())),
        is_runtime: Some(is_runtime),
        is_optional: Some(is_optional),
        is_pinned: Some(is_pinned),
        is_direct: Some(true),
        resolved_package: None,
        extra_data: (!extra_data.is_empty()).then_some(extra_data),
    })
}

fn classify_scope(scope: &str) -> (bool, bool) {
    let scope_lower = scope.to_lowercase();

    if scope_lower.contains("test") {
        return (false, true);
    }

    if matches!(
        scope_lower.as_str(),
        "compileonly" | "compileonlyapi" | "annotationprocessor" | "kapt" | "ksp"
    ) {
        return (false, false);
    }

    (true, false)
}

fn resolve_gradle_script_interpolations(
    path: &Path,
    content: &str,
    raw_dependencies: &mut [RawDep],
) {
    let properties = load_gradle_script_properties(path, content);
    if properties.is_empty() {
        return;
    }

    for raw in raw_dependencies.iter_mut() {
        raw.namespace = interpolate_gradle_string(&raw.namespace, &properties);
        raw.name = interpolate_gradle_string(&raw.name, &properties);
        raw.version = interpolate_gradle_string(&raw.version, &properties);
    }
}

fn load_gradle_script_properties(path: &Path, content: &str) -> HashMap<String, String> {
    let mut properties = load_gradle_properties(path);

    let literal_assignment_patterns = [
        regex::Regex::new(
            r#"(?m)^\s*(?:const\s+)?(?:val|var|def)\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?::[^=\n]+)?=\s*['\"]([^'\"]+)['\"]"#,
        )
        .expect("valid regex"),
        regex::Regex::new(r#"(?m)^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*['\"]([^'\"]+)['\"]"#)
            .expect("valid regex"),
    ];

    for pattern in literal_assignment_patterns {
        for captures in pattern
            .captures_iter(content)
            .capped("gradle literal assignments")
        {
            let Some(name) = captures.get(1).map(|value| value.as_str().trim()) else {
                continue;
            };
            let Some(raw_value) = captures.get(2).map(|value| value.as_str()) else {
                continue;
            };
            let resolved = interpolate_gradle_string(raw_value, &properties);
            properties.insert(name.to_string(), resolved);
        }
    }

    let delegated_project_property_pattern = regex::Regex::new(
        r#"(?m)^\s*(?:val|var)\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?::[^=\n]+)?\s+by\s+project\b"#,
    )
    .expect("valid regex");

    for captures in delegated_project_property_pattern
        .captures_iter(content)
        .capped("gradle delegated project properties")
    {
        let Some(name) = captures.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };
        if let Some(value) = properties.get(name).cloned() {
            properties.insert(name.to_string(), value);
        }
    }

    properties
}

fn load_gradle_properties(path: &Path) -> HashMap<String, String> {
    for ancestor in path.ancestors() {
        let gradle_properties = ancestor.join("gradle.properties");
        if !gradle_properties.is_file() {
            continue;
        }

        let Ok(content) = read_file_to_string(&gradle_properties, None) else {
            continue;
        };

        let mut properties = HashMap::new();
        for line in content.lines().capped("gradle.properties lines") {
            let trimmed = line.split('#').next().unwrap_or("").trim();
            if trimmed.is_empty() {
                continue;
            }

            let Some((key, value)) = trimmed.split_once('=').or_else(|| trimmed.split_once(':'))
            else {
                continue;
            };

            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                continue;
            }
            properties.insert(key.to_string(), value.to_string());
        }
        return properties;
    }

    HashMap::new()
}

fn interpolate_gradle_string(value: &str, properties: &HashMap<String, String>) -> String {
    if !value.contains('$') {
        return truncate_field(value.to_string());
    }

    let chars = value.chars().collect::<Vec<_>>();
    let mut rendered = String::new();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '$' {
            rendered.push(chars[i]);
            i += 1;
            continue;
        }

        if i + 1 >= chars.len() {
            rendered.push(chars[i]);
            break;
        }

        if chars[i + 1] == '{' {
            let start = i;
            i += 2;
            let mut reference = String::new();
            while i < chars.len() && chars[i] != '}' {
                reference.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == '}' {
                i += 1;
            }

            if let Some(resolved) = properties.get(reference.trim()) {
                rendered.push_str(resolved);
            } else {
                rendered.push_str(&value[start..i]);
            }
            continue;
        }

        let start = i;
        i += 1;
        let mut reference = String::new();
        while i < chars.len() && matches!(chars[i], 'A'..='Z' | 'a'..='z' | '0'..='9' | '_') {
            reference.push(chars[i]);
            i += 1;
        }

        if reference.is_empty() {
            rendered.push('$');
            continue;
        }

        if let Some(resolved) = properties.get(reference.as_str()) {
            rendered.push_str(resolved);
        } else {
            rendered.push_str(&value[start..i]);
        }
    }

    truncate_field(rendered)
}

fn resolve_gradle_buildsrc_symbolic_refs(path: &Path, raw_dependencies: &mut [RawDep]) {
    let ancestor_build_src_dir = find_build_src_dir(path);
    let ancestor_constants = ancestor_build_src_dir
        .as_deref()
        .and_then(load_build_src_constants);
    let sibling_build_src_tiers = if ancestor_build_src_dir.is_none() {
        find_nearby_sibling_build_src_tiers(path)
    } else {
        Vec::new()
    };

    for raw in raw_dependencies.iter_mut() {
        let Some(symbolic_ref) = raw.symbolic_ref.as_deref() else {
            continue;
        };

        let resolved = ancestor_constants
            .as_ref()
            .and_then(|constants| {
                let mut visiting = HashSet::new();
                resolve_build_src_value(symbolic_ref, constants, &mut visiting)
            })
            .or_else(|| {
                resolve_nearby_sibling_build_src_value(symbolic_ref, &sibling_build_src_tiers)
            });
        let Some(resolved) = resolved else {
            continue;
        };
        if !resolved.contains(':') {
            continue;
        }

        let resolved_dependency = parse_colon_string(&resolved, &raw.scope);
        raw.namespace = resolved_dependency.namespace;
        raw.name = resolved_dependency.name;
        raw.version = resolved_dependency.version;
    }
}

fn find_build_src_dir(path: &Path) -> Option<PathBuf> {
    for ancestor in path.ancestors() {
        let build_src_dir = ancestor.join("buildSrc");
        if build_src_dir.is_dir() {
            return Some(build_src_dir);
        }
    }
    None
}

fn find_nearby_sibling_build_src_tiers(path: &Path) -> Vec<Vec<PathBuf>> {
    let mut tiers = Vec::new();

    for ancestor in path
        .ancestors()
        .skip(1)
        .capped("gradle sibling buildSrc ancestors")
    {
        let sibling_dirs = collect_sibling_build_src_dirs(ancestor, path);
        if !sibling_dirs.is_empty() {
            tiers.push(sibling_dirs);
        }
    }

    tiers
}

fn collect_sibling_build_src_dirs(ancestor: &Path, current_path: &Path) -> Vec<PathBuf> {
    if !ancestor.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(ancestor) else {
        return Vec::new();
    };

    let mut build_src_dirs = Vec::new();
    for entry in entries.flatten().capped("gradle sibling directory entries") {
        let child_dir = entry.path();
        if !child_dir.is_dir() || current_path.starts_with(&child_dir) {
            continue;
        }

        let build_src_dir = child_dir.join("buildSrc");
        if !build_src_dir.is_dir() || !has_gradle_settings_file(&child_dir) {
            continue;
        }

        build_src_dirs.push(build_src_dir);
    }

    build_src_dirs.sort();
    build_src_dirs
}

fn has_gradle_settings_file(dir: &Path) -> bool {
    dir.join("settings.gradle").is_file() || dir.join("settings.gradle.kts").is_file()
}

fn resolve_nearby_sibling_build_src_value(
    symbolic_ref: &str,
    sibling_build_src_tiers: &[Vec<PathBuf>],
) -> Option<String> {
    let tier_limit = capped_iteration_limit(
        sibling_build_src_tiers.len(),
        "gradle sibling buildSrc tiers",
    );
    for sibling_build_src_dirs in sibling_build_src_tiers.iter().take(tier_limit) {
        let mut resolved_value: Option<String> = None;

        let dir_limit =
            capped_iteration_limit(sibling_build_src_dirs.len(), "gradle sibling buildSrc dirs");
        for build_src_dir in sibling_build_src_dirs.iter().take(dir_limit) {
            let Some(constants) = load_build_src_constants(build_src_dir) else {
                continue;
            };

            let mut visiting = HashSet::new();
            let Some(candidate) = resolve_build_src_value(symbolic_ref, &constants, &mut visiting)
            else {
                continue;
            };
            if !candidate.contains(':') {
                continue;
            }

            match &resolved_value {
                None => resolved_value = Some(candidate),
                Some(existing) if existing == &candidate => {}
                Some(_) => return None,
            }
        }

        if resolved_value.is_some() {
            return resolved_value;
        }
    }

    None
}

fn load_build_src_constants(build_src_dir: &Path) -> Option<BuildSrcConstMap> {
    let cache = BUILD_SRC_CONSTANT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(cached) = guard.get(build_src_dir)
    {
        return cached.clone();
    }

    let parsed = parse_build_src_constants_dir(build_src_dir);

    if let Ok(mut guard) = cache.lock() {
        guard.insert(build_src_dir.to_path_buf(), parsed.clone());
    }

    parsed
}

fn parse_build_src_constants_dir(build_src_dir: &Path) -> Option<BuildSrcConstMap> {
    let mut kotlin_files = Vec::new();
    for source_dir in [
        build_src_dir.join("src").join("main").join("java"),
        build_src_dir.join("src").join("main").join("kotlin"),
    ] {
        collect_build_src_kotlin_files(&source_dir, &mut kotlin_files);
    }

    if kotlin_files.is_empty() {
        return None;
    }

    let mut constants = HashMap::new();
    let limit = capped_iteration_limit(kotlin_files.len(), "gradle buildSrc kotlin files");
    for file in kotlin_files.into_iter().take(limit) {
        let Ok(content) = read_file_to_string(&file, None) else {
            continue;
        };
        constants.extend(parse_build_src_constants(&content));
    }

    (!constants.is_empty()).then_some(constants)
}

fn collect_build_src_kotlin_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if files.len() >= MAX_ITERATION_COUNT || !dir.is_dir() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries
        .flatten()
        .capped("gradle buildSrc directory entries")
    {
        if files.len() >= MAX_ITERATION_COUNT {
            break;
        }

        let path = entry.path();
        if path.is_dir() {
            collect_build_src_kotlin_files(&path, files);
            continue;
        }

        if path.extension().is_some_and(|ext| ext == "kt") {
            files.push(path);
        }
    }
}

fn parse_build_src_constants(content: &str) -> BuildSrcConstMap {
    let tokens = lex(content);
    let mut constants = HashMap::new();
    let mut object_stack = Vec::new();
    let mut brace_stack: Vec<Option<String>> = Vec::new();
    let mut i = 0;

    while i < tokens.len() && i < MAX_ITERATION_COUNT {
        if let Some((name, consumed)) = parse_object_declaration(&tokens[i..]) {
            object_stack.push(name.clone());
            brace_stack.push(Some(name));
            i += consumed;
            continue;
        }

        if let Some((name, expr, consumed)) = parse_build_src_const_definition(&tokens[i..]) {
            let scope = object_stack.join(".");
            let full_name = if scope.is_empty() {
                name.clone()
            } else {
                format!("{scope}.{name}")
            };
            constants.insert(
                truncate_field(full_name),
                BuildSrcConst {
                    scope: truncate_field(scope),
                    expr,
                },
            );
            i += consumed;
            continue;
        }

        match &tokens[i] {
            Tok::OpenBrace => brace_stack.push(None),
            Tok::CloseBrace => {
                if let Some(marker) = brace_stack.pop()
                    && marker.is_some()
                {
                    object_stack.pop();
                }
            }
            _ => {}
        }

        i += 1;
    }

    constants
}

fn parse_object_declaration(tokens: &[Tok]) -> Option<(String, usize)> {
    if let [Tok::Ident(keyword), Tok::Ident(name), Tok::OpenBrace, ..] = tokens
        && keyword == "object"
    {
        return Some((truncate_field(name.clone()), 3));
    }
    None
}

fn parse_build_src_const_definition(tokens: &[Tok]) -> Option<(String, BuildSrcExpr, usize)> {
    let mut cursor = 0;

    while let Some(Tok::Ident(modifier)) = tokens.get(cursor) {
        if matches!(
            modifier.as_str(),
            "private" | "internal" | "public" | "protected"
        ) {
            cursor += 1;
            continue;
        }
        break;
    }

    if !matches!(tokens.get(cursor), Some(Tok::Ident(keyword)) if keyword == "const")
        || !matches!(tokens.get(cursor + 1), Some(Tok::Ident(keyword)) if keyword == "val")
    {
        return None;
    }

    let Tok::Ident(name) = tokens.get(cursor + 2)? else {
        return None;
    };
    if tokens.get(cursor + 3) != Some(&Tok::Equals) {
        return None;
    }

    let expr = match tokens.get(cursor + 4)? {
        Tok::Str(value) => BuildSrcExpr::Literal(truncate_field(value.clone())),
        Tok::Ident(value) => BuildSrcExpr::Ref(truncate_field(value.clone())),
        _ => return None,
    };

    Some((truncate_field(name.clone()), expr, cursor + 5))
}

fn resolve_build_src_value(
    key: &str,
    constants: &BuildSrcConstMap,
    visiting: &mut HashSet<String>,
) -> Option<String> {
    if !visiting.insert(key.to_string()) {
        return None;
    }

    let resolved = constants
        .get(key)
        .and_then(|constant| resolve_build_src_expr(constant, constants, visiting));
    visiting.remove(key);
    resolved
}

fn resolve_build_src_expr(
    constant: &BuildSrcConst,
    constants: &BuildSrcConstMap,
    visiting: &mut HashSet<String>,
) -> Option<String> {
    match &constant.expr {
        BuildSrcExpr::Literal(value) => Some(interpolate_build_src_string(
            value,
            &constant.scope,
            constants,
            visiting,
        )),
        BuildSrcExpr::Ref(reference) => {
            resolve_build_src_symbol(&constant.scope, reference, constants, visiting)
        }
    }
}

fn resolve_build_src_symbol(
    scope: &str,
    reference: &str,
    constants: &BuildSrcConstMap,
    visiting: &mut HashSet<String>,
) -> Option<String> {
    if reference.contains('.') {
        return resolve_build_src_value(reference, constants, visiting);
    }

    let mut current_scope = Some(scope);
    while let Some(scope_name) = current_scope {
        if !scope_name.is_empty() {
            let candidate = format!("{scope_name}.{reference}");
            if let Some(value) = resolve_build_src_value(&candidate, constants, visiting) {
                return Some(value);
            }
        }

        current_scope = scope_name.rsplit_once('.').map(|(parent, _)| parent);
    }

    resolve_build_src_value(reference, constants, visiting)
}

fn interpolate_build_src_string(
    value: &str,
    scope: &str,
    constants: &BuildSrcConstMap,
    visiting: &mut HashSet<String>,
) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let mut rendered = String::new();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '$' {
            rendered.push(chars[i]);
            i += 1;
            continue;
        }

        if i + 1 >= chars.len() {
            rendered.push(chars[i]);
            break;
        }

        if chars[i + 1] == '{' {
            let start = i;
            i += 2;
            let mut reference = String::new();
            while i < chars.len() && chars[i] != '}' {
                reference.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == '}' {
                i += 1;
            }

            if let Some(resolved) = resolve_build_src_symbol(scope, &reference, constants, visiting)
            {
                rendered.push_str(&resolved);
            } else {
                rendered.push_str(&value[start..i]);
            }
            continue;
        }

        let start = i;
        i += 1;
        let mut reference = String::new();
        while i < chars.len() && matches!(chars[i], 'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '.') {
            reference.push(chars[i]);
            i += 1;
        }

        if reference.is_empty() {
            rendered.push('$');
            continue;
        }

        if let Some(resolved) = resolve_build_src_symbol(scope, &reference, constants, visiting) {
            rendered.push_str(&resolved);
        } else {
            rendered.push_str(&value[start..i]);
        }
    }

    truncate_field(rendered)
}

#[derive(Debug, Clone)]
struct GradleCatalogEntry {
    namespace: String,
    name: String,
    version: Option<String>,
}

fn resolve_gradle_version_catalog_aliases(path: &Path, dependencies: &mut [Dependency]) {
    let Some(catalog_path) = find_gradle_version_catalog(path) else {
        return;
    };
    let Some(entries) = parse_gradle_version_catalog(&catalog_path) else {
        return;
    };

    for dep in dependencies.iter_mut() {
        let alias = dep
            .extra_data
            .as_ref()
            .and_then(|data| data.get("catalog_alias"))
            .and_then(|value| value.as_str());
        let Some(alias) = alias else {
            continue;
        };
        let Some(entry) = entries.get(alias) else {
            continue;
        };

        let mut purl = PackageUrl::new("maven", &entry.name).ok();
        if let Some(ref mut purl) = purl {
            if !entry.namespace.is_empty() {
                let _ = purl.with_namespace(&entry.namespace);
            }
            if let Some(version) = &entry.version {
                let _ = purl.with_version(version);
            }
        }

        dep.purl = purl.map(|p| truncate_field(p.to_string()));
        dep.extracted_requirement = entry.version.as_ref().map(|v| truncate_field(v.clone()));
        dep.is_pinned = Some(entry.version.is_some());
    }
}

fn find_gradle_version_catalog(path: &Path) -> Option<std::path::PathBuf> {
    for ancestor in path.ancestors() {
        let nested = ancestor.join("gradle").join("libs.versions.toml");
        if nested.is_file() {
            return Some(nested);
        }

        let sibling = ancestor.join("libs.versions.toml");
        if sibling.is_file() {
            return Some(sibling);
        }
    }

    None
}

fn parse_gradle_version_catalog(
    path: &Path,
) -> Option<std::collections::HashMap<String, GradleCatalogEntry>> {
    let content = read_file_to_string(path, None).ok()?;
    let mut section = "";
    let mut versions = std::collections::HashMap::new();
    let mut libraries = std::collections::HashMap::new();

    for line in content.lines().capped("gradle version catalog lines") {
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed.trim_matches(&['[', ']'][..]);
            continue;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim().to_string();

        match section {
            "versions" => {
                versions.insert(key, truncate_field(strip_quotes(&value).to_string()));
            }
            "libraries" => {
                libraries.insert(key, value);
            }
            _ => {}
        }
    }

    let mut result = std::collections::HashMap::new();
    // `libraries` is a std HashMap; sort by alias so truncation drops a
    // deterministic set of entries when the cap is exceeded.
    let mut libraries: Vec<(String, String)> = libraries.into_iter().collect();
    libraries.sort_by(|a, b| a.0.cmp(&b.0));
    let limit = capped_iteration_limit(libraries.len(), "gradle version catalog libraries");
    for (alias, raw_value) in libraries.into_iter().take(limit) {
        let Some(entry) = parse_gradle_catalog_entry(&raw_value, &versions) else {
            continue;
        };
        result.insert(truncate_field(alias.replace('-', ".")), entry);
    }

    Some(result)
}

fn parse_gradle_catalog_entry(
    raw_value: &str,
    versions: &std::collections::HashMap<String, String>,
) -> Option<GradleCatalogEntry> {
    if raw_value.starts_with('"') && raw_value.ends_with('"') {
        let notation = strip_quotes(raw_value);
        let mut parts = notation.split(':');
        let namespace = truncate_field(parts.next()?.to_string());
        let name = truncate_field(parts.next()?.to_string());
        let version = parts.next().map(|v| truncate_field(v.to_string()));
        return Some(GradleCatalogEntry {
            namespace,
            name,
            version,
        });
    }

    if !(raw_value.starts_with('{') && raw_value.ends_with('}')) {
        return None;
    }

    let inner = &raw_value[1..raw_value.len() - 1];
    let mut fields = std::collections::HashMap::new();
    for pair in inner.split(',').capped("gradle catalog entry fields") {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        fields.insert(
            truncate_field(key.trim().to_string()),
            truncate_field(strip_quotes(value.trim()).to_string()),
        );
    }

    let (namespace, name) = if let Some(module) = fields.get("module") {
        let (group, artifact) = module.split_once(':')?;
        (
            truncate_field(group.to_string()),
            truncate_field(artifact.to_string()),
        )
    } else {
        (
            truncate_field(fields.get("group")?.to_string()),
            truncate_field(fields.get("name")?.to_string()),
        )
    };

    let version = if let Some(version) = fields.get("version") {
        Some(truncate_field(version.to_string()))
    } else if let Some(version_ref) = fields.get("version.ref") {
        versions.get(version_ref).cloned().map(truncate_field)
    } else {
        None
    };

    Some(GradleCatalogEntry {
        namespace,
        name,
        version,
    })
}

fn strip_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value)
}

/// Extract the project's declared Maven coordinates (`group` and `version`) from
/// top-level Gradle statements so multi-project assembly can build an honest
/// `pkg:maven/<group>/<name>@<version>` identity for each subproject. Gradle does
/// not carry the project *name* in the build script (it defaults to the directory
/// or is set in `settings.gradle`), so only `group`/`version` are recovered here.
///
/// Only literal, top-level (brace-depth 0) assignments are recovered — both the
/// Kotlin/`=` form (`group = "com.example"`) and the Groovy method-call form
/// (`group "com.example"`). Values referencing variables are interpolated through
/// the same `gradle.properties`/script-constant resolution used for dependency
/// versions; an unresolved value is left unset rather than guessed.
fn extract_gradle_project_coordinates(
    path: &Path,
    content: &str,
    tokens: &[Tok],
) -> Option<std::collections::HashMap<String, serde_json::Value>> {
    let mut group: Option<String> = None;
    let mut version: Option<String> = None;
    let mut depth: i32 = 0;

    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Tok::OpenBrace => depth += 1,
            Tok::CloseBrace => depth = depth.saturating_sub(1),
            Tok::Ident(name) if depth == 0 && (name == "group" || name == "version") => {
                // Accept `group = "x"`, `group "x"`, and `group("x")`.
                let mut cursor = i + 1;
                if tokens.get(cursor) == Some(&Tok::Equals) {
                    cursor += 1;
                }
                let mut had_paren = false;
                if tokens.get(cursor) == Some(&Tok::OpenParen) {
                    had_paren = true;
                    cursor += 1;
                }
                if let Some(Tok::Str(value)) = tokens.get(cursor) {
                    let literal = value.clone();
                    if name == "group" && group.is_none() {
                        group = Some(literal);
                    } else if name == "version" && version.is_none() {
                        version = Some(literal);
                    }
                    i = cursor + if had_paren { 2 } else { 1 };
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if group.is_none() && version.is_none() {
        return None;
    }

    let properties = load_gradle_script_properties(path, content);
    let mut extra_data = std::collections::HashMap::new();
    if let Some(group) = group {
        let resolved = interpolate_gradle_string(&group, &properties);
        if !resolved.contains('$') && !resolved.is_empty() {
            extra_data.insert("group".to_string(), json!(truncate_field(resolved)));
        }
    }
    if let Some(version) = version {
        let resolved = interpolate_gradle_string(&version, &properties);
        if !resolved.contains('$') && !resolved.is_empty() {
            extra_data.insert("version".to_string(), json!(truncate_field(resolved)));
        }
    }

    (!extra_data.is_empty()).then_some(extra_data)
}

fn extract_gradle_license_metadata(
    tokens: &[Tok],
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<crate::models::LicenseDetection>,
) {
    let mut i = 0;
    while i < tokens.len() {
        if let Tok::Ident(name) = &tokens[i]
            && name == "licenses"
            && i + 1 < tokens.len()
            && tokens[i + 1] == Tok::OpenBrace
            && let Some(block_end) = find_matching_brace(tokens, i + 1)
        {
            let inner = &tokens[i + 2..block_end];
            if let Some((license_name, license_url)) = parse_license_block(inner) {
                let extracted =
                    format_gradle_license_statement(&license_name, license_url.as_deref());
                // Resolve the declared `license { name; url }` through the shared
                // name/url normalizer so Gradle recognizes the same breadth of
                // declared licenses as Maven (free-text names, license URLs, SPDX
                // keys) instead of a handful of hardcoded patterns.
                let normalized =
                    normalize_declared_name_and_url(Some(&license_name), license_url.as_deref());
                let matched_text = extracted.as_deref().unwrap_or(&license_name);
                let (declared, declared_spdx, detections) = build_declared_license_data(
                    normalized,
                    DeclaredLicenseMatchMetadata::single_line(matched_text),
                );
                return (
                    extracted.map(truncate_field),
                    declared.map(truncate_field),
                    declared_spdx.map(truncate_field),
                    detections,
                );
            }
            i = block_end + 1;
            continue;
        }
        i += 1;
    }

    (None, None, None, Vec::new())
}

fn parse_license_block(tokens: &[Tok]) -> Option<(String, Option<String>)> {
    let mut i = 0;
    while i < tokens.len() {
        if let Tok::Ident(name) = &tokens[i]
            && name == "license"
            && i + 1 < tokens.len()
            && tokens[i + 1] == Tok::OpenBrace
            && let Some(block_end) = find_matching_brace(tokens, i + 1)
        {
            let mut license_name = None;
            let mut license_url = None;
            let block = &tokens[i + 2..block_end];
            let mut j = 0;
            while j < block.len() {
                if let Tok::Ident(label) = &block[j] {
                    let normalized = label.strip_suffix(".set").unwrap_or(label);
                    if (normalized == "name" || normalized == "url")
                        && let Some(value) = next_string_literal(block, j + 1)
                    {
                        if normalized == "name" {
                            license_name = Some(value);
                        } else {
                            license_url = Some(value);
                        }
                    }
                }
                j += 1;
            }

            return license_name.map(|name| (name, license_url));
        }
        i += 1;
    }
    None
}

fn next_string_literal(tokens: &[Tok], start: usize) -> Option<String> {
    for token in tokens.iter().skip(start) {
        match token {
            Tok::Str(value) => return Some(truncate_field(value.clone())),
            Tok::MalformedStr(value) => return Some(truncate_field(value.clone())),
            Tok::Ident(_) | Tok::Colon | Tok::Equals | Tok::OpenParen | Tok::CloseParen => continue,
            _ => break,
        }
    }
    None
}

fn find_matching_brace(tokens: &[Tok], start: usize) -> Option<usize> {
    if tokens.get(start) != Some(&Tok::OpenBrace) {
        return None;
    }
    let mut depth = 1;
    let mut i = start + 1;
    while i < tokens.len() && depth > 0 {
        match &tokens[i] {
            Tok::OpenBrace => {
                depth += 1;
                if depth > MAX_RECURSION_DEPTH {
                    warn!(
                        "Gradle parser: nesting depth exceeded {} in find_matching_brace",
                        MAX_RECURSION_DEPTH
                    );
                    break;
                }
            }
            Tok::CloseBrace => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn format_gradle_license_statement(name: &str, url: Option<&str>) -> Option<String> {
    let mut output = format!("- license:\n    name: {name}\n");
    if let Some(url) = url {
        output.push_str(&format!("    url: {url}\n"));
    }
    Some(truncate_field(output))
}

#[cfg(test)]
#[path = "gradle_test.rs"]
mod tests;
