// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Bespoke CocoaPods directory assembly.
//!
//! A CocoaPods directory can hold several `.podspec` files plus a `Podfile` and
//! `Podfile.lock`. Unlike the generic per-identity split, the podspecs are not
//! peers: one is the *primary* package (the one no sibling podspec depends on)
//! that also absorbs the directory's `Podfile`/`Podfile.lock` metadata, while
//! each remaining podspec becomes its own package. A directory with a single
//! podspec falls back to the generic single-package
//! [`assemble_single_sibling_package`](super::sibling_merge::assemble_single_sibling_package).
//!
//! Attached to the CocoaPods [`AssemblerConfig`](super::AssemblerConfig) via its
//! [`directory_merger`](super::AssemblerConfig::directory_merger) field.

use std::collections::HashSet;
use std::path::Path;

use crate::models::{DatasourceId, FileInfo, Package};

use super::sibling_merge::{
    assemble_single_sibling_package, build_directory_merge_output, collect_pending_dependencies,
    is_handled_by, matches_pattern, should_skip_assembly_package_data,
};
use super::{AssemblerConfig, DirectoryMergeOutput};

#[derive(Clone, Copy)]
struct CocoapodsPodspecCandidate {
    file_idx: usize,
    package_data_idx: usize,
}

/// Assemble a CocoaPods directory. With two or more podspecs, emit one primary
/// package (carrying the `Podfile`/`Podfile.lock` siblings) plus one package per
/// remaining podspec; otherwise fall back to the generic single-package merge.
pub(super) fn assemble_cocoapods_packages(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    let podspec_candidates = collect_cocoapods_podspec_candidates(config, files, file_indices);
    if podspec_candidates.len() <= 1 {
        return assemble_single_sibling_package(config, files, file_indices)
            .into_iter()
            .collect();
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

            for pkg_data in &file.package_data {
                if !is_handled_by(pkg_data, config) {
                    continue;
                }

                let Some(datasource_id) = pkg_data.datasource_id else {
                    continue;
                };

                // Podspecs are handled as their own (primary or secondary)
                // packages, not merged into the primary like the Podfile siblings.
                if is_cocoapods_podspec_datasource(datasource_id) {
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

    results
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
