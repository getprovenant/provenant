// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::path::Path;

use crate::parser_warn as warn;
use packageurl::PackageUrl;
use serde_json::{Map as JsonMap, Value};

use crate::models::{DatasourceId, Dependency, PackageData, PackageType, Party, PartyType};
use crate::parsers::rfc822::{self, Rfc822Metadata};
use crate::parsers::utils::{capped_iteration_limit, split_name_email, truncate_field};

use super::PackageParser;

pub struct VcpkgManifestParser;
pub struct VcpkgConfigurationParser;
pub struct VcpkgLockParser;
pub struct VcpkgControlParser;

impl PackageParser for VcpkgManifestParser {
    const PACKAGE_TYPE: PackageType = PackageType::Vcpkg;

    fn is_match(path: &Path) -> bool {
        path.file_name().and_then(|name| name.to_str()) == Some("vcpkg.json")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match crate::parsers::utils::read_file_to_string(path, None) {
            Ok(content) => content,
            Err(e) => {
                warn!("Failed to read vcpkg.json at {:?}: {}", path, e);
                return vec![default_package_data()];
            }
        };

        let json: Value = match serde_json::from_str(&content) {
            Ok(json) => json,
            Err(e) => {
                warn!("Failed to parse vcpkg.json at {:?}: {}", path, e);
                return vec![default_package_data()];
            }
        };

        vec![parse_vcpkg_manifest(path, &json)]
    }

    fn metadata() -> Vec<super::metadata::ParserMetadata> {
        vec![super::metadata::ParserMetadata {
            description: "vcpkg manifest file",
            file_patterns: &["**/vcpkg.json"],
            package_type: "vcpkg",
            primary_language: "",
            documentation_url: Some("https://learn.microsoft.com/en-us/vcpkg/reference/vcpkg-json"),
        }]
    }
}

impl PackageParser for VcpkgConfigurationParser {
    const PACKAGE_TYPE: PackageType = PackageType::Vcpkg;

    fn is_match(path: &Path) -> bool {
        path.file_name().and_then(|name| name.to_str()) == Some("vcpkg-configuration.json")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match crate::parsers::utils::read_file_to_string(path, None) {
            Ok(content) => content,
            Err(e) => {
                warn!(
                    "Failed to read vcpkg-configuration.json at {:?}: {}",
                    path, e
                );
                return vec![default_configuration_package_data()];
            }
        };

        let json: Value = match serde_json::from_str(&content) {
            Ok(json) => json,
            Err(e) => {
                warn!(
                    "Failed to parse vcpkg-configuration.json at {:?}: {}",
                    path, e
                );
                return vec![default_configuration_package_data()];
            }
        };

        vec![parse_vcpkg_configuration(&json)]
    }

    fn metadata() -> Vec<super::metadata::ParserMetadata> {
        vec![super::metadata::ParserMetadata {
            description: "vcpkg configuration file",
            file_patterns: &["**/vcpkg-configuration.json"],
            package_type: "vcpkg",
            primary_language: "C++",
            documentation_url: Some(
                "https://learn.microsoft.com/en-us/vcpkg/reference/vcpkg-configuration-json",
            ),
        }]
    }
}

impl PackageParser for VcpkgLockParser {
    const PACKAGE_TYPE: PackageType = PackageType::Vcpkg;

    fn is_match(path: &Path) -> bool {
        path.file_name().and_then(|name| name.to_str()) == Some("vcpkg-lock.json")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match crate::parsers::utils::read_file_to_string(path, None) {
            Ok(content) => content,
            Err(e) => {
                warn!("Failed to read vcpkg-lock.json at {:?}: {}", path, e);
                return vec![default_lock_package_data()];
            }
        };

        let json: Value = match serde_json::from_str(&content) {
            Ok(json) => json,
            Err(e) => {
                warn!("Failed to parse vcpkg-lock.json at {:?}: {}", path, e);
                return vec![default_lock_package_data()];
            }
        };

        vec![parse_vcpkg_lock(&json)]
    }

    fn metadata() -> Vec<super::metadata::ParserMetadata> {
        vec![super::metadata::ParserMetadata {
            description: "vcpkg registry lockfile",
            file_patterns: &["**/vcpkg-lock.json"],
            package_type: "vcpkg",
            primary_language: "C++",
            documentation_url: Some("https://github.com/microsoft/vcpkg-tool"),
        }]
    }
}

