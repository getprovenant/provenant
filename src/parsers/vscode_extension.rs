// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Parser for VS Code extension `.vsixmanifest` metadata files.
//!
//! This parser handles the already-extracted `extension.vsixmanifest` XML file
//! found in packaged VSIX extensions. It does not unpack `.vsix` archives and it
//! does not parse OpenVSX API responses.

use std::collections::HashMap;
use std::path::Path;

use packageurl::PackageUrl;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde_json::Value as JsonValue;

use super::PackageParser;
use super::metadata::ParserMetadata;
use crate::models::{DatasourceId, PackageData, PackageType, Party, PartyType};
use crate::parser_warn as warn;
use crate::parsers::utils::{MAX_ITERATION_COUNT, read_file_to_string, truncate_field};

const PACKAGE_TYPE: PackageType = PackageType::VscodeExtension;
const DATASOURCE_ID: DatasourceId = DatasourceId::VscodeExtensionVsixManifest;

pub struct VscodeExtensionManifestParser;

#[derive(Default)]
struct VsixManifestMetadata {
    id: Option<String>,
    publisher: Option<String>,
    version: Option<String>,
    language: Option<String>,
    target_platform: Option<String>,
    display_name: Option<String>,
    description: Option<String>,
    more_info: Option<String>,
    license_file: Option<String>,
    release_notes: Option<String>,
    icon: Option<String>,
    preview_image: Option<String>,
    tags: Option<String>,
    extension_type: Option<String>,
}

#[derive(Clone, Copy)]
enum MetadataTextElement {
    DisplayName,
    Description,
    MoreInfo,
    License,
    ReleaseNotes,
    Icon,
    PreviewImage,
    Tags,
    ExtensionType,
}

impl PackageParser for VscodeExtensionManifestParser {
    const PACKAGE_TYPE: PackageType = PACKAGE_TYPE;

    fn is_match(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "extension.vsixmanifest")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(content) => content,
            Err(error) => {
                warn!(
                    "Failed to read extension.vsixmanifest at {:?}: {}",
                    path, error
                );
                return vec![default_package_data()];
            }
        };

        vec![parse_vscode_extension_manifest(&content, path)]
    }

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "VS Code extension VSIX manifest",
            file_patterns: &["**/extension.vsixmanifest"],
            package_type: "vscode-extension",
            primary_language: "",
            documentation_url: Some(
                "https://learn.microsoft.com/en-us/visualstudio/extensibility/vsix-extension-schema-2-0-reference",
            ),
        }]
    }
}

fn default_package_data() -> PackageData {
    PackageData {
        package_type: Some(PACKAGE_TYPE),
        datasource_id: Some(DATASOURCE_ID),
        ..Default::default()
    }
}

fn parse_vscode_extension_manifest(content: &str, path: &Path) -> PackageData {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut metadata = VsixManifestMetadata::default();
    let mut in_metadata = false;
    let mut current_text_element = None;
    let mut buf = Vec::new();
    let mut iteration_count = 0usize;

    loop {
        iteration_count += 1;
        if iteration_count > MAX_ITERATION_COUNT {
            warn!(
                "Iteration limit exceeded in extension.vsixmanifest at {:?}; stopping at {} items",
                path, MAX_ITERATION_COUNT
            );
            break;
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                let name = local_xml_name(element.name().as_ref());
                match name.as_str() {
                    "Metadata" => in_metadata = true,
                    "Identity" if in_metadata => parse_identity(&element, &mut metadata),
                    _ => {
                        current_text_element = in_metadata
                            .then(|| metadata_text_element(name.as_str()))
                            .flatten();
                    }
                }
            }
            Ok(Event::Empty(element)) => {
                if in_metadata && local_xml_name(element.name().as_ref()) == "Identity" {
                    parse_identity(&element, &mut metadata);
                }
            }
            Ok(Event::Text(text)) => {
                if let Some(element) = current_text_element
                    && let Some(value) = text.decode().ok().map(|value| value.trim().to_string())
                    && !value.is_empty()
                {
                    apply_metadata_text(&mut metadata, element, value);
                }
            }
            Ok(Event::End(element)) => {
                let name = local_xml_name(element.name().as_ref());
                if name == "Metadata" {
                    in_metadata = false;
                }
                current_text_element = None;
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                warn!(
                    "Error parsing extension.vsixmanifest at {:?}: {}",
                    path, error
                );
                break;
            }
            _ => {}
        }

        buf.clear();
    }

    package_data_from_metadata(metadata)
}

fn parse_identity(element: &BytesStart, metadata: &mut VsixManifestMetadata) {
    metadata.id = attr_value(element, b"Id").map(truncate_field);
    metadata.publisher = attr_value(element, b"Publisher").map(truncate_field);
    metadata.version = attr_value(element, b"Version").map(truncate_field);
    metadata.language = attr_value(element, b"Language").map(truncate_field);
    metadata.target_platform = attr_value(element, b"TargetPlatform")
        .or_else(|| attr_value(element, b"targetPlatform"))
        .map(truncate_field);
}

