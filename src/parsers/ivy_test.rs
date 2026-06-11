// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use super::PackageParser;
use super::ivy::IvyXmlParser;
use crate::models::{DatasourceId, PackageType};

#[test]
fn test_is_match() {
    assert!(IvyXmlParser::is_match(&PathBuf::from("project/ivy.xml")));
    assert!(IvyXmlParser::is_match(&PathBuf::from(
        "/home/user/app/ivy.xml"
    )));
    assert!(!IvyXmlParser::is_match(&PathBuf::from("ivy.xml.bak")));
    assert!(!IvyXmlParser::is_match(&PathBuf::from("pom.xml")));
    assert!(!IvyXmlParser::is_match(&PathBuf::from("ivyconfig.xml")));
}

#[test]
fn test_parses_info_and_dependencies() {
    let path = PathBuf::from("testdata/ivy-golden/basic/ivy.xml");
    let packages = IvyXmlParser::extract_packages(&path);

    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.package_type, Some(PackageType::Ivy));
    assert_eq!(pkg.datasource_id, Some(DatasourceId::AntIvyXml));
    assert_eq!(pkg.namespace.as_deref(), Some("org.apache.example"));
    assert_eq!(pkg.name.as_deref(), Some("example-core"));
    assert_eq!(pkg.version.as_deref(), Some("4.5.6"));
    assert_eq!(
        pkg.purl.as_deref(),
        Some("pkg:ivy/org.apache.example/example-core@4.5.6")
    );

    assert_eq!(pkg.dependencies.len(), 3);

    let commons = &pkg.dependencies[0];
    assert_eq!(
        commons.purl.as_deref(),
        Some("pkg:ivy/commons-lang/commons-lang")
    );
    assert_eq!(commons.extracted_requirement.as_deref(), Some("2.6"));
    assert_eq!(commons.scope.as_deref(), Some("compile"));
    assert_eq!(commons.is_direct, Some(true));

    let junit = &pkg.dependencies[1];
    assert_eq!(junit.scope.as_deref(), Some("test"));
    assert_eq!(junit.extracted_requirement.as_deref(), Some("4.13.2"));

    // Dependency without org falls back to a name-only ivy purl.
    let local = &pkg.dependencies[2];
    assert_eq!(local.purl.as_deref(), Some("pkg:ivy/local-only"));
    assert_eq!(local.extracted_requirement.as_deref(), Some("1.0"));
}

#[test]
fn test_parses_info_without_dependencies() {
    let path = PathBuf::from("testdata/ivy-golden/no-deps/ivy.xml");
    let packages = IvyXmlParser::extract_packages(&path);

    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.namespace.as_deref(), Some("com.example"));
    assert_eq!(pkg.name.as_deref(), Some("lonely"));
    assert_eq!(pkg.version.as_deref(), Some("0.1.0"));
    assert!(pkg.dependencies.is_empty());
}

#[test]
fn test_malformed_xml_falls_back_to_default() {
    use std::io::Write;
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ivy.xml");
    let mut file = std::fs::File::create(&path).expect("create");
    file.write_all(b"<ivy-module><info organisation=")
        .expect("write");
    drop(file);

    let packages = IvyXmlParser::extract_packages(&path);
    assert_eq!(packages.len(), 1);
    // datasource_id is set even on the error path.
    assert_eq!(packages[0].datasource_id, Some(DatasourceId::AntIvyXml));
}
