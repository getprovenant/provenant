// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Dart pub workspace assembly.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use packageurl::PackageUrl;

use crate::models::{DatasourceId, FileInfo, Package, PackageData, PackageUid, TopLevelDependency};

use super::topology::normalize_lexical_path;
use super::{ASSEMBLERS, AssemblerConfig, sibling_merge};

pub(super) struct DartWorkspaceRootHint {
    pub(super) root_dir: PathBuf,
    pub(super) root_pubspec_idx: usize,
    pub(super) member_paths: Vec<String>,
}

pub(super) struct DartWorkspaceMemberDomain {
    pub(super) pubspec_idx: usize,
    pub(super) dir_path: PathBuf,
    pub(super) dir_file_indices: Vec<usize>,
}

pub(super) struct DartWorkspaceDomain {
    pub(super) root_dir: PathBuf,
    pub(super) root_pubspec_idx: usize,
    pub(super) root_lock_idx: Option<usize>,
    pub(super) members: Vec<DartWorkspaceMemberDomain>,
}

struct Candidate {
    manifest_idx: usize,
    package_uid: PackageUid,
    direct_dependency_names: HashSet<String>,
}

pub(super) fn collect_dart_workspace_hints(files: &[FileInfo]) -> Vec<DartWorkspaceRootHint> {
    let mut hints = Vec::new();
    for (idx, file) in files.iter().enumerate() {
        let path = Path::new(&file.path);
        if path.file_name().and_then(|name| name.to_str()) != Some("pubspec.yaml") {
            continue;
        }
        for data in &file.package_data {
            if data.datasource_id != Some(DatasourceId::PubspecYaml) {
                continue;
            }
            let member_paths = data
                .extra_data
                .as_ref()
                .and_then(|extra| extra.get("workspace_members"))
                .and_then(|members| members.as_array())
                .into_iter()
                .flatten()
                .filter_map(|member| member.as_str().map(str::to_string))
                .collect::<Vec<_>>();
            if member_paths.is_empty() {
                continue;
            }
            let Some(root_dir) = path.parent() else {
                continue;
            };
            hints.push(DartWorkspaceRootHint {
                root_dir: root_dir.to_path_buf(),
                root_pubspec_idx: idx,
                member_paths,
            });
        }
    }
    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

pub(super) fn plan_dart_workspace_domains(
    files: &[FileInfo],
    hints: &[&DartWorkspaceRootHint],
) -> Vec<DartWorkspaceDomain> {
    let mut domains = Vec::new();
    for hint in hints {
        let mut members = hint
            .member_paths
            .iter()
            .filter_map(|member| {
                let dir_path = normalize_lexical_path(&hint.root_dir.join(member));
                let manifest_path = dir_path.join("pubspec.yaml");
                let pubspec_idx = files.iter().position(|file| {
                    Path::new(&file.path) == manifest_path
                        && file.package_data.iter().any(|data| {
                            data.datasource_id == Some(DatasourceId::PubspecYaml)
                                && data.purl.is_some()
                        })
                })?;
                let dir_file_indices = files
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, file)| {
                        (Path::new(&file.path).parent() == Some(dir_path.as_path())).then_some(idx)
                    })
                    .collect();
                Some(DartWorkspaceMemberDomain {
                    pubspec_idx,
                    dir_path,
                    dir_file_indices,
                })
            })
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.dir_path.cmp(&right.dir_path));
        members.dedup_by_key(|member| member.pubspec_idx);
        if members.is_empty() {
            continue;
        }
        domains.push(DartWorkspaceDomain {
            root_dir: hint.root_dir.clone(),
            root_pubspec_idx: hint.root_pubspec_idx,
            root_lock_idx: find_root_lock(files, &hint.root_dir),
            members,
        });
    }
    domains.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    domains
}

