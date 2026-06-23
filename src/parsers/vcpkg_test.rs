// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::models::{DatasourceId, PackageType};
use std::path::PathBuf;

use super::PackageParser;
use super::vcpkg::{
    VcpkgConfigurationParser, VcpkgControlParser, VcpkgLockParser, VcpkgManifestParser,
};

#[test]
fn test_vcpkg_manifest_is_match() {
    assert!(VcpkgManifestParser::is_match(&PathBuf::from(
        "/tmp/vcpkg.json"
    )));
    assert!(!VcpkgManifestParser::is_match(&PathBuf::from(
        "/tmp/vcpkg-configuration.json"
    )));
    assert!(!VcpkgManifestParser::is_match(&PathBuf::from(
        "/tmp/vcpkg-lock.json"
    )));
}

#[test]
fn test_vcpkg_lock_is_match() {
    assert!(VcpkgLockParser::is_match(&PathBuf::from(
        "/tmp/vcpkg-lock.json"
    )));
    assert!(!VcpkgLockParser::is_match(&PathBuf::from(
        "/tmp/vcpkg.json"
    )));
}

#[test]
fn test_vcpkg_configuration_is_match() {
    assert!(VcpkgConfigurationParser::is_match(&PathBuf::from(
        "/tmp/vcpkg-configuration.json"
    )));
    assert!(!VcpkgConfigurationParser::is_match(&PathBuf::from(
        "/tmp/vcpkg.json"
    )));
    assert!(!VcpkgConfigurationParser::is_match(&PathBuf::from(
        "/tmp/vcpkg-lock.json"
    )));
}

#[test]
fn test_vcpkg_control_is_match() {
    assert!(VcpkgControlParser::is_match(&PathBuf::from(
        "/tmp/vcpkg/ports/ace/CONTROL"
    )));
    assert!(!VcpkgControlParser::is_match(&PathBuf::from(
        "/tmp/vcpkg/ports/ace/portfile.cmake"
    )));
    assert!(!VcpkgControlParser::is_match(&PathBuf::from(
        "/tmp/vcpkg/CONTROL"
    )));
    assert!(!VcpkgControlParser::is_match(&PathBuf::from(
        "/tmp/vcpkg/ports/ace/subdir/CONTROL"
    )));
}

