// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared SBOM inventory construction for the CycloneDX and SPDX renderers.
//!
//! Both renderers historically sourced their component/package inventory only
//! from top-level detected packages (`output.packages`), leaving the resolved
//! dependencies (`output.dependencies`) as graph edges that referenced nothing
//! — a dangling, incomplete BOM. This module promotes every resolved
//! dependency into one shared inventory so those edges resolve to real
//! inventory entries in both formats.
//!
//! See ADR 0012 for the dedup, identity, license-honesty, and metadata-honesty
//! rules this module implements.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::str::FromStr;

use packageurl::PackageUrl;

use crate::output_schema::{
    Output, OutputPackage, OutputResolvedPackage, OutputTopLevelDependency as TopLevelDependency,
};

/// One member of the shared SBOM inventory.
///
/// Detected packages borrow from the scan output; promoted dependencies own a
/// synthesized [`OutputPackage`]. Format-specific presentation (CycloneDX
/// `scope`, SPDX `FilesAnalyzed` / `DESCRIBES`) is derived from the entry kind
/// and proven `is_optional` — never stored as format vocabulary here.
pub(crate) enum InventoryEntry<'a> {
    Detected {
        package: &'a OutputPackage,
    },
    Promoted {
        // Boxed so the borrowed Detected variant does not inflate every entry
        // to the size of a full OutputPackage (clippy::large_enum_variant).
        package: Box<OutputPackage>,
        /// Merged proven optionality across duplicate purls (`None` = unknown).
        is_optional: Option<bool>,
    },
}

impl<'a> InventoryEntry<'a> {
    pub(crate) fn package(&self) -> &OutputPackage {
        match self {
            Self::Detected { package } => package,
            Self::Promoted { package, .. } => package.as_ref(),
        }
    }

    pub(crate) fn is_promoted(&self) -> bool {
        matches!(self, Self::Promoted { .. })
    }

    /// Proven optionality for a promoted dependency; detected packages are
    /// treated as required for CycloneDX scope mapping.
    pub(crate) fn is_optional(&self) -> Option<bool> {
        match self {
            Self::Detected { .. } => Some(false),
            Self::Promoted { is_optional, .. } => *is_optional,
        }
    }
}

/// The identical component/package inventory used by CycloneDX and SPDX.
pub(crate) struct SbomInventory<'a> {
    pub entries: Vec<InventoryEntry<'a>>,
}

impl<'a> SbomInventory<'a> {
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn packages(&self) -> impl Iterator<Item = &OutputPackage> {
        self.entries.iter().map(InventoryEntry::package)
    }
}

/// Build the shared inventory: detected packages, then every resolved
/// dependency promoted and deduped by purl (ADR 0012).
pub(crate) fn build_inventory(output: &Output) -> SbomInventory<'_> {
    let mut entries: Vec<InventoryEntry<'_>> = output
        .packages
        .iter()
        .map(|package| InventoryEntry::Detected { package })
        .collect();

    for (package, is_optional) in promote_dependencies(&output.packages, &output.dependencies) {
        entries.push(InventoryEntry::Promoted {
            package: Box::new(package),
            is_optional,
        });
    }

    SbomInventory { entries }
}

/// Resolve owner→child dependency edges among inventory members.
///
/// `entry_ids` is parallel to `inventory.entries` (CycloneDX `bom-ref` or SPDX
/// package id). Children resolve only to inventory members that share the
/// dependency purl; unresolved endpoints and self-edges are dropped so every
/// emitted edge points at a real inventory entry.
pub(crate) fn dependency_edges(
    inventory: &SbomInventory<'_>,
    dependencies: &[TopLevelDependency],
    entry_ids: &[String],
) -> BTreeMap<String, BTreeSet<String>> {
    debug_assert_eq!(inventory.entries.len(), entry_ids.len());

    let mut uid_to_id: HashMap<&str, &str> = HashMap::new();
    let mut purl_to_ids: HashMap<&str, Vec<&str>> = HashMap::new();
    for (entry, id) in inventory.entries.iter().zip(entry_ids.iter()) {
        let package = entry.package();
        uid_to_id.insert(package.package_uid.as_str(), id.as_str());
        if let Some(purl) = package.purl.as_deref() {
            purl_to_ids.entry(purl).or_default().push(id.as_str());
        }
    }

    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for dep in dependencies {
        let Some(owner_uid) = dep.for_package_uid.as_deref() else {
            continue;
        };
        let Some(owner_id) = uid_to_id.get(owner_uid).copied() else {
            continue;
        };
        let Some(purl) = dependency_edge_purl(dep) else {
            continue;
        };
        let Some(child_ids) = purl_to_ids.get(purl.as_str()) else {
            continue;
        };
        for &child_id in child_ids {
            if owner_id != child_id {
                edges
                    .entry(owner_id.to_string())
                    .or_default()
                    .insert(child_id.to_string());
            }
        }
    }

    edges
}

