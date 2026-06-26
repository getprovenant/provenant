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

#[derive(Clone, Copy)]
struct CocoapodsPodspecCandidate {
    file_idx: usize,
    package_data_idx: usize,
}

struct PendingDependency {
    dependency: crate::models::Dependency,
    datafile_path: String,
    datasource_id: DatasourceId,
}

/// Assemble a single package from sibling files in a directory.
///
/// Iterates over `sibling_file_patterns` in order, finds matching files among
/// `file_indices`, and merges their package data into a single `Package`.
/// Dependencies from all matched files are hoisted to the top level.
///
/// Returns `None` if no files with valid package data are found.
pub fn assemble_siblings(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    if let Some(results) = assemble_cocoapods_multiple_podspecs(config, files, file_indices) {
        return results;
    }

    if let Some(results) = assemble_maven_distinct_gav_packages(config, files, file_indices) {
        return results;
    }

    assemble_single_sibling_package(config, files, file_indices)
        .into_iter()
        .collect()
}

fn assemble_single_sibling_package(
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

/// Assemble a directory holding multiple INDEPENDENT Maven `.pom` files, each
/// with a distinct group:artifact:version (GAV) identity, into one package per
/// distinct GAV.
///
/// The Maven sibling assembler exists to fold a single module (one `pom.xml`
/// plus its sibling `pom.properties` / `META-INF/MANIFEST.MF`, all describing
/// the SAME package) into one package. But a directory can also hold many
/// standalone `.pom` fixtures or a local-repo layout where each `.pom` carries a
/// different GAV. The default single-package sibling merge would collapse all of
/// them into one top-level package; instead each distinct GAV must stay its own
/// package.
///
/// Identity is keyed off the package `purl` (the GAV proxy). Datafiles with no
/// purl (a `pom.properties` or `MANIFEST.MF` carrying only supplementary,
/// purl-less metadata) are not attached to any package in the multi-GAV path;
/// this function returns `None` (falling through to the default merge path) only
/// when fewer than two distinct Maven purls are present, so the single-module
/// case (one `pom.xml` + purl-less siblings) is byte-for-byte unchanged.
///
/// Multiple datafiles that share the SAME purl are still merged into one package
/// (their datafiles are all attached), so a duplicate-GAV datafile is never left
/// orphaned.
fn assemble_maven_distinct_gav_packages(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Option<Vec<DirectoryMergeOutput>> {
    if !is_maven_sibling_config(config) {
        return None;
    }

    // Collect every Maven datafile carrying a concrete purl identity, keyed by
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
        return None;
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

    Some(results)
}

/// Recognize the Maven/Java sibling assembler config. Both `MavenPom` and
/// `MavenPomProperties` are required so this stays bound to that one config even
/// if a future assembler reuses `MavenPom` alongside unrelated datasource IDs.
fn is_maven_sibling_config(config: &AssemblerConfig) -> bool {
    config.datasource_ids.contains(&DatasourceId::MavenPom)
        && config
            .datasource_ids
            .contains(&DatasourceId::MavenPomProperties)
}

fn assemble_cocoapods_multiple_podspecs(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Option<Vec<DirectoryMergeOutput>> {
    if !is_cocoapods_sibling_config(config) {
        return None;
    }

    let podspec_candidates = collect_cocoapods_podspec_candidates(config, files, file_indices);
    if podspec_candidates.len() <= 1 {
        return None;
    }

    let primary_position = choose_primary_cocoapods_podspec(files, &podspec_candidates);
    let primary_candidate = podspec_candidates[primary_position];
    let primary_pkg_data =
        &files[primary_candidate.file_idx].package_data[primary_candidate.package_data_idx];
    let primary_datafile_path = files[primary_candidate.file_idx].path.clone();
    let mut primary_package =
        Package::from_package_data(primary_pkg_data, primary_datafile_path.clone());
    let mut primary_pending_dependencies =
        collect_pending_dependencies(primary_pkg_data, &primary_datafile_path);
    let mut primary_affected_indices = vec![primary_candidate.file_idx];

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

            for (package_data_idx, pkg_data) in file.package_data.iter().enumerate() {
                if !is_handled_by(pkg_data, config) {
                    continue;
                }

                let Some(datasource_id) = pkg_data.datasource_id else {
                    continue;
                };

                if is_cocoapods_podspec_datasource(datasource_id) {
                    if idx == primary_candidate.file_idx
                        && package_data_idx == primary_candidate.package_data_idx
                    {
                        continue;
                    }

                    continue;
                }

                if should_skip_assembly_package_data(Some(&primary_package), pkg_data) {
                    continue;
                }

                let datafile_path = file.path.clone();
                file_used = true;
                primary_package.update(pkg_data, datafile_path.clone());
                primary_pending_dependencies
                    .extend(collect_pending_dependencies(pkg_data, &datafile_path));
            }

            if file_used {
                primary_affected_indices.push(idx);
            }
        }
    }

    primary_affected_indices.sort_unstable();
    primary_affected_indices.dedup();

    let mut results = vec![build_directory_merge_output(
        Some(primary_package),
        primary_pending_dependencies,
        primary_affected_indices,
    )];

    for (position, candidate) in podspec_candidates.into_iter().enumerate() {
        if position == primary_position {
            continue;
        }

        let pkg_data = &files[candidate.file_idx].package_data[candidate.package_data_idx];
        let datafile_path = files[candidate.file_idx].path.clone();
        let package = Package::from_package_data(pkg_data, datafile_path.clone());
        let pending_dependencies = collect_pending_dependencies(pkg_data, &datafile_path);

        results.push(build_directory_merge_output(
            Some(package),
            pending_dependencies,
            vec![candidate.file_idx],
        ));
    }

    Some(results)
}

fn collect_cocoapods_podspec_candidates(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<CocoapodsPodspecCandidate> {
    let mut candidates = Vec::new();

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

            for (package_data_idx, pkg_data) in file.package_data.iter().enumerate() {
                if !is_handled_by(pkg_data, config) {
                    continue;
                }

                let Some(datasource_id) = pkg_data.datasource_id else {
                    continue;
                };

                if !is_cocoapods_podspec_datasource(datasource_id) {
                    continue;
                }

                candidates.push(CocoapodsPodspecCandidate {
                    file_idx: idx,
                    package_data_idx,
                });
            }
        }
    }

    candidates
}