#[test]
fn test_parse_vcpkg_project_manifest() {
    let path = PathBuf::from("testdata/vcpkg/project/vcpkg.json");
    let pkg = VcpkgManifestParser::extract_first_package(&path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::VcpkgJson));
    assert_eq!(pkg.name.as_deref(), Some("sample-project"));
    assert_eq!(pkg.version.as_deref(), Some("1.0.0"));
    assert_eq!(
        pkg.description.as_deref(),
        Some("A sample vcpkg project manifest")
    );
    assert_eq!(
        pkg.homepage_url.as_deref(),
        Some("https://example.com/sample-project")
    );
    assert_eq!(pkg.extracted_license_statement.as_deref(), Some("MIT"));
    assert_eq!(
        pkg.purl.as_deref(),
        Some("pkg:generic/vcpkg/sample-project@1.0.0")
    );

    let extra = pkg.extra_data.as_ref().expect("extra_data should exist");
    assert_eq!(
        extra.get("builtin-baseline"),
        Some(&serde_json::json!(
            "3426db05b996481ca31e95fff3734cf23e0f51bc"
        ))
    );
    assert_eq!(
        extra.get("supports"),
        Some(&serde_json::json!("windows | linux"))
    );
    assert!(extra.get("overrides").is_some());
    assert!(extra.get("configuration").is_some());

    assert_eq!(pkg.dependencies.len(), 3);
    let fmt = pkg
        .dependencies
        .iter()
        .find(|dep| dep.purl.as_deref() == Some("pkg:generic/vcpkg/fmt"))
        .expect("expected fmt dependency");
    assert_eq!(fmt.scope.as_deref(), Some("dependencies"));
    assert_eq!(fmt.extracted_requirement.as_deref(), Some("fmt"));
    assert_eq!(fmt.is_runtime, Some(true));
    assert_eq!(fmt.is_optional, Some(false));
    assert_eq!(fmt.is_direct, Some(true));
    assert_eq!(fmt.is_pinned, Some(false));

    let cpprestsdk = pkg
        .dependencies
        .iter()
        .find(|dep| dep.purl.as_deref() == Some("pkg:generic/vcpkg/cpprestsdk"))
        .expect("expected cpprestsdk dependency");
    assert_eq!(
        cpprestsdk.extracted_requirement.as_deref(),
        Some("2.10.18#1")
    );
    assert_eq!(cpprestsdk.is_runtime, Some(false));
    assert_eq!(cpprestsdk.is_optional, Some(false));
    assert_eq!(cpprestsdk.is_direct, Some(true));
    assert_eq!(cpprestsdk.is_pinned, Some(false));
    let cpprestsdk_extra = cpprestsdk.extra_data.as_ref().expect("expected extra_data");
    assert_eq!(
        cpprestsdk_extra.get("features"),
        Some(&serde_json::json!(["websockets"]))
    );
    assert_eq!(
        cpprestsdk_extra.get("default-features"),
        Some(&serde_json::json!(false))
    );
    assert_eq!(cpprestsdk_extra.get("host"), Some(&serde_json::json!(true)));
    assert_eq!(
        cpprestsdk_extra.get("platform"),
        Some(&serde_json::json!("windows"))
    );

    // zlib is pinned by the manifest `overrides` array, so the declared
    // `version>=` floor is preserved while the dependency is marked pinned and
    // the exact override version is recorded alongside it.
    let zlib = pkg
        .dependencies
        .iter()
        .find(|dep| dep.purl.as_deref() == Some("pkg:generic/vcpkg/zlib"))
        .expect("expected zlib dependency");
    assert_eq!(zlib.extracted_requirement.as_deref(), Some("1.3.1#2"));
    assert_eq!(zlib.is_pinned, Some(true));
    let zlib_extra = zlib.extra_data.as_ref().expect("expected zlib extra_data");
    assert_eq!(
        zlib_extra.get("override_version"),
        Some(&serde_json::json!("1.3.1#2"))
    );

    // fmt has no override entry, so it stays unpinned with no override metadata.
    assert!(
        fmt.extra_data
            .as_ref()
            .map(|extra| !extra.contains_key("override_version"))
            .unwrap_or(true)
    );
}

#[test]
fn test_parse_vcpkg_port_manifest() {
    let path = PathBuf::from("testdata/vcpkg/port/vcpkg.json");
    let pkg = VcpkgManifestParser::extract_first_package(&path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::VcpkgJson));
    assert_eq!(pkg.name.as_deref(), Some("fmt"));
    assert_eq!(pkg.version.as_deref(), Some("10.1.1#7"));
    assert_eq!(
        pkg.description.as_deref(),
        Some("Formatting library for C++.")
    );
    assert_eq!(
        pkg.homepage_url.as_deref(),
        Some("https://github.com/fmtlib/fmt")
    );
    assert_eq!(pkg.extracted_license_statement.as_deref(), Some("MIT"));
    assert_eq!(
        pkg.purl.as_deref(),
        Some("pkg:generic/vcpkg/fmt@10.1.1%237")
    );
    assert_eq!(pkg.parties.len(), 1);
    assert_eq!(pkg.parties[0].role.as_deref(), Some("maintainer"));
    assert_eq!(pkg.parties[0].name.as_deref(), Some("fmt maintainers"));
    assert_eq!(pkg.parties[0].email.as_deref(), Some("fmt@example.com"));

    let extra = pkg.extra_data.as_ref().expect("extra_data should exist");
    assert_eq!(
        extra.get("default-features"),
        Some(&serde_json::json!(["unicode"]))
    );
    assert!(extra.get("features").is_some());

    assert_eq!(pkg.dependencies.len(), 3);
    assert!(
        pkg.dependencies
            .iter()
            .all(|dep| dep.scope.as_deref() == Some("dependencies"))
    );

    assert!(
        pkg.dependencies
            .iter()
            .filter(|dep| dep.purl.as_deref() != Some("pkg:generic/vcpkg/icu"))
            .all(|dep| dep.is_runtime == Some(false))
    );

    let icu = pkg
        .dependencies
        .iter()
        .find(|dep| dep.purl.as_deref() == Some("pkg:generic/vcpkg/icu"))
        .expect("expected icu feature dependency");
    assert_eq!(icu.is_runtime, Some(true));
    assert_eq!(icu.is_direct, Some(true));
    assert_eq!(
        icu.extra_data
            .as_ref()
            .and_then(|extra| extra.get("feature"))
            .and_then(|value| value.as_str()),
        Some("unicode")
    );
}

