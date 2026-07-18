// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::models::DatasourceId;
use std::path::PathBuf;

fn extra(pkg: &PackageData, key: &str) -> Option<JsonValue> {
    pkg.extra_data.as_ref().and_then(|d| d.get(key).cloned())
}

fn projects(pkg: &PackageData) -> Vec<String> {
    extra(pkg, "projects")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

#[test]
fn test_is_match() {
    assert!(GradleSettingsParser::is_match(&PathBuf::from(
        "settings.gradle"
    )));
    assert!(GradleSettingsParser::is_match(&PathBuf::from(
        "a/b/settings.gradle.kts"
    )));
    assert!(!GradleSettingsParser::is_match(&PathBuf::from(
        "build.gradle"
    )));
    assert!(!GradleSettingsParser::is_match(&PathBuf::from(
        "settings.gradle.bak"
    )));
}

#[test]
fn test_extracts_include_projects_groovy() {
    let tokens =
        lex("rootProject.name = 'my-root'\ninclude ':app', ':libs:core'\ninclude ':libs:util'\n");
    let (projects, root) = extract_settings(&tokens);
    assert_eq!(projects, vec!["app", "libs/core", "libs/util"]);
    assert_eq!(root.as_deref(), Some("my-root"));
}

#[test]
fn test_extracts_include_projects_kotlin_parens() {
    let tokens = lex(r#"rootProject.name = "kt-root"
include(":app")
include(":libs:core", ":libs:util")
"#);
    let (projects, root) = extract_settings(&tokens);
    assert_eq!(projects, vec!["app", "libs/core", "libs/util"]);
    assert_eq!(root.as_deref(), Some("kt-root"));
}

#[test]
fn test_include_flat_maps_to_sibling_dir() {
    let tokens = lex("includeFlat 'sibling-a', 'sibling-b'\n");
    let (projects, _root) = extract_settings(&tokens);
    assert_eq!(projects, vec!["../sibling-a", "../sibling-b"]);
}

#[test]
fn test_non_literal_include_arguments_are_skipped() {
    let tokens = lex("include(someVariable)\ninclude ':real'\n");
    let (projects, _root) = extract_settings(&tokens);
    assert_eq!(projects, vec!["real"]);
}

#[test]
fn test_extract_packages_sets_datasource_and_no_purl() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.gradle");
    std::fs::write(&path, "rootProject.name = 'root'\ninclude ':a'\n").unwrap();

    let packages = GradleSettingsParser::extract_packages(&path);
    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.datasource_id, Some(DatasourceId::GradleSettings));
    assert!(pkg.purl.is_none());
    assert_eq!(projects(pkg), vec!["a"]);
    assert_eq!(
        extra(pkg, "root_project_name").and_then(|v| v.as_str().map(str::to_string)),
        Some("root".to_string())
    );
}