fn choose_primary_cocoapods_podspec(
    files: &[FileInfo],
    podspec_candidates: &[CocoapodsPodspecCandidate],
) -> usize {
    let sibling_names: HashSet<&str> = podspec_candidates
        .iter()
        .filter_map(|candidate| {
            files[candidate.file_idx].package_data[candidate.package_data_idx]
                .name
                .as_deref()
        })
        .collect();

    let referenced_sibling_names: HashSet<String> = podspec_candidates
        .iter()
        .flat_map(|candidate| {
            files[candidate.file_idx].package_data[candidate.package_data_idx]
                .dependencies
                .iter()
                .filter_map(|dependency| dependency.purl.as_deref())
                .filter_map(extract_cocoapods_name_from_purl)
                .filter(|name| sibling_names.contains(name.as_str()))
        })
        .collect();

    podspec_candidates
        .iter()
        .position(|candidate| {
            files[candidate.file_idx].package_data[candidate.package_data_idx]
                .name
                .as_deref()
                .is_some_and(|name| !referenced_sibling_names.contains(name))
        })
        .unwrap_or(0)
}

fn collect_pending_dependencies(
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

fn build_directory_merge_output(
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

fn is_cocoapods_sibling_config(config: &AssemblerConfig) -> bool {
    config
        .datasource_ids
        .contains(&DatasourceId::CocoapodsPodspec)
        && config
            .datasource_ids
            .contains(&DatasourceId::CocoapodsPodfile)
        && config
            .datasource_ids
            .contains(&DatasourceId::CocoapodsPodfileLock)
}

fn is_cocoapods_podspec_datasource(datasource_id: DatasourceId) -> bool {
    matches!(
        datasource_id,
        DatasourceId::CocoapodsPodspec | DatasourceId::CocoapodsPodspecJson
    )
}

fn extract_cocoapods_name_from_purl(purl: &str) -> Option<String> {
    let after_type = purl.strip_prefix("pkg:cocoapods/")?;
    let without_query = after_type.split('?').next().unwrap_or(after_type);
    let name_part = without_query.split('@').next().unwrap_or(without_query);
    Some(name_part.to_string())
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
fn is_handled_by(pkg_data: &PackageData, config: &AssemblerConfig) -> bool {
    pkg_data
        .datasource_id
        .is_some_and(|dsid| config.datasource_ids.contains(&dsid))
}

fn should_skip_assembly_package_data(package: Option<&Package>, pkg_data: &PackageData) -> bool {
    should_skip_composer_lock_virtual_package(pkg_data)
        || should_skip_placeholder_only_cocoapods_podspec(pkg_data)
        || package.is_some_and(|existing_package| {
            should_skip_npm_lock_merge(existing_package, pkg_data)
                || should_skip_bun_lock_merge(existing_package, pkg_data)
                || should_skip_python_uv_lock_merge(existing_package, pkg_data)
                || should_skip_python_pip_cache_merge(existing_package, pkg_data)
        })
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
