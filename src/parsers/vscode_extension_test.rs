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
        package_data.vcs_url.as_deref(),
        Some("https://github.com/microsoft/vscode-python.git")
    );
    assert_eq!(
        package_data.bug_tracking_url.as_deref(),
        Some("https://github.com/microsoft/vscode-python/issues")
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
            "debugging".to_string(),
            "multi-root ready".to_string()
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
        Some("extension/LICENSE.txt")
    );
    assert_eq!(
        extra_data.get("engine").and_then(|value| value.as_str()),
        Some("^1.82.0")
    );
    assert_eq!(
        extra_data
            .get("categories")
            .and_then(|value| value.as_array()),
        Some(&vec![
            serde_json::json!("Programming Languages"),
            serde_json::json!("Debuggers"),
            serde_json::json!("Linters"),
        ])
    );
}

/// Real VS Code / Open VSX marketplace manifests use comma-separated `<Tags>`
/// and expose links through `<Properties>`, not the Visual Studio schema's
/// top-level elements. This is the shape that regressed the original parser.
#[test]
fn test_parses_marketplace_comma_tags_and_properties() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("extension.vsixmanifest");
    std::fs::write(
        &path,
        r#"<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Language="en-US" Id="prettier-vscode" Version="11.0.0" Publisher="esbenp" />
    <DisplayName>Prettier - Code formatter</DisplayName>
    <Description xml:space="preserve">Code formatter using prettier</Description>
    <Tags>prettier,formatter,javascript,multi-root ready</Tags>
    <Categories>Formatters</Categories>
    <Properties>
      <Property Id="Microsoft.VisualStudio.Code.Engine" Value="^1.80.0" />
      <Property Id="Microsoft.VisualStudio.Services.Links.Source" Value="https://github.com/prettier/prettier-vscode.git" />
      <Property Id="Microsoft.VisualStudio.Services.Links.Support" Value="https://github.com/prettier/prettier-vscode/issues" />
    </Properties>
    <License>extension/LICENSE.txt</License>
  </Metadata>
</PackageManifest>"#,
    )
    .expect("write manifest");

    let package_data = VscodeExtensionManifestParser::extract_first_package(&path);

    assert_eq!(
        package_data.keywords,
        vec![
            "prettier".to_string(),
            "formatter".to_string(),
            "javascript".to_string(),
            "multi-root ready".to_string(),
        ]
    );
    assert_eq!(
        package_data.vcs_url.as_deref(),
        Some("https://github.com/prettier/prettier-vscode.git")
    );
    assert_eq!(
        package_data.bug_tracking_url.as_deref(),
        Some("https://github.com/prettier/prettier-vscode/issues")
    );
    // No `<MoreInfo>`/`Learn` link present, so homepage stays honestly empty.
    assert_eq!(package_data.homepage_url, None);
    let extra_data = package_data.extra_data.as_ref().expect("extra_data");
    assert_eq!(
        extra_data.get("engine").and_then(|value| value.as_str()),
        Some("^1.80.0")
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

#[test]
fn test_combines_mixed_text_and_cdata_metadata() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("extension.vsixmanifest");
    std::fs::write(
        &path,
        r#"<PackageManifest Version="2.0.0">
  <Metadata>
    <Identity Id="sample" Version="1.0.0" Publisher="example" />
    <DisplayName>Sample <![CDATA[Extension]]></DisplayName>
    <Description>Uses <![CDATA[<VS Code>]]> helpers</Description>
  </Metadata>
</PackageManifest>"#,
    )
    .expect("write manifest");

    let package_data = VscodeExtensionManifestParser::extract_first_package(&path);

    assert_eq!(
        package_data.description.as_deref(),
        Some("Uses <VS Code> helpers")
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
