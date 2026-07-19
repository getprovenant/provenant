// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Hackage cabal/stack assembly and directory-scoped project topology.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::models::{DatasourceId, FileInfo, Package, PackageData, PackageUid, TopLevelDependency};

use super::path_identity::scanned_file_dir;
use super::topology::apply_directory_merge_result;

pub(super) struct HackageProjectHint {
    pub(super) root_dir: PathBuf,
}

pub(super) struct HackageProjectDomain {
    pub(super) root_dir: PathBuf,
    pub(super) root_dir_file_indices: Vec<usize>,
}

struct HackageSource<'a> {
    file_index: usize,
    datafile_path: String,
    package_data: &'a PackageData,
}

pub(super) fn collect_hackage_project_hints(files: &[FileInfo]) -> Vec<HackageProjectHint> {
    let mut seen = HashSet::new();
    let mut hints = Vec::new();

    for file in files {
        let path = Path::new(&file.path);
        let file_name = path.file_name().and_then(|name| name.to_str());
        if !matches!(file_name, Some("cabal.project" | "stack.yaml")) {
            continue;
        }

        let has_project_surface = file.package_data.iter().any(|pkg_data| {
            matches!(
                pkg_data.datasource_id,
                Some(DatasourceId::HackageCabalProject | DatasourceId::HackageStackYaml)
            )
        });
        if !has_project_surface {
            continue;
        }

        let Some(root_dir) = scanned_file_dir(&file.path) else {
            continue;
        };
        if seen.insert(root_dir.clone()) {
            hints.push(HackageProjectHint { root_dir });
        }
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

pub(super) fn plan_hackage_project_domains(
    dir_files: &HashMap<PathBuf, Vec<usize>>,
    workspace_hints: &[&HackageProjectHint],
) -> Vec<HackageProjectDomain> {
    let mut domains = Vec::new();

    for hint in workspace_hints {
        let root_dir_file_indices = dir_files.get(&hint.root_dir).cloned().unwrap_or_default();
        if root_dir_file_indices.is_empty() {
            continue;
        }

        domains.push(HackageProjectDomain {
            root_dir: hint.root_dir.clone(),
            root_dir_file_indices,
        });
    }

    domains.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    domains
}

pub(super) fn apply_hackage_project_domain(
    domain: &HackageProjectDomain,
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let results = assemble_hackage_packages(files, &domain.root_dir_file_indices);
    for result in results {
        apply_directory_merge_result(files, packages, dependencies, result);
    }
}

pub fn assemble_hackage_packages(
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<(Option<Package>, Vec<TopLevelDependency>, Vec<usize>)> {
    let mut cabal_sources = Vec::new();
    let mut project_sources = Vec::new();

    for &file_index in file_indices {
        let file = &files[file_index];
        for package_data in &file.package_data {
            match package_data.datasource_id {
                Some(DatasourceId::HackageCabal) => cabal_sources.push(HackageSource {
                    file_index,
                    datafile_path: file.path.clone(),
                    package_data,
                }),
                Some(DatasourceId::HackageCabalProject | DatasourceId::HackageStackYaml) => {
                    project_sources.push(HackageSource {
                        file_index,
                        datafile_path: file.path.clone(),
                        package_data,
                    })
                }
                _ => {}
            }
        }
    }

    if cabal_sources.is_empty() {
        let dependencies = hoist_sources_without_package(&project_sources, None);
        return (!dependencies.is_empty())
            .then_some((None, dependencies, Vec::new()))
            .into_iter()
            .collect();
    }

    if cabal_sources.len() == 1 {
        let cabal = &cabal_sources[0];
        if cabal.package_data.purl.is_none() {
            let dependencies = hoist_sources_without_package(&project_sources, None);
            return (!dependencies.is_empty())
                .then_some((None, dependencies, Vec::new()))
                .into_iter()
                .collect();
        }

        let mut package =
            Package::from_package_data(cabal.package_data, cabal.datafile_path.clone());
        let mut assigned_indices = vec![cabal.file_index];

        for source in &project_sources {
            package.update(source.package_data, source.datafile_path.clone());
            assigned_indices.push(source.file_index);
        }

        assigned_indices.sort_unstable();
        assigned_indices.dedup();

        let dependencies = hoist_sources_with_package(
            cabal_sources.iter().chain(project_sources.iter()),
            Some(package.package_uid.clone()),
        );

        return vec![(Some(package), dependencies, assigned_indices)];
    }

    let mut results = Vec::new();

    for source in cabal_sources {
        if source.package_data.purl.is_none() {
            continue;
        }

        let package = Package::from_package_data(source.package_data, source.datafile_path.clone());
        let dependencies =
            hoist_sources_with_package(std::iter::once(&source), Some(package.package_uid.clone()));
        results.push((Some(package), dependencies, vec![source.file_index]));
    }

    let unowned_dependencies = hoist_sources_without_package(&project_sources, None);
    if !unowned_dependencies.is_empty() {
        results.push((None, unowned_dependencies, Vec::new()));
    }

    results
}

fn hoist_sources_with_package<'a>(
    sources: impl Iterator<Item = &'a HackageSource<'a>>,
    for_package_uid: Option<PackageUid>,
) -> Vec<TopLevelDependency> {
    sources
        .flat_map(|source| {
            source
                .package_data
                .dependencies
                .iter()
                .filter_map(|dependency| {
                    dependency.purl.as_ref().map(|_| {
                        TopLevelDependency::from_dependency(
                            dependency,
                            source.datafile_path.clone(),
                            source
                                .package_data
                                .datasource_id
                                .expect("hackage datasource id should be present"),
                            for_package_uid.clone(),
                        )
                    })
                })
        })
        .collect()
}

fn hoist_sources_without_package<'a>(
    sources: &'a [HackageSource<'a>],
    for_package_uid: Option<PackageUid>,
) -> Vec<TopLevelDependency> {
    hoist_sources_with_package(sources.iter(), for_package_uid)
}
