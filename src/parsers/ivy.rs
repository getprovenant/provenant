// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Parser for Apache Ant/Ivy `ivy.xml` dependency manifests.
//!
//! Recovers the module identity from `<info organisation/module/revision>` and
//! direct dependencies from `<dependencies><dependency org/name/rev/conf>`.
//! Parsing is static and bounded: the file size is capped before reading and the
//! XML event loop is bounded by `MAX_ITERATION_COUNT`.

use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::PackageParser;
use super::metadata::ParserMetadata;
use crate::models::{DatasourceId, Dependency, PackageData, PackageType};
use crate::parser_warn as warn;
use crate::parsers::utils::{MAX_ITERATION_COUNT, read_file_to_string, truncate_field};

pub struct IvyXmlParser;

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

fn default_ivy_package_data() -> PackageData {
    PackageData {
        package_type: Some(PackageType::Ivy),
        primary_language: Some("Java".to_string()),
        datasource_id: Some(DatasourceId::AntIvyXml),
        ..Default::default()
    }
}

fn attr_value(element: &BytesStart, key: &[u8]) -> Option<String> {
    element
        .attributes()
        .filter_map(|attr| attr.ok())
        .find(|attr| attr.key.as_ref() == key)
        .and_then(|attr| String::from_utf8(attr.value.to_vec()).ok())
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
