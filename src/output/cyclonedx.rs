// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Write};
use uuid::Uuid;

use crate::output_schema::{
    Output, OutputPackage as Package, OutputTopLevelDependency as TopLevelDependency,
};
use crate::utils::time::{convert_header_timestamp_to_iso_utc, fallback_iso_utc_timestamp};

use super::sbom::{self, InventoryEntry, SbomInventory};
use super::shared::{io_other, xml_escape};

/// CycloneDX specification version this renderer emits.
const CYCLONEDX_SPEC_VERSION: &str = "1.7";
/// Canonical `$schema` URL for the emitted JSON spec version.
const CYCLONEDX_JSON_SCHEMA: &str = "http://cyclonedx.org/schema/bom-1.7.schema.json";
/// XML namespace for the emitted spec version.
const CYCLONEDX_XML_NAMESPACE: &str = "http://cyclonedx.org/schema/bom/1.7";

/// How a component's license expression was established, mapped onto the
/// CycloneDX `licenses[].acknowledgement` field. A parser-declared expression
/// (the same source SPDX renders as `PackageLicenseDeclared`) is `declared`; a
/// file/source-detected expression is `concluded`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LicenseAcknowledgement {
    Declared,
    Concluded,
}

impl LicenseAcknowledgement {
    fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Concluded => "concluded",
        }
    }
}

/// CycloneDX `scope` from shared inventory intent. Detected packages are
/// `required`; promoted dependencies map proven `is_optional` only.
fn cyclonedx_scope(entry: &InventoryEntry<'_>) -> Option<&'static str> {
    match entry.is_optional() {
        Some(true) => Some("optional"),
        Some(false) => Some("required"),
        None => None,
    }
}

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
    let inventory = sbom::build_inventory(output);
    let component_refs = component_bom_refs_for(&inventory);
    let dependency_graph =
        build_dependency_graph(&inventory, &output.dependencies, &component_refs);

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<bom xmlns=\"{}\" serialNumber=\"{}\" version=\"1\">\n",
        CYCLONEDX_XML_NAMESPACE,
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
    // `component` (the BOM subject) must follow `tools` per the CycloneDX
    // 1.7 XSD `metadata` element order.
    if let Some(root_idx) = select_root_package_index(&output.packages) {
        write_component_xml(
            &mut xml,
            &output.packages[root_idx],
            None,
            "application",
            None,
        );
    }
    xml.push_str("  </metadata>\n");

    xml.push_str("  <components>\n");
    for (idx, entry) in inventory.entries.iter().enumerate() {
        write_component_xml(
            &mut xml,
            entry.package(),
            Some(&component_refs[idx]),
            "library",
            cyclonedx_scope(entry),
        );
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

/// Write a single `<component>` element (used for both `metadata.component`
/// and entries in `<components>`), in the CycloneDX 1.7 XSD element order:
/// `author` precedes `name`/`version`, which precede `description`, which
/// precedes `scope`, `hashes`, `licenses`, `purl`, `externalReferences`.
///
/// `bom_ref` is `None` for `metadata.component`: CycloneDX requires
/// every `bom-ref` in a document to be unique, so the root package's
/// existing `components` entry (which already carries that `bom-ref`) is
/// the single source of truth for it and `metadata.component` does not
/// repeat it.
fn write_component_xml(
    xml: &mut String,
    pkg: &Package,
    bom_ref: Option<&str>,
    component_type: &str,
    scope: Option<&str>,
) {
    let name = pkg.name.as_deref().unwrap_or("unknown");
    let version = pkg.version.as_deref().unwrap_or("unknown");
    match bom_ref {
        Some(bom_ref) => xml.push_str(&format!(
            "    <component type=\"{component_type}\" bom-ref=\"{}\">\n",
            xml_escape(bom_ref)
        )),
        None => xml.push_str(&format!("    <component type=\"{component_type}\">\n")),
    }
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
    if let Some(scope) = scope {
        xml.push_str("      <scope>");
        xml.push_str(&xml_escape(scope));
        xml.push_str("</scope>\n");
    }
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
    if let Some((license_expression, acknowledgement)) = cyclonedx_license_expression(pkg) {
        xml.push_str(&format!(
            "      <licenses><expression acknowledgement=\"{}\">",
            acknowledgement.as_str()
        ));
        xml.push_str(&xml_escape(&license_expression));
        xml.push_str("</expression></licenses>\n");
    }
    // `purl` must follow `licenses`/`copyright`/`cpe` per the CycloneDX 1.7 XSD.
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

fn build_cyclonedx_json(output: &Output) -> Value {
    let timestamp = output
        .headers
        .first()
        .and_then(|h| convert_header_timestamp_to_iso_utc(&h.end_timestamp))
        .unwrap_or_else(|| fallback_iso_utc_timestamp().to_string());
    let inventory = sbom::build_inventory(output);
    let component_refs = component_bom_refs_for(&inventory);

    let components = inventory
        .entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let mut obj = Map::new();
            obj.insert("type".to_string(), Value::String("library".to_string()));
            component_json_fields(
                &mut obj,
                entry.package(),
                Some(&component_refs[idx]),
                cyclonedx_scope(entry),
            );
            Value::Object(obj)
        })
        .collect::<Vec<_>>();

    let dependencies = build_dependency_graph(&inventory, &output.dependencies, &component_refs)
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
            "$schema": CYCLONEDX_JSON_SCHEMA,
            "bomFormat": "CycloneDX",
            "specVersion": CYCLONEDX_SPEC_VERSION,
            "version": 1,
            "components": [],
            "dependencies": [],
        })
    } else {
        let mut metadata = Map::new();
        metadata.insert("timestamp".to_string(), Value::String(timestamp));
        metadata.insert(
            "tools".to_string(),
            json!([{ "name": "Provenant", "version": crate::version::BUILD_VERSION }]),
        );
        if let Some(root_idx) = select_root_package_index(&output.packages) {
            let mut root_obj = Map::new();
            root_obj.insert("type".to_string(), Value::String("application".to_string()));
            // No `bom-ref`: CycloneDX requires every `bom-ref` in a
            // document to be unique, and the root package's existing
            // `components` entry already carries that `bom-ref`.
            component_json_fields(&mut root_obj, &output.packages[root_idx], None, None);
            metadata.insert("component".to_string(), Value::Object(root_obj));
        }

        json!({
            "$schema": CYCLONEDX_JSON_SCHEMA,
            "bomFormat": "CycloneDX",
            "specVersion": CYCLONEDX_SPEC_VERSION,
            "serialNumber": format!("urn:uuid:{}", Uuid::new_v4()),
            "version": 1,
            "metadata": metadata,
            "components": components,
            "dependencies": dependencies,
        })
    }
}

