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
        let (projects, root_project_name) = extract_settings(&tokens);

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

/// Extract the declared subproject directories and the optional root project
/// name from a tokenized settings script.
fn extract_settings(tokens: &[Tok]) -> (Vec<String>, Option<String>) {
    let mut projects: Vec<String> = Vec::new();
    let mut root_project_name: Option<String> = None;

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

        i += 1;
    }

    (projects, root_project_name)
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