impl PackageParser for VcpkgControlParser {
    const PACKAGE_TYPE: PackageType = PackageType::Vcpkg;

    fn is_match(path: &Path) -> bool {
        path.file_name().and_then(|name| name.to_str()) == Some("CONTROL")
            && path
                .parent()
                .and_then(Path::parent)
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                == Some("ports")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match crate::parsers::utils::read_file_to_string(path, None) {
            Ok(content) => content,
            Err(e) => {
                warn!("Failed to read vcpkg CONTROL at {:?}: {}", path, e);
                return vec![default_control_package_data()];
            }
        };

        vec![parse_vcpkg_control(&content)]
    }

    fn metadata() -> Vec<super::metadata::ParserMetadata> {
        vec![super::metadata::ParserMetadata {
            description: "vcpkg classic port CONTROL file",
            file_patterns: &["**/ports/*/CONTROL"],
            package_type: "vcpkg",
            primary_language: "C++",
            documentation_url: Some(
                "https://learn.microsoft.com/en-us/vcpkg/maintainers/control-files",
            ),
        }]
    }
}

fn default_package_data() -> PackageData {
    PackageData {
        package_type: Some(PackageType::Vcpkg),
        datasource_id: Some(DatasourceId::VcpkgJson),
        ..Default::default()
    }
}

fn default_lock_package_data() -> PackageData {
    PackageData {
        package_type: Some(PackageType::Vcpkg),
        datasource_id: Some(DatasourceId::VcpkgLockJson),
        is_private: true,
        ..Default::default()
    }
}

fn default_configuration_package_data() -> PackageData {
    PackageData {
        package_type: Some(PackageType::Vcpkg),
        datasource_id: Some(DatasourceId::VcpkgConfigurationJson),
        is_private: true,
        ..Default::default()
    }
}

fn default_control_package_data() -> PackageData {
    PackageData {
        package_type: Some(PackageType::Vcpkg),
        datasource_id: Some(DatasourceId::VcpkgControl),
        primary_language: Some("C++".to_string()),
        is_private: true,
        ..Default::default()
    }
}

fn parse_vcpkg_manifest(path: &Path, json: &Value) -> PackageData {
    let name = get_non_empty_string(json, "name").map(truncate_field);
    let version = manifest_version(json).map(truncate_field);
    let description = get_string_or_array(json, "description").map(truncate_field);
    let homepage_url = get_non_empty_string(json, "homepage").map(truncate_field);
    let extracted_license_statement = get_string_or_array(json, "license").map(truncate_field);
    let parties = extract_maintainers(json);
    let dependencies = extract_dependencies(json);
    let extra_data = build_extra_data(path, json);

    PackageData {
        package_type: Some(PackageType::Vcpkg),
        namespace: None,
        name: name.clone(),
        version: version.clone(),
        primary_language: Some("C++".to_string()),
        description,
        parties,
        homepage_url,
        extracted_license_statement,
        is_private: name.is_none(),
        dependencies,
        extra_data,
        datasource_id: Some(DatasourceId::VcpkgJson),
        purl: name
            .as_deref()
            .and_then(|name| build_vcpkg_purl(name, version.as_deref()))
            .map(truncate_field),
        ..default_package_data()
    }
}

fn parse_vcpkg_lock(json: &Value) -> PackageData {
    PackageData {
        primary_language: Some("C++".to_string()),
        extra_data: build_lock_extra_data(json),
        ..default_lock_package_data()
    }
}

fn parse_vcpkg_configuration(json: &Value) -> PackageData {
    PackageData {
        primary_language: Some("C++".to_string()),
        extra_data: build_configuration_extra_data(json),
        ..default_configuration_package_data()
    }
}