/// The purl a dependency edge points at: the dependency's own purl, falling
/// back to its resolved package's purl. Empty purls are treated as absent.
pub(crate) fn dependency_edge_purl(dep: &TopLevelDependency) -> Option<String> {
    dep.purl
        .clone()
        .or_else(|| dep.resolved_package.as_ref().and_then(|rp| rp.purl.clone()))
        .filter(|purl| !purl.is_empty())
}

/// Promote resolved dependencies, deduped by purl.
///
/// A dependency whose purl a detected package already owns is skipped (the
/// package is the richer, file-backed representation). Dependencies that share
/// a purl collapse to one component whose proven `is_optional` is merged and
/// whose empty metadata fields are filled from later richer
/// `resolved_package` values. Order is stable: first appearance of each new
/// purl wins its position.
fn promote_dependencies(
    packages: &[OutputPackage],
    dependencies: &[TopLevelDependency],
) -> Vec<(OutputPackage, Option<bool>)> {
    let existing_purls: HashSet<&str> = packages
        .iter()
        .filter_map(|pkg| pkg.purl.as_deref())
        .collect();

    let mut index_by_purl: HashMap<String, usize> = HashMap::new();
    let mut promoted: Vec<(OutputPackage, Option<bool>)> = Vec::new();

    for dep in dependencies {
        let Some(purl) = dependency_edge_purl(dep) else {
            continue;
        };
        if existing_purls.contains(purl.as_str()) {
            continue;
        }

        if let Some(&idx) = index_by_purl.get(&purl) {
            let (package, is_optional) = &mut promoted[idx];
            *is_optional = merge_optional(*is_optional, dep.is_optional);
            if let Some(resolved) = dep.resolved_package.as_deref() {
                fill_missing_from_resolved(package, resolved);
            }
            continue;
        }

        index_by_purl.insert(purl.clone(), promoted.len());
        promoted.push((synthesize_package(dep, &purl), dep.is_optional));
    }

    promoted
}

/// Merge proven optionality. A proven `required` (`Some(false)`) always wins;
/// `optional` fills an otherwise-unknown value; unknown never overrides known.
fn merge_optional(current: Option<bool>, next: Option<bool>) -> Option<bool> {
    match (current, next) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), _) | (_, Some(true)) => Some(true),
        (None, None) => None,
    }
}

/// Build an [`OutputPackage`] for a promoted dependency.
///
/// Identity (`type`/`namespace`/`name`/`version`) comes from the purl so even a
/// bare lockfile entry with no resolved metadata becomes a valid component. All
/// richer fields — including every license field — are copied faithfully from
/// the statically-extracted `resolved_package` when present, and left
/// unset otherwise. Nothing is fetched or guessed (ADR 0012).
fn synthesize_package(dep: &TopLevelDependency, purl: &str) -> OutputPackage {
    let mut package = purl_identity_package(purl, &dep.dependency_uid);

    if let Some(resolved) = dep.resolved_package.as_deref() {
        enrich_from_resolved(&mut package, resolved);
    }

    package
}