#[test]
fn test_invalid_vcpkg_manifest_returns_default_package() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let path = temp_dir.path().join("vcpkg.json");
    std::fs::write(&path, "{ invalid json }").expect("Failed to write invalid vcpkg.json");

    let pkg = VcpkgManifestParser::extract_first_package(&path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::VcpkgJson));
    assert!(pkg.name.is_none());
    assert!(pkg.dependencies.is_empty());
}

#[test]
fn test_parse_vcpkg_lock_preserves_registry_revisions() {
    let path = PathBuf::from("testdata/vcpkg/lock/vcpkg-lock.json");
    let pkg = VcpkgLockParser::extract_first_package(&path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::VcpkgLockJson));
    assert!(pkg.name.is_none());
    assert!(pkg.version.is_none());
    assert!(pkg.purl.is_none());
    assert!(pkg.is_private);
    assert!(pkg.dependencies.is_empty());

    let extra = pkg.extra_data.as_ref().expect("extra_data should exist");
    let registry_locks = extra
        .get("registry_locks")
        .and_then(serde_json::Value::as_array)
        .expect("registry_locks should be an array");
    assert_eq!(registry_locks.len(), 2);

    let microsoft_registry = registry_locks
        .iter()
        .find(|entry| {
            entry.get("location").and_then(serde_json::Value::as_str)
                == Some("https://github.com/microsoft/vcpkg")
        })
        .expect("expected microsoft/vcpkg registry lock");
    assert_eq!(
        microsoft_registry["references"]["HEAD"],
        serde_json::json!("0123456789abcdef0123456789abcdef01234567")
    );
    assert_eq!(
        microsoft_registry["references"]["release/2024.02"],
        serde_json::json!("89abcdef0123456789abcdef0123456789abcdef")
    );

    let local_registry = registry_locks
        .iter()
        .find(|entry| {
            entry.get("location").and_then(serde_json::Value::as_str)
                == Some("/opt/private-vcpkg-registry")
        })
        .expect("expected local registry lock");
    assert_eq!(
        local_registry["references"]["HEAD"],
        serde_json::json!("fedcba9876543210fedcba9876543210fedcba98")
    );
}

#[test]
fn test_invalid_vcpkg_lock_returns_default_package() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let path = temp_dir.path().join("vcpkg-lock.json");
    std::fs::write(&path, "{ invalid json }").expect("Failed to write invalid vcpkg-lock.json");

    let pkg = VcpkgLockParser::extract_first_package(&path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::VcpkgLockJson));
    assert!(pkg.name.is_none());
    assert!(pkg.is_private);
    assert!(pkg.extra_data.is_none());
}

