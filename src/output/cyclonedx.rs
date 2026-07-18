// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Write};

use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::output_schema::{
    Output, OutputPackage as Package, OutputTopLevelDependency as TopLevelDependency,
};
use crate::utils::time::{convert_header_timestamp_to_iso_utc, fallback_iso_utc_timestamp};

use super::shared::{io_other, xml_escape};

pub(crate) fn write_cyclonedx_json(output: &Output, writer: &mut dyn Write) -> io::Result<()> {
    let bom = build_cyclonedx_json(output);
    serde_json::to_writer_pretty(&mut *writer, &bom).map_err(io_other)?;
    writer.write_all(b"\n")
}

pub(crate) fn write_cyclonedx_xml(output: &Output, writer: &mut dyn Write) -> io::Result<()> {
    let serial = format!("urn:uuid:{}", Uuid::new_v4());
    let timestamp = output
        .headers
        .first()
        .and_then(|h| convert_header_timestamp_to_iso_utc(&h.end_timestamp))
        .unwrap_or_else(|| fallback_iso_utc_timestamp().to_string());
    let component_refs = component_bom_refs(&output.packages);
    let dependency_graph = build_dependency_graph(output, &component_refs);

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<bom xmlns=\"http://cyclonedx.org/schema/bom/1.3\" serialNumber=\"{}\" version=\"1\">\n",
        xml_escape(&serial)
    ));
    xml.push_str("  <metadata>\n");
    xml.push_str("    <timestamp>");
    xml.push_str(&xml_escape(&timestamp));
    xml.push_str("</timestamp>\n");
    xml.push_str("    <tools>\n");
    xml.push_str("      <tool><vendor>Provenant</vendor><name>Provenant</name><version>");
    xml.push_str(crate::version::BUILD_VERSION);
    xml.push_str("</version></tool>\n");
    xml.push_str("    </tools>\n");
    xml.push_str("  </metadata>\n");

    xml.push_str("  <components>\n");
    for (idx, pkg) in output.packages.iter().enumerate() {
        let name = pkg.name.as_deref().unwrap_or("unknown");
        let version = pkg.version.as_deref().unwrap_or("unknown");
        let bom_ref = &component_refs[idx];
        xml.push_str(&format!(
            "    <component type=\"library\" bom-ref=\"{}\">\n",
            xml_escape(bom_ref)
        ));
        // CycloneDX 1.3 XSD requires this element order within <component>:
        // author precedes name/version, which precede description, which
        // precedes scope.
        if let Some(author) = package_author(pkg) {
            xml.push_str("      <author>");
            xml.push_str(&xml_escape(&author));
            xml.push_str("</author>\n");
        }
        xml.push_str("      <name>");
        xml.push_str(&xml_escape(name));
        xml.push_str("</name>\n");
        xml.push_str("      <version>");
        xml.push_str(&xml_escape(version));
        xml.push_str("</version>\n");
        if let Some(description) = &pkg.description {
            xml.push_str("      <description>");
            xml.push_str(&xml_escape(description));
            xml.push_str("</description>\n");
        }
        xml.push_str("      <scope>required</scope>\n");
        let hashes = component_hashes(pkg);
        if !hashes.is_empty() {
            xml.push_str("      <hashes>\n");
            for (alg, content) in hashes {
                xml.push_str("        <hash alg=\"");
                xml.push_str(alg);
                xml.push_str("\">");
                xml.push_str(&xml_escape(&content));
                xml.push_str("</hash>\n");
            }
            xml.push_str("      </hashes>\n");
        }
        if let Some(license_expression) = cyclonedx_license_expression(pkg) {
            xml.push_str("      <licenses><expression>");
            xml.push_str(&xml_escape(&license_expression));
            xml.push_str("</expression></licenses>\n");
        }
        // `purl` must follow `licenses`/`copyright`/`cpe` per the CycloneDX 1.3 XSD.
        if let Some(purl) = &pkg.purl {
            xml.push_str("      <purl>");
            xml.push_str(&xml_escape(purl));
            xml.push_str("</purl>\n");
        }
        let external_refs = component_external_references(pkg);
        if !external_refs.is_empty() {
            xml.push_str("      <externalReferences>\n");
            for (ref_type, url) in external_refs {
                xml.push_str("        <reference type=\"");
                xml.push_str(ref_type);
                xml.push_str("\"><url>");
                xml.push_str(&xml_escape(&url));
                xml.push_str("</url></reference>\n");
            }
            xml.push_str("      </externalReferences>\n");
        }
        xml.push_str("    </component>\n");
    }
    xml.push_str("  </components>\n");

    if !dependency_graph.is_empty() {
        xml.push_str("  <dependencies>\n");
        for (dep_ref, depends_on) in &dependency_graph {
            xml.push_str(&format!(
                "    <dependency ref=\"{}\">\n",
                xml_escape(dep_ref)
            ));
            for child in depends_on {
                xml.push_str(&format!(
                    "      <dependency ref=\"{}\"/>\n",
                    xml_escape(child)
                ));
            }
            xml.push_str("    </dependency>\n");
        }
        xml.push_str("  </dependencies>\n");
    }
    xml.push_str("</bom>\n");

    writer.write_all(xml.as_bytes())
}