/// Populate the fields shared by every CycloneDX `component` object (JSON):
/// an optional `bom-ref` (omitted for `metadata.component`, see
/// [`build_cyclonedx_json`]), `name`, `version`, `description`, `author`,
/// an optional `scope` (regular components only; `metadata.component` has
/// no scope of its own), `purl`, `hashes`, `licenses`,
/// `externalReferences`. Callers insert `type` before calling this.
fn component_json_fields(
    obj: &mut Map<String, Value>,
    pkg: &Package,
    bom_ref: Option<&str>,
    scope: Option<&str>,
) {
    if let Some(bom_ref) = bom_ref {
        obj.insert("bom-ref".to_string(), Value::String(bom_ref.to_string()));
    }
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
    if let Some(scope) = scope {
        obj.insert("scope".to_string(), Value::String(scope.to_string()));
    }
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
    if let Some((license_expression, acknowledgement)) = cyclonedx_license_expression(pkg) {
        obj.insert(
            "licenses".to_string(),
            Value::Array(vec![json!({
                "expression": license_expression,
                "acknowledgement": acknowledgement.as_str(),
            })]),
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
}

/// Assign a unique `bom-ref` per package. Prefer purl when it does not collide;
/// fall back to `package_uid` (always unique) or a synthetic index-based ref.
fn component_bom_refs_for(inventory: &SbomInventory<'_>) -> Vec<String> {
    component_bom_refs(inventory.packages())
}

fn component_bom_refs<'a>(packages: impl IntoIterator<Item = &'a Package>) -> Vec<String> {
    let packages: Vec<&Package> = packages.into_iter().collect();
    let mut purl_counts: HashMap<&str, usize> = HashMap::new();
    for pkg in &packages {
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

/// Select the package that should become CycloneDX `metadata.component`,
/// i.e. the "subject" of the BOM, if one can be identified without guessing.
///
/// Selection rule:
/// - A scan with exactly one package always has that package as the root.
/// - A scan with several packages has a root only when exactly one
///   package's shallowest datafile path is strictly closer to the scan root
///   (fewer path components) than every other package's shallowest datafile
///   path. This is the workspace/monorepo root-manifest signature: a
///   top-level `Cargo.toml`/`package.json` sitting above nested member
///   manifests.
/// - Anything else — no packages, or several packages tied for the
///   shallowest path — is ambiguous, so `metadata.component` is omitted
///   rather than guessed.
fn select_root_package_index(packages: &[Package]) -> Option<usize> {
    if packages.len() == 1 {
        return Some(0);
    }

    let mut depths: Vec<(usize, usize)> = packages
        .iter()
        .enumerate()
        .filter_map(|(idx, pkg)| package_min_datafile_depth(pkg).map(|depth| (idx, depth)))
        .collect();
    depths.sort_by_key(|(_, depth)| *depth);

    match depths.as_slice() {
        [(idx, shallowest), rest @ ..] if rest.iter().all(|(_, depth)| depth > shallowest) => {
            Some(*idx)
        }
        _ => None,
    }
}

fn package_min_datafile_depth(pkg: &Package) -> Option<usize> {
    pkg.datafile_paths
        .iter()
        .map(|path| {
            path.replace('\\', "/")
                .split('/')
                .filter(|component| !component.is_empty())
                .count()
        })
        .min()
}

/// Build a CycloneDX dependency graph: one unique entry per inventory
/// `bom-ref`, with `dependsOn` listing only inventory members.
///
/// Edges come from the shared [`sbom::dependency_edges`] walker. Components
/// with no outgoing edges still get an empty `dependsOn` entry so the graph
/// is complete. Unresolved endpoints are never emitted as bare purls.
fn build_dependency_graph(
    inventory: &SbomInventory<'_>,
    dependencies: &[TopLevelDependency],
    component_refs: &[String],
) -> BTreeMap<String, BTreeSet<String>> {
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    // Every component appears in the graph (CycloneDX recommends empty nodes for
    // leaves so the graph is explicit).
    for bom_ref in component_refs {
        graph.entry(bom_ref.clone()).or_default();
    }

    for (owner_ref, children) in sbom::dependency_edges(inventory, dependencies, component_refs) {
        graph.entry(owner_ref).or_default().extend(children);
    }

    graph
}

/// The single license expression to emit for a component, paired with how it
/// was established. Preference order: declared SPDX expression, then declared
/// raw expression (both parser-`declared`), then the first source/file
/// detection (`concluded`).
fn cyclonedx_license_expression(pkg: &Package) -> Option<(String, LicenseAcknowledgement)> {
    pkg.declared_license_expression_spdx
        .clone()
        .or_else(|| pkg.declared_license_expression.clone())
        .map(|expression| (expression, LicenseAcknowledgement::Declared))
        .or_else(|| {
            pkg.license_detections.first().map(|d| {
                (
                    d.license_expression_spdx.clone(),
                    LicenseAcknowledgement::Concluded,
                )
            })
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
        let inventory = sbom::build_inventory(&output);
        let refs = component_bom_refs_for(&inventory);
        let graph = build_dependency_graph(&inventory, &output.dependencies, &refs);

        // Owners + promoted unique deps (jason, plug).
        assert_eq!(graph.len(), 4);
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
        assert!(graph["pkg:hex/jason@1.4.1"].is_empty());
        assert!(graph["pkg:hex/plug@1.15.0"].is_empty());
        for (r, deps) in &graph {
            assert!(!deps.contains(r), "self-edge on {r}");
            for child in deps {
                assert!(graph.contains_key(child), "dangling dependsOn ref: {child}");
            }
        }
    }

    #[test]
    fn component_bom_refs_disambiguate_duplicate_purls() {
        let mut a = sample_package("shared", "1.0.0", "uid-1");
        let mut b = sample_package("shared", "1.0.0", "uid-2");
        a.purl = Some("pkg:hex/shared@1.0.0".to_string());
        b.purl = Some("pkg:hex/shared@1.0.0".to_string());
        let refs = component_bom_refs([&a, &b]);
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
        let inventory = sbom::build_inventory(&output);
        let refs = component_bom_refs_for(&inventory);
        let graph = build_dependency_graph(&inventory, &output.dependencies, &refs);
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
        let inventory = sbom::build_inventory(&output);
        let refs = component_bom_refs_for(&inventory);
        assert_eq!(refs[1], "uid-shared-a");
        assert_eq!(refs[2], "uid-shared-b");
        let graph = build_dependency_graph(&inventory, &output.dependencies, &refs);
        assert_eq!(
            graph["pkg:hex/app@1.0.0"],
            BTreeSet::from(["uid-shared-a".to_string(), "uid-shared-b".to_string()])
        );
        assert!(!graph["pkg:hex/app@1.0.0"].contains("pkg:hex/shared@1.0.0"));
    }

    fn with_datafile_path(mut pkg: Package, path: &str) -> Package {
        pkg.datafile_paths = vec![path.to_string()];
        pkg
    }

    #[test]
    fn select_root_package_index_picks_sole_package() {
        let only = sample_package("app", "1.0.0", "uid-app");
        assert_eq!(select_root_package_index(&[only]), Some(0));
    }

    #[test]
    fn select_root_package_index_picks_unique_shallowest_workspace_root() {
        let root = with_datafile_path(
            sample_package("workspace", "1.0.0", "uid-root"),
            "package.json",
        );
        let member_a = with_datafile_path(
            sample_package("widget-a", "1.0.0", "uid-a"),
            "packages/widget-a/package.json",
        );
        let member_b = with_datafile_path(
            sample_package("widget-b", "1.0.0", "uid-b"),
            "packages/widget-b/package.json",
        );
        let packages = vec![member_a, root, member_b];
        assert_eq!(select_root_package_index(&packages), Some(1));
    }

    #[test]
    fn select_root_package_index_omits_when_multiple_roots_tie() {
        let member_a = with_datafile_path(
            sample_package("widget-a", "1.0.0", "uid-a"),
            "packages/widget-a/package.json",
        );
        let member_b = with_datafile_path(
            sample_package("widget-b", "1.0.0", "uid-b"),
            "packages/widget-b/package.json",
        );
        let packages = vec![member_a, member_b];
        assert_eq!(select_root_package_index(&packages), None);
    }

    #[test]
    fn select_root_package_index_omits_when_no_packages() {
        assert_eq!(select_root_package_index(&[]), None);
    }

    #[test]
    fn select_root_package_index_picks_shallowest_root_with_windows_style_paths() {
        // Datafile paths can carry `\`-separated components regardless of the
        // host OS the scan runs on; depth must be computed consistently.
        let root = with_datafile_path(
            sample_package("workspace", "1.0.0", "uid-root"),
            "package.json",
        );
        let member = with_datafile_path(
            sample_package("member", "1.0.0", "uid-member"),
            "packages\\member\\package.json",
        );
        let packages = vec![member, root];
        assert_eq!(select_root_package_index(&packages), Some(1));
    }

    fn package_from_data(data: crate::models::PackageData) -> Package {
        Package::from(&crate::models::Package::from_package_data(
            &data,
            "package.json".to_string(),
        ))
    }

    #[test]
    fn license_expression_tags_parser_declared_as_declared() {
        let pkg = package_from_data(crate::models::PackageData {
            package_type: Some(crate::models::PackageType::Npm),
            declared_license_expression: Some("mit".to_string()),
            declared_license_expression_spdx: Some("MIT".to_string()),
            ..Default::default()
        });
        assert_eq!(
            cyclonedx_license_expression(&pkg),
            Some(("MIT".to_string(), LicenseAcknowledgement::Declared))
        );
    }

    #[test]
    fn license_expression_tags_source_detection_as_concluded() {
        let pkg = package_from_data(crate::models::PackageData {
            package_type: Some(crate::models::PackageType::Npm),
            // No declared expression: the only evidence is a source/file
            // detection, which CycloneDX marks `concluded`.
            license_detections: vec![crate::models::LicenseDetection {
                license_expression: "apache-2.0".to_string(),
                license_expression_spdx: "Apache-2.0".to_string(),
                matches: vec![],
                detection_log: vec![],
                identifier: String::new(),
            }],
            ..Default::default()
        });
        assert_eq!(
            cyclonedx_license_expression(&pkg),
            Some(("Apache-2.0".to_string(), LicenseAcknowledgement::Concluded))
        );
    }

    #[test]
    fn license_expression_prefers_declared_over_detection() {
        // A package that both declares and has a detection reports the declared
        // expression tagged `declared` (unchanged selection, matching SPDX).
        let pkg = package_from_data(crate::models::PackageData {
            package_type: Some(crate::models::PackageType::Npm),
            declared_license_expression_spdx: Some("MIT".to_string()),
            license_detections: vec![crate::models::LicenseDetection {
                license_expression: "apache-2.0".to_string(),
                license_expression_spdx: "Apache-2.0".to_string(),
                matches: vec![],
                detection_log: vec![],
                identifier: String::new(),
            }],
            ..Default::default()
        });
        assert_eq!(
            cyclonedx_license_expression(&pkg),
            Some(("MIT".to_string(), LicenseAcknowledgement::Declared))
        );
    }

    #[test]
    fn json_emits_spec_version_schema_and_license_acknowledgement() {
        let mut declared = package_from_data(crate::models::PackageData {
            package_type: Some(crate::models::PackageType::Npm),
            name: Some("declared-pkg".to_string()),
            version: Some("1.0.0".to_string()),
            purl: Some("pkg:npm/declared-pkg@1.0.0".to_string()),
            declared_license_expression_spdx: Some("MIT".to_string()),
            ..Default::default()
        });
        declared.package_uid = "uid-declared".to_string();
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![declared],
            dependencies: vec![],
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };

        let bom = build_cyclonedx_json(&output);
        assert_eq!(bom["specVersion"], "1.7");
        assert_eq!(bom["$schema"], CYCLONEDX_JSON_SCHEMA);
        assert_eq!(bom["bomFormat"], "CycloneDX");
        let license = &bom["components"][0]["licenses"][0];
        assert_eq!(license["expression"], "MIT");
        assert_eq!(license["acknowledgement"], "declared");
    }

    #[test]
    fn empty_document_carries_spec_version_and_schema() {
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![],
            dependencies: vec![],
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let bom = build_cyclonedx_json(&output);
        assert_eq!(bom["specVersion"], "1.7");
        assert_eq!(bom["$schema"], CYCLONEDX_JSON_SCHEMA);
    }

    #[test]
    fn metadata_component_has_no_bom_ref_to_avoid_duplicating_the_unique_key() {
        // CycloneDX requires every `bom-ref` in a document to be unique.
        // The root package already gets a `bom-ref` in `components`, so
        // `metadata.component` must not repeat it (schema-guard against
        // reintroducing the duplicate-key regression).
        let root = with_datafile_path(
            sample_package("workspace", "1.0.0", "uid-root"),
            "package.json",
        );
        let member = with_datafile_path(
            sample_package("widget-a", "1.0.0", "uid-a"),
            "packages/widget-a/package.json",
        );
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![root, member],
            dependencies: vec![],
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };

        let bom = build_cyclonedx_json(&output);
        assert!(bom["metadata"]["component"]["bom-ref"].is_null());
        assert_eq!(bom["metadata"]["component"]["type"], "application");
        assert_eq!(bom["metadata"]["component"]["name"], "workspace");
        assert!(bom["components"][0]["bom-ref"].is_string());
    }
}
