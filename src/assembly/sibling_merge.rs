// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::HashSet;
use std::path::Path;

use glob::Pattern;

use crate::models::{DatasourceId, FileInfo, Package, PackageData, TopLevelDependency};

use super::{
    AssemblerConfig, DirectoryMergeOutput, should_skip_placeholder_only_cocoapods_podspec,
};

pub(super) struct PendingDependency {
    dependency: crate::models::Dependency,
    datafile_path: String,
    datasource_id: DatasourceId,
}

/// Assemble a directory of sibling files into one package
/// ([`AssemblyMode::SiblingMerge`](super::AssemblyMode::SiblingMerge)).
///
/// Iterates over `sibling_file_patterns` in order, finds matching files among
/// `file_indices`, and merges their package data into a single `Package`.
/// Dependencies from all matched files are hoisted to the top level.
///
/// This is the generic directory engine — it carries no ecosystem-specific
/// branching. Layouts that need more than one package per directory opt into a
/// different strategy instead: [`assemble_siblings_per_identity`] for the generic
/// "one package per distinct identity" case
/// ([`AssemblyMode::SiblingMergePerIdentity`](super::AssemblyMode::SiblingMergePerIdentity)),
/// or a dedicated module attached through
/// [`AssemblerConfig::directory_merger`](super::AssemblerConfig::directory_merger)
/// for bespoke layouts (e.g. CocoaPods).
pub fn assemble_siblings(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    assemble_single_sibling_package(config, files, file_indices)
        .into_iter()
        .collect()
}