/// A minimal component carrying only purl-derived identity; every other field
/// is unset. `package_uid` is the dependency uid, which is unique and never
/// used as a `bom-ref` (a promoted component's `bom-ref` is its unique purl).
fn purl_identity_package(purl: &str, dependency_uid: &str) -> OutputPackage {
    let (package_type, namespace, name, version) = match PackageUrl::from_str(purl) {
        Ok(parsed) => (
            crate::models::PackageType::from_str(parsed.ty())
                .ok()
                .map(crate::output_schema::OutputPackageType::from),
            parsed.namespace().map(str::to_string),
            Some(parsed.name().to_string()),
            parsed.version().map(str::to_string),
        ),
        Err(_) => (None, None, None, None),
    };

    OutputPackage {
        package_type,
        namespace,
        name,
        version,
        qualifiers: None,
        subpath: None,
        primary_language: None,
        description: None,
        release_date: None,
        parties: Vec::new(),
        keywords: Vec::new(),
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
        license_detections: Vec::new(),
        other_license_expression: None,
        other_license_expression_spdx: None,
        other_license_detections: Vec::new(),
        extracted_license_statement: None,
        notice_text: None,
        source_packages: Vec::new(),
        is_private: false,
        is_virtual: false,
        extra_data: None,
        repository_homepage_url: None,
        repository_download_url: None,
        api_data_url: None,
        purl: Some(purl.to_string()),
        package_uid: dependency_uid.to_string(),
        datafile_paths: Vec::new(),
        datasource_ids: Vec::new(),
    }
}

/// Fill richer fields from statically-extracted resolved-package metadata.
/// Identity fields already set from the purl are only overwritten when the
/// resolved package carries a non-empty value.
fn enrich_from_resolved(package: &mut OutputPackage, resolved: &OutputResolvedPackage) {
    fill_identity_from_resolved(package, resolved);
    package.qualifiers = resolved.qualifiers.clone();
    package.subpath = resolved.subpath.clone();
    package.primary_language = resolved.primary_language.clone();
    package.description = resolved.description.clone();
    package.release_date = resolved.release_date.clone();
    package.parties = resolved.parties.clone();
    package.keywords = resolved.keywords.clone();
    package.homepage_url = resolved.homepage_url.clone();
    package.download_url = resolved.download_url.clone();
    package.size = resolved.size;
    package.sha1 = resolved.sha1.clone();
    package.md5 = resolved.md5.clone();
    package.sha256 = resolved.sha256.clone();
    package.sha512 = resolved.sha512.clone();
    package.bug_tracking_url = resolved.bug_tracking_url.clone();
    package.code_view_url = resolved.code_view_url.clone();
    package.vcs_url = resolved.vcs_url.clone();
    package.copyright = resolved.copyright.clone();
    package.holder = resolved.holder.clone();
    // License fields carried faithfully from static metadata; never fetched or
    // guessed. Absent here means genuinely unknown (ADR 0012).
    package.declared_license_expression = resolved.declared_license_expression.clone();
    package.declared_license_expression_spdx = resolved.declared_license_expression_spdx.clone();
    package.license_detections = resolved.license_detections.clone();
    package.other_license_expression = resolved.other_license_expression.clone();
    package.other_license_expression_spdx = resolved.other_license_expression_spdx.clone();
    package.other_license_detections = resolved.other_license_detections.clone();
    package.extracted_license_statement = resolved.extracted_license_statement.clone();
    package.notice_text = resolved.notice_text.clone();
    package.source_packages = resolved.source_packages.clone();
    package.is_private = resolved.is_private;
    package.is_virtual = resolved.is_virtual;
    package.extra_data = resolved.extra_data.clone();
    package.repository_homepage_url = resolved.repository_homepage_url.clone();
    package.repository_download_url = resolved.repository_download_url.clone();
    package.api_data_url = resolved.api_data_url.clone();
    if let Some(datasource_id) = resolved.datasource_id.clone() {
        package.datasource_ids = vec![datasource_id];
    }
}

