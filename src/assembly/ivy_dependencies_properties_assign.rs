// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use crate::models::{DatasourceId, FileInfo, Package, PackageType, PackageUid, TopLevelDependency};

struct IvyProjectRoot {
    root: PathBuf,
    package_index: usize,
    package_uid: PackageUid,
}

pub fn assign_ivy_dependencies_properties_to_projects(
    files: &mut [FileInfo],
    packages: &mut [Package],
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let project_roots = collect_ivy_project_roots(packages);
    if project_roots.is_empty() {
        return;
    }

    for file in files.iter_mut() {
        if !is_ivy_dependencies_properties_file(file) || !file.for_packages.is_empty() {
            continue;
        }

        let path = Path::new(&file.path);
        let Some(project_root) = find_colocated_project_root(path, &project_roots) else {
            continue;
        };

        if !file.for_packages.contains(&project_root.package_uid) {
            file.for_packages.push(project_root.package_uid.clone());
        }

        let package = &mut packages[project_root.package_index];
        if !package.datafile_paths.contains(&file.path) {
            package.datafile_paths.push(file.path.clone());
        }
        if !package
            .datasource_ids
            .contains(&DatasourceId::AntIvyDependenciesProperties)
        {
            package
                .datasource_ids
                .push(DatasourceId::AntIvyDependenciesProperties);
        }

        for dependency in dependencies.iter_mut() {
            if dependency.datasource_id == DatasourceId::AntIvyDependenciesProperties
                && dependency.datafile_path == file.path
                && dependency.for_package_uid.is_none()
            {
                dependency.for_package_uid = Some(project_root.package_uid.clone());
            }
        }

        if dependencies.iter().any(|dependency| {
            dependency.datasource_id == DatasourceId::AntIvyDependenciesProperties
                && dependency.datafile_path == file.path
        }) {
            continue;
        }

        for pkg_data in &file.package_data {
            if pkg_data.datasource_id != Some(DatasourceId::AntIvyDependenciesProperties) {
                continue;
            }

            dependencies.extend(
                pkg_data
                    .dependencies
                    .iter()
                    .filter(|dep| dep.purl.is_some())
                    .map(|dep| {
                        TopLevelDependency::from_dependency(
                            dep,
                            file.path.clone(),
                            DatasourceId::AntIvyDependenciesProperties,
                            Some(project_root.package_uid.clone()),
                        )
                    }),
            );
        }
    }
}

fn collect_ivy_project_roots(packages: &[Package]) -> Vec<IvyProjectRoot> {
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

            Some(IvyProjectRoot {
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
    project_roots: &'a [IvyProjectRoot],
) -> Option<&'a IvyProjectRoot> {
    let parent = path.parent()?;
    let mut matches = project_roots.iter().filter(|root| root.root == parent);
    let first = matches.next();
    let second = matches.next();

    match (first, second) {
        (Some(root), None) => Some(root),
        _ => None,
    }
}
