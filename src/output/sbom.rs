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
    /// Per-owner version evidence: `(owner package_uid, version-less purl
    /// identity)` → the versioned purls that owner's own dependency edges
    /// resolved that identity to. Scoped by owner so an unversioned requirement
    /// under one package never inherits a version another package resolved the
    /// same name to. See ADR 0012.
    version_index: VersionIndex,
}

impl<'a> SbomInventory<'a> {
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn packages(&self) -> impl Iterator<Item = &OutputPackage> {
        self.entries.iter().map(InventoryEntry::package)
    }

    /// Resolve a dependency edge to the purl its component and graph edges use.
    /// An already-versioned edge keeps its purl; an unversioned one is upgraded
    /// only from that edge's own `resolved_package`, or from a single
    /// unambiguous versioned sibling under the **same owner** — never across
    /// owners and never by guessing among multiple versions. Promotion and
    /// graph-edge resolution both route through this so a versioned component
    /// and the edges that target it stay consistent and the graph stays closed.
    pub(crate) fn resolve_edge_purl(&self, dep: &TopLevelDependency) -> Option<String> {
        resolve_edge_purl(dep, &self.version_index)
    }
}

/// Build the shared inventory: detected packages, then every resolved
/// dependency promoted and deduped by purl (ADR 0012).
pub(crate) fn build_inventory(output: &Output) -> SbomInventory<'_> {
    let version_index = build_version_index(&output.dependencies);

    let mut entries: Vec<InventoryEntry<'_>> = output
        .packages
        .iter()
        .map(|package| InventoryEntry::Detected { package })
        .collect();

    for (package, is_optional) in
        promote_dependencies(&output.packages, &output.dependencies, &version_index)
    {
        entries.push(InventoryEntry::Promoted {
            package: Box::new(package),
            is_optional,
        });
    }

    SbomInventory {
        entries,
        version_index,
    }
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
        let Some(purl) = inventory.resolve_edge_purl(dep) else {
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

/// Per-owner version evidence keyed by `(owner package_uid, version-less purl
/// identity)`. The value is the set of distinct versioned purls that owner's
/// dependency edges resolved that identity to.
type VersionIndex = HashMap<(String, String), BTreeSet<String>>;

/// A purl's version-less identity and whether it carries a version. `None` when
/// the string does not parse as a purl.
///
/// The identity strips **only** the version: type, namespace, name,
/// qualifiers, and subpath are all preserved (rebuilt through `PackageUrl` so
/// the string is canonical, with qualifiers sorted). Two purls therefore share
/// an identity only when they are the same coordinate at different versions —
/// variants that differ by a qualifier (`?arch=`, `?classifier=`, …) or subpath
/// stay distinct and are never conflated.
fn purl_version_less_identity(purl: &str) -> Option<(String, bool)> {
    let parsed = PackageUrl::from_str(purl).ok()?;
    let versioned = parsed.version().is_some();

    let mut identity = PackageUrl::new(parsed.ty().to_string(), parsed.name().to_string()).ok()?;
    if let Some(namespace) = parsed.namespace() {
        identity.with_namespace(namespace.to_string()).ok()?;
    }
    for (key, value) in parsed.qualifiers() {
        identity
            .add_qualifier(key.to_string(), value.to_string())
            .ok()?;
    }
    if let Some(subpath) = parsed.subpath() {
        identity.with_subpath(subpath.to_string()).ok()?;
    }
    Some((identity.to_string(), versioned))
}

/// Owner (`for_package_uid`) key for a dependency edge; `None` collapses to an
/// empty owner so unowned edges only ever match each other, never a real owner.
fn owner_key(dep: &TopLevelDependency) -> String {
    dep.for_package_uid.clone().unwrap_or_default()
}

/// Build per-owner version evidence from dependency edges only. Each edge
/// records the versioned purls it carries (its own purl and its resolved
/// package's purl) under its owner and version-less identity. Detected packages
/// are intentionally excluded — they are not owner-scoped, so using them as
/// version evidence could assign one package's resolved version to another
/// package's requirement.
fn build_version_index(dependencies: &[TopLevelDependency]) -> VersionIndex {
    let mut index: VersionIndex = HashMap::new();
    for dep in dependencies {
        let owner = owner_key(dep);
        let purls = [
            dep.purl.as_deref(),
            dep.resolved_package
                .as_ref()
                .and_then(|rp| rp.purl.as_deref()),
        ];
        for purl in purls.into_iter().flatten() {
            if let Some((identity, true)) = purl_version_less_identity(purl) {
                index
                    .entry((owner.clone(), identity))
                    .or_default()
                    .insert(purl.to_string());
            }
        }
    }
    index
}

/// Resolve a dependency edge to the purl its component and graph edges use.
///
/// An already-versioned edge keeps its purl. An unversioned edge is upgraded
/// only from proof about that same edge — first its own `resolved_package`
/// purl, then a single unambiguous versioned sibling recorded under the **same
/// owner** and identity. When the owner has zero or several candidate versions
/// for the identity, the edge keeps its unversioned purl (honest-unknown), and
/// no version is ever borrowed from a different owner.
fn resolve_edge_purl(dep: &TopLevelDependency, version_index: &VersionIndex) -> Option<String> {
    let declared = dependency_edge_purl(dep)?;
    let (identity, versioned) = purl_version_less_identity(&declared)?;
    if versioned {
        return Some(declared);
    }

    // The edge's own resolved-package purl is its actual resolution — trust it
    // when it is versioned and the same coordinate.
    if let Some(resolved_purl) = dep
        .resolved_package
        .as_ref()
        .and_then(|rp| rp.purl.as_deref())
        && let Some((resolved_identity, true)) = purl_version_less_identity(resolved_purl)
        && resolved_identity == identity
    {
        return Some(resolved_purl.to_string());
    }

    // Same-owner sibling evidence, only when unambiguous.
    if let Some(candidates) = version_index.get(&(owner_key(dep), identity))
        && candidates.len() == 1
        && let Some(only) = candidates.iter().next()
    {
        return Some(only.clone());
    }

    Some(declared)
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
    version_index: &VersionIndex,
) -> Vec<(OutputPackage, Option<bool>)> {
    let existing_purls: HashSet<&str> = packages
        .iter()
        .filter_map(|pkg| pkg.purl.as_deref())
        .collect();

    let mut index_by_purl: HashMap<String, usize> = HashMap::new();
    let mut promoted: Vec<(OutputPackage, Option<bool>)> = Vec::new();

    for dep in dependencies {
        // Upgrade an unversioned requirement edge to its resolved coordinate so
        // it dedups against the detected package or lockfile-resolved component
        // for the same dependency instead of becoming a second, unversioned one.
        let Some(purl) = resolve_edge_purl(dep, version_index) else {
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

    fn dependency_owned(
        purl: Option<&str>,
        resolved: Option<ResolvedPackage>,
        uid: &str,
        owner: &str,
    ) -> TopLevelDependency {
        TopLevelDependency::from(&crate::models::TopLevelDependency {
            purl: purl.map(str::to_string),
            extracted_requirement: None,
            scope: None,
            is_runtime: None,
            is_optional: Some(false),
            is_pinned: None,
            is_direct: None,
            resolved_package: resolved.map(Box::new),
            extra_data: None,
            dependency_uid: crate::models::DependencyUid::from_raw(uid.to_string()),
            for_package_uid: Some(crate::models::PackageUid::from_raw(owner.to_string())),
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

    fn output_with(packages: Vec<OutputPackage>, deps: Vec<TopLevelDependency>) -> Output {
        Output {
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
        }
    }

    fn licensed_detected_package(purl: &str, spdx: &str) -> OutputPackage {
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
                declared_license_expression: Some(spdx.to_lowercase()),
                declared_license_expression_spdx: Some(spdx.to_string()),
                ..Default::default()
            },
            "package.json".to_string(),
        ))
    }

    // Vendored join (issue #1320): with `node_modules/<dep>/package.json` on
    // disk the dependency is a detected, licensed package at its versioned purl.
    // The manifest yields an unversioned requirement edge and the lockfile a
    // versioned resolution — both under the same owner. The unversioned edge
    // resolves onto the detected package, leaving ONE licensed, versioned
    // component — never a second bare `pkg:npm/leftpad`.
    #[test]
    fn unversioned_requirement_merges_into_vendored_detected_package() {
        let packages = vec![licensed_detected_package("pkg:npm/leftpad@1.0.0", "MIT")];
        let deps = vec![
            dependency_owned(Some("pkg:npm/leftpad"), None, "req", "owner-1"),
            dependency_owned(Some("pkg:npm/leftpad@1.0.0"), None, "lock", "owner-1"),
        ];
        let output = output_with(packages, deps);
        let inventory = build_inventory(&output);

        assert_eq!(
            inventory.entries.len(),
            1,
            "must not add a second component"
        );
        assert!(!inventory.entries[0].is_promoted());
        let package = inventory.entries[0].package();
        assert_eq!(package.purl.as_deref(), Some("pkg:npm/leftpad@1.0.0"));
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("MIT")
        );
    }

    // P1 (cross-owner): two packages can resolve the same requirement to
    // different versions. An unversioned edge must take its version only from
    // its own owner's resolution — never from another owner's.
    #[test]
    fn unversioned_requirement_never_inherits_version_across_owners() {
        let deps = vec![
            // owner A: requirement + its own resolution to 1.0.0
            dependency_owned(Some("pkg:npm/lib"), None, "a-req", "owner-a"),
            dependency_owned(Some("pkg:npm/lib@1.0.0"), None, "a-lock", "owner-a"),
            // owner B: requirement + its own resolution to 2.0.0
            dependency_owned(Some("pkg:npm/lib"), None, "b-req", "owner-b"),
            dependency_owned(Some("pkg:npm/lib@2.0.0"), None, "b-lock", "owner-b"),
        ];
        let index = build_version_index(&deps);
        // Each owner's unversioned requirement resolves to that owner's version.
        assert_eq!(
            resolve_edge_purl(&deps[0], &index).as_deref(),
            Some("pkg:npm/lib@1.0.0"),
            "owner A keeps its own 1.0.0"
        );
        assert_eq!(
            resolve_edge_purl(&deps[2], &index).as_deref(),
            Some("pkg:npm/lib@2.0.0"),
            "owner B keeps its own 2.0.0 — no cross-owner assignment"
        );

        // Both versioned components still exist and the graph stays closed.
        let output = output_with(vec![], deps);
        let inventory = build_inventory(&output);
        let purls: BTreeSet<&str> = inventory
            .packages()
            .filter_map(|pkg| pkg.purl.as_deref())
            .collect();
        assert_eq!(
            purls,
            BTreeSet::from(["pkg:npm/lib@1.0.0", "pkg:npm/lib@2.0.0"])
        );
        let ids: Vec<String> = (0..inventory.entries.len())
            .map(|i| format!("id-{i}"))
            .collect();
        let edges = dependency_edges(&inventory, &output.dependencies, &ids);
        for children in edges.values() {
            for child in children {
                assert!(
                    ids.contains(child),
                    "every edge resolves within the document"
                );
            }
        }
    }

    // P1 (qualifiers/subpath): purls that differ only by a qualifier or subpath
    // are DIFFERENT coordinates. An unversioned edge for one must not be rewritten
    // to a versioned sibling that carries a different qualifier/subpath.
    #[test]
    fn qualifier_and_subpath_variants_are_not_conflated() {
        let deps = vec![
            // A versioned candidate that carries a qualifier.
            dependency_owned(
                Some("pkg:maven/g/a@1.0.0?classifier=sources"),
                None,
                "v",
                "owner-1",
            ),
            // An unversioned requirement WITHOUT that qualifier — different identity.
            dependency_owned(Some("pkg:maven/g/a"), None, "req", "owner-1"),
        ];
        let index = build_version_index(&deps);
        assert_eq!(
            resolve_edge_purl(&deps[1], &index).as_deref(),
            Some("pkg:maven/g/a"),
            "a qualifier-bearing sibling must not version a bare requirement"
        );

        // And a subpath variant is likewise its own identity.
        let subpath_deps = vec![
            dependency_owned(Some("pkg:golang/ex.com/m@1.0.0#sub"), None, "v", "owner-1"),
            dependency_owned(Some("pkg:golang/ex.com/m"), None, "req", "owner-1"),
        ];
        let subpath_index = build_version_index(&subpath_deps);
        assert_eq!(
            resolve_edge_purl(&subpath_deps[1], &subpath_index).as_deref(),
            Some("pkg:golang/ex.com/m"),
            "a subpath-bearing sibling must not version a bare requirement"
        );
    }

    // Versioned-purl gap (issue #1320): the resolved version lives only on a
    // sibling lockfile edge. An unversioned requirement edge with no detected
    // package must still be promoted at the resolved coordinate and pick up the
    // license the sibling's resolved package carries.
    #[test]
    fn unversioned_requirement_coalesces_to_versioned_sibling_edge() {
        let mut resolved = ResolvedPackage::new(
            PackageType::Npm,
            String::new(),
            "left-pad".to_string(),
            "1.3.0".to_string(),
        );
        resolved.purl = Some("pkg:npm/left-pad@1.3.0".to_string());
        resolved.declared_license_expression_spdx = Some("MIT".to_string());

        let deps = vec![
            dependency(Some("pkg:npm/left-pad"), Some(false), None, "req"),
            dependency(
                Some("pkg:npm/left-pad@1.3.0"),
                Some(false),
                Some(resolved),
                "lock",
            ),
        ];
        let output = output_with(vec![], deps);
        let inventory = build_inventory(&output);

        assert_eq!(inventory.entries.len(), 1, "the two edges collapse to one");
        let package = inventory.entries[0].package();
        assert_eq!(package.purl.as_deref(), Some("pkg:npm/left-pad@1.3.0"));
        assert_eq!(package.version.as_deref(), Some("1.3.0"));
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("MIT")
        );
    }

    // Honest-unknowns: when an unversioned requirement could match more than one
    // resolved version, Provenant must not guess. The edge keeps its unversioned
    // purl and each resolved version remains its own component.
    #[test]
    fn ambiguous_versions_leave_unversioned_requirement_unresolved() {
        let deps = vec![
            dependency(Some("pkg:npm/multi"), Some(false), None, "req"),
            dependency(Some("pkg:npm/multi@1.0.0"), Some(false), None, "v1"),
            dependency(Some("pkg:npm/multi@2.0.0"), Some(false), None, "v2"),
        ];
        let output = output_with(vec![], deps);
        let inventory = build_inventory(&output);

        let purls: BTreeSet<&str> = inventory
            .packages()
            .filter_map(|pkg| pkg.purl.as_deref())
            .collect();
        assert_eq!(
            purls,
            BTreeSet::from([
                "pkg:npm/multi",
                "pkg:npm/multi@1.0.0",
                "pkg:npm/multi@2.0.0",
            ]),
            "an ambiguous unversioned requirement stays unversioned"
        );
    }
}
