// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::project_dependency_assign::{ProjectFileAssignment, ProjectRoot, assign_project_files};
use crate::models::{DatasourceId, Dependency, FileInfo, Package, PackageType, TopLevelDependency};

pub fn assign_ivy_dependencies_properties_to_projects(
    files: &mut [FileInfo],
    packages: &mut [Package],
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let project_roots = collect_ivy_project_roots(packages);
    if project_roots.is_empty() {
        return;
    }

    assign_project_files(
        files,
        packages,
        dependencies,
        ProjectFileAssignment {
            datasource_id: DatasourceId::AntIvyDependenciesProperties,
            project_roots: &project_roots,
            is_relevant_file: is_ivy_dependencies_properties_file,
            find_root: find_colocated_project_root,
            // Keep any coordinate that names a dependency, matching the unowned
            // hoist path: `build_maven_dependency` always records the version as
            // an `extracted_requirement`, so an entry whose purl fails to build
            // is still attached rather than silently dropped.
            include_dependency: |dep: &Dependency| {
                dep.purl.is_some() || dep.extracted_requirement.is_some()
            },
        },
    );
}

fn collect_ivy_project_roots(packages: &[Package]) -> Vec<ProjectRoot> {
    packages
        .iter()
        .enumerate()
        .filter(|(_, package)| package.package_type == Some(PackageType::Ivy))
        .filter(|(_, package)| package.datasource_ids.contains(&DatasourceId::AntIvyXml))
        .filter_map(|(package_index, package)| {
            if package.package_uid.is_empty() {
                return None;
            }

            let root = package
                .datafile_paths
                .iter()
                .find(|path| is_ivy_xml_path(path))
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

fn is_ivy_xml_path(path: &str) -> bool {
    Path::new(path).file_name().and_then(|name| name.to_str()) == Some("ivy.xml")
}

fn is_ivy_dependencies_properties_file(file: &FileInfo) -> bool {
    file.package_data
        .iter()
        .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::AntIvyDependenciesProperties))
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