/// On purl collision, only fill fields that are still empty so a later richer
/// `resolved_package` upgrades a bare first occurrence without wiping earlier
/// evidence.
fn fill_missing_from_resolved(package: &mut OutputPackage, resolved: &OutputResolvedPackage) {
    fill_identity_from_resolved(package, resolved);
    if package.qualifiers.is_none() {
        package.qualifiers = resolved.qualifiers.clone();
    }
    if package.subpath.is_none() {
        package.subpath = resolved.subpath.clone();
    }
    if package.primary_language.is_none() {
        package.primary_language = resolved.primary_language.clone();
    }
    if package.description.is_none() {
        package.description = resolved.description.clone();
    }
    if package.release_date.is_none() {
        package.release_date = resolved.release_date.clone();
    }
    if package.parties.is_empty() {
        package.parties = resolved.parties.clone();
    }
    if package.keywords.is_empty() {
        package.keywords = resolved.keywords.clone();
    }
    if package.homepage_url.is_none() {
        package.homepage_url = resolved.homepage_url.clone();
    }
    if package.download_url.is_none() {
        package.download_url = resolved.download_url.clone();
    }
    if package.size.is_none() {
        package.size = resolved.size;
    }
    if package.sha1.is_none() {
        package.sha1 = resolved.sha1.clone();
    }
    if package.md5.is_none() {
        package.md5 = resolved.md5.clone();
    }
    if package.sha256.is_none() {
        package.sha256 = resolved.sha256.clone();
    }
    if package.sha512.is_none() {
        package.sha512 = resolved.sha512.clone();
    }
    if package.bug_tracking_url.is_none() {
        package.bug_tracking_url = resolved.bug_tracking_url.clone();
    }
    if package.code_view_url.is_none() {
        package.code_view_url = resolved.code_view_url.clone();
    }
    if package.vcs_url.is_none() {
        package.vcs_url = resolved.vcs_url.clone();
    }
    if package.copyright.is_none() {
        package.copyright = resolved.copyright.clone();
    }
    if package.holder.is_none() {
        package.holder = resolved.holder.clone();
    }
    if package.declared_license_expression.is_none() {
        package.declared_license_expression = resolved.declared_license_expression.clone();
    }
    if package.declared_license_expression_spdx.is_none() {
        package.declared_license_expression_spdx =
            resolved.declared_license_expression_spdx.clone();
    }
    if package.license_detections.is_empty() {
        package.license_detections = resolved.license_detections.clone();
    }
    if package.other_license_expression.is_none() {
        package.other_license_expression = resolved.other_license_expression.clone();
    }
    if package.other_license_expression_spdx.is_none() {
        package.other_license_expression_spdx = resolved.other_license_expression_spdx.clone();
    }
    if package.other_license_detections.is_empty() {
        package.other_license_detections = resolved.other_license_detections.clone();
    }
    if package.extracted_license_statement.is_none() {
        package.extracted_license_statement = resolved.extracted_license_statement.clone();
    }
    if package.notice_text.is_none() {
        package.notice_text = resolved.notice_text.clone();
    }
    if package.source_packages.is_empty() {
        package.source_packages = resolved.source_packages.clone();
    }
    if package.extra_data.is_none() {
        package.extra_data = resolved.extra_data.clone();
    }
    if package.repository_homepage_url.is_none() {
        package.repository_homepage_url = resolved.repository_homepage_url.clone();
    }
    if package.repository_download_url.is_none() {
        package.repository_download_url = resolved.repository_download_url.clone();
    }
    if package.api_data_url.is_none() {
        package.api_data_url = resolved.api_data_url.clone();
    }
    if package.datasource_ids.is_empty()
        && let Some(datasource_id) = resolved.datasource_id.clone()
    {
        package.datasource_ids = vec![datasource_id];
    }
}