fn parse_vcpkg_control(content: &str) -> PackageData {
    let paragraphs = rfc822::parse_rfc822_paragraphs(content);
    let Some(source_paragraph) = paragraphs.first() else {
        return default_control_package_data();
    };

    let Some(name) = rfc822::get_header_first(&source_paragraph.headers, "source")
        .map(truncate_field)
        .filter(|name| !name.is_empty())
    else {
        return default_control_package_data();
    };

    let version = control_version(source_paragraph).map(truncate_field);
    let description = rfc822::get_header_first(&source_paragraph.headers, "description")
        .map(truncate_field)
        .filter(|description| !description.is_empty());
    let homepage_url =
        rfc822::get_header_first(&source_paragraph.headers, "homepage").map(truncate_field);

    let feature_paragraphs = &paragraphs[1..];
    let feature_limit = capped_iteration_limit(
        feature_paragraphs.len(),
        "vcpkg CONTROL feature dependency paragraphs",
    );

    let mut dependencies = extract_control_dependencies(source_paragraph, None);
    for feature_paragraph in feature_paragraphs.iter().take(feature_limit) {
        if let Some(feature) = rfc822::get_header_first(&feature_paragraph.headers, "feature")
            .map(truncate_field)
            .filter(|feature| !feature.is_empty())
        {
            dependencies.extend(extract_control_dependencies(
                feature_paragraph,
                Some(feature.as_str()),
            ));
        }
    }

    PackageData {
        package_type: Some(PackageType::Vcpkg),
        name: Some(name.clone()),
        version: version.clone(),
        primary_language: Some("C++".to_string()),
        description,
        homepage_url,
        dependencies,
        extra_data: build_control_extra_data(source_paragraph, feature_paragraphs),
        datasource_id: Some(DatasourceId::VcpkgControl),
        purl: build_vcpkg_purl(&name, version.as_deref()).map(truncate_field),
        is_private: false,
        ..default_control_package_data()
    }
}

fn control_version(paragraph: &Rfc822Metadata) -> Option<String> {
    let version = rfc822::get_header_first(&paragraph.headers, "version")?;
    let port_version = rfc822::get_header_first(&paragraph.headers, "port-version")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|port_version| *port_version > 0);

    match port_version {
        Some(port_version) => Some(format!("{}#{}", version, port_version)),
        None => Some(version),
    }
}

fn build_control_extra_data(
    source_paragraph: &Rfc822Metadata,
    feature_paragraphs: &[Rfc822Metadata],
) -> Option<HashMap<String, Value>> {
    let mut extra = HashMap::new();

    if let Some(default_features) =
        rfc822::get_header_first(&source_paragraph.headers, "default-features")
    {
        let features =
            string_list_to_json_array(&default_features, "vcpkg CONTROL default features");
        if !features.is_empty() {
            extra.insert("default-features".to_string(), Value::Array(features));
        }
    }

    if let Some(supports) = rfc822::get_header_first(&source_paragraph.headers, "supports")
        .filter(|supports| !supports.is_empty())
    {
        extra.insert(
            "supports".to_string(),
            Value::String(truncate_field(supports)),
        );
    }

    let features = extract_control_feature_metadata(feature_paragraphs);
    if !features.is_empty() {
        extra.insert("features".to_string(), Value::Array(features));
    }

    (!extra.is_empty()).then_some(extra)
}

fn extract_control_feature_metadata(feature_paragraphs: &[Rfc822Metadata]) -> Vec<Value> {
    let feature_limit =
        capped_iteration_limit(feature_paragraphs.len(), "vcpkg CONTROL feature paragraphs");
    feature_paragraphs
        .iter()
        .take(feature_limit)
        .filter_map(|paragraph| {
            let name = rfc822::get_header_first(&paragraph.headers, "feature")
                .map(truncate_field)
                .filter(|name| !name.is_empty())?;

            let mut feature = JsonMap::new();
            feature.insert("name".to_string(), Value::String(name));
            if let Some(description) = rfc822::get_header_first(&paragraph.headers, "description")
                .map(truncate_field)
                .filter(|description| !description.is_empty())
            {
                feature.insert("description".to_string(), Value::String(description));
            }
            if let Some(build_depends) =
                rfc822::get_header_first(&paragraph.headers, "build-depends")
            {
                let dependencies =
                    string_list_to_json_array(&build_depends, "vcpkg CONTROL feature dependencies");
                if !dependencies.is_empty() {
                    feature.insert("build-depends".to_string(), Value::Array(dependencies));
                }
            }

            Some(Value::Object(feature))
        })
        .collect()
}

fn extract_control_dependencies(
    paragraph: &Rfc822Metadata,
    feature_name: Option<&str>,
) -> Vec<Dependency> {
    let Some(build_depends) = rfc822::get_header_first(&paragraph.headers, "build-depends") else {
        return Vec::new();
    };

    split_control_list(&build_depends, "vcpkg CONTROL dependencies")
        .into_iter()
        .filter_map(|dependency| parse_control_dependency_entry(&dependency, feature_name))
        .collect()
}