#[test]
fn test_parse_vcpkg_configuration_preserves_registry_and_overlay_provenance() {
    let path = PathBuf::from("testdata/vcpkg/configuration/vcpkg-configuration.json");
    let pkg = VcpkgConfigurationParser::extract_first_package(&path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(
        pkg.datasource_id,
        Some(DatasourceId::VcpkgConfigurationJson)
    );
    assert!(pkg.name.is_none());
    assert!(pkg.version.is_none());
    assert!(pkg.purl.is_none());
    assert!(pkg.is_private);
    assert!(pkg.dependencies.is_empty());

    let extra = pkg.extra_data.as_ref().expect("extra_data should exist");
    assert_eq!(
        extra
            .get("default-registry")
            .and_then(|registry| registry.get("repository"))
            .and_then(serde_json::Value::as_str),
        Some("https://github.com/microsoft/vcpkg")
    );
    let registries = extra
        .get("registries")
        .and_then(serde_json::Value::as_array)
        .expect("registries should be an array");
    assert_eq!(registries.len(), 1);
    assert_eq!(
        extra.get("overlay-ports"),
        Some(&serde_json::json!(["./ports"]))
    );
    assert_eq!(
        extra.get("overlay-triplets"),
        Some(&serde_json::json!(["./triplets"]))
    );
}

#[test]
fn test_parse_vcpkg_control_preserves_classic_port_metadata() {
    let path = PathBuf::from("testdata/vcpkg/classic/ports/ace/CONTROL");
    let pkg = VcpkgControlParser::extract_first_package(&path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::VcpkgControl));
    assert_eq!(pkg.name.as_deref(), Some("ace"));
    assert_eq!(pkg.version.as_deref(), Some("6.5.5#2"));
    assert_eq!(
        pkg.description.as_deref(),
        Some("The ADAPTIVE Communication Environment")
    );
    assert_eq!(pkg.homepage_url.as_deref(), Some("https://example.com/ace"));
    assert_eq!(pkg.purl.as_deref(), Some("pkg:generic/vcpkg/ace@6.5.5%232"));
    assert!(!pkg.is_private);

    assert_eq!(pkg.dependencies.len(), 5);
    let zlib = pkg
        .dependencies
        .iter()
        .find(|dep| dep.purl.as_deref() == Some("pkg:generic/vcpkg/zlib"))
        .expect("expected zlib dependency");
    assert_eq!(zlib.scope.as_deref(), Some("build-depends"));
    assert_eq!(zlib.extracted_requirement.as_deref(), Some("zlib"));
    assert_eq!(zlib.is_runtime, Some(true));
    assert_eq!(zlib.is_optional, Some(false));
    assert_eq!(zlib.is_direct, Some(true));
    assert_eq!(zlib.is_pinned, Some(false));

    let curl_openssl = pkg
        .dependencies
        .iter()
        .find(|dep| dep.extracted_requirement.as_deref() == Some("curl[core,openssl] (!windows)"))
        .expect("expected curl openssl dependency");
    assert_eq!(curl_openssl.purl.as_deref(), Some("pkg:generic/vcpkg/curl"));
    let curl_extra = curl_openssl
        .extra_data
        .as_ref()
        .expect("expected curl extra_data");
    assert_eq!(
        curl_extra.get("features"),
        Some(&serde_json::json!(["core", "openssl"]))
    );
    assert_eq!(
        curl_extra.get("platform"),
        Some(&serde_json::json!("!windows"))
    );

    let feature_dep = pkg
        .dependencies
        .iter()
        .find(|dep| dep.purl.as_deref() == Some("pkg:generic/vcpkg/vcpkg-cmake"))
        .expect("expected feature dependency");
    assert_eq!(feature_dep.is_optional, None);
    assert_eq!(
        feature_dep
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("feature")),
        Some(&serde_json::json!("tools"))
    );

    let extra = pkg.extra_data.as_ref().expect("extra_data should exist");
    assert_eq!(
        extra.get("default-features"),
        Some(&serde_json::json!(["ssl"]))
    );
    assert_eq!(
        extra.get("supports"),
        Some(&serde_json::json!("!(uwp|arm)"))
    );
    let features = extra
        .get("features")
        .and_then(serde_json::Value::as_array)
        .expect("features should be preserved");
    assert_eq!(features.len(), 1);
    assert_eq!(features[0]["name"], serde_json::json!("tools"));
}

