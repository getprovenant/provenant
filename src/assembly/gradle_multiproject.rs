// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Gradle multi-project topology: settings-driven roots, package materialization,
//! and nested file ownership.

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::models::{DatasourceId, FileInfo, Package, PackageData, TopLevelDependency};

use super::path_identity::{normalize_lexical_path, scanned_file_dir};
use super::topology::assign_unowned_files_to_anchors;

pub(super) struct GradleMultiProjectRootHint {
    pub(super) root_dir: PathBuf,
    pub(super) project_paths: Vec<String>,
    pub(super) root_project_name: Option<String>,
    /// Literal `project(...).projectDir` remaps parsed from the settings script,
    /// keyed by a project's default (include-derived) relative directory. A
    /// remapped project is resolved at its override directory instead of the
    /// default; see `src/parsers/gradle_settings.rs`.
    pub(super) project_dir_overrides: HashMap<String, String>,
}

pub(super) struct GradleMultiProjectDomain {
    pub(super) root_dir: PathBuf,
    pub(super) root_build_idx: Option<usize>,
    pub(super) root_project_name: Option<String>,
    pub(super) member_build_indices: Vec<usize>,
}

pub(super) fn collect_gradle_multi_project_hints(
    files: &[FileInfo],
) -> Vec<GradleMultiProjectRootHint> {
    let mut hints = Vec::new();

    for file in files {
        let path = Path::new(&file.path);
        if !matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("settings.gradle" | "settings.gradle.kts")
        ) {
            continue;
        }
        let Some(project_paths) = file.package_data.iter().find_map(|data| {
            (data.datasource_id == Some(DatasourceId::GradleSettings))
                .then_some(data.extra_data.as_ref())
                .flatten()
                .and_then(|extra| extra.get("projects"))
                .and_then(|projects| projects.as_array())
                .map(|projects| {
                    projects
                        .iter()
                        .filter_map(|project| project.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
        }) else {
            continue;
        };
        if project_paths.is_empty() {
            continue;
        }
        let Some(root_dir) = scanned_file_dir(&file.path) else {
            continue;
        };
        let root_project_name = file.package_data.iter().find_map(|data| {
            (data.datasource_id == Some(DatasourceId::GradleSettings))
                .then_some(data.extra_data.as_ref())
                .flatten()
                .and_then(|extra| extra.get("root_project_name"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
        let project_dir_overrides = file
            .package_data
            .iter()
            .find_map(|data| {
                (data.datasource_id == Some(DatasourceId::GradleSettings))
                    .then_some(data.extra_data.as_ref())
                    .flatten()
                    .and_then(|extra| extra.get("project_dir_overrides"))
                    .and_then(|value| value.as_object())
                    .map(|object| {
                        object
                            .iter()
                            .filter_map(|(key, value)| {
                                value.as_str().map(|value| (key.clone(), value.to_string()))
                            })
                            .collect::<HashMap<_, _>>()
                    })
            })
            .unwrap_or_default();
        hints.push(GradleMultiProjectRootHint {
            root_dir,
            project_paths,
            root_project_name,
            project_dir_overrides,
        });
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

pub(super) fn plan_gradle_multi_project_domains(
    files: &[FileInfo],
    hints: &[&GradleMultiProjectRootHint],
) -> Vec<GradleMultiProjectDomain> {
    let mut domains = Vec::new();

    for hint in hints {
        let root_build_idx = find_gradle_build_index(files, &hint.root_dir);
        let member_build_indices = hint
            .project_paths
            .iter()
            .filter_map(|project| {
                // A literal `projectDir` remap relocates the member's directory;
                // fall back to the include-derived path when none applies.
                let relative_dir = hint
                    .project_dir_overrides
                    .get(project)
                    .map(String::as_str)
                    .unwrap_or(project.as_str());
                let project_dir = normalize_lexical_path(&hint.root_dir.join(relative_dir));
                find_gradle_build_index(files, &project_dir)
            })
            .collect::<Vec<_>>();

        if root_build_idx.is_none() && member_build_indices.is_empty() {
            continue;
        }
        domains.push(GradleMultiProjectDomain {
            root_dir: hint.root_dir.clone(),
            root_build_idx,
            root_project_name: hint.root_project_name.clone(),
            member_build_indices,
        });
    }

    domains.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    domains
}

fn find_gradle_build_index(files: &[FileInfo], directory: &Path) -> Option<usize> {
    files.iter().position(|file| {
        let path = Path::new(&file.path);
        scanned_file_dir(&file.path).as_deref() == Some(directory)
            && matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("build.gradle" | "build.gradle.kts")
            )
            && file
                .package_data
                .iter()
                .any(|data| data.datasource_id == Some(DatasourceId::BuildGradle))
    })
}

pub(super) fn apply_gradle_multi_project_domains<'a>(
    domains: impl IntoIterator<Item = &'a GradleMultiProjectDomain>,
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let mut scope_roots = Vec::new();
    let mut anchor_indices = Vec::new();

    for domain in domains {
        scope_roots.push(normalize_lexical_path(&domain.root_dir));
        if let Some(root_idx) = domain.root_build_idx {
            ensure_gradle_package(
                root_idx,
                domain.root_project_name.as_deref(),
                files,
                packages,
                dependencies,
            );
            anchor_indices.push(root_idx);
        }
        for &member_idx in &domain.member_build_indices {
            if let Some(member_dir) = scanned_file_dir(&files[member_idx].path) {
                scope_roots.push(member_dir);
            }
            ensure_gradle_package(member_idx, None, files, packages, dependencies);
            anchor_indices.push(member_idx);
        }
    }

    assign_unowned_files_to_anchors(
        files,
        &scope_roots,
        &anchor_indices,
        &[OsStr::new("build")],
        &[],
    );
}

/// Materialize a package for a Gradle (sub)project from its `build.gradle`.
///
/// Gradle build scripts carry no package identity on their own — the project
/// *name* lives in `settings.gradle` (or defaults to the directory name) and the
/// Maven coordinates (`group`/`version`) are top-level statements the parser
/// stashes in `extra_data`. Assembly is the only layer that can combine these
/// cross-file facts, so the multi-project topology builds the package here rather
/// than in per-directory sibling merge (which is why Gradle build directories are
/// never added to a `claimed_*_dirs` set: ordinary merge still runs and only
/// hoists dependencies, which this function re-owns to the project package).
///
/// The purl is built as `pkg:maven/<group>/<name>@<version>` when a `group` is
/// declared; without a group the package keeps its name only (`purl: None`) —
/// an honest partial identity rather than a fabricated Maven coordinate.
fn ensure_gradle_package(
    build_idx: usize,
    name_override: Option<&str>,
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
) {
    if !files[build_idx].for_packages.is_empty() {
        return;
    }

    let Some(mut package_data) = files[build_idx]
        .package_data
        .iter()
        .find(|data| data.datasource_id == Some(DatasourceId::BuildGradle))
        .cloned()
    else {
        return;
    };
    let Some(build_dir) = scanned_file_dir(&files[build_idx].path) else {
        return;
    };

    let name = name_override
        .map(str::to_string)
        .or_else(|| package_data.name.clone())
        .or_else(|| {
            build_dir
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "gradle-root".to_string());
    package_data.name = Some(name.clone());

    let group = gradle_extra_string(&package_data, "group");
    let version = gradle_extra_string(&package_data, "version");
    if package_data.namespace.is_none() {
        package_data.namespace = group.clone();
    }
    if package_data.version.is_none() {
        package_data.version = version.clone();
    }
    // Build an honest Maven purl only when a group is present; a name-only
    // Gradle project stays purl-less rather than inventing a coordinate.
    if package_data.purl.is_none()
        && let Some(group) = group.as_deref()
        && let Ok(mut purl) = packageurl::PackageUrl::new("maven", name.as_str())
    {
        let _ = purl.with_namespace(group);
        if let Some(version) = package_data.version.as_deref() {
            let _ = purl.with_version(version);
        }
        package_data.purl = Some(purl.to_string());
    }

    let build_path = files[build_idx].path.clone();
    let mut package = Package::from_package_data(&package_data, build_path.clone());
    let mut datafile_indices = vec![build_idx];
    for (idx, file) in files.iter().enumerate() {
        let path = Path::new(&file.path);
        if scanned_file_dir(&file.path).as_deref() != Some(build_dir.as_path())
            || path.file_name().and_then(|name| name.to_str()) != Some("gradle.lockfile")
        {
            continue;
        }
        if let Some(lock_data) = file
            .package_data
            .iter()
            .find(|data| data.datasource_id == Some(DatasourceId::GradleLockfile))
        {
            package.update(lock_data, file.path.clone());
            datafile_indices.push(idx);
        }
    }

    let package_uid = package.package_uid.clone();
    let datafile_paths: HashSet<String> = datafile_indices
        .iter()
        .map(|idx| files[*idx].path.clone())
        .collect();
    dependencies.retain(|dependency| !datafile_paths.contains(&dependency.datafile_path));

    for idx in &datafile_indices {
        files[*idx].for_packages.push(package_uid.clone());
        for data in &files[*idx].package_data {
            let Some(datasource_id) = data.datasource_id else {
                continue;
            };
            if !matches!(
                datasource_id,
                DatasourceId::BuildGradle | DatasourceId::GradleLockfile
            ) {
                continue;
            }
            dependencies.extend(
                data.dependencies
                    .iter()
                    .filter(|dep| dep.purl.is_some())
                    .map(|dependency| {
                        TopLevelDependency::from_dependency(
                            dependency,
                            files[*idx].path.clone(),
                            datasource_id,
                            Some(package_uid.clone()),
                        )
                    }),
            );
        }
    }

    packages.push(package);
}

fn gradle_extra_string(data: &PackageData, key: &str) -> Option<String> {
    data.extra_data
        .as_ref()
        .and_then(|extra| extra.get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}
