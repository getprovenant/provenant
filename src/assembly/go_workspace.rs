// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Go workspace (`go.work`) directory-scoped topology.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::models::{DatasourceId, FileInfo, Package, TopLevelDependency};

use super::topology::apply_directory_merge_result;
use super::{ASSEMBLERS, AssemblerConfig, sibling_merge};

pub(super) struct GoWorkspaceRootHint {
    pub(super) root_dir: PathBuf,
}

pub(super) struct GoWorkspaceDomain {
    pub(super) root_dir: PathBuf,
    pub(super) root_dir_file_indices: Vec<usize>,
}

pub(super) fn collect_go_workspace_hints(files: &[FileInfo]) -> Vec<GoWorkspaceRootHint> {
    let mut seen = HashSet::new();
    let mut hints = Vec::new();

    for file in files {
        let path = Path::new(&file.path);
        if path.file_name().and_then(|name| name.to_str()) != Some("go.work") {
            continue;
        }

        let has_go_work_data = file
            .package_data
            .iter()
            .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::GoWork));
        if !has_go_work_data {
            continue;
        }

        let Some(parent) = path.parent() else {
            continue;
        };
        let root_dir = parent.to_path_buf();
        if seen.insert(root_dir.clone()) {
            hints.push(GoWorkspaceRootHint { root_dir });
        }
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

pub(super) fn plan_go_workspace_domains(
    dir_files: &HashMap<PathBuf, Vec<usize>>,
    workspace_hints: &[&GoWorkspaceRootHint],
) -> Vec<GoWorkspaceDomain> {
    let mut domains = Vec::new();

    for hint in workspace_hints {
        let root_dir_file_indices = dir_files.get(&hint.root_dir).cloned().unwrap_or_default();
        if root_dir_file_indices.is_empty() {
            continue;
        }

        domains.push(GoWorkspaceDomain {
            root_dir: hint.root_dir.clone(),
            root_dir_file_indices,
        });
    }

    domains.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    domains
}

pub(super) fn apply_go_workspace_domain(
    domain: &GoWorkspaceDomain,
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let Some(result) = sibling_merge::assemble_siblings(
        go_assembler_config(),
        files,
        &domain.root_dir_file_indices,
    )
    .into_iter()
    .next() else {
        return;
    };

    apply_directory_merge_result(files, packages, dependencies, result);
}

fn go_assembler_config() -> &'static AssemblerConfig {
    ASSEMBLERS
        .iter()
        .find(|config| config.datasource_ids.contains(&DatasourceId::GoWork))
        .expect("Go assembler config must exist")
}
