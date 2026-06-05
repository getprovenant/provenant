// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::models::{DatasourceId, PackageType};
use crate::parsers::PackageParser;
use crate::parsers::julia::{JuliaManifestTomlParser, JuliaProjectTomlParser};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/julia")
}

#[test]
fn test_project_toml_is_match() {
    assert!(JuliaProjectTomlParser::is_match(
        PathBuf::from("Project.toml").as_path()
    ));
    assert!(JuliaProjectTomlParser::is_match(
        PathBuf::from("project.toml").as_path()
    ));
    assert!(!JuliaProjectTomlParser::is_match(
        PathBuf::from("Manifest.toml").as_path()
    ));
    assert!(!JuliaProjectTomlParser::is_match(
        PathBuf::from("Cargo.toml").as_path()
    ));
}

#[test]
fn test_manifest_toml_is_match() {
    assert!(JuliaManifestTomlParser::is_match(
        PathBuf::from("Manifest.toml").as_path()
    ));
    assert!(JuliaManifestTomlParser::is_match(
        PathBuf::from("manifest.toml").as_path()
    ));
    assert!(!JuliaManifestTomlParser::is_match(
        PathBuf::from("Project.toml").as_path()
    ));
}

#[test]
fn test_project_toml_basic_extraction() {
    let test_file = test_data_dir().join("basic/Project.toml");
    if !test_file.exists() {
        return;
    }
    let packages = JuliaProjectTomlParser::extract_packages(&test_file);
    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.package_type, Some(PackageType::Julia));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::JuliaProjectToml));
    assert_eq!(pkg.primary_language, Some("Julia".to_string()));
    assert_eq!(pkg.name.as_deref(), Some("MyPackage"));
    assert_eq!(pkg.version.as_deref(), Some("1.0.0"));
}

#[test]
fn test_manifest_toml_basic_extraction() {
    let test_file = test_data_dir().join("basic/Manifest.toml");
    if !test_file.exists() {
        return;
    }
    let packages = JuliaManifestTomlParser::extract_packages(&test_file);
    assert!(!packages.is_empty());
    for pkg in &packages {
        assert_eq!(pkg.package_type, Some(PackageType::Julia));
        assert_eq!(pkg.datasource_id, Some(DatasourceId::JuliaManifestToml));
    }
}

#[test]
fn test_project_toml_dependencies() {
    let test_file = test_data_dir().join("basic/Project.toml");
    if !test_file.exists() {
        return;
    }
    let packages = JuliaProjectTomlParser::extract_packages(&test_file);
    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.dependencies.len(), 2);

    let json_dep = pkg
        .dependencies
        .iter()
        .find(|d| d.purl.as_deref().unwrap_or_default().contains("/JSON"))
        .expect("JSON dep");
    assert!(json_dep.is_pinned.unwrap_or(false));

    let http_dep = pkg
        .dependencies
        .iter()
        .find(|d| d.purl.as_deref().unwrap_or_default().contains("/HTTP"))
        .expect("HTTP dep");
    assert!(!http_dep.is_pinned.unwrap_or(true));
}

#[test]
fn test_project_toml_license() {
    let test_file = test_data_dir().join("basic/Project.toml");
    if !test_file.exists() {
        return;
    }
    let packages = JuliaProjectTomlParser::extract_packages(&test_file);
    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.extracted_license_statement.as_deref(), Some("MIT"));
    assert!(pkg.declared_license_expression_spdx.is_some());
}

#[test]
fn test_project_toml_singular_author_field() {
    let temp_dir = std::env::temp_dir().join(format!(
        "provenant-julia-author-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).expect("create temp test dir");

    let test_file = temp_dir.join("Project.toml");
    fs::write(
        &test_file,
        r#"name = "Plots"
uuid = "91a5bcdd-55d7-5caf-9e0b-520d859cae80"
version = "2.0.0"
author = ["Tom Breloff (@tbreloff)"]
"#,
    )
    .expect("write temp Project.toml");

    let packages = JuliaProjectTomlParser::extract_packages(&test_file);
    let pkg = &packages[0];

    assert_eq!(pkg.parties.len(), 1);
    assert_eq!(pkg.parties[0].role.as_deref(), Some("author"));
    assert_eq!(
        pkg.parties[0].name.as_deref(),
        Some("Tom Breloff (@tbreloff)")
    );

    fs::remove_dir_all(&temp_dir).expect("remove temp test dir");
}

// Manifest.toml format 1.0 (Julia 1.0-1.6) lists packages as root-level [[PackageName]]
// array-of-tables rather than nesting them under a [deps] table (format 2.0, Julia 1.7+).
#[test]
fn test_manifest_toml_format_1_root_level_packages() {
    let temp_dir = std::env::temp_dir().join(format!(
        "provenant-julia-manifest-v1-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).expect("create temp test dir");

    let test_file = temp_dir.join("Manifest.toml");
    fs::write(
        &test_file,
        r#"# This file is machine-generated - editing it directly is not advised
julia_version = "1.6.7"
manifest_format = "1.0"

[[JSON]]
deps = ["Dates", "Mmap", "Parsers", "Unicode"]
git-tree-sha1 = "31e996f0a15c7b280ba9f76636b3609a8aaa8c3a"
uuid = "682c06a0-de6a-54ab-a142-c8b1cf79cde6"
version = "0.21.4"

[[HTTP]]
deps = ["Base64", "Dates", "Sockets"]
git-tree-sha1 = "60ed5f1643927479f845b0135bb369b031b541fa"
uuid = "cd3eb016-35fb-5094-929b-558a96fad6f3"
version = "0.9.17"
"#,
    )
    .expect("write temp Manifest.toml");

    let packages = JuliaManifestTomlParser::extract_packages(&test_file);

    assert_eq!(packages.len(), 2);
    assert!(
        packages
            .iter()
            .all(|p| p.datasource_id == Some(DatasourceId::JuliaManifestToml))
    );
    let json = packages
        .iter()
        .find(|p| p.name.as_deref() == Some("JSON"))
        .expect("JSON package");
    assert_eq!(json.version.as_deref(), Some("0.21.4"));
    assert!(
        packages
            .iter()
            .any(|p| p.name.as_deref() == Some("HTTP") && p.version.as_deref() == Some("0.9.17"))
    );

    fs::remove_dir_all(&temp_dir).expect("remove temp test dir");
}
