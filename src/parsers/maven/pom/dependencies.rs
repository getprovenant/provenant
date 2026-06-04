// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use crate::models::Dependency;
use crate::parsers::maven::coordinates::build_maven_purl;
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

impl MavenDependencyData {
    pub(super) fn has_management_coordinates(&self) -> bool {
        self.group_id.is_some() || self.artifact_id.is_some() || self.version.is_some()
    }
}

pub(super) fn parse_maven_bool(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
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