fn build_cyclonedx_json(output: &Output) -> Value {
    let timestamp = output
        .headers
        .first()
        .and_then(|h| convert_header_timestamp_to_iso_utc(&h.end_timestamp))
        .unwrap_or_else(|| fallback_iso_utc_timestamp().to_string());
    let component_refs = component_bom_refs(&output.packages);

    let components = output
        .packages
        .iter()
        .enumerate()
        .map(|(idx, pkg)| {
            let mut obj = Map::new();
            obj.insert("type".to_string(), Value::String("library".to_string()));
            obj.insert(
                "bom-ref".to_string(),
                Value::String(component_refs[idx].clone()),
            );
            obj.insert(
                "name".to_string(),
                Value::String(pkg.name.clone().unwrap_or_else(|| "unknown".to_string())),
            );
            obj.insert(
                "version".to_string(),
                Value::String(pkg.version.clone().unwrap_or_else(|| "unknown".to_string())),
            );
            if let Some(description) = &pkg.description {
                obj.insert(
                    "description".to_string(),
                    Value::String(description.clone()),
                );
            }
            if let Some(author) = package_author(pkg) {
                obj.insert("author".to_string(), Value::String(author));
            }
            obj.insert("scope".to_string(), Value::String("required".to_string()));
            if let Some(purl) = &pkg.purl {
                obj.insert("purl".to_string(), Value::String(purl.clone()));
            }
            let hashes = component_hashes(pkg)
                .into_iter()
                .map(|(alg, content)| json!({"alg": alg, "content": content}))
                .collect::<Vec<_>>();
            if !hashes.is_empty() {
                obj.insert("hashes".to_string(), Value::Array(hashes));
            }
            if let Some(license_expression) = cyclonedx_license_expression(pkg) {
                obj.insert(
                    "licenses".to_string(),
                    Value::Array(vec![json!({ "expression": license_expression })]),
                );
            }
            let external_refs = component_external_references(pkg)
                .into_iter()
                .map(|(ref_type, url)| json!({"type": ref_type, "url": url}))
                .collect::<Vec<_>>();
            if !external_refs.is_empty() {
                obj.insert(
                    "externalReferences".to_string(),
                    Value::Array(external_refs),
                );
            }
            Value::Object(obj)
        })
        .collect::<Vec<_>>();

    let dependencies = build_dependency_graph(output, &component_refs)
        .into_iter()
        .map(|(reference, depends_on)| {
            json!({
                "ref": reference,
                "dependsOn": depends_on.into_iter().collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    if components.is_empty() && dependencies.is_empty() {
        json!({
            "bomFormat": "CycloneDX",
            "specVersion": "1.3",
            "version": 1,
            "components": [],
            "dependencies": [],
        })
    } else {
        json!({
            "bomFormat": "CycloneDX",
            "specVersion": "1.3",
            "serialNumber": format!("urn:uuid:{}", Uuid::new_v4()),
            "version": 1,
                "metadata": {
                    "timestamp": timestamp,
                    "tools": [
                        {
                            "name": "Provenant",
                            "version": crate::version::BUILD_VERSION
                        }
                    ]
                },
            "components": components,
            "dependencies": dependencies,
        })
    }
}

/// Assign a unique `bom-ref` per package. Prefer purl when it does not collide;
/// fall back to `package_uid` (always unique) or a synthetic index-based ref.
fn component_bom_refs(packages: &[Package]) -> Vec<String> {
    let mut purl_counts: HashMap<&str, usize> = HashMap::new();
    for pkg in packages {
        if let Some(purl) = pkg.purl.as_deref() {
            *purl_counts.entry(purl).or_default() += 1;
        }
    }

    packages
        .iter()
        .enumerate()
        .map(|(idx, pkg)| match pkg.purl.as_deref() {
            Some(purl) if purl_counts.get(purl).copied().unwrap_or(0) == 1 => purl.to_string(),
            Some(_) => pkg.package_uid.clone(),
            None if !pkg.package_uid.is_empty() => pkg.package_uid.clone(),
            None => format!("component-{}", idx + 1),
        })
        .collect()
}

/// Build a CycloneDX dependency graph: one unique entry per `ref`, with
/// `dependsOn` listing component `bom-ref`s (or dependency purls when the
/// target is not itself a component).
///
/// Edges are owner → dependency (`for_package_uid` → package bom-ref). Child
/// refs are resolved through [`resolve_depends_on_refs`] so a disambiguated
/// `package_uid` bom-ref is used whenever the dependency purl maps to
/// component(s). Self-edges are dropped. Components with no outgoing edges
/// still get an empty `dependsOn` entry so the graph is complete.
fn build_dependency_graph(
    output: &Output,
    component_refs: &[String],
) -> BTreeMap<String, BTreeSet<String>> {
    let mut uid_to_ref: HashMap<&str, &str> = HashMap::new();
    for (pkg, bom_ref) in output.packages.iter().zip(component_refs.iter()) {
        uid_to_ref.insert(pkg.package_uid.as_str(), bom_ref.as_str());
    }

    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    // Every component appears in the graph (CycloneDX recommends empty nodes for
    // leaves so the graph is explicit).
    for bom_ref in component_refs {
        graph.entry(bom_ref.clone()).or_default();
    }

    for dep in &output.dependencies {
        let Some(dep_purl) = dependency_edge_purl(dep) else {
            continue;
        };
        let child_refs = resolve_depends_on_refs(&dep_purl, &output.packages, component_refs);

        if let Some(owner_uid) = dep.for_package_uid.as_deref()
            && let Some(owner_ref) = uid_to_ref.get(owner_uid).copied()
        {
            for child_ref in child_refs {
                if owner_ref != child_ref {
                    graph
                        .entry(owner_ref.to_string())
                        .or_default()
                        .insert(child_ref);
                }
            }
            continue;
        }

        // Unowned / unknown-owner dependency: emit as a leaf node so its ref
        // still appears once (no duplicate refs across hoisted rows).
        for child_ref in child_refs {
            graph.entry(child_ref).or_default();
        }
    }

    graph
}

/// Map a dependency purl onto component `bom-ref`s.
///
/// When the purl uniquely identifies a component, returns that component's
/// bom-ref (purl or disambiguated `package_uid`). When several assembled
/// packages share the purl, returns every matching bom-ref so `dependsOn`
/// never points at a bare purl that no component owns. When no component
/// matches, returns the purl itself (external / unresolved dependency).
fn resolve_depends_on_refs(
    dep_purl: &str,
    packages: &[Package],
    component_refs: &[String],
) -> Vec<String> {
    let matching: Vec<String> = packages
        .iter()
        .zip(component_refs.iter())
        .filter(|(pkg, _)| pkg.purl.as_deref() == Some(dep_purl))
        .map(|(_, bom_ref)| bom_ref.clone())
        .collect();
    if matching.is_empty() {
        vec![dep_purl.to_string()]
    } else {
        matching
    }
}

fn dependency_edge_purl(dep: &TopLevelDependency) -> Option<String> {
    dep.purl
        .clone()
        .or_else(|| {
            dep.resolved_package
                .as_ref()
                .and_then(|resolved| resolved.purl.clone())
        })
        .filter(|purl| !purl.is_empty())
}

fn cyclonedx_license_expression(pkg: &Package) -> Option<String> {
    pkg.declared_license_expression_spdx
        .clone()
        .or_else(|| pkg.declared_license_expression.clone())
        .or_else(|| {
            pkg.license_detections
                .first()
                .map(|d| d.license_expression_spdx.clone())
        })
}

fn package_author(pkg: &Package) -> Option<String> {
    pkg.parties.iter().find_map(|party| party.name.clone())
}

fn component_hashes(pkg: &Package) -> Vec<(&'static str, String)> {
    let mut hashes = Vec::new();
    if let Some(sha1) = &pkg.sha1 {
        hashes.push(("SHA-1", sha1.clone()));
    }
    if let Some(sha256) = &pkg.sha256 {
        hashes.push(("SHA-256", sha256.clone()));
    }
    if let Some(sha512) = &pkg.sha512 {
        hashes.push(("SHA-512", sha512.clone()));
    }
    if let Some(md5) = &pkg.md5 {
        hashes.push(("MD5", md5.clone()));
    }
    hashes
}

fn component_external_references(pkg: &Package) -> Vec<(&'static str, String)> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();
    let mut push_ref = |ref_type: &'static str, url: &Option<String>| {
        if let Some(url) = url
            && seen.insert((ref_type, url.clone()))
        {
            refs.push((ref_type, url.clone()));
        }
    };
    push_ref("bom", &pkg.api_data_url);
    push_ref("issue-tracker", &pkg.bug_tracking_url);
    push_ref("distribution", &pkg.download_url);
    push_ref("distribution", &pkg.repository_download_url);
    push_ref("website", &pkg.homepage_url);
    push_ref("website", &pkg.repository_homepage_url);
    push_ref("vcs", &pkg.vcs_url);
    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DatasourceId;
    use crate::output_schema::OutputDatasourceId;

    fn sample_package(name: &str, version: &str, uid: &str) -> Package {
        Package {
            package_type: None,
            namespace: None,
            name: Some(name.to_string()),
            version: Some(version.to_string()),
            qualifiers: None,
            subpath: None,
            primary_language: None,
            description: None,
            release_date: None,
            parties: vec![],
            keywords: vec![],
            homepage_url: None,
            download_url: None,
            size: None,
            sha1: None,
            md5: None,
            sha256: None,
            sha512: None,
            bug_tracking_url: None,
            code_view_url: None,
            vcs_url: None,
            copyright: None,
            holder: None,
            declared_license_expression: None,
            declared_license_expression_spdx: None,
            license_detections: vec![],
            other_license_expression: None,
            other_license_expression_spdx: None,
            other_license_detections: vec![],
            extracted_license_statement: None,
            notice_text: None,
            source_packages: vec![],
            is_private: false,
            is_virtual: false,
            extra_data: None,
            repository_homepage_url: None,
            repository_download_url: None,
            api_data_url: None,
            purl: Some(format!("pkg:hex/{name}@{version}")),
            package_uid: uid.to_string(),
            datafile_paths: vec![],
            datasource_ids: vec![],
        }
    }

    fn sample_dep(purl: &str, for_package_uid: Option<&str>) -> TopLevelDependency {
        TopLevelDependency {
            purl: Some(purl.to_string()),
            extracted_requirement: None,
            scope: None,
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(true),
            is_direct: Some(true),
            // Intentionally identical resolved purl: the old emitter turned this
            // into a self-edge; the owner→dep graph must not.
            resolved_package: None,
            extra_data: None,
            dependency_uid: format!("{purl}?uuid=dep"),
            for_package_uid: for_package_uid.map(str::to_string),
            datafile_path: "mix.lock".to_string(),
            datasource_id: OutputDatasourceId::from(DatasourceId::HexMixLock),
            namespace: None,
        }
    }

    #[test]
    fn dependency_graph_dedups_shared_deps_across_owners() {
        let pkg_a = sample_package("a", "0.1.0", "uid-a");
        let pkg_b = sample_package("b", "0.1.0", "uid-b");
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![pkg_a, pkg_b],
            dependencies: vec![
                sample_dep("pkg:hex/jason@1.4.1", Some("uid-a")),
                sample_dep("pkg:hex/jason@1.4.1", Some("uid-b")),
                sample_dep("pkg:hex/plug@1.15.0", Some("uid-a")),
                sample_dep("pkg:hex/plug@1.15.0", Some("uid-b")),
            ],
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let refs = component_bom_refs(&output.packages);
        let graph = build_dependency_graph(&output, &refs);

        assert_eq!(graph.len(), 2, "one unique entry per owner package");
        assert_eq!(
            graph["pkg:hex/a@0.1.0"],
            BTreeSet::from([
                "pkg:hex/jason@1.4.1".to_string(),
                "pkg:hex/plug@1.15.0".to_string()
            ])
        );
        assert_eq!(
            graph["pkg:hex/b@0.1.0"],
            BTreeSet::from([
                "pkg:hex/jason@1.4.1".to_string(),
                "pkg:hex/plug@1.15.0".to_string()
            ])
        );
        // No self-edges and no duplicate refs.
        for (r, deps) in &graph {
            assert!(!deps.contains(r), "self-edge on {r}");
        }
    }

    #[test]
    fn component_bom_refs_disambiguate_duplicate_purls() {
        let mut a = sample_package("shared", "1.0.0", "uid-1");
        let mut b = sample_package("shared", "1.0.0", "uid-2");
        a.purl = Some("pkg:hex/shared@1.0.0".to_string());
        b.purl = Some("pkg:hex/shared@1.0.0".to_string());
        let refs = component_bom_refs(&[a, b]);
        assert_eq!(refs[0], "uid-1");
        assert_eq!(refs[1], "uid-2");
        assert_ne!(refs[0], refs[1]);
    }

    #[test]
    fn depends_on_uses_component_bom_ref_when_target_purl_is_unique() {
        let owner = sample_package("app", "1.0.0", "uid-app");
        let mut lib = sample_package("lib", "2.0.0", "uid-lib");
        lib.purl = Some("pkg:hex/lib@2.0.0".to_string());
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![owner, lib],
            dependencies: vec![sample_dep("pkg:hex/lib@2.0.0", Some("uid-app"))],
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let refs = component_bom_refs(&output.packages);
        let graph = build_dependency_graph(&output, &refs);
        assert_eq!(
            graph["pkg:hex/app@1.0.0"],
            BTreeSet::from(["pkg:hex/lib@2.0.0".to_string()])
        );
    }

    #[test]
    fn depends_on_uses_disambiguated_bom_refs_when_target_purl_collides() {
        let owner = sample_package("app", "1.0.0", "uid-app");
        let mut shared_a = sample_package("shared", "1.0.0", "uid-shared-a");
        let mut shared_b = sample_package("shared", "1.0.0", "uid-shared-b");
        shared_a.purl = Some("pkg:hex/shared@1.0.0".to_string());
        shared_b.purl = Some("pkg:hex/shared@1.0.0".to_string());
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![owner, shared_a, shared_b],
            dependencies: vec![sample_dep("pkg:hex/shared@1.0.0", Some("uid-app"))],
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let refs = component_bom_refs(&output.packages);
        assert_eq!(refs[1], "uid-shared-a");
        assert_eq!(refs[2], "uid-shared-b");
        let graph = build_dependency_graph(&output, &refs);
        assert_eq!(
            graph["pkg:hex/app@1.0.0"],
            BTreeSet::from(["uid-shared-a".to_string(), "uid-shared-b".to_string()])
        );
        assert!(!graph["pkg:hex/app@1.0.0"].contains("pkg:hex/shared@1.0.0"));
    }
}