fn parse_control_dependency_entry(entry: &str, feature_name: Option<&str>) -> Option<Dependency> {
    let raw_entry = entry.trim();
    if raw_entry.is_empty() {
        return None;
    }

    let (without_platform, platform) = split_dependency_platform(raw_entry);
    let (name, features) = split_dependency_features(without_platform);
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    let mut extra_data = HashMap::new();
    if !features.is_empty() {
        extra_data.insert("features".to_string(), Value::Array(features));
    }
    if let Some(platform) = platform {
        extra_data.insert(
            "platform".to_string(),
            Value::String(truncate_field(platform)),
        );
    }
    if let Some(feature_name) = feature_name {
        extra_data.insert(
            "feature".to_string(),
            Value::String(truncate_field(feature_name.to_string())),
        );
    }
    let is_optional = if feature_name.is_some() {
        None
    } else {
        Some(false)
    };

    Some(Dependency {
        purl: build_vcpkg_purl(name, None).map(truncate_field),
        extracted_requirement: Some(truncate_field(raw_entry.to_string())),
        scope: Some("build-depends".to_string()),
        is_runtime: Some(true),
        is_optional,
        is_pinned: Some(false),
        is_direct: Some(true),
        resolved_package: None,
        extra_data: (!extra_data.is_empty()).then_some(extra_data),
    })
}

fn split_control_list(value: &str, context: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut start = 0;
    let mut bracket_depth = 0_u32;
    let mut paren_depth = 0_u32;

    for (index, ch) in value.char_indices() {
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            ',' if bracket_depth == 0 && paren_depth == 0 => {
                entries.push(value[start..index].trim().to_string());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    entries.push(value[start..].trim().to_string());

    let limit = capped_iteration_limit(entries.len(), context);
    entries
        .into_iter()
        .take(limit)
        .filter(|entry| !entry.is_empty())
        .collect()
}

fn string_list_to_json_array(value: &str, context: &str) -> Vec<Value> {
    split_control_list(value, context)
        .into_iter()
        .map(|entry| Value::String(truncate_field(entry)))
        .collect()
}

fn split_dependency_platform(entry: &str) -> (&str, Option<String>) {
    let entry = entry.trim();
    if !entry.ends_with(')') {
        return (entry, None);
    }

    let mut depth = 0_i32;
    for (index, ch) in entry.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    let dependency = entry[..index].trim_end();
                    let platform = entry[index + 1..entry.len() - 1].trim();
                    if dependency.is_empty() || platform.is_empty() {
                        return (entry, None);
                    }
                    return (dependency, Some(platform.to_string()));
                }
            }
            _ => {}
        }
    }

    (entry, None)
}

fn split_dependency_features(entry: &str) -> (&str, Vec<Value>) {
    let entry = entry.trim();
    let Some(start) = entry.find('[') else {
        return (entry, Vec::new());
    };
    if !entry.ends_with(']') {
        return (entry, Vec::new());
    }

    let name = entry[..start].trim();
    let mut features = Vec::new();
    let mut remaining = entry[start..].trim();
    while !remaining.is_empty() {
        let Some(after_open) = remaining.strip_prefix('[') else {
            return (entry, Vec::new());
        };
        let Some(end) = after_open.find(']') else {
            return (entry, Vec::new());
        };

        features.extend(
            after_open[..end]
                .split(',')
                .map(str::trim)
                .filter(|feature| !feature.is_empty())
                .map(|feature| Value::String(truncate_field(feature.to_string()))),
        );
        remaining = after_open[end + 1..].trim_start();
    }

    (name, features)
}

/// Preserve registry and overlay provenance from a standalone
/// `vcpkg-configuration.json`. This file declares where dependencies are
/// resolved from (registries) and which local overlays apply, but it has no
/// package identity of its own, so the metadata is preserved verbatim.
fn build_configuration_extra_data(json: &Value) -> Option<HashMap<String, Value>> {
    let mut extra = HashMap::new();
    for field in [
        "default-registry",
        "registries",
        "overlay-ports",
        "overlay-triplets",
    ] {
        if let Some(value) = json.get(field) {
            extra.insert(field.to_string(), value.clone());
        }
    }

    (!extra.is_empty()).then_some(extra)
}

fn build_lock_extra_data(json: &Value) -> Option<HashMap<String, Value>> {
    let registry_locks = extract_lock_registry_entries(json);
    if registry_locks.is_empty() {
        return None;
    }

    let mut extra = HashMap::new();
    extra.insert("registry_locks".to_string(), Value::Array(registry_locks));
    Some(extra)
}

