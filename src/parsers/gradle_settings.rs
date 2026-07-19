// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Parser for Gradle settings files (`settings.gradle`, `settings.gradle.kts`).
//!
//! A Gradle settings file defines the *structure* of a multi-project build: the
//! set of subprojects (`include`/`includeFlat`) and the root project name. It is
//! not itself a package, so this parser emits no `purl`; it records the declared
//! subproject directories (and the root project name) into `extra_data` so
//! `src/assembly/gradle_multiproject_merge.rs` can plan multi-project topology.
//!
//! The parser is intentionally thin and bounded: it reuses the Gradle build-file
//! lexer to tokenize the settings script, extracts only *literal* `include` /
//! `includeFlat` string arguments, and never executes Gradle. Non-literal
//! arguments (variables, method calls, computed paths) are skipped rather than
//! guessed at.
//!
//! # Project path normalization
//!
//! Gradle project paths are colon-delimited and rooted at the settings
//! directory: `include ':libs:core'` declares a project at `libs/core`, and
//! `include ':app'` a project at `app`. `includeFlat 'sibling'` declares a
//! project in a sibling directory of the root, i.e. `../sibling`.
//!
//! # `projectDir` remaps
//!
//! Gradle lets a build relocate an included project's directory with a
//! `project(':path').projectDir = <file>` statement in the settings script.
//! Only the fully literal forms are recovered, and only when both the project
//! path and the target directory are string literals:
//!
//! - `project(':app').projectDir = file('custom/app')`
//! - `project(':app').projectDir = new File(settingsDir, 'custom/app')`
//!   (also the Kotlin `File(rootDir, "custom/app")` form)
//!
//! The recovered override directory is always interpreted relative to the
//! settings directory, matching how Gradle resolves `file(...)` and the
//! `settingsDir`/`rootDir` bases in a settings script. Any remap whose project
//! path or target is dynamic (a variable, string interpolation, a computed
//! path, or a `File` form with an unrecognized base) is skipped rather than
//! guessed at, and remaps are recorded only for the non-`includeFlat`
//! (colon-path) project form. The overrides are stashed under
//! `extra_data.project_dir_overrides` (default relative dir → override relative
//! dir) so `src/assembly/topology.rs` can resolve the relocated member.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value as JsonValue;

use super::PackageParser;
use super::gradle::{Tok, lex};
use super::metadata::ParserMetadata;
use crate::models::{DatasourceId, PackageData, PackageType};
use crate::parser_warn as warn;
use crate::parsers::utils::{read_file_to_string, truncate_field};

/// Parser for Gradle `settings.gradle` / `settings.gradle.kts` files.
pub struct GradleSettingsParser;

impl PackageParser for GradleSettingsParser {
    const PACKAGE_TYPE: PackageType = PackageType::Maven;

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "Gradle settings script (multi-project structure)",
            file_patterns: &["**/settings.gradle", "**/settings.gradle.kts"],
            package_type: "maven",
            primary_language: "Java",
            documentation_url: Some(
                "https://docs.gradle.org/current/userguide/multi_project_builds.html",
            ),
        }]
    }

    fn is_match(path: &Path) -> bool {
        path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name == "settings.gradle" || name == "settings.gradle.kts"
        })
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(content) => content,
            Err(error) => {
                warn!("Failed to read Gradle settings file {:?}: {}", path, error);
                return vec![default_package_data()];
            }
        };

        let tokens = lex(&content);
        let (projects, root_project_name, project_dir_overrides) = extract_settings(&tokens);

        let mut extra_data: HashMap<String, JsonValue> = HashMap::new();
        if !projects.is_empty() {
            extra_data.insert(
                "projects".to_string(),
                JsonValue::Array(projects.into_iter().map(JsonValue::String).collect()),
            );
        }
        if let Some(root_project_name) = root_project_name {
            extra_data.insert(
                "root_project_name".to_string(),
                JsonValue::String(root_project_name),
            );
        }
        if !project_dir_overrides.is_empty() {
            extra_data.insert(
                "project_dir_overrides".to_string(),
                JsonValue::Object(
                    project_dir_overrides
                        .into_iter()
                        .map(|(key, value)| (key, JsonValue::String(value)))
                        .collect(),
                ),
            );
        }

        let mut package = default_package_data();
        if !extra_data.is_empty() {
            package.extra_data = Some(extra_data);
        }
        vec![package]
    }
}

