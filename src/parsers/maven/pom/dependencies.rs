// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::models::Dependency;
use std::collections::HashMap;

#[derive(Clone, Default)]
pub(super) struct MavenDependencyData {
    pub(super) group_id: Option<String>,
    pub(super) artifact_id: Option<String>,
    pub(super) version: Option<String>,
    pub(super) classifier: Option<String>,
    pub(super) type_: Option<String>,
    pub(super) scope: Option<String>,
    pub(super) optional: Option<String>,
    pub(super) system_path: Option<String>,
    pub(super) message: Option<String>,
}

pub(super) fn parse_maven_bool(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

fn normalize_maven_packaging(packaging: Option<&str>) -> Option<&str> {
    match packaging.map(str::trim).filter(|value| !value.is_empty()) {
        Some(
            "ejb3" | "ear" | "aar" | "apk" | "gem" | "jar" | "nar" | "pom" | "so" | "swc" | "tar"
            | "tar.gz" | "war" | "xar" | "zip",
        ) => packaging.map(str::trim),
        Some(_) => Some("jar"),
        None => None,
    }
}

pub(super) fn build_maven_qualifiers(
    classifier: Option<&str>,
    packaging: Option<&str>,
) -> Option<HashMap<String, String>> {
    let mut qualifiers = HashMap::new();

    if let Some(classifier) = classifier.filter(|value| !value.trim().is_empty()) {
        qualifiers.insert("classifier".to_string(), classifier.to_string());
    }

    if let Some(packaging) = normalize_maven_packaging(packaging)
        .filter(|value| !value.is_empty() && *value != "jar" && *value != "pom")
    {
        qualifiers.insert("type".to_string(), packaging.to_string());
    }

    (!qualifiers.is_empty()).then_some(qualifiers)
}

pub(super) fn build_maven_purl(
    group_id: &str,
    artifact_id: &str,
    version: Option<&str>,
    classifier: Option<&str>,
    packaging: Option<&str>,
) -> String {
    let mut purl = format!(
        "pkg:maven/{}/{}",
        percent_encode_purl_component(group_id),
        percent_encode_purl_component(artifact_id)
    );

    if let Some(version) = version.filter(|value| !value.trim().is_empty()) {
        purl.push('@');
        purl.push_str(&percent_encode_purl_component(version));
    }

    let qualifiers = build_maven_qualifiers(classifier, packaging);
    if let Some(qualifiers) = qualifiers {
        let mut query_parts = Vec::new();
        if let Some(classifier) = qualifiers.get("classifier") {
            query_parts.push(format!(
                "classifier={}",
                percent_encode_purl_component(classifier)
            ));
        }
        if let Some(type_) = qualifiers.get("type") {
            query_parts.push(format!("type={}", percent_encode_purl_component(type_)));
        }

        if !query_parts.is_empty() {
            purl.push('?');
            purl.push_str(&query_parts.join("&"));
        }
    }

    purl
}

fn percent_encode_purl_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

pub(super) fn build_maven_download_url(
    group_id: &str,
    artifact_id: &str,
    version: &str,
    classifier: Option<&str>,
    packaging: Option<&str>,
) -> String {
    const BASE_URL: &str = "https://repo1.maven.org/maven2";
    let group_path = group_id.replace('.', "/");
    let extension = normalize_maven_packaging(packaging)
        .filter(|value| *value != "pom")
        .unwrap_or("jar");
    let classifier_suffix = classifier
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("-{value}"))
        .unwrap_or_default();

    format!(
        "{}/{}/{}/{}/{}-{}{}.{}",
        BASE_URL,
        group_path,
        artifact_id,
        version,
        artifact_id,
        version,
        classifier_suffix,
        extension
    )
}

pub(super) fn build_maven_source_package(namespace: &str, name: &str, version: &str) -> String {
    build_maven_purl(namespace, name, Some(version), Some("sources"), None)
}

pub(super) fn dependency_extra_data(
    dependency: &MavenDependencyData,
) -> Option<HashMap<String, serde_json::Value>> {
    let mut extra_data = HashMap::new();

    if let Some(classifier) = dependency
        .classifier
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        extra_data.insert(
            "classifier".to_string(),
            serde_json::Value::String(classifier.clone()),
        );
    }
    if let Some(type_) = dependency
        .type_
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        extra_data.insert("type".to_string(), serde_json::Value::String(type_.clone()));
    }
    if let Some(system_path) = dependency
        .system_path
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        extra_data.insert(
            "system_path".to_string(),
            serde_json::Value::String(system_path.clone()),
        );
    }
    if let Some(message) = dependency
        .message
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        extra_data.insert(
            "message".to_string(),
            serde_json::Value::String(message.clone()),
        );
    }

    (!extra_data.is_empty()).then_some(extra_data)
}

