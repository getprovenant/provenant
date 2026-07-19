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
    let (projects, root, overrides) = extract_settings(&tokens);
    assert_eq!(projects, vec!["app", "libs/core", "libs/util"]);
    assert_eq!(root.as_deref(), Some("my-root"));
    assert!(overrides.is_empty());
}

#[test]
fn test_extracts_include_projects_kotlin_parens() {
    let tokens = lex(r#"rootProject.name = "kt-root"
include(":app")
include(":libs:core", ":libs:util")
"#);
    let (projects, root, _overrides) = extract_settings(&tokens);
    assert_eq!(projects, vec!["app", "libs/core", "libs/util"]);
    assert_eq!(root.as_deref(), Some("kt-root"));
}

#[test]
fn test_include_flat_maps_to_sibling_dir() {
    let tokens = lex("includeFlat 'sibling-a', 'sibling-b'\n");
    let (projects, _root, _overrides) = extract_settings(&tokens);
    assert_eq!(projects, vec!["../sibling-a", "../sibling-b"]);
}

#[test]
fn test_non_literal_include_arguments_are_skipped() {
    let tokens = lex("include(someVariable)\ninclude ':real'\n");
    let (projects, _root, _overrides) = extract_settings(&tokens);
    assert_eq!(projects, vec!["real"]);
}

#[test]
fn test_project_dir_remap_file_helper_groovy() {
    let tokens = lex("include ':app'\nproject(':app').projectDir = file('custom/app')\n");
    let (projects, _root, overrides) = extract_settings(&tokens);
    assert_eq!(projects, vec!["app"]);
    assert_eq!(overrides.get("app").map(String::as_str), Some("custom/app"));
}

#[test]
fn test_project_dir_remap_new_file_with_settings_dir_base() {
    let tokens = lex(
        "include ':libs:core'\nproject(':libs:core').projectDir = new File(settingsDir, 'vendor/core')\n",
    );
    let (projects, _root, overrides) = extract_settings(&tokens);
    assert_eq!(projects, vec!["libs/core"]);
    assert_eq!(
        overrides.get("libs/core").map(String::as_str),
        Some("vendor/core")
    );
}

#[test]
fn test_project_dir_remap_kotlin_file_ctor() {
    let tokens =
        lex("include(\":app\")\nproject(\":app\").projectDir = File(rootDir, \"custom/app\")\n");
    let (_projects, _root, overrides) = extract_settings(&tokens);
    assert_eq!(overrides.get("app").map(String::as_str), Some("custom/app"));
}

#[test]
fn test_non_literal_project_dir_remap_is_skipped() {
    // A computed/base-relative target without a recognized settings-root base,
    // and a variable project path, are both skipped rather than guessed.
    let tokens = lex(
        "project(':app').projectDir = new File(buildDir, 'app')\nproject(someVar).projectDir = file('x')\n",
    );
    let (_projects, _root, overrides) = extract_settings(&tokens);
    assert!(overrides.is_empty());
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