fn default_package_data() -> PackageData {
    PackageData {
        package_type: Some(GradleSettingsParser::PACKAGE_TYPE),
        datasource_id: Some(DatasourceId::GradleSettings),
        ..Default::default()
    }
}

/// Extract the declared subproject directories, the optional root project name,
/// and any literal `projectDir` remaps from a tokenized settings script.
///
/// The returned overrides map a project's default (include-derived) relative
/// directory to its remapped relative directory; see the module docs for the
/// bounded set of remap forms recovered.
fn extract_settings(tokens: &[Tok]) -> (Vec<String>, Option<String>, HashMap<String, String>) {
    let mut projects: Vec<String> = Vec::new();
    let mut root_project_name: Option<String> = None;
    let mut project_dir_overrides: HashMap<String, String> = HashMap::new();

    let mut i = 0;
    while i < tokens.len() {
        let Tok::Ident(name) = &tokens[i] else {
            i += 1;
            continue;
        };

        if name == "rootProject.name" {
            let mut cursor = i + 1;
            if tokens.get(cursor) == Some(&Tok::Equals) {
                cursor += 1;
            }
            if let Some(Tok::Str(value)) = tokens.get(cursor) {
                let trimmed = value.trim();
                if !trimmed.is_empty() && root_project_name.is_none() {
                    root_project_name = Some(truncate_field(trimmed.to_string()));
                }
            }
            i += 1;
            continue;
        }

        if name == "include" || name == "includeFlat" {
            let is_flat = name == "includeFlat";
            let (literals, next) = collect_string_arguments(tokens, i + 1);
            for literal in literals {
                if let Some(dir) = project_path_to_relative_dir(&literal, is_flat)
                    && !projects.contains(&dir)
                {
                    projects.push(dir);
                }
            }
            i = next;
            continue;
        }

        if name == "project"
            && let Some((default_dir, override_dir, next)) = parse_project_dir_remap(tokens, i)
        {
            project_dir_overrides
                .entry(default_dir)
                .or_insert(override_dir);
            i = next;
            continue;
        }

        i += 1;
    }

    (projects, root_project_name, project_dir_overrides)
}

/// Parse a `project(':path').projectDir = <file>` remap starting at the
/// `project` identifier token. Returns the project's default (include-derived)
/// relative directory, the literal override directory, and the index to resume
/// scanning from. Returns `None` for any non-literal or unrecognized form.
fn parse_project_dir_remap(tokens: &[Tok], start: usize) -> Option<(String, String, usize)> {
    // project ( "<:path>" ) projectDir = <file>
    if tokens.get(start + 1) != Some(&Tok::OpenParen) {
        return None;
    }
    let Some(Tok::Str(project_path)) = tokens.get(start + 2) else {
        return None;
    };
    if tokens.get(start + 3) != Some(&Tok::CloseParen) {
        return None;
    }
    match tokens.get(start + 4) {
        Some(Tok::Ident(field)) if field == "projectDir" => {}
        _ => return None,
    }
    if tokens.get(start + 5) != Some(&Tok::Equals) {
        return None;
    }

    // A remap only applies to the ordinary colon-path project form; a project
    // declared through `includeFlat` uses a different default directory that we
    // deliberately do not attempt to reconcile here.
    let default_dir = project_path_to_relative_dir(project_path, false)?;
    let (override_dir, next) = parse_file_expression(tokens, start + 6)?;
    Some((default_dir, override_dir, next))
}

