// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::coordinates::{build_maven_purl, infer_meta_inf_maven_coordinates};
use super::default_package_data;
use crate::models::{DatasourceId, Dependency, PackageData, PackageType, Party, PartyType};
use crate::parser_warn as warn;
use crate::parsers::utils::{MAX_ITERATION_COUNT, read_file_to_string, truncate_field};
use std::collections::HashMap;
use std::path::Path;

/// Parse MANIFEST.MF file (JAR manifest format)
///
/// Detects and handles both regular JAR manifests and OSGi bundle manifests.
/// If Bundle-SymbolicName is present, treats the manifest as an OSGi bundle
/// and extracts OSGi-specific metadata including Import-Package and Require-Bundle
/// dependencies.
pub(super) fn parse_manifest_mf(path: &Path) -> PackageData {
    let content = match read_file_to_string(path, None).map_err(|e| e.to_string()) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to read MANIFEST.MF at {:?}: {}", path, e);
            return default_package_data(DatasourceId::JavaJarManifest);
        }
    };

    let mut package_data = default_package_data(DatasourceId::JavaJarManifest);

    let mut headers: Vec<(String, String)> = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for line in content.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            current_value.push_str(line.trim());
        } else if let Some(colon_pos) = line.find(':') {
            if let Some(key) = current_key.take() {
                headers.push((key, current_value.trim().to_string()));
                current_value.clear();
            }

            let key = line[..colon_pos].trim().to_string();
            let value = line[colon_pos + 1..].trim().to_string();
            current_key = Some(key);
            current_value = value;
        }
    }

    if let Some(key) = current_key {
        headers.push((key, current_value.trim().to_string()));
    }

    let headers_map: HashMap<String, String> = headers.iter().cloned().collect();
    let bundle_symbolic_name = headers_map.get("Bundle-SymbolicName").map(|bsn| {
        if let Some(semicolon_pos) = bsn.find(';') {
            bsn[..semicolon_pos].trim().to_string()
        } else {
            bsn.clone()
        }
    });
    let is_osgi = bundle_symbolic_name.is_some();
    let bundle_version = headers_map.get("Bundle-Version").cloned();
    let bundle_name = headers_map.get("Bundle-Name").cloned();
    let implementation_title = headers_map.get("Implementation-Title").cloned();
    let implementation_version = headers_map.get("Implementation-Version").cloned();
    let implementation_vendor_id = headers_map.get("Implementation-Vendor-Id").cloned();
    let inferred_maven_coordinates = infer_meta_inf_maven_coordinates(path);
    let preferred_maven_identity = if let (Some(namespace), Some(name), Some(version)) = (
        implementation_vendor_id.clone(),
        implementation_title.clone(),
        implementation_version.clone(),
    ) {
        Some((namespace, name, version))
    } else if let (Some(coords), Some(version)) = (
        inferred_maven_coordinates.as_ref(),
        implementation_version.clone(),
    ) {
        Some((coords.group_id.clone(), coords.artifact_id.clone(), version))
    } else {
        None
    };

    if is_osgi {
        if let Some((preferred_maven_namespace, preferred_maven_name, preferred_maven_version)) =
            preferred_maven_identity
        {
            package_data.package_type = Some(PackageType::Maven);
            package_data.datasource_id = Some(DatasourceId::JavaJarManifest);
            package_data.namespace = Some(preferred_maven_namespace.clone());
            package_data.name = Some(preferred_maven_name.clone());
            package_data.version = Some(preferred_maven_version.clone());
            package_data.purl = Some(build_maven_purl(
                &preferred_maven_namespace,
                &preferred_maven_name,
                Some(&preferred_maven_version),
                None,
                None,
            ));

            let mut extra_data = package_data.extra_data.take().unwrap_or_default();
            if let Some(bundle_symbolic_name) = &bundle_symbolic_name {
                extra_data.insert(
                    "osgi_bundle_symbolic_name".to_string(),
                    serde_json::Value::String(bundle_symbolic_name.clone()),
                );
            }
            if let Some(bundle_name) = &bundle_name {
                extra_data.insert(
                    "osgi_bundle_name".to_string(),
                    serde_json::Value::String(bundle_name.clone()),
                );
            }
            if let Some(bundle_version) = &bundle_version {
                extra_data.insert(
                    "osgi_bundle_version".to_string(),
                    serde_json::Value::String(bundle_version.clone()),
                );
            }
            package_data.extra_data = (!extra_data.is_empty()).then_some(extra_data);
        } else {
            package_data.package_type = Some(PackageType::Osgi);
            package_data.datasource_id = Some(DatasourceId::JavaOsgiManifest);

            if let Some(bsn) = &bundle_symbolic_name {
                package_data.name = Some(bsn.clone());
            }

            package_data.version = bundle_version.clone();

            if let (Some(name), Some(version)) = (&package_data.name, &package_data.version) {
                package_data.purl = Some(format!("pkg:osgi/{}@{}", name, version));
            }
        }

        if let Some(desc) = headers_map.get("Bundle-Description") {
            package_data.description = Some(desc.clone());
        } else if let Some(name) = &bundle_name {
            package_data.description = Some(name.clone());
        }

        if let Some(vendor) = headers_map
            .get("Bundle-Vendor")
            .or_else(|| headers_map.get("Implementation-Vendor"))
        {
            package_data.parties.push(Party {
                r#type: Some(PartyType::Organization),
                role: Some("vendor".to_string()),
                name: Some(vendor.clone()),
                email: None,
                url: None,
                organization: None,
                organization_url: None,
                timezone: None,
            });
        }

        package_data.homepage_url = headers_map.get("Bundle-DocURL").cloned();
        package_data.extracted_license_statement = headers_map.get("Bundle-License").cloned();

        if let Some(import_pkg) = headers_map.get("Import-Package") {
            let deps = parse_osgi_package_list(import_pkg, "import");
            package_data.dependencies.extend(deps);
        }

        if let Some(require_bundle) = headers_map.get("Require-Bundle") {
            let deps = parse_osgi_bundle_list(require_bundle, "require-bundle");
            package_data.dependencies.extend(deps);
        }

        if let Some(export_pkg) = headers_map.get("Export-Package") {
            let mut extra_data = package_data.extra_data.take().unwrap_or_default();
            extra_data.insert(
                "export_packages".to_string(),
                serde_json::Value::String(export_pkg.clone()),
            );
            package_data.extra_data = Some(extra_data);
        }
    } else {
        package_data.package_type = Some(PackageType::Maven);
        package_data.datasource_id = Some(DatasourceId::JavaJarManifest);

        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut vendor: Option<String> = None;

        for (key, value) in &headers {
            match key.as_str() {
                "Bundle-Name" if name.is_none() => name = Some(value.clone()),
                "Implementation-Title" if name.is_none() => name = Some(value.clone()),
                "Bundle-Version" if version.is_none() => version = Some(value.clone()),
                "Implementation-Version" if version.is_none() => version = Some(value.clone()),
                "Implementation-Vendor" | "Bundle-Vendor" if vendor.is_none() => {
                    vendor = Some(value.clone())
                }
                _ => {}
            }
        }

        package_data.name = name;
        package_data.version = version;

        if let Some(vendor_name) = vendor {
            package_data.parties.push(Party {
                r#type: Some(PartyType::Organization),
                role: Some("vendor".to_string()),
                name: Some(vendor_name),
                email: None,
                url: None,
                organization: None,
                organization_url: None,
                timezone: None,
            });
        }

        if let Some(coords) = infer_meta_inf_maven_coordinates(path) {
            package_data.namespace = Some(coords.group_id);
        }

        if let (Some(group_id), Some(artifact_id), Some(version)) = (
            &package_data.namespace,
            &package_data.name,
            &package_data.version,
        ) {
            package_data.purl = Some(build_maven_purl(
                group_id,
                artifact_id,
                Some(version),
                None,
                None,
            ));
        } else if package_data.name.is_none() && package_data.version.is_none() {
            package_data.package_type = Some(PackageType::Jar);
        }
    }

    package_data.name = package_data.name.map(truncate_field);
    package_data.version = package_data.version.map(truncate_field);
    package_data.namespace = package_data.namespace.map(truncate_field);
    package_data.core.description = package_data.core.description.map(truncate_field);
    package_data.core.homepage_url = package_data.core.homepage_url.map(truncate_field);
    package_data.core.extracted_license_statement = package_data
        .core
        .extracted_license_statement
        .map(truncate_field);
    package_data.core.purl = package_data.core.purl.map(truncate_field);
    for dep in &mut package_data.dependencies {
        dep.purl = dep.purl.take().map(truncate_field);
        dep.extracted_requirement = dep.extracted_requirement.take().map(truncate_field);
    }

    package_data
}