pub(super) fn assemble_single_sibling_package(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Option<DirectoryMergeOutput> {
    let mut package: Option<Package> = None;
    let mut pending_dependencies = Vec::new();
    let mut affected_indices = Vec::new();
    let mut saw_unpackageable_npm_manifest = false;

    for &pattern in config.sibling_file_patterns {
        for &idx in file_indices {
            let file = &files[idx];
            let file_name = Path::new(&file.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if !matches_pattern(file_name, pattern) {
                continue;
            }

            if file.package_data.is_empty() {
                continue;
            }

            let mut file_used = false;

            for pkg_data in &file.package_data {
                if !is_handled_by(pkg_data, config) {
                    continue;
                }

                if pkg_data.datasource_id == Some(DatasourceId::NpmPackageJson)
                    && pkg_data.purl.is_none()
                {
                    saw_unpackageable_npm_manifest = true;
                }

                if should_skip_assembly_package_data(package.as_ref(), pkg_data) {
                    continue;
                }

                let datafile_path = file.path.clone();
                let Some(datasource_id) = pkg_data.datasource_id else {
                    continue;
                };
                file_used = true;

                match &mut package {
                    None => {
                        if (pkg_data.purl.is_some() || has_assemblable_identity(pkg_data))
                            && !should_skip_npm_lock_package_creation(
                                pkg_data,
                                saw_unpackageable_npm_manifest,
                            )
                        {
                            package =
                                Some(Package::from_package_data(pkg_data, datafile_path.clone()));
                        }
                    }
                    Some(pkg) => {
                        pkg.update(pkg_data, datafile_path.clone());
                    }
                }

                for dep in &pkg_data.dependencies {
                    if dep.purl.is_some() {
                        pending_dependencies.push(PendingDependency {
                            dependency: dep.clone(),
                            datafile_path: datafile_path.clone(),
                            datasource_id,
                        });
                    }
                }
            }

            if file_used {
                affected_indices.push(idx);
            }
        }
    }

    let for_package_uid = package.as_ref().map(|p| p.package_uid.clone());
    let dependencies: Vec<TopLevelDependency> = pending_dependencies
        .into_iter()
        .map(|pending| {
            TopLevelDependency::from_dependency(
                &pending.dependency,
                pending.datafile_path,
                pending.datasource_id,
                for_package_uid.clone(),
            )
        })
        .collect();

    if package.is_some() || !dependencies.is_empty() {
        Some((package, dependencies, affected_indices))
    } else {
        None
    }
}

/// Assemble a directory whose manifests carry multiple distinct package
/// identities into one package per identity
/// ([`AssemblyMode::SiblingMergePerIdentity`](super::AssemblyMode::SiblingMergePerIdentity)).
///
/// The default [`assemble_siblings`] engine folds a directory into a single
/// package, which is correct for one module (e.g. a `pom.xml` plus its
/// purl-less `pom.properties` / `META-INF/MANIFEST.MF` siblings). But a
/// directory can also hold several independent manifests — a flat set of
/// standalone Maven `.pom` fixtures, or a local-repository layout where each
/// `.pom` carries a different group:artifact:version. Collapsing those into one
/// package loses identities, so each distinct identity stays its own package.
///
/// Identity is keyed off the package `purl`. When fewer than two distinct purls
/// are present this falls back to the [`assemble_siblings`] single-package
/// result unchanged, so the ordinary one-module directory (one purled manifest
/// plus purl-less supplementary siblings) is byte-for-byte identical. Datafiles
/// sharing the same purl merge into one package (all datafiles attached, so no
/// duplicate-identity file is orphaned); purl-less supplementary files in a
/// genuinely multi-identity directory are ambiguous and are not attached to any
/// package.
pub(super) fn assemble_siblings_per_identity(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    // Collect every handled datafile carrying a concrete purl identity, keyed by
    // (file_idx, package_data_idx). Distinct purls mean independent packages.
    let mut purled: Vec<(usize, usize, &str)> = Vec::new();
    for &idx in file_indices {
        for (pkg_data_idx, pkg_data) in files[idx].package_data.iter().enumerate() {
            if !is_handled_by(pkg_data, config) {
                continue;
            }
            if let Some(purl) = pkg_data.purl.as_deref() {
                purled.push((idx, pkg_data_idx, purl));
            }
        }
    }

    let distinct_purls: HashSet<&str> = purled.iter().map(|(_, _, purl)| *purl).collect();
    if distinct_purls.len() < 2 {
        // Zero or one distinct identity: the default single-package merge already
        // produces the correct result (and keeps supplementary purl-less files
        // merged into the one module package).
        return assemble_single_sibling_package(config, files, file_indices)
            .into_iter()
            .collect();
    }

    // Group datafiles by purl, preserving first-seen order for deterministic
    // output. Datafiles that share a purl merge into the same package so none is
    // left orphaned; in the standalone-`.pom` layouts this guards against each
    // group is typically a single file.
    let mut purl_order: Vec<&str> = Vec::new();
    let mut groups: std::collections::HashMap<&str, Vec<(usize, usize)>> =
        std::collections::HashMap::new();
    for (idx, pkg_data_idx, purl) in &purled {
        groups.entry(purl).or_insert_with(|| {
            purl_order.push(purl);
            Vec::new()
        });
        if let Some(group) = groups.get_mut(purl) {
            group.push((*idx, *pkg_data_idx));
        }
    }

    let mut results = Vec::new();
    for purl in purl_order {
        let group = &groups[purl];

        let mut package: Option<Package> = None;
        let mut pending_dependencies: Vec<PendingDependency> = Vec::new();
        let mut affected_indices: Vec<usize> = Vec::new();

        for &(idx, pkg_data_idx) in group {
            let pkg_data = &files[idx].package_data[pkg_data_idx];
            let datafile_path = files[idx].path.clone();

            match &mut package {
                None => {
                    package = Some(Package::from_package_data(pkg_data, datafile_path.clone()));
                }
                Some(pkg) => pkg.update(pkg_data, datafile_path.clone()),
            }

            // Mirror the default Maven sibling path: only hoist dependencies that
            // carry a resolvable purl.
            if let Some(datasource_id) = pkg_data.datasource_id {
                pending_dependencies.extend(
                    pkg_data
                        .dependencies
                        .iter()
                        .filter(|dep| dep.purl.is_some())
                        .map(|dep| PendingDependency {
                            dependency: dep.clone(),
                            datafile_path: datafile_path.clone(),
                            datasource_id,
                        }),
                );
            }

            if !affected_indices.contains(&idx) {
                affected_indices.push(idx);
            }
        }

        results.push(build_directory_merge_output(
            package,
            pending_dependencies,
            affected_indices,
        ));
    }

    // Handled files with no purl cannot join a specific identity in a
    // multi-identity directory, but their dependencies are still hoisted
    // (unowned) rather than dropped — the same visibility they had before the
    // directory was split per identity.
    let mut orphan_pending: Vec<PendingDependency> = Vec::new();
    for &idx in file_indices {
        for pkg_data in &files[idx].package_data {
            if !is_handled_by(pkg_data, config) || pkg_data.purl.is_some() {
                continue;
            }
            orphan_pending.extend(collect_pending_dependencies(pkg_data, &files[idx].path));
        }
    }
    if !orphan_pending.is_empty() {
        results.push(build_directory_merge_output(
            None,
            orphan_pending,
            Vec::new(),
        ));
    }

    results
}

pub(super) fn collect_pending_dependencies(
    pkg_data: &PackageData,
    datafile_path: &str,
) -> Vec<PendingDependency> {
    let Some(datasource_id) = pkg_data.datasource_id else {
        return Vec::new();
    };

    pkg_data
        .dependencies
        .iter()
        .filter(|dep| dep.purl.is_some() || dep.extracted_requirement.is_some())
        .cloned()
        .map(|dependency| PendingDependency {
            dependency,
            datafile_path: datafile_path.to_string(),
            datasource_id,
        })
        .collect()
}

pub(super) fn build_directory_merge_output(
    package: Option<Package>,
    pending_dependencies: Vec<PendingDependency>,
    affected_indices: Vec<usize>,
) -> DirectoryMergeOutput {
    let for_package_uid = package.as_ref().map(|p| p.package_uid.clone());
    let dependencies = pending_dependencies
        .into_iter()
        .map(|pending| {
            TopLevelDependency::from_dependency(
                &pending.dependency,
                pending.datafile_path,
                pending.datasource_id,
                for_package_uid.clone(),
            )
        })
        .collect();

    (package, dependencies, affected_indices)
}

/// Check if a filename matches a pattern. Supports:
/// - Exact match (e.g., "package.json")
/// - Case-insensitive match (e.g., "Cargo.toml" vs "cargo.toml")
/// - Glob-style prefix wildcard (e.g., "*.podspec" matches "MyLib.podspec")
pub(crate) fn matches_pattern(file_name: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        if let Ok(glob_pattern) = Pattern::new(pattern)
            && glob_pattern.matches(file_name)
        {
            return true;
        }

        let lower_name = file_name.to_ascii_lowercase();
        let lower_pattern = pattern.to_ascii_lowercase();
        if let Ok(glob_pattern) = Pattern::new(&lower_pattern) {
            return glob_pattern.matches(&lower_name);
        }

        false
    } else {
        file_name == pattern || file_name.eq_ignore_ascii_case(pattern)
    }
}

/// Check if a PackageData's datasource_id is handled by this assembler config.
pub(super) fn is_handled_by(pkg_data: &PackageData, config: &AssemblerConfig) -> bool {
    pkg_data
        .datasource_id
        .is_some_and(|dsid| config.datasource_ids.contains(&dsid))
}

/// Decides whether a candidate `PackageData` must NOT be folded into the
/// directory's package, keyed by the candidate's own `DatasourceId`. This is the
/// per-record merge-compatibility analog of
/// [`AssemblerConfig::directory_merger`](super::AssemblerConfig::directory_merger):
/// a new ecosystem registers one [`merge_skip_rule_for`] arm rather than
/// extending a hand-maintained boolean chain in the generic merger.
pub(super) fn should_skip_assembly_package_data(
    package: Option<&Package>,
    pkg_data: &PackageData,
) -> bool {
    pkg_data
        .datasource_id
        .and_then(merge_skip_rule_for)
        .is_some_and(|rule| rule(package, pkg_data))
}

/// A merge-skip rule, evaluated against the already-merged directory package (if
/// any) and the candidate. Rules whose decision does not depend on the existing
/// package simply ignore it.
type MergeSkipRule = fn(Option<&Package>, &PackageData) -> bool;

/// Registry mapping a candidate's `DatasourceId` to its merge-skip rule, or
/// `None` when that datasource has no merge-compatibility constraint.
fn merge_skip_rule_for(datasource_id: DatasourceId) -> Option<MergeSkipRule> {
    match datasource_id {
        DatasourceId::PhpComposerLock => Some(skip_composer_lock_virtual_package),
        DatasourceId::CocoapodsPodspec => Some(skip_placeholder_only_cocoapods_podspec),
        DatasourceId::NpmPackageLockJson => Some(skip_npm_lock_identity_mismatch),
        DatasourceId::BunLock | DatasourceId::BunLockb => Some(skip_bun_lock_identity_mismatch),
        DatasourceId::PypiUvLock => Some(skip_python_uv_lock_identity_mismatch),
        DatasourceId::PypiWheel | DatasourceId::PypiPipOriginJson => {
            Some(skip_python_pip_cache_identity_mismatch)
        }
        _ => None,
    }
}

// Adapters give every rule the uniform `MergeSkipRule` signature and encode
// whether the rule needs an existing package to compare against. The underlying
// predicates below carry the actual logic.
fn skip_composer_lock_virtual_package(_package: Option<&Package>, pkg_data: &PackageData) -> bool {
    should_skip_composer_lock_virtual_package(pkg_data)
}

fn skip_placeholder_only_cocoapods_podspec(
    _package: Option<&Package>,
    pkg_data: &PackageData,
) -> bool {
    should_skip_placeholder_only_cocoapods_podspec(pkg_data)
}

fn skip_npm_lock_identity_mismatch(package: Option<&Package>, pkg_data: &PackageData) -> bool {
    package.is_some_and(|existing| should_skip_npm_lock_merge(existing, pkg_data))
}

fn skip_bun_lock_identity_mismatch(package: Option<&Package>, pkg_data: &PackageData) -> bool {
    package.is_some_and(|existing| should_skip_bun_lock_merge(existing, pkg_data))
}

fn skip_python_uv_lock_identity_mismatch(
    package: Option<&Package>,
    pkg_data: &PackageData,
) -> bool {
    package.is_some_and(|existing| should_skip_python_uv_lock_merge(existing, pkg_data))
}

fn skip_python_pip_cache_identity_mismatch(
    package: Option<&Package>,
    pkg_data: &PackageData,
) -> bool {
    package.is_some_and(|existing| should_skip_python_pip_cache_merge(existing, pkg_data))
}

fn should_skip_composer_lock_virtual_package(pkg_data: &PackageData) -> bool {
    pkg_data.datasource_id == Some(DatasourceId::PhpComposerLock)
        && pkg_data.is_virtual
        && pkg_data.purl.is_some()
}

fn should_skip_npm_lock_merge(package: &Package, pkg_data: &PackageData) -> bool {
    pkg_data.datasource_id == Some(DatasourceId::NpmPackageLockJson)
        && !npm_package_identity_matches(package, pkg_data)
}

fn should_skip_bun_lock_merge(package: &Package, pkg_data: &PackageData) -> bool {
    pkg_data
        .datasource_id
        .is_some_and(|id| matches!(id, DatasourceId::BunLock | DatasourceId::BunLockb))
        && !npm_package_identity_matches(package, pkg_data)
}

fn npm_package_identity_matches(package: &Package, pkg_data: &PackageData) -> bool {
    if let (Some(package_name), Some(candidate_name)) = (
        normalized_identity_value(package.name.as_deref()),
        normalized_identity_value(pkg_data.name.as_deref()),
    ) && package_name != candidate_name
    {
        return false;
    }

    if let (Some(package_version), Some(candidate_version)) = (
        normalized_identity_value(package.version.as_deref()),
        normalized_identity_value(pkg_data.version.as_deref()),
    ) && package_version != candidate_version
    {
        return false;
    }

    normalized_identity_value(package.name.as_deref()).is_some()
        && normalized_identity_value(pkg_data.name.as_deref()).is_some()
}

fn normalized_identity_value(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn has_assemblable_identity(pkg_data: &PackageData) -> bool {
    let has_name = normalized_identity_value(pkg_data.name.as_deref()).is_some();
    has_name
        && (pkg_data.package_type.is_some()
            || pkg_data.datasource_id == Some(DatasourceId::BuckMetadata))
}

fn should_skip_python_uv_lock_merge(package: &Package, pkg_data: &PackageData) -> bool {
    pkg_data.datasource_id == Some(DatasourceId::PypiUvLock)
        && package.datasource_ids.iter().any(|id| {
            matches!(
                id,
                DatasourceId::PypiPyprojectToml | DatasourceId::PypiPoetryPyprojectToml
            )
        })
        && !python_uv_identity_matches(package, pkg_data)
}

fn should_skip_python_pip_cache_merge(package: &Package, pkg_data: &PackageData) -> bool {
    pkg_data.datasource_id.is_some_and(|dsid| {
        matches!(
            dsid,
            DatasourceId::PypiWheel | DatasourceId::PypiPipOriginJson
        )
    }) && package.datasource_ids.iter().any(|dsid| {
        matches!(
            dsid,
            DatasourceId::PypiWheel | DatasourceId::PypiPipOriginJson
        )
    }) && !python_uv_identity_matches(package, pkg_data)
}

fn python_uv_identity_matches(package: &Package, pkg_data: &PackageData) -> bool {
    if let (Some(package_name), Some(candidate_name)) = (
        normalized_identity_value(package.name.as_deref()),
        normalized_identity_value(pkg_data.name.as_deref()),
    ) && package_name != candidate_name
    {
        return false;
    }

    if let (Some(package_version), Some(candidate_version)) = (
        normalized_identity_value(package.version.as_deref()),
        normalized_identity_value(pkg_data.version.as_deref()),
    ) && package_version != candidate_version
    {
        return false;
    }

    true
}

fn should_skip_npm_lock_package_creation(
    pkg_data: &PackageData,
    saw_unpackageable_npm_manifest: bool,
) -> bool {
    saw_unpackageable_npm_manifest
        && pkg_data.datasource_id == Some(DatasourceId::NpmPackageLockJson)
}
