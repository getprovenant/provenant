// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Parser for Apache Ant/Ivy `ivy.xml` dependency manifests.
//!
//! Recovers the module identity from `<info organisation/module/revision>` and
//! direct dependencies from `<dependencies><dependency org/name/rev/conf>`.
//! Parsing is static and bounded: the file size is capped before reading and the
//! XML event loop is bounded by `MAX_ITERATION_COUNT`.

use std::collections::HashMap;
use std::path::Path;

use packageurl::PackageUrl;
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value as JsonValue;

use super::PackageParser;
use super::metadata::ParserMetadata;
use crate::models::{DatasourceId, Dependency, PackageData, PackageType, ResolvedPackage};
use crate::parser_warn as warn;
use crate::parsers::utils::{
    CappedIterExt, MAX_ITERATION_COUNT, read_file_to_string, truncate_field,
};

pub struct IvyXmlParser;
pub struct IvyDependenciesPropertiesParser;

impl PackageParser for IvyXmlParser {
    const PACKAGE_TYPE: PackageType = PackageType::Ivy;

    fn is_match(path: &Path) -> bool {
        path.to_str().is_some_and(|p| p.ends_with("/ivy.xml"))
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(content) => content,
            Err(e) => {
                warn!("Failed to read ivy.xml at {:?}: {}", path, e);
                return vec![default_ivy_package_data()];
            }
        };

        vec![interpret_ivy_xml(&content, path)]
    }

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "Apache Ant/Ivy dependency manifest",
            file_patterns: &["**/ivy.xml"],
            package_type: "ivy",
            primary_language: "Java",
            documentation_url: Some(
                "https://ant.apache.org/ivy/history/latest-milestone/ivyfile.html",
            ),
        }]
    }
}

impl PackageParser for IvyDependenciesPropertiesParser {
    const PACKAGE_TYPE: PackageType = PackageType::Maven;

    fn is_match(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_ivy_dependencies_properties_filename)
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(content) => content,
            Err(e) => {
                warn!(
                    "Failed to read dependencies.properties at {:?}: {}",
                    path, e
                );
                return vec![default_ivy_dependencies_properties_data(Vec::new())];
            }
        };

        let scope = dependency_scope_from_filename(path);
        let dependencies = extract_ivy_dependencies_properties(&content, scope.as_deref());

        vec![default_ivy_dependencies_properties_data(dependencies)]
    }

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "Ivy-style dependencies.properties Maven dependency list",
            file_patterns: &["**/dependencies.properties", "**/*-dependencies.properties"],
            package_type: "maven",
            primary_language: "Java",
            documentation_url: Some(
                "https://github.com/aboutcode-org/scancode-toolkit/issues/3468",
            ),
        }]
    }
}

fn default_ivy_package_data() -> PackageData {
    PackageData {
        package_type: Some(PackageType::Ivy),
        primary_language: Some("Java".to_string()),
        datasource_id: Some(DatasourceId::AntIvyXml),
        ..Default::default()
    }
}

fn default_ivy_dependencies_properties_data(dependencies: Vec<Dependency>) -> PackageData {
    PackageData {
        package_type: Some(IvyDependenciesPropertiesParser::PACKAGE_TYPE),
        primary_language: Some("Java".to_string()),
        dependencies,
        datasource_id: Some(DatasourceId::AntIvyDependenciesProperties),
        ..Default::default()
    }
}