fn extract_lock_registry_entries(json: &Value) -> Vec<Value> {
    let Some(registries) = json.as_object() else {
        return Vec::new();
    };

    let registry_limit = capped_iteration_limit(registries.len(), "vcpkg lock registries");
    registries
        .iter()
        .take(registry_limit)
        .filter_map(|(location, references)| {
            let references = references.as_object()?;
            let reference_limit = capped_iteration_limit(references.len(), "vcpkg lock references");
            let mut normalized_references = JsonMap::new();
            for (reference, revision) in references.iter().take(reference_limit) {
                let Some(revision) = revision
                    .as_str()
                    .map(str::trim)
                    .filter(|revision| !revision.is_empty())
                else {
                    continue;
                };

                normalized_references.insert(
                    truncate_field(reference.to_string()),
                    Value::String(truncate_field(revision.to_string())),
                );
            }

            if normalized_references.is_empty() {
                return None;
            }

            let mut entry = JsonMap::new();
            entry.insert(
                "location".to_string(),
                Value::String(truncate_field(location.to_string())),
            );
            entry.insert(
                "references".to_string(),
                Value::Object(normalized_references),
            );
            Some(Value::Object(entry))
        })
        .collect()
}

fn manifest_version(json: &Value) -> Option<String> {
    let version = [
        "version",
        "version-semver",
        "version-date",
        "version-string",
    ]
    .into_iter()
    .find_map(|field| get_non_empty_string(json, field));

    match (version, json.get("port-version").and_then(Value::as_i64)) {
        (Some(version), Some(port_version)) if port_version > 0 => {
            Some(format!("{}#{}", version, port_version))
        }
        (version, _) => version,
    }
}

fn extract_maintainers(json: &Value) -> Vec<Party> {
    let Some(value) = json.get("maintainers") else {
        return Vec::new();
    };

    let maintainers: Vec<String> = match value {
        Value::String(s) => vec![s.clone()],
        Value::Array(values) => {
            let limit = capped_iteration_limit(values.len(), "vcpkg maintainers");
            values
                .iter()
                .take(limit)
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        }
        _ => Vec::new(),
    };

    maintainers
        .into_iter()
        .map(|entry| {
            let (name, email) = split_name_email(&entry);
            Party {
                r#type: Some(PartyType::Person),
                role: Some("maintainer".to_string()),
                name: name.map(truncate_field),
                email: email.map(truncate_field),
                url: None,
                organization: None,
                organization_url: None,
                timezone: None,
            }
        })
        .collect()
}

fn extract_dependencies(json: &Value) -> Vec<Dependency> {
    let overrides = extract_overrides(json);

    let mut dependencies: Vec<Dependency> = json
        .get("dependencies")
        .and_then(Value::as_array)
        .map(|deps| {
            let limit = capped_iteration_limit(deps.len(), "vcpkg dependencies");
            deps.iter()
                .take(limit)
                .filter_map(|dep| parse_dependency_entry(dep, &overrides))
                .collect()
        })
        .unwrap_or_default();

    if let Some(features) = json.get("features").and_then(Value::as_object) {
        let features_limit = capped_iteration_limit(features.len(), "vcpkg features");
        for (feature_name, feature_value) in features.iter().take(features_limit) {
            let Some(feature_dependencies) =
                feature_value.get("dependencies").and_then(Value::as_array)
            else {
                continue;
            };

            let feature_deps_limit =
                capped_iteration_limit(feature_dependencies.len(), "vcpkg feature dependencies");
            for dependency in feature_dependencies
                .iter()
                .take(feature_deps_limit)
                .filter_map(|dep| parse_dependency_entry(dep, &overrides))
                .map(|mut dependency| {
                    let mut extra_data = dependency.extra_data.take().unwrap_or_default();
                    extra_data.insert(
                        "feature".to_string(),
                        Value::String(feature_name.to_string()),
                    );
                    dependency.extra_data = Some(extra_data);
                    dependency
                })
            {
                dependencies.push(dependency);
            }
        }
    }

    dependencies
}

/// Map of dependency name to the exact version pinned by the manifest's
/// `overrides` array. An override is an author-declared hard pin, so it proves
/// version intent even though the dependency entry itself only declares a
/// `version>=` floor.
fn extract_overrides(json: &Value) -> HashMap<String, String> {
    let Some(overrides) = json.get("overrides").and_then(Value::as_array) else {
        return HashMap::new();
    };

    let limit = capped_iteration_limit(overrides.len(), "vcpkg overrides");
    overrides
        .iter()
        .take(limit)
        .filter_map(|entry| {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())?;
            let version = manifest_version(entry)?;
            Some((name.to_string(), truncate_field(version)))
        })
        .collect()
}

