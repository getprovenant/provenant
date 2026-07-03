// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared machinery for post-assembly passes that attach a supplementary
//! dependency file to the project package that owns its directory — for example
//! Python `requirements/*.txt` sub-files or a co-located Ant/Ivy
//! `dependencies.properties`. Each caller supplies how project roots are
//! collected, which files are relevant, how a file maps to a root, and which of
//! the file's dependencies are worth materializing; the bookkeeping is shared.

use std::path::{Path, PathBuf};

use crate::models::{DatasourceId, Dependency, FileInfo, Package, PackageUid, TopLevelDependency};

/// A candidate owning package, identified by the directory that roots its
/// project manifest.
pub(super) struct ProjectRoot {
    pub root: PathBuf,
    pub package_index: usize,
    pub package_uid: PackageUid,
}

/// The caller-specific strategy for a single assignment pass: which datasource
/// it owns, the candidate project roots, and the predicates that decide file
/// relevance, root selection, and which dependencies to materialize.
pub(super) struct ProjectFileAssignment<'roots, RelevantFn, FindFn, IncludeFn> {
    pub datasource_id: DatasourceId,
    pub project_roots: &'roots [ProjectRoot],
    pub is_relevant_file: RelevantFn,
    pub find_root: FindFn,
    pub include_dependency: IncludeFn,
}

/// Attach each relevant, still-unowned file to the project package returned by
/// `find_root`, recording the datafile/datasource on the package and assigning
/// the file's dependencies. Already-hoisted top-level entries for the file are
/// back-filled with the owning package; otherwise the dependencies are
/// materialized from the file's package data, keeping only entries that satisfy
/// `include_dependency`.
pub(super) fn assign_project_files<RelevantFn, FindFn, IncludeFn>(
    files: &mut [FileInfo],
    packages: &mut [Package],
    dependencies: &mut Vec<TopLevelDependency>,
    assignment: ProjectFileAssignment<'_, RelevantFn, FindFn, IncludeFn>,
) where
    RelevantFn: Fn(&FileInfo) -> bool,
    FindFn: for<'a> Fn(&Path, &'a [ProjectRoot]) -> Option<&'a ProjectRoot>,
    IncludeFn: Fn(&Dependency) -> bool,
{
    let ProjectFileAssignment {
        datasource_id,
        project_roots,
        is_relevant_file,
        find_root,
        include_dependency,
    } = assignment;

    for file in files.iter_mut() {
        if !is_relevant_file(file) || !file.for_packages.is_empty() {
            continue;
        }

        let Some(project_root) = find_root(Path::new(&file.path), project_roots) else {
            continue;
        };

        if !file.for_packages.contains(&project_root.package_uid) {
            file.for_packages.push(project_root.package_uid.clone());
        }

        let package = &mut packages[project_root.package_index];
        if !package.datafile_paths.contains(&file.path) {
            package.datafile_paths.push(file.path.clone());
        }
        if !package.datasource_ids.contains(&datasource_id) {
            package.datasource_ids.push(datasource_id);
        }

        // Back-fill any already-hoisted top-level dependencies for this file and
        // note whether the file has already been materialized.
        let mut already_materialized = false;
        for dependency in dependencies.iter_mut() {
            if dependency.datasource_id == datasource_id && dependency.datafile_path == file.path {
                already_materialized = true;
                if dependency.for_package_uid.is_none() {
                    dependency.for_package_uid = Some(project_root.package_uid.clone());
                }
            }
        }
        if already_materialized {
            continue;
        }

        for pkg_data in &file.package_data {
            if pkg_data.datasource_id != Some(datasource_id) {
                continue;
            }

            dependencies.extend(
                pkg_data
                    .dependencies
                    .iter()
                    .filter(|dep| include_dependency(dep))
                    .map(|dep| {
                        TopLevelDependency::from_dependency(
                            dep,
                            file.path.clone(),
                            datasource_id,
                            Some(project_root.package_uid.clone()),
                        )
                    }),
            );
        }
    }
}