fn attr_value(element: &BytesStart, key: &[u8]) -> Option<String> {
    element
        .attributes()
        .filter_map(|attr| attr.ok())
        .find(|attr| attr.key.as_ref() == key)
        // Decode XML entities (e.g. `&amp;`) so namespaces/purls carry the real value.
        .and_then(|attr| {
            attr.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.into_owned())
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn interpret_ivy_xml(content: &str, path: &Path) -> PackageData {
    let mut package_data = default_ivy_package_data();

    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut namespace: Option<String> = None;
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut dependencies: Vec<Dependency> = Vec::new();

    let mut buf = Vec::new();
    let mut in_dependencies = false;
    let mut iteration_count: usize = 0;

    loop {
        iteration_count += 1;
        if iteration_count > MAX_ITERATION_COUNT {
            warn!(
                "Iteration limit exceeded in ivy.xml at {:?}; stopping at {} items",
                path, MAX_ITERATION_COUNT
            );
            break;
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let tag = e.name();
                match tag.as_ref() {
                    b"info" => {
                        namespace = attr_value(&e, b"organisation");
                        name = attr_value(&e, b"module");
                        version = attr_value(&e, b"revision");
                    }
                    b"dependencies" => in_dependencies = true,
                    b"dependency" if in_dependencies => {
                        if let Some(dep) = parse_ivy_dependency(&e) {
                            dependencies.push(dep);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"dependencies" => {
                in_dependencies = false;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing ivy.xml at {:?}: {}", path, e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    package_data.namespace = namespace.map(truncate_field);
    package_data.name = name.map(truncate_field);
    package_data.version = version.map(truncate_field);
    package_data.dependencies = dependencies;

    if let (Some(namespace), Some(name)) = (&package_data.namespace, &package_data.name) {
        package_data.purl = Some(truncate_field(build_ivy_purl(
            namespace,
            name,
            package_data.version.as_deref(),
        )));
    }

    package_data
}

fn is_ivy_dependencies_properties_filename(name: &str) -> bool {
    name == "dependencies.properties" || name.ends_with("-dependencies.properties")
}

fn dependency_scope_from_filename(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix("-dependencies.properties"))
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(|scope| truncate_field(scope.to_string()))
}

fn extract_ivy_dependencies_properties(content: &str, scope: Option<&str>) -> Vec<Dependency> {
    parse_properties_entries(content)
        .into_iter()
        .filter_map(|(key, value)| parse_ivy_dependencies_entry(&key, &value, scope))
        .collect()
}

fn parse_properties_entries(content: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut current = String::new();

    for raw_line in content
        .lines()
        .capped("Ivy dependencies.properties raw lines")
    {
        let line = raw_line.trim_end_matches('\r');
        let trimmed_end = line.trim_end();
        let is_continuation = trimmed_end.ends_with('\\');
        let line_without_slash = if is_continuation {
            trimmed_end.trim_end_matches('\\')
        } else {
            line
        };

        if !current.is_empty() {
            current.push_str(line_without_slash.trim_start());
        } else {
            current.push_str(line_without_slash);
        }

        if is_continuation {
            continue;
        }

        let logical_line = current.trim();
        if !logical_line.is_empty()
            && !logical_line.starts_with('#')
            && !logical_line.starts_with('!')
            && let Some((key, value)) = logical_line.split_once('=')
        {
            let key = key.trim();
            let value = value.trim();
            if !key.is_empty() && !value.is_empty() {
                entries.push((key.to_string(), value.to_string()));
            }
        }

        current.clear();
    }

    entries
}

fn parse_ivy_dependencies_entry(key: &str, value: &str, scope: Option<&str>) -> Option<Dependency> {
    if let Some((group, artifact, version)) = parse_maven_gav(value) {
        return Some(build_maven_dependency(
            group,
            artifact,
            version,
            Some(key),
            "value_gav",
            scope,
        ));
    }

    let (group, artifact) = parse_maven_ga(key)?;
    if value.contains(':') {
        return None;
    }

    Some(build_maven_dependency(
        group,
        artifact,
        value,
        Some(key),
        "key_ga",
        scope,
    ))
}

fn parse_maven_gav(value: &str) -> Option<(&str, &str, &str)> {
    let mut parts = value.split(':').map(str::trim);
    let group = parts.next()?.trim();
    let artifact = parts.next()?.trim();
    let version = parts.next()?.trim();
    if parts.next().is_some() || group.is_empty() || artifact.is_empty() || version.is_empty() {
        return None;
    }

    Some((group, artifact, version))
}

fn parse_maven_ga(value: &str) -> Option<(&str, &str)> {
    let mut parts = value.split(':').map(str::trim);
    let group = parts.next()?.trim();
    let artifact = parts.next()?.trim();
    if parts.next().is_some() || group.is_empty() || artifact.is_empty() {
        return None;
    }

    Some((group, artifact))
}

fn build_maven_dependency(
    group: &str,
    artifact: &str,
    version: &str,
    property_name: Option<&str>,
    coordinate_format: &str,
    scope: Option<&str>,
) -> Dependency {
    let group = truncate_field(group.to_string());
    let artifact = truncate_field(artifact.to_string());
    let version = truncate_field(version.to_string());

    let purl = PackageUrl::new("maven", &artifact)
        .ok()
        .and_then(|mut purl| {
            purl.with_namespace(&group).ok()?;
            purl.with_version(&version).ok()?;
            Some(truncate_field(purl.to_string()))
        });

    let mut extra_data = HashMap::from([
        (
            "coordinate_format".to_string(),
            JsonValue::String(coordinate_format.to_string()),
        ),
        ("group".to_string(), JsonValue::String(group.clone())),
        ("artifact".to_string(), JsonValue::String(artifact.clone())),
    ]);
    if let Some(property_name) = property_name.filter(|name| !name.trim().is_empty()) {
        extra_data.insert(
            "property_name".to_string(),
            JsonValue::String(truncate_field(property_name.to_string())),
        );
    }

    let resolved_package = ResolvedPackage {
        primary_language: Some("Java".to_string()),
        datasource_id: Some(DatasourceId::AntIvyDependenciesProperties),
        purl: purl.clone(),
        ..ResolvedPackage::new(PackageType::Maven, group, artifact, version.clone())
    };

    Dependency {
        purl,
        extracted_requirement: Some(version),
        scope: scope.map(|scope| truncate_field(scope.to_string())),
        is_runtime: None,
        is_optional: None,
        is_pinned: Some(true),
        is_direct: Some(true),
        resolved_package: Some(Box::new(resolved_package)),
        extra_data: Some(extra_data),
    }
}

fn parse_ivy_dependency(element: &BytesStart) -> Option<Dependency> {
    let org = attr_value(element, b"org");
    let name = attr_value(element, b"name")?;
    let rev = attr_value(element, b"rev");
    let conf = attr_value(element, b"conf");

    let purl = match &org {
        Some(org) => build_ivy_purl(org, &name, None),
        None => format!("pkg:ivy/{}", name),
    };

    Some(Dependency {
        purl: Some(truncate_field(purl)),
        extracted_requirement: rev.map(truncate_field),
        scope: conf.map(truncate_field),
        is_runtime: None,
        is_optional: None,
        is_pinned: None,
        is_direct: Some(true),
        resolved_package: None,
        extra_data: None,
    })
}

fn build_ivy_purl(organisation: &str, module: &str, revision: Option<&str>) -> String {
    let mut purl = format!("pkg:ivy/{}/{}", organisation, module);
    if let Some(revision) = revision.filter(|value| !value.trim().is_empty()) {
        purl.push('@');
        purl.push_str(revision);
    }
    purl
}