fn parse_dependency_entry(
    value: &Value,
    overrides: &HashMap<String, String>,
) -> Option<Dependency> {
    match value {
        Value::String(name) => {
            let pinned = overrides.get(name.trim());
            Some(Dependency {
                purl: build_vcpkg_purl(name, None).map(truncate_field),
                extracted_requirement: Some(truncate_field(name.clone())),
                scope: Some("dependencies".to_string()),
                is_runtime: Some(true),
                is_optional: Some(false),
                is_pinned: Some(pinned.is_some()),
                is_direct: Some(true),
                resolved_package: None,
                extra_data: pinned.map(|version| override_extra_data(version)),
            })
        }
        Value::Object(obj) => {
            let name = obj.get("name").and_then(Value::as_str)?.trim();
            if name.is_empty() {
                return None;
            }

            let extracted_requirement = obj
                .get("version>=")
                .and_then(Value::as_str)
                .map(|v| truncate_field(v.to_owned()))
                .or_else(|| Some(truncate_field(name.to_string())));

            let host = obj.get("host").and_then(Value::as_bool).unwrap_or(false);
            let pinned = overrides.get(name);
            let mut extra = HashMap::new();
            for field in [
                "version>=",
                "features",
                "default-features",
                "host",
                "platform",
            ] {
                if let Some(field_value) = obj.get(field) {
                    extra.insert(field.to_string(), field_value.clone());
                }
            }
            if let Some(version) = pinned {
                extra.insert(
                    "override_version".to_string(),
                    Value::String(version.clone()),
                );
            }

            Some(Dependency {
                purl: build_vcpkg_purl(name, None).map(truncate_field),
                extracted_requirement,
                scope: Some("dependencies".to_string()),
                is_runtime: Some(!host),
                is_optional: Some(false),
                is_pinned: Some(pinned.is_some()),
                is_direct: Some(true),
                resolved_package: None,
                extra_data: (!extra.is_empty()).then_some(extra),
            })
        }
        _ => None,
    }
}

fn override_extra_data(version: &str) -> HashMap<String, Value> {
    let mut extra = HashMap::new();
    extra.insert(
        "override_version".to_string(),
        Value::String(version.to_owned()),
    );
    extra
}

fn build_extra_data(path: &Path, json: &Value) -> Option<HashMap<String, Value>> {
    let mut extra = HashMap::new();
    for field in [
        "builtin-baseline",
        "overrides",
        "supports",
        "default-features",
        "features",
        "configuration",
        "vcpkg-configuration",
        "documentation",
    ] {
        if let Some(value) = json.get(field) {
            extra.insert(field.to_string(), value.clone());
        }
    }

    if !extra.contains_key("configuration")
        && !extra.contains_key("vcpkg-configuration")
        && let Some(config) = read_sibling_configuration(path)
    {
        extra.insert("configuration".to_string(), config);
    }

    (!extra.is_empty()).then_some(extra)
}

fn read_sibling_configuration(path: &Path) -> Option<Value> {
    let sibling_path = path.with_file_name("vcpkg-configuration.json");
    let content = crate::parsers::utils::read_file_to_string(&sibling_path, None).ok()?;
    match serde_json::from_str(&content) {
        Ok(value) => Some(value),
        Err(e) => {
            warn!(
                "Failed to parse sibling vcpkg-configuration.json at {:?}: {}",
                sibling_path, e
            );
            None
        }
    }
}

fn get_non_empty_string(json: &Value, field: &str) -> Option<String> {
    json.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn get_string_or_array(json: &Value, field: &str) -> Option<String> {
    match json.get(field) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Array(values)) => {
            let collected: Vec<_> = values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            (!collected.is_empty()).then(|| collected.join("\n"))
        }
        _ => None,
    }
}

fn build_vcpkg_purl(name: &str, version: Option<&str>) -> Option<String> {
    let mut purl = PackageUrl::new("generic", name).ok()?;
    purl.with_namespace("vcpkg").ok()?;
    if let Some(version) = version {
        purl.with_version(version).ok()?;
    }
    Some(purl.to_string())
}
