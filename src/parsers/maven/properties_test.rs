// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::super::PackageParser;
use super::MavenParser;
use crate::models::{DatasourceId, PackageType};
use std::path::PathBuf;

#[test]
fn test_parse_pom_properties() {
    let pom_props_path = PathBuf::from("testdata/maven/test1/pom.properties");
    let package_data = MavenParser::extract_first_package(&pom_props_path);

    assert_eq!(package_data.package_type, Some(PackageType::Maven));
    assert_eq!(
        package_data.datasource_id,
        Some(DatasourceId::MavenPomProperties)
    );
    assert_eq!(package_data.namespace, Some("com.example.test".to_string()));
    assert_eq!(package_data.name, Some("test-library".to_string()));
    assert_eq!(package_data.version, Some("1.2.3".to_string()));
    assert_eq!(
        package_data.purl,
        Some("pkg:maven/com.example.test/test-library@1.2.3".to_string())
    );
}

#[test]
fn test_pom_properties_purl_generation() {
    let pom_props_path = PathBuf::from("testdata/maven/test4/pom.properties");
    let package_data = MavenParser::extract_first_package(&pom_props_path);

    assert_eq!(
        package_data.purl,
        Some("pkg:maven/org.apache.commons/commons-lang3@3.12.0".to_string())
    );
    assert_eq!(
        package_data.namespace,
        Some("org.apache.commons".to_string())
    );
    assert_eq!(package_data.name, Some("commons-lang3".to_string()));
    assert_eq!(package_data.version, Some("3.12.0".to_string()));
}

#[test]
fn test_is_match_pom_properties() {
    let valid_path = PathBuf::from("/some/path/pom.properties");
    let invalid_path = PathBuf::from("/some/path/not_pom.properties");

    assert!(MavenParser::is_match(&valid_path));
    assert!(!MavenParser::is_match(&invalid_path));
}