#[test]
fn test_parse_vcpkg_control_flattens_repeated_dependency_feature_brackets() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let path = temp_dir.path().join("CONTROL");
    std::fs::write(
        &path,
        r#"Source: sample
Version: 1.0.0
Build-Depends: curl[core][openssl], zlib
"#,
    )
    .expect("Failed to write CONTROL");

    let pkg = VcpkgControlParser::extract_first_package(&path);

    let curl = pkg
        .dependencies
        .iter()
        .find(|dep| dep.purl.as_deref() == Some("pkg:generic/vcpkg/curl"))
        .expect("expected curl dependency");
    assert_eq!(
        curl.extra_data
            .as_ref()
            .and_then(|extra| extra.get("features")),
        Some(&serde_json::json!(["core", "openssl"]))
    );
}

#[test]
fn test_invalid_vcpkg_control_returns_default_package() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let path = temp_dir.path().join("CONTROL");
    std::fs::write(&path, "this is not a valid CONTROL file")
        .expect("Failed to write invalid CONTROL");

    let pkg = VcpkgControlParser::extract_first_package(&path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::VcpkgControl));
    assert!(pkg.name.is_none());
    assert!(pkg.is_private);
    assert!(pkg.dependencies.is_empty());
}

#[test]
fn test_invalid_vcpkg_configuration_returns_default_package() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let path = temp_dir.path().join("vcpkg-configuration.json");
    std::fs::write(&path, "{ invalid json }")
        .expect("Failed to write invalid vcpkg-configuration.json");

    let pkg = VcpkgConfigurationParser::extract_first_package(&path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(
        pkg.datasource_id,
        Some(DatasourceId::VcpkgConfigurationJson)
    );
    assert!(pkg.name.is_none());
    assert!(pkg.is_private);
    assert!(pkg.extra_data.is_none());
}

#[test]
fn test_parse_vcpkg_manifest_reads_sibling_configuration_when_not_embedded() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let manifest_path = temp_dir.path().join("vcpkg.json");
    let config_path = temp_dir.path().join("vcpkg-configuration.json");

    std::fs::write(
        &manifest_path,
        r#"{
            "name": "cfg-project",
            "version-string": "0.1.0",
            "dependencies": ["fmt"]
        }"#,
    )
    .expect("Failed to write manifest");
    std::fs::write(
        &config_path,
        r#"{
            "default-registry": {
                "kind": "git",
                "repository": "https://github.com/microsoft/vcpkg",
                "baseline": "0123456789abcdef0123456789abcdef01234567"
            }
        }"#,
    )
    .expect("Failed to write config");

    let pkg = VcpkgManifestParser::extract_first_package(&manifest_path);
    let extra = pkg.extra_data.as_ref().expect("extra_data should exist");
    let configuration = extra
        .get("configuration")
        .expect("expected sibling configuration metadata");

    assert_eq!(
        configuration["default-registry"]["repository"],
        serde_json::json!("https://github.com/microsoft/vcpkg")
    );
}

#[test]
fn test_parse_vcpkg_project_manifest_without_identity() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let manifest_path = temp_dir.path().join("vcpkg.json");
    std::fs::write(
        &manifest_path,
        r#"{
            "dependencies": [
                "fmt",
                { "name": "zlib", "version>=": "1.3.1#2" }
            ],
            "builtin-baseline": "3426db05b996481ca31e95fff3734cf23e0f51bc"
        }"#,
    )
    .expect("Failed to write manifest");

    let pkg = VcpkgManifestParser::extract_first_package(&manifest_path);

    assert_eq!(pkg.package_type, Some(PackageType::Vcpkg));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::VcpkgJson));
    assert!(pkg.name.is_none());
    assert!(pkg.version.is_none());
    assert!(pkg.purl.is_none());
    assert_eq!(pkg.dependencies.len(), 2);
    assert!(
        pkg.dependencies
            .iter()
            .any(|dep| dep.purl.as_deref() == Some("pkg:generic/vcpkg/fmt"))
    );
    assert!(
        pkg.dependencies
            .iter()
            .any(|dep| dep.purl.as_deref() == Some("pkg:generic/vcpkg/zlib"))
    );
}
