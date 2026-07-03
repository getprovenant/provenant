// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::project_dependency_assign::{ProjectFileAssignment, ProjectRoot, assign_project_files};
use crate::models::{DatasourceId, Dependency, FileInfo, Package, PackageType, TopLevelDependency};

const PYTHON_PROJECT_ROOT_FILENAMES: &[&str] = &[
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "Pipfile",
    "Pipfile.lock",
    "poetry.lock",
    "pylock.toml",
    "uv.lock",
];

pub fn assign_python_requirements_to_projects(
    files: &mut [FileInfo],
    packages: &mut [Package],
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let project_roots = collect_python_project_roots(packages);
    if project_roots.is_empty() {
        return;
    }

    assign_project_files(
        files,
        packages,
        dependencies,
        ProjectFileAssignment {
            datasource_id: DatasourceId::PipRequirements,
            project_roots: &project_roots,
            is_relevant_file: is_requirements_subdir_file,
            find_root: find_nearest_project_root,
            include_dependency: |dep: &Dependency| dep.purl.is_some(),
        },
    );
}

fn collect_python_project_roots(packages: &[Package]) -> Vec<ProjectRoot> {
    packages
        .iter()
        .enumerate()
        .filter(|(_, package)| package.package_type == Some(PackageType::Pypi))
        .filter_map(|(package_index, package)| {
            if package.package_uid.is_empty() {
                return None;
            }

            let root = package
                .datafile_paths
                .iter()
                .find(|path| is_python_project_root_path(path))
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

fn is_python_project_root_path(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| PYTHON_PROJECT_ROOT_FILENAMES.contains(&name))
}

fn is_requirements_subdir_file(file: &FileInfo) -> bool {
    if !file
        .package_data
        .iter()
        .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::PipRequirements))
    {
        return false;
    }

    let path = Path::new(&file.path);
    path.parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .filter_map(|ancestor| ancestor.file_name())
        .filter_map(|name| name.to_str())
        .any(|name| name == "requirements")
}

fn find_nearest_project_root<'a>(
    path: &Path,
    project_roots: &'a [ProjectRoot],
) -> Option<&'a ProjectRoot> {
    let mut current_dir = path.parent().and_then(|parent| parent.parent());

    while let Some(dir) = current_dir {
        let mut matches = project_roots.iter().filter(|root| root.root == dir);
        let first = matches.next();
        let second = matches.next();

        match (first, second) {
            (Some(root), None) => return Some(root),
            (Some(_), Some(_)) => return None,
            (None, _) => {
                current_dir = dir.parent();
            }
        }
    }

    None
}