fn fill_identity_from_resolved(package: &mut OutputPackage, resolved: &OutputResolvedPackage) {
    if package.package_type.is_none() {
        package.package_type = Some(resolved.package_type.clone());
    }
    if package.namespace.is_none() && !resolved.namespace.is_empty() {
        package.namespace = Some(resolved.namespace.clone());
    }
    if package.name.is_none() && !resolved.name.is_empty() {
        package.name = Some(resolved.name.clone());
    }
    if package.version.is_none() && !resolved.version.is_empty() {
        package.version = Some(resolved.version.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DatasourceId, PackageData, PackageType, ResolvedPackage};

    fn detected_package(purl: &str) -> OutputPackage {
        // e.g. "pkg:npm/dep@1.0.0" -> name "dep", version "1.0.0".
        let tail = purl.rsplit('/').next().unwrap_or("pkg");
        let (name, version) = tail
            .split_once('@')
            .map(|(n, v)| (n.to_string(), Some(v.to_string())))
            .unwrap_or_else(|| (tail.to_string(), None));
        OutputPackage::from(&crate::models::Package::from_package_data(
            &PackageData {
                package_type: Some(PackageType::Npm),
                name: Some(name),
                version,
                purl: Some(purl.to_string()),
                ..Default::default()
            },
            "package.json".to_string(),
        ))
    }

    fn dependency(
        purl: Option<&str>,
        is_optional: Option<bool>,
        resolved: Option<ResolvedPackage>,
        uid: &str,
    ) -> TopLevelDependency {
        TopLevelDependency::from(&crate::models::TopLevelDependency {
            purl: purl.map(str::to_string),
            extracted_requirement: None,
            scope: None,
            is_runtime: None,
            is_optional,
            is_pinned: None,
            is_direct: None,
            resolved_package: resolved.map(Box::new),
            extra_data: None,
            dependency_uid: crate::models::DependencyUid::from_raw(uid.to_string()),
            for_package_uid: None,
            datafile_path: "package-lock.json".to_string(),
            datasource_id: DatasourceId::NpmPackageLockJson,
            namespace: None,
        })
    }

    fn promoted_optional(entries: &[InventoryEntry<'_>]) -> Option<bool> {
        match entries.iter().find(|e| e.is_promoted()) {
            Some(InventoryEntry::Promoted { is_optional, .. }) => *is_optional,
            _ => None,
        }
    }

    #[test]
    fn skips_dependency_already_represented_by_a_detected_package() {
        let packages = vec![detected_package("pkg:npm/dep@1.0.0")];
        let deps = vec![dependency(
            Some("pkg:npm/dep@1.0.0"),
            Some(false),
            None,
            "d1",
        )];
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages,
            dependencies: deps,
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let inventory = build_inventory(&output);
        assert_eq!(inventory.entries.len(), 1);
        assert!(!inventory.entries[0].is_promoted());
    }

    #[test]
    fn collapses_shared_purls_and_merges_optional_to_required() {
        let deps = vec![
            dependency(Some("pkg:npm/dep@1.0.0"), Some(true), None, "d1"),
            dependency(Some("pkg:npm/dep@1.0.0"), Some(false), None, "d2"),
        ];
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![],
            dependencies: deps,
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let inventory = build_inventory(&output);
        assert_eq!(inventory.entries.len(), 1, "shared purls collapse to one");
        assert_eq!(
            promoted_optional(&inventory.entries),
            Some(false),
            "a single proven-required occurrence wins the merged optionality"
        );
    }

    #[test]
    fn bare_dependency_gets_identity_from_purl_and_no_license() {
        let deps = vec![dependency(Some("pkg:npm/bare@3.0.0"), None, None, "d1")];
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![],
            dependencies: deps,
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let inventory = build_inventory(&output);
        assert_eq!(inventory.entries.len(), 1);
        let package = inventory.entries[0].package();
        assert_eq!(package.name.as_deref(), Some("bare"));
        assert_eq!(package.version.as_deref(), Some("3.0.0"));
        assert_eq!(package.purl.as_deref(), Some("pkg:npm/bare@3.0.0"));
        assert!(package.declared_license_expression_spdx.is_none());
        assert!(package.declared_license_expression.is_none());
        assert!(package.license_detections.is_empty());
        assert_eq!(
            promoted_optional(&inventory.entries),
            None,
            "unproven optionality stays unset"
        );
    }

    #[test]
    fn carries_declared_license_from_resolved_package() {
        let mut resolved = ResolvedPackage::new(
            PackageType::Npm,
            String::new(),
            "licensed".to_string(),
            "1.0.0".to_string(),
        );
        resolved.purl = Some("pkg:npm/licensed@1.0.0".to_string());
        resolved.declared_license_expression = Some("mit".to_string());
        resolved.declared_license_expression_spdx = Some("MIT".to_string());

        let deps = vec![dependency(
            Some("pkg:npm/licensed@1.0.0"),
            Some(false),
            Some(resolved),
            "d1",
        )];
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![],
            dependencies: deps,
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let inventory = build_inventory(&output);
        assert_eq!(inventory.entries.len(), 1);
        assert_eq!(
            inventory.entries[0]
                .package()
                .declared_license_expression_spdx
                .as_deref(),
            Some("MIT")
        );
    }

    #[test]
    fn purl_collision_fills_license_from_later_richer_resolved_package() {
        let mut resolved = ResolvedPackage::new(
            PackageType::Npm,
            String::new(),
            "dep".to_string(),
            "1.0.0".to_string(),
        );
        resolved.purl = Some("pkg:npm/dep@1.0.0".to_string());
        resolved.declared_license_expression_spdx = Some("MIT".to_string());

        let deps = vec![
            dependency(Some("pkg:npm/dep@1.0.0"), Some(true), None, "d1"),
            dependency(Some("pkg:npm/dep@1.0.0"), Some(false), Some(resolved), "d2"),
        ];
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![],
            dependencies: deps,
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let inventory = build_inventory(&output);
        assert_eq!(inventory.entries.len(), 1);
        assert_eq!(
            inventory.entries[0]
                .package()
                .declared_license_expression_spdx
                .as_deref(),
            Some("MIT"),
            "bare-then-licensed collapse must retain the license"
        );
        assert_eq!(promoted_optional(&inventory.entries), Some(false));
    }

    #[test]
    fn skips_dependency_without_a_purl() {
        let deps = vec![dependency(None, Some(false), None, "d1")];
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![],
            dependencies: deps,
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        assert!(build_inventory(&output).is_empty());
    }

    #[test]
    fn falls_back_to_resolved_package_purl_when_dependency_purl_is_absent() {
        let mut resolved = ResolvedPackage::new(
            PackageType::Npm,
            String::new(),
            "resolved-only".to_string(),
            "2.0.0".to_string(),
        );
        resolved.purl = Some("pkg:npm/resolved-only@2.0.0".to_string());
        let deps = vec![dependency(None, None, Some(resolved), "d1")];
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![],
            dependencies: deps,
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let inventory = build_inventory(&output);
        assert_eq!(inventory.entries.len(), 1);
        assert_eq!(
            inventory.entries[0].package().purl.as_deref(),
            Some("pkg:npm/resolved-only@2.0.0")
        );
    }

    #[test]
    fn dependency_edges_drop_unresolved_endpoints() {
        let mut root = detected_package("pkg:npm/root@1.0.0");
        root.package_uid = "uid-root".to_string();
        let output = Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![root],
            dependencies: vec![],
            license_detections: vec![],
            files: vec![],
            license_references: vec![],
            license_rule_references: vec![],
        };
        // Inventory has only the root; the edge targets a purl that is not in
        // inventory (simulating a pre-promotion dangling ref).
        let inventory = build_inventory(&output);
        let dangling = TopLevelDependency::from(&crate::models::TopLevelDependency {
            purl: Some("pkg:npm/missing@1.0.0".to_string()),
            extracted_requirement: None,
            scope: None,
            is_runtime: None,
            is_optional: Some(false),
            is_pinned: None,
            is_direct: None,
            resolved_package: None,
            extra_data: None,
            dependency_uid: crate::models::DependencyUid::from_raw("d-missing".to_string()),
            for_package_uid: Some(crate::models::PackageUid::from_raw("uid-root".to_string())),
            datafile_path: "package-lock.json".to_string(),
            datasource_id: DatasourceId::NpmPackageLockJson,
            namespace: None,
        });
        let edges = dependency_edges(&inventory, &[dangling], &["bom-root".to_string()]);
        assert!(
            edges.is_empty(),
            "an edge whose target is not in inventory must not be emitted"
        );
    }
}