fn attr_value(element: &BytesStart, key: &[u8]) -> Option<String> {
    element
        .attributes()
        .filter_map(|attr| attr.ok())
        .find(|attr| local_xml_name(attr.key.as_ref()).as_bytes() == key)
        .and_then(|attr| {
            attr.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.into_owned())
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn local_xml_name(name: &[u8]) -> String {
    let name = String::from_utf8_lossy(name);
    name.rsplit(':').next().unwrap_or(&name).to_string()
}

fn metadata_text_element(name: &str) -> Option<MetadataTextElement> {
    match name {
        "DisplayName" => Some(MetadataTextElement::DisplayName),
        "Description" => Some(MetadataTextElement::Description),
        "MoreInfo" => Some(MetadataTextElement::MoreInfo),
        "License" => Some(MetadataTextElement::License),
        "ReleaseNotes" => Some(MetadataTextElement::ReleaseNotes),
        "Icon" => Some(MetadataTextElement::Icon),
        "PreviewImage" => Some(MetadataTextElement::PreviewImage),
        "Tags" => Some(MetadataTextElement::Tags),
        "ExtensionType" => Some(MetadataTextElement::ExtensionType),
        _ => None,
    }
}

fn apply_metadata_text(
    metadata: &mut VsixManifestMetadata,
    element: MetadataTextElement,
    value: String,
) {
    let value = truncate_field(value);
    match element {
        MetadataTextElement::DisplayName => metadata.display_name = Some(value),
        MetadataTextElement::Description => metadata.description = Some(value),
        MetadataTextElement::MoreInfo => metadata.more_info = Some(value),
        MetadataTextElement::License => metadata.license_file = Some(value),
        MetadataTextElement::ReleaseNotes => metadata.release_notes = Some(value),
        MetadataTextElement::Icon => metadata.icon = Some(value),
        MetadataTextElement::PreviewImage => metadata.preview_image = Some(value),
        MetadataTextElement::Tags => metadata.tags = Some(value),
        MetadataTextElement::ExtensionType => metadata.extension_type = Some(value),
    }
}

fn package_data_from_metadata(metadata: VsixManifestMetadata) -> PackageData {
    let keywords = metadata
        .tags
        .as_deref()
        .map(parse_semicolon_list)
        .unwrap_or_default();

    let mut extra_data = HashMap::new();
    insert_extra_string(
        &mut extra_data,
        "display_name",
        metadata.display_name.clone(),
    );
    insert_extra_string(
        &mut extra_data,
        "identity_language",
        metadata.language.clone(),
    );
    insert_extra_string(
        &mut extra_data,
        "target_platform",
        metadata.target_platform.clone(),
    );
    insert_extra_string(
        &mut extra_data,
        "license_file",
        metadata.license_file.clone(),
    );
    insert_extra_string(
        &mut extra_data,
        "release_notes",
        metadata.release_notes.clone(),
    );
    insert_extra_string(&mut extra_data, "icon", metadata.icon.clone());
    insert_extra_string(
        &mut extra_data,
        "preview_image",
        metadata.preview_image.clone(),
    );
    insert_extra_string(
        &mut extra_data,
        "extension_type",
        metadata.extension_type.clone(),
    );

    let qualifiers = metadata
        .target_platform
        .clone()
        .map(|target_platform| HashMap::from([("platform".to_string(), target_platform)]));

    let purl = match (&metadata.publisher, &metadata.id) {
        (Some(publisher), Some(id)) => build_vscode_extension_purl(
            publisher,
            id,
            metadata.version.as_deref(),
            metadata.target_platform.as_deref(),
        ),
        _ => None,
    };

    let parties = metadata
        .publisher
        .clone()
        .map_or_else(Vec::new, |publisher| {
            vec![Party {
                r#type: Some(PartyType::Organization),
                role: Some("publisher".to_string()),
                name: Some(publisher),
                email: None,
                url: None,
                organization: None,
                organization_url: None,
                timezone: None,
            }]
        });

    PackageData {
        package_type: Some(PACKAGE_TYPE),
        namespace: metadata.publisher,
        name: metadata.id,
        version: metadata.version,
        qualifiers,
        description: metadata.description,
        parties,
        keywords,
        homepage_url: metadata.more_info,
        extra_data: (!extra_data.is_empty()).then_some(extra_data),
        datasource_id: Some(DATASOURCE_ID),
        purl,
        ..Default::default()
    }
}

fn insert_extra_string(
    extra_data: &mut HashMap<String, JsonValue>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        extra_data.insert(key.to_string(), JsonValue::String(value));
    }
}

fn parse_semicolon_list(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| truncate_field(item.to_string()))
        .collect()
}

fn build_vscode_extension_purl(
    publisher: &str,
    id: &str,
    version: Option<&str>,
    target_platform: Option<&str>,
) -> Option<String> {
    let mut purl = PackageUrl::new(PACKAGE_TYPE.as_str(), id).ok()?;
    purl.with_namespace(publisher.to_string()).ok()?;

    if let Some(version) = version {
        purl.with_version(version.to_string()).ok()?;
    }

    if let Some(target_platform) = target_platform {
        purl.add_qualifier("platform", target_platform.to_string())
            .ok()?;
    }

    Some(truncate_field(purl.to_string()))
}
