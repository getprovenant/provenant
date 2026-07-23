// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared SBOM inventory construction for the CycloneDX and SPDX renderers.
//!
//! Both renderers historically sourced their component/package inventory only
//! from top-level detected packages (`output.packages`), leaving the resolved
//! dependencies (`output.dependencies`) as graph edges that referenced nothing
//! — a dangling, incomplete BOM. This module promotes every resolved
//! dependency to a component so those edges resolve to real inventory entries.
//!
//! See ADR 0012 for the dedup, identity, license-honesty, and metadata-honesty
//! rules this module implements.

use std::collections::HashMap;
use std::str::FromStr;

use packageurl::PackageUrl;

use crate::output_schema::{
    OutputPackage, OutputResolvedPackage, OutputTopLevelDependency as TopLevelDependency,
};

/// A resolved dependency promoted to an SBOM component, paired with the honest
/// CycloneDX `scope` derived from proven dependency intent (`None` when intent
/// was not proven; SPDX ignores this).
pub(crate) struct PromotedComponent {
    pub package: OutputPackage,
    pub scope: Option<&'static str>,
}

/// The purl a dependency edge points at: the dependency's own purl, falling
/// back to its resolved package's purl. Empty purls are treated as absent.
pub(crate) fn dependency_edge_purl(dep: &TopLevelDependency) -> Option<String> {
    dep.purl
        .clone()
        .or_else(|| dep.resolved_package.as_ref().and_then(|rp| rp.purl.clone()))
        .filter(|purl| !purl.is_empty())
}

/// Promote resolved dependencies to components, deduped by purl.
///
/// A dependency whose purl a detected package already owns is skipped (the
/// package is the richer, file-backed representation). Dependencies that share
/// a purl collapse to one component whose `scope` merges their proven intent.
/// Order is stable: first appearance of each new purl wins its position.
pub(crate) fn promote_dependencies(
    packages: &[OutputPackage],
    dependencies: &[TopLevelDependency],
) -> Vec<PromotedComponent> {
    let existing_purls: HashMap<&str, ()> = packages
        .iter()
        .filter_map(|pkg| pkg.purl.as_deref().map(|purl| (purl, ())))
        .collect();

    let mut index_by_purl: HashMap<String, usize> = HashMap::new();
    let mut promoted: Vec<PromotedComponent> = Vec::new();

    for dep in dependencies {
        let Some(purl) = dependency_edge_purl(dep) else {
            continue;
        };
        if existing_purls.contains_key(purl.as_str()) {
            continue;
        }

        if let Some(&idx) = index_by_purl.get(&purl) {
            promoted[idx].scope = merge_scope(promoted[idx].scope, dep.is_optional);
            continue;
        }

        index_by_purl.insert(purl.clone(), promoted.len());
        promoted.push(PromotedComponent {
            package: synthesize_package(dep, &purl),
            scope: scope_from_optional(dep.is_optional),
        });
    }

    promoted
}

/// CycloneDX component `scope` from a proven `is_optional`. Unknown → omitted.
fn scope_from_optional(is_optional: Option<bool>) -> Option<&'static str> {
    match is_optional {
        Some(true) => Some("optional"),
        Some(false) => Some("required"),
        None => None,
    }
}

/// Merge a new occurrence's proven intent into an existing scope. A proven
/// `required` is the strongest claim and always wins; `optional` fills an
/// otherwise-unknown scope; unknown never overrides a known scope.
fn merge_scope(current: Option<&'static str>, is_optional: Option<bool>) -> Option<&'static str> {
    match (current, scope_from_optional(is_optional)) {
        (Some("required"), _) | (_, Some("required")) => Some("required"),
        (Some(existing), _) => Some(existing),
        (None, other) => other,
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

    #[test]
    fn skips_dependency_already_represented_by_a_detected_package() {
        let packages = vec![detected_package("pkg:npm/dep@1.0.0")];
        let deps = vec![dependency(
            Some("pkg:npm/dep@1.0.0"),
            Some(false),
            None,
            "d1",
        )];
        let promoted = promote_dependencies(&packages, &deps);
        assert!(
            promoted.is_empty(),
            "a dependency whose purl a package already owns must not double-count"
        );
    }

    #[test]
    fn collapses_shared_purls_and_merges_scope_to_required() {
        let deps = vec![
            dependency(Some("pkg:npm/dep@1.0.0"), Some(true), None, "d1"),
            dependency(Some("pkg:npm/dep@1.0.0"), Some(false), None, "d2"),
        ];
        let promoted = promote_dependencies(&[], &deps);
        assert_eq!(promoted.len(), 1, "shared purls collapse to one component");
        assert_eq!(
            promoted[0].scope,
            Some("required"),
            "a single proven-required occurrence wins the merged scope"
        );
    }

    #[test]
    fn bare_dependency_gets_identity_from_purl_and_no_license() {
        let deps = vec![dependency(Some("pkg:npm/bare@3.0.0"), None, None, "d1")];
        let promoted = promote_dependencies(&[], &deps);
        assert_eq!(promoted.len(), 1);
        let package = &promoted[0].package;
        assert_eq!(package.name.as_deref(), Some("bare"));
        assert_eq!(package.version.as_deref(), Some("3.0.0"));
        assert_eq!(package.purl.as_deref(), Some("pkg:npm/bare@3.0.0"));
        assert!(package.declared_license_expression_spdx.is_none());
        assert!(package.declared_license_expression.is_none());
        assert!(package.license_detections.is_empty());
        assert_eq!(
            promoted[0].scope, None,
            "unproven optionality leaves scope unset"
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
        let promoted = promote_dependencies(&[], &deps);
        assert_eq!(promoted.len(), 1);
        assert_eq!(
            promoted[0]
                .package
                .declared_license_expression_spdx
                .as_deref(),
            Some("MIT")
        );
    }

    #[test]
    fn skips_dependency_without_a_purl() {
        let deps = vec![dependency(None, Some(false), None, "d1")];
        assert!(promote_dependencies(&[], &deps).is_empty());
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
        let promoted = promote_dependencies(&[], &deps);
        assert_eq!(promoted.len(), 1);
        assert_eq!(
            promoted[0].package.purl.as_deref(),
            Some("pkg:npm/resolved-only@2.0.0")
        );
    }
}
