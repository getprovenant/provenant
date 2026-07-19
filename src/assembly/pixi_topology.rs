// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Pixi project directory-scoped topology.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::models::{DatasourceId, FileInfo, Package, TopLevelDependency};

use super::topology::apply_directory_merge_result;
use super::{ASSEMBLERS, AssemblerConfig, sibling_merge};

pub(super) struct PixiRootHint {
    pub(super) root_dir: PathBuf,
}

pub(super) struct PixiDomain {
    pub(super) root_dir: PathBuf,
    pub(super) root_dir_file_indices: Vec<usize>,
}

pub(super) fn collect_pixi_root_hints(files: &[FileInfo]) -> Vec<PixiRootHint> {
    let mut seen = HashSet::new();
    let mut hints = Vec::new();

    for file in files {
        let path = Path::new(&file.path);
        if path.file_name().and_then(|name| name.to_str()) != Some("pixi.toml") {
            continue;
        }

        let has_pixi_manifest = file
            .package_data
            .iter()
            .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::PixiToml));
        if !has_pixi_manifest {
            continue;
        }

        let Some(parent) = path.parent() else {
            continue;
        };
        let root_dir = parent.to_path_buf();
        if seen.insert(root_dir.clone()) {
            hints.push(PixiRootHint { root_dir });
        }
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

pub(super) fn plan_pixi_domains(
    dir_files: &HashMap<PathBuf, Vec<usize>>,
    workspace_hints: &[&PixiRootHint],
) -> Vec<PixiDomain> {
    let mut domains = Vec::new();

    for hint in workspace_hints {
        let root_dir_file_indices = dir_files.get(&hint.root_dir).cloned().unwrap_or_default();
        if root_dir_file_indices.is_empty() {
            continue;
        }

        domains.push(PixiDomain {
            root_dir: hint.root_dir.clone(),
            root_dir_file_indices,
        });
    }

    domains.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    domains
}

pub(super) fn apply_pixi_domain(
    domain: &PixiDomain,
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let Some(result) = sibling_merge::assemble_siblings(
        pixi_assembler_config(),
        files,
        &domain.root_dir_file_indices,
    )
    .into_iter()
    .next() else {
        return;
    };

    apply_directory_merge_result(files, packages, dependencies, result);
}

fn pixi_assembler_config() -> &'static AssemblerConfig {
    ASSEMBLERS
        .iter()
        .find(|config| config.datasource_ids.contains(&DatasourceId::PixiToml))
        .expect("Pixi assembler config must exist")
}