pub(super) fn apply_dart_workspace_domain(
    domain: &DartWorkspaceDomain,
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let mut candidates = Vec::new();
    let mut all_uids = Vec::new();

    if let Some(root_data) = pubspec_data(&files[domain.root_pubspec_idx]).cloned()
        && root_data.purl.is_some()
    {
        let package =
            Package::from_package_data(&root_data, files[domain.root_pubspec_idx].path.clone());
        let uid = package.package_uid.clone();
        dependencies.extend(
            root_data
                .dependencies
                .iter()
                .filter(|dep| dep.purl.is_some())
                .map(|dependency| {
                    TopLevelDependency::from_dependency(
                        dependency,
                        files[domain.root_pubspec_idx].path.clone(),
                        DatasourceId::PubspecYaml,
                        Some(uid.clone()),
                    )
                }),
        );
        files[domain.root_pubspec_idx]
            .for_packages
            .push(uid.clone());
        candidates.push(Candidate {
            manifest_idx: domain.root_pubspec_idx,
            package_uid: uid.clone(),
            direct_dependency_names: direct_dependency_names(&root_data),
        });
        all_uids.push(uid);
        packages.push(package);
    }

    for member in &domain.members {
        let Some((package, member_dependencies, affected_indices)) =
            sibling_merge::assemble_siblings(
                dart_assembler_config(),
                files,
                &member.dir_file_indices,
            )
            .into_iter()
            .next()
        else {
            continue;
        };
        let Some(package) = package else {
            dependencies.extend(member_dependencies);
            continue;
        };
        let uid = package.package_uid.clone();
        for idx in affected_indices {
            files[idx].for_packages.push(uid.clone());
        }
        dependencies.extend(member_dependencies);
        if let Some(member_data) = pubspec_data(&files[member.pubspec_idx]) {
            candidates.push(Candidate {
                manifest_idx: member.pubspec_idx,
                package_uid: uid.clone(),
                direct_dependency_names: direct_dependency_names(member_data),
            });
        }
        all_uids.push(uid);
        packages.push(package);
    }

    if let Some(lock_idx) = domain.root_lock_idx {
        attribute_shared_lock(lock_idx, files, dependencies, &candidates);
        files[lock_idx]
            .for_packages
            .extend(all_uids.iter().cloned());
    }

    let root_uid = candidates
        .iter()
        .find(|candidate| candidate.manifest_idx == domain.root_pubspec_idx)
        .map(|candidate| candidate.package_uid.clone());
    for file in files.iter_mut() {
        if !file.for_packages.is_empty() {
            continue;
        }
        let Some(file_dir) = Path::new(&file.path).parent() else {
            continue;
        };
        if let Some(member) = domain
            .members
            .iter()
            .filter(|member| file_dir.starts_with(&member.dir_path))
            .max_by_key(|member| member.dir_path.as_os_str().len())
            && let Some(candidate) = candidates
                .iter()
                .find(|candidate| candidate.manifest_idx == member.pubspec_idx)
        {
            file.for_packages.push(candidate.package_uid.clone());
            continue;
        }
        // Root-level files are attributed only when the root `pubspec.yaml`
        // is itself a real package. A workspace-only root (no package
        // identity, e.g. `publish_to: none` with no name) leaves its files
        // unowned rather than over-claiming them into every member package,
        // which would pollute per-package file and license attribution.
        if file_dir.starts_with(&domain.root_dir)
            && let Some(root_uid) = &root_uid
        {
            file.for_packages.push(root_uid.clone());
        }
    }
}

fn attribute_shared_lock(
    lock_idx: usize,
    files: &[FileInfo],
    dependencies: &mut Vec<TopLevelDependency>,
    candidates: &[Candidate],
) {
    for lock_data in &files[lock_idx].package_data {
        if lock_data.datasource_id != Some(DatasourceId::PubspecLock) {
            continue;
        }
        for dependency in lock_data
            .dependencies
            .iter()
            .filter(|dep| dep.purl.is_some())
        {
            let name = dependency.purl.as_deref().and_then(purl_name);
            let owners = candidates.iter().filter(|candidate| {
                name.as_ref()
                    .is_some_and(|name| candidate.direct_dependency_names.contains(name))
            });
            let mut attributed = false;
            for owner in owners {
                attributed = true;
                dependencies.push(TopLevelDependency::from_dependency(
                    dependency,
                    files[lock_idx].path.clone(),
                    DatasourceId::PubspecLock,
                    Some(owner.package_uid.clone()),
                ));
            }
            if !attributed {
                dependencies.push(TopLevelDependency::from_dependency(
                    dependency,
                    files[lock_idx].path.clone(),
                    DatasourceId::PubspecLock,
                    None,
                ));
            }
        }
    }
}

fn direct_dependency_names(data: &PackageData) -> HashSet<String> {
    data.dependencies
        .iter()
        .filter(|dependency| dependency.is_direct == Some(true))
        .filter_map(|dependency| dependency.purl.as_deref().and_then(purl_name))
        .collect()
}

fn purl_name(purl: &str) -> Option<String> {
    PackageUrl::from_str(purl)
        .ok()
        .map(|purl| purl.name().to_string())
}

fn pubspec_data(file: &FileInfo) -> Option<&PackageData> {
    file.package_data
        .iter()
        .find(|data| data.datasource_id == Some(DatasourceId::PubspecYaml))
}

fn find_root_lock(files: &[FileInfo], root_dir: &Path) -> Option<usize> {
    files.iter().position(|file| {
        let path = Path::new(&file.path);
        path.parent() == Some(root_dir)
            && path.file_name().and_then(|name| name.to_str()) == Some("pubspec.lock")
            && file
                .package_data
                .iter()
                .any(|data| data.datasource_id == Some(DatasourceId::PubspecLock))
    })
}

fn dart_assembler_config() -> &'static AssemblerConfig {
    ASSEMBLERS
        .iter()
        .find(|config| config.datasource_ids.contains(&DatasourceId::PubspecYaml))
        .expect("Dart assembler config must exist")
}
