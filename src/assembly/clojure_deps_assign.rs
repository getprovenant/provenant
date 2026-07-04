// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::project_dependency_assign::{ProjectFileAssignment, ProjectRoot, assign_project_files};
use crate::models::{DatasourceId, Dependency, FileInfo, Package, PackageType, TopLevelDependency};

pub fn assign_clojure_deps_edn_to_projects(
    files: &mut [FileInfo],
    packages: &mut [Package],
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let project_roots = collect_clojure_project_roots(packages);
    if project_roots.is_empty() {
        return;
    }

    assign_project_files(
        files,
        packages,
        dependencies,
        ProjectFileAssignment {
            datasource_id: DatasourceId::ClojureDepsEdn,
            project_roots: &project_roots,
            is_relevant_file: is_clojure_deps_edn_file,
            find_root: find_colocated_project_root,
            include_dependency: |_dep: &Dependency| true,
        },
    );
}

fn collect_clojure_project_roots(packages: &[Package]) -> Vec<ProjectRoot> {
    packages
        .iter()
        .enumerate()
        .filter(|(_, package)| package.package_type == Some(PackageType::Maven))
        .filter(|(_, package)| {
            package
                .datasource_ids
                .contains(&DatasourceId::ClojureProjectClj)
        })
        .filter_map(|(package_index, package)| {
            if package.package_uid.is_empty() {
                return None;
            }

            let root = package
                .datafile_paths
                .iter()
                .find(|path| is_project_clj_path(path))
                .and_then(|path| Path::new(path).parent())?
                .to_path_buf();

            Some(ProjectRoot {
                root,
                package_index,
                package_uid: package.package_uid.clone(),
            })
        })
        .collect()
}

fn is_project_clj_path(path: &str) -> bool {
    Path::new(path).file_name().and_then(|name| name.to_str()) == Some("project.clj")
}

fn is_clojure_deps_edn_file(file: &FileInfo) -> bool {
    file.package_data
        .iter()
        .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::ClojureDepsEdn))
}

fn find_colocated_project_root<'a>(
    path: &Path,
    project_roots: &'a [ProjectRoot],
) -> Option<&'a ProjectRoot> {
    let parent = path.parent()?;
    let mut matches = project_roots.iter().filter(|root| root.root == parent);
    let first = matches.next();
    let second = matches.next();

    match (first, second) {
        (Some(root), None) => Some(root),
        _ => None,
    }
}