/// Parse the right-hand side of a `projectDir` assignment into a single literal
/// directory relative to the settings directory. Recognizes `file('literal')`
/// and `[new] File(<settings-root base>, 'literal')`; anything else yields
/// `None`. Returns the literal and the index just past the closing paren.
fn parse_file_expression(tokens: &[Tok], start: usize) -> Option<(String, usize)> {
    let mut cursor = start;
    // Tolerate a leading `new` (Groovy/Java `new File(...)`).
    if matches!(tokens.get(cursor), Some(Tok::Ident(kw)) if kw == "new") {
        cursor += 1;
    }
    let Some(Tok::Ident(func)) = tokens.get(cursor) else {
        return None;
    };
    let is_file_ctor = func == "File";
    let is_file_helper = func == "file";
    if !is_file_ctor && !is_file_helper {
        return None;
    }
    cursor += 1;
    if tokens.get(cursor) != Some(&Tok::OpenParen) {
        return None;
    }
    cursor += 1;

    // `File(base, 'literal')` carries a settings-root base before the literal;
    // `file('literal')` carries only the literal. Require a recognized base for
    // the `File` constructor so a base-relative path is not misread.
    if is_file_ctor {
        match tokens.get(cursor) {
            Some(Tok::Ident(base)) if is_settings_root_base(base) => cursor += 1,
            _ => return None,
        }
        if tokens.get(cursor) != Some(&Tok::Comma) {
            return None;
        }
        cursor += 1;
    }

    let Some(Tok::Str(literal)) = tokens.get(cursor) else {
        return None;
    };
    cursor += 1;
    if tokens.get(cursor) != Some(&Tok::CloseParen) {
        return None;
    }
    cursor += 1;

    let trimmed = literal.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    Some((truncate_field(trimmed.to_string()), cursor))
}

/// Whether `ident` names the settings/root directory in a settings script, i.e.
/// a base against which a literal `File(base, 'literal')` resolves to a
/// settings-directory-relative path.
fn is_settings_root_base(ident: &str) -> bool {
    matches!(ident, "settingsDir" | "rootDir" | "rootProject.projectDir")
}

/// Collect the run of literal string arguments to an `include`/`includeFlat`
/// call starting at `start`, tolerating an optional wrapping paren and
/// comma separators. Stops at the first token that is not a string literal or a
/// separator, so a non-literal argument (`include(someVar)`) contributes nothing
/// rather than a guessed path. Returns the collected literals and the index to
/// resume scanning from.
fn collect_string_arguments(tokens: &[Tok], start: usize) -> (Vec<String>, usize) {
    let mut literals = Vec::new();
    let mut i = start;
    let mut saw_open_paren = false;

    while i < tokens.len() {
        match &tokens[i] {
            Tok::OpenParen if !saw_open_paren && literals.is_empty() => {
                saw_open_paren = true;
                i += 1;
            }
            Tok::Comma => i += 1,
            Tok::Str(value) => {
                literals.push(value.clone());
                i += 1;
            }
            Tok::CloseParen if saw_open_paren => {
                i += 1;
                break;
            }
            _ => break,
        }
    }

    (literals, i)
}

/// Convert a declared Gradle project path to a directory relative to the
/// settings file's directory. Returns `None` for an empty/degenerate path.
fn project_path_to_relative_dir(project_path: &str, is_flat: bool) -> Option<String> {
    let trimmed = project_path.trim().trim_start_matches(':');
    if trimmed.is_empty() {
        return None;
    }

    if is_flat {
        // `includeFlat 'name'` places the project in a sibling directory of the
        // root project, i.e. `../name`.
        return Some(truncate_field(format!("../{trimmed}")));
    }

    let segments: Vec<&str> = trimmed.split(':').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return None;
    }
    Some(truncate_field(segments.join("/")))
}

#[cfg(test)]
#[path = "gradle_settings_test.rs"]
mod tests;