/// Parse OSGi Import-Package header into dependencies.
pub(super) fn parse_osgi_package_list(package_list: &str, scope: &str) -> Vec<Dependency> {
    let mut dependencies = Vec::new();

    for package_entry in split_osgi_list(package_list)
        .into_iter()
        .take(MAX_ITERATION_COUNT)
    {
        let package_entry = package_entry.trim();
        if package_entry.is_empty() {
            continue;
        }

        let package_name = if let Some(semicolon_pos) = package_entry.find(';') {
            package_entry[..semicolon_pos].trim()
        } else {
            package_entry
        };

        if package_name.is_empty() {
            continue;
        }

        let version_requirement = extract_osgi_version(package_entry);
        let is_optional = package_entry.contains("resolution:=optional");

        dependencies.push(Dependency {
            purl: Some(format!("pkg:osgi/{}", package_name)),
            extracted_requirement: version_requirement,
            scope: Some(scope.to_string()),
            is_runtime: Some(true),
            is_optional: Some(is_optional),
            is_pinned: None,
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        });
    }

    dependencies
}

/// Parse OSGi Require-Bundle header into dependencies.
pub(super) fn parse_osgi_bundle_list(bundle_list: &str, scope: &str) -> Vec<Dependency> {
    let mut dependencies = Vec::new();

    for bundle_entry in split_osgi_list(bundle_list)
        .into_iter()
        .take(MAX_ITERATION_COUNT)
    {
        let bundle_entry = bundle_entry.trim();
        if bundle_entry.is_empty() {
            continue;
        }

        let bundle_name = if let Some(semicolon_pos) = bundle_entry.find(';') {
            bundle_entry[..semicolon_pos].trim()
        } else {
            bundle_entry
        };

        if bundle_name.is_empty() {
            continue;
        }

        let version_requirement = extract_osgi_bundle_version(bundle_entry);
        let is_optional = bundle_entry.contains("resolution:=optional");

        dependencies.push(Dependency {
            purl: Some(format!("pkg:osgi/{}", bundle_name)),
            extracted_requirement: version_requirement,
            scope: Some(scope.to_string()),
            is_runtime: Some(!is_optional),
            is_optional: Some(is_optional),
            is_pinned: None,
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        });
    }

    dependencies
}

/// Split OSGi comma-separated list, respecting quoted strings.
pub(super) fn split_osgi_list(list: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in list.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ',' if !in_quotes => {
                if !current.trim().is_empty() {
                    result.push(current.trim().to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }

    result
}

fn extract_osgi_directive(entry: &str, directive: &str) -> Option<String> {
    let needle = format!("{}=", directive);
    let version_pos = entry.find(&needle)?;
    let after_value = &entry[version_pos + needle.len()..];

    if let Some(stripped) = after_value.strip_prefix('"') {
        stripped.find('"').map(|end| stripped[..end].to_string())
    } else {
        let end = after_value.find(';').unwrap_or(after_value.len());
        Some(after_value[..end].trim().to_string())
    }
}

pub(super) fn extract_osgi_version(entry: &str) -> Option<String> {
    extract_osgi_directive(entry, "version")
}

pub(super) fn extract_osgi_bundle_version(entry: &str) -> Option<String> {
    extract_osgi_directive(entry, "bundle-version")
}
