// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use super::PackageParser;
use super::vscode_extension::VscodeExtensionManifestParser;
use crate::models::{DatasourceId, PackageType, PartyType};

#[test]
fn test_is_match() {
    assert!(VscodeExtensionManifestParser::is_match(&PathBuf::from(
        "extension.vsixmanifest"
    )));
    assert!(VscodeExtensionManifestParser::is_match(&PathBuf::from(
        "extension/extension.vsixmanifest"
    )));
    assert!(!VscodeExtensionManifestParser::is_match(&PathBuf::from(
        "package.json"
    )));
    assert!(!VscodeExtensionManifestParser::is_match(&PathBuf::from(
        "extension.vsixmanifest.bak"
    )));
}

#[test]
fn test_extracts_identity_and_metadata() {
    let package_data = VscodeExtensionManifestParser::extract_first_package(&PathBuf::from(
        "testdata/vscode-extension-golden/basic/extension.vsixmanifest",
    ));

    assert_eq!(
        package_data.package_type,
        Some(PackageType::VscodeExtension)
    );
    assert_eq!(
        package_data.datasource_id,
        Some(DatasourceId::VscodeExtensionVsixManifest)
    );
    assert_eq!(package_data.namespace.as_deref(), Some("ms-python"));
    assert_eq!(package_data.name.as_deref(), Some("python"));
    assert_eq!(package_data.version.as_deref(), Some("2023.25.10292213"));
    assert_eq!(
        package_data.purl.as_deref(),
        Some("pkg:vscode-extension/ms-python/python@2023.25.10292213?platform=linux-x64")
    );
    assert_eq!(
        package_data.description.as_deref(),
        Some("Python language support for VS Code")
    );
    assert_eq!(
        package_data.homepage_url.as_deref(),
        Some("https://marketplace.visualstudio.com/items?itemName=ms-python.python")
    );
    assert_eq!(
        package_data
            .qualifiers
            .as_ref()
            .and_then(|qualifiers| { qualifiers.get("platform").map(std::string::String::as_str) }),
        Some("linux-x64")
    );
    assert_eq!(
        package_data.keywords,
        vec![
            "python".to_string(),
            "linting".to_string(),
            "debugging".to_string()
        ]
    );

    let publisher = package_data
        .parties
        .first()
        .expect("publisher party should be recorded");
    assert_eq!(publisher.r#type, Some(PartyType::Organization));
    assert_eq!(publisher.role.as_deref(), Some("publisher"));
    assert_eq!(publisher.name.as_deref(), Some("ms-python"));

    let extra_data = package_data
        .extra_data
        .as_ref()
        .expect("metadata fields should be preserved in extra_data");
    assert_eq!(
        extra_data
            .get("display_name")
            .and_then(|value| value.as_str()),
        Some("Python")
    );
    assert_eq!(
        extra_data
            .get("identity_language")
            .and_then(|value| value.as_str()),
        Some("en-US")
    );
    assert_eq!(
        extra_data
            .get("license_file")
            .and_then(|value| value.as_str()),
        Some("LICENSE.txt")
    );
}

#[test]
fn test_missing_identity_still_reports_datasource() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("extension.vsixmanifest");
    std::fs::write(
        &path,
        r#"<PackageManifest Version="2.0.0"><Metadata><DisplayName>No identity</DisplayName></Metadata></PackageManifest>"#,
    )
    .expect("write manifest");

    let package_data = VscodeExtensionManifestParser::extract_first_package(&path);

    assert_eq!(
        package_data.package_type,
        Some(PackageType::VscodeExtension)
    );
    assert_eq!(
        package_data.datasource_id,
        Some(DatasourceId::VscodeExtensionVsixManifest)
    );
    assert_eq!(package_data.name, None);
    assert_eq!(package_data.purl, None);
    assert_eq!(
        package_data
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("display_name"))
            .and_then(|value| value.as_str()),
        Some("No identity")
    );
}

#[test]
fn test_extracts_cdata_metadata_text() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("extension.vsixmanifest");
    std::fs::write(
        &path,
        r#"<PackageManifest Version="2.0.0">
  <Metadata>
    <Identity Id="sample" Version="1.0.0" Publisher="example" />
    <DisplayName><![CDATA[Sample Extension]]></DisplayName>
    <Description><![CDATA[Adds <VS Code> helpers]]></Description>
  </Metadata>
</PackageManifest>"#,
    )
    .expect("write manifest");

    let package_data = VscodeExtensionManifestParser::extract_first_package(&path);

    assert_eq!(package_data.name.as_deref(), Some("sample"));
    assert_eq!(
        package_data.description.as_deref(),
        Some("Adds <VS Code> helpers")
    );
    assert_eq!(
        package_data
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("display_name"))
            .and_then(|value| value.as_str()),
        Some("Sample Extension")
    );
}