pub(super) fn dependency_management_entry_to_value(
    dependency: &MavenDependencyData,
) -> serde_json::Map<String, serde_json::Value> {
    let mut dep_obj = serde_json::Map::new();

    if let Some(group_id) = dependency.group_id.as_ref() {
        dep_obj.insert(
            "groupId".to_string(),
            serde_json::Value::String(group_id.clone()),
        );
    }
    if let Some(artifact_id) = dependency.artifact_id.as_ref() {
        dep_obj.insert(
            "artifactId".to_string(),
            serde_json::Value::String(artifact_id.clone()),
        );
    }
    if let Some(version) = dependency.version.as_ref() {
        dep_obj.insert(
            "version".to_string(),
            serde_json::Value::String(version.clone()),
        );
    }
    if let Some(scope) = dependency.scope.as_ref() {
        dep_obj.insert(
            "scope".to_string(),
            serde_json::Value::String(scope.clone()),
        );
    }
    if let Some(type_) = dependency.type_.as_ref() {
        dep_obj.insert("type".to_string(), serde_json::Value::String(type_.clone()));
    }
    if let Some(classifier) = dependency.classifier.as_ref() {
        dep_obj.insert(
            "classifier".to_string(),
            serde_json::Value::String(classifier.clone()),
        );
    }
    if let Some(optional) = dependency.optional.as_deref() {
        dep_obj.insert(
            "optional".to_string(),
            serde_json::Value::Bool(parse_maven_bool(Some(optional))),
        );
    }
    if let Some(message) = dependency.message.as_ref() {
        dep_obj.insert(
            "message".to_string(),
            serde_json::Value::String(message.clone()),
        );
    }

    dep_obj
}

pub(super) fn maven_dependency_to_dependency(
    dependency_data: &MavenDependencyData,
    fallback_scope: Option<&str>,
    force_non_runtime: bool,
) -> Option<Dependency> {
    let group_id = dependency_data.group_id.as_ref()?;
    let artifact_id = dependency_data.artifact_id.as_ref()?;
    let version = dependency_data.version.clone();
    let scope = dependency_data
        .scope
        .clone()
        .or_else(|| fallback_scope.map(str::to_string));
    let explicit_optional = parse_maven_bool(dependency_data.optional.as_deref());

    let (is_runtime, is_optional) = if force_non_runtime {
        (Some(false), Some(explicit_optional))
    } else {
        match scope.as_deref() {
            Some("test") | Some("provided") => (Some(false), Some(true)),
            Some(_) => (Some(true), Some(explicit_optional)),
            None => (None, Some(explicit_optional)),
        }
    };

    Some(Dependency {
        purl: Some(build_maven_purl(
            group_id,
            artifact_id,
            version.as_deref(),
            dependency_data.classifier.as_deref(),
            dependency_data.type_.as_deref(),
        )),
        extracted_requirement: version.clone(),
        scope,
        is_runtime,
        is_optional,
        is_pinned: version.as_deref().map(is_maven_version_pinned),
        is_direct: Some(true),
        resolved_package: None,
        extra_data: dependency_extra_data(dependency_data),
    })
}

pub(super) fn is_maven_version_pinned(version_str: &str) -> bool {
    let trimmed = version_str.trim();

    if trimmed.is_empty() {
        return false;
    }

    if trimmed.contains('[')
        || trimmed.contains(']')
        || trimmed.contains('(')
        || trimmed.contains(')')
    {
        return false;
    }

    if trimmed.eq_ignore_ascii_case("LATEST") || trimmed.eq_ignore_ascii_case("RELEASE") {
        return false;
    }

    true
}

pub(super) fn build_maven_url(
    group_id: &Option<String>,
    artifact_id: &Option<String>,
    version: &Option<String>,
    filename: Option<&str>,
) -> Option<String> {
    const BASE_URL: &str = "https://repo1.maven.org/maven2";

    let group_id = group_id.as_ref()?;
    let artifact_id = artifact_id.as_ref()?;

    let group_path = group_id.replace('.', "/");
    let filename_str = filename.unwrap_or("");

    let url = if let Some(ver) = version {
        format!(
            "{}/{}/{}/{}/{}",
            BASE_URL, group_path, artifact_id, ver, filename_str
        )
    } else {
        format!(
            "{}/{}/{}/{}",
            BASE_URL, group_path, artifact_id, filename_str
        )
    };

    Some(url)
}
