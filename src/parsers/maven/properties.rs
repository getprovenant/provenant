// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::default_package_data;
use crate::models::{DatasourceId, PackageData, PackageType};
use crate::parser_warn as warn;
use crate::parsers::utils::{read_file_to_string, truncate_field};
use std::path::Path;

/// Parse pom.properties file (Java properties format)
pub(super) fn parse_pom_properties(path: &Path) -> PackageData {
    let content = match read_file_to_string(path, None).map_err(|e| e.to_string()) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to read pom.properties at {:?}: {}", path, e);
            return PackageData {
                package_type: Some(PackageType::Maven),
                primary_language: Some("Java".to_string()),
                datasource_id: Some(DatasourceId::MavenPomProperties),
                ..Default::default()
            };
        }
    };

    let mut package_data = default_package_data(DatasourceId::MavenPomProperties);
    package_data.package_type = Some(PackageType::Maven);
    package_data.primary_language = Some("Java".to_string());
    package_data.datasource_id = Some(DatasourceId::MavenPomProperties);

    let MavenPomProperties {
        group_id,
        artifact_id,
        version,
    } = interpret_pom_properties(&content);

    package_data.namespace = group_id.map(truncate_field);
    package_data.name = artifact_id.map(truncate_field);
    package_data.version = version.map(truncate_field);

    if let (Some(group_id), Some(artifact_id), Some(version)) = (
        &package_data.namespace,
        &package_data.name,
        &package_data.version,
    ) {
        package_data.purl = Some(truncate_field(format!(
            "pkg:maven/{}/{}@{}",
            group_id, artifact_id, version
        )));
    }

    package_data
}

/// Maven coordinates recovered from a `pom.properties` body.
pub(crate) struct MavenPomProperties {
    pub(crate) group_id: Option<String>,
    pub(crate) artifact_id: Option<String>,
    pub(crate) version: Option<String>,
}

/// Interpret `pom.properties` text into raw Maven coordinates.
///
/// Shared by the file-backed [`parse_pom_properties`] and by JVM-archive
/// introspection, which reads the body from a bounded ZIP entry.
pub(crate) fn interpret_pom_properties(content: &str) -> MavenPomProperties {
    let mut group_id: Option<String> = None;
    let mut artifact_id: Option<String> = None;
    let mut version: Option<String> = None;

    let mut continuation = String::new();

    for line in content.lines() {
        let current_line = if continuation.is_empty() {
            line.to_string()
        } else {
            format!("{}{}", continuation, line)
        };
        continuation.clear();

        if current_line.ends_with('\\') {
            continuation = current_line[..current_line.len() - 1].to_string();
            continue;
        }

        let trimmed = current_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        if let Some(eq_pos) = current_line.find('=') {
            let key = current_line[..eq_pos].trim();
            let value = current_line[eq_pos + 1..].trim();

            match key {
                "groupId" => group_id = Some(value.to_string()),
                "artifactId" => artifact_id = Some(value.to_string()),
                "version" => version = Some(value.to_string()),
                _ => {}
            }
        }
    }

    MavenPomProperties {
        group_id,
        artifact_id,
        version,
    }
}
