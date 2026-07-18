// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::models::{DatasourceId, FileInfo, Package, PackageUid, TopLevelDependency};

use super::AssemblerConfig;
use super::cargo_workspace_merge::{
    CargoWorkspaceDomain, CargoWorkspaceRootHint, apply_cargo_workspace_domain,
    collect_cargo_workspace_hints, plan_cargo_workspace_domains,
};
use super::hackage_merge;
use super::mix_umbrella_merge::{
    MixUmbrellaDomain, MixUmbrellaRootHint, apply_mix_umbrella_domain, collect_mix_umbrella_hints,
    plan_mix_umbrella_domains,
};
use super::npm_workspace_merge::{
    NpmWorkspaceDomain, NpmWorkspaceRootHint, apply_npm_workspace_domain,
    collect_npm_workspace_hints, plan_npm_workspace_domains,
};
use super::{ASSEMBLERS, DirectoryMergeOutput, sibling_merge};

pub(super) struct GoWorkspaceRootHint {
    root_dir: PathBuf,
}

pub(super) struct GoWorkspaceDomain {
    root_dir: PathBuf,
    root_dir_file_indices: Vec<usize>,
}

pub(super) struct PixiRootHint {
    root_dir: PathBuf,
}

pub(super) struct PixiDomain {
    root_dir: PathBuf,
    root_dir_file_indices: Vec<usize>,
}

pub(super) struct HackageProjectHint {
    root_dir: PathBuf,
}

pub(super) struct HackageProjectDomain {
    root_dir: PathBuf,
    root_dir_file_indices: Vec<usize>,
}

/// A `pom.xml` that declares a non-empty `<modules>` list (stashed by the parser
/// under `extra_data.modules`). Every such POM — root or nested — contributes its
/// own reactor anchor; see [`MavenReactorDomain`] and
/// [`TopologyPlan::apply_maven_reactor_domains`] for how anchors combine.
pub(super) struct MavenReactorRootHint {
    root_dir: PathBuf,
    root_pom_idx: usize,
    module_paths: Vec<String>,
}

/// A reactor root plus the module `pom.xml` files that were actually found on
/// disk. Module strings that do not resolve to a scanned, purl-bearing `pom.xml`
/// are dropped rather than guessed at.
pub(super) struct MavenReactorDomain {
    root_dir: PathBuf,
    root_pom_idx: usize,
    member_pom_indices: Vec<usize>,
}

pub(super) enum TopologyHint {
    CargoWorkspaceRoot(CargoWorkspaceRootHint),
    GoWorkspaceRoot(GoWorkspaceRootHint),
    HackageProject(HackageProjectHint),
    MixUmbrellaRoot(MixUmbrellaRootHint),
    MavenReactorRoot(MavenReactorRootHint),
    NpmWorkspaceRoot(NpmWorkspaceRootHint),
    PixiRoot(PixiRootHint),
}

pub(super) enum TopologyDomain {
    CargoWorkspace(CargoWorkspaceDomain),
    GoWorkspace(GoWorkspaceDomain),
    HackageProject(HackageProjectDomain),
    MixUmbrella(MixUmbrellaDomain),
    MavenReactor(MavenReactorDomain),
    NpmWorkspace(NpmWorkspaceDomain),
    Pixi(PixiDomain),
}

pub(super) struct TopologyPlan {
    domains: Vec<TopologyDomain>,
    claimed_cargo_dirs: HashSet<PathBuf>,
    claimed_go_dirs: HashSet<PathBuf>,
    claimed_hackage_dirs: HashSet<PathBuf>,
    claimed_mix_dirs: HashSet<PathBuf>,
    claimed_npm_dirs: HashSet<PathBuf>,
    claimed_pixi_dirs: HashSet<PathBuf>,
}

impl TopologyPlan {
    pub(super) fn build(files: &[FileInfo], dir_files: &HashMap<PathBuf, Vec<usize>>) -> Self {
        let mut hints = Vec::new();
        hints.extend(
            collect_cargo_workspace_hints(files)
                .into_iter()
                .map(TopologyHint::CargoWorkspaceRoot),
        );
        hints.extend(
            collect_go_workspace_hints(files)
                .into_iter()
                .map(TopologyHint::GoWorkspaceRoot),
        );
        hints.extend(
            collect_hackage_project_hints(files)
                .into_iter()
                .map(TopologyHint::HackageProject),
        );
        hints.extend(
            collect_mix_umbrella_hints(files)
                .into_iter()
                .map(TopologyHint::MixUmbrellaRoot),
        );
        hints.extend(
            collect_maven_reactor_hints(files)
                .into_iter()
                .map(TopologyHint::MavenReactorRoot),
        );
        hints.extend(
            collect_npm_workspace_hints(files)
                .into_iter()
                .map(TopologyHint::NpmWorkspaceRoot),
        );
        hints.extend(
            collect_pixi_root_hints(files)
                .into_iter()
                .map(TopologyHint::PixiRoot),
        );

        let mut domains = Vec::new();
        let mut claimed_cargo_dirs = HashSet::new();
        let mut claimed_go_dirs = HashSet::new();
        let mut claimed_hackage_dirs = HashSet::new();
        let mut claimed_mix_dirs = HashSet::new();
        let mut claimed_npm_dirs = HashSet::new();
        let mut claimed_pixi_dirs = HashSet::new();

        let cargo_workspace_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(hint) => Some(hint),
                TopologyHint::GoWorkspaceRoot(_) => None,
                TopologyHint::HackageProject(_) => None,
                TopologyHint::MixUmbrellaRoot(_) => None,
                TopologyHint::MavenReactorRoot(_) => None,
                TopologyHint::NpmWorkspaceRoot(_) => None,
                TopologyHint::PixiRoot(_) => None,
            })
            .collect();

        for domain in plan_cargo_workspace_domains(files, dir_files, &cargo_workspace_hints) {
            claimed_cargo_dirs.insert(domain.root_dir.clone());
            claimed_cargo_dirs.extend(domain.members.iter().map(|member| member.dir_path.clone()));
            domains.push(TopologyDomain::CargoWorkspace(domain));
        }

        let go_workspace_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(_) => None,
                TopologyHint::GoWorkspaceRoot(hint) => Some(hint),
                TopologyHint::HackageProject(_) => None,
                TopologyHint::MixUmbrellaRoot(_) => None,
                TopologyHint::MavenReactorRoot(_) => None,
                TopologyHint::NpmWorkspaceRoot(_) => None,
                TopologyHint::PixiRoot(_) => None,
            })
            .collect();

        for domain in plan_go_workspace_domains(dir_files, &go_workspace_hints) {
            claimed_go_dirs.insert(domain.root_dir.clone());
            domains.push(TopologyDomain::GoWorkspace(domain));
        }

        let hackage_project_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(_) => None,
                TopologyHint::GoWorkspaceRoot(_) => None,
                TopologyHint::HackageProject(hint) => Some(hint),
                TopologyHint::MixUmbrellaRoot(_) => None,
                TopologyHint::MavenReactorRoot(_) => None,
                TopologyHint::NpmWorkspaceRoot(_) => None,
                TopologyHint::PixiRoot(_) => None,
            })
            .collect();

        for domain in plan_hackage_project_domains(dir_files, &hackage_project_hints) {
            claimed_hackage_dirs.insert(domain.root_dir.clone());
            domains.push(TopologyDomain::HackageProject(domain));
        }

        let mix_umbrella_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(_) => None,
                TopologyHint::GoWorkspaceRoot(_) => None,
                TopologyHint::HackageProject(_) => None,
                TopologyHint::MixUmbrellaRoot(hint) => Some(hint),
                TopologyHint::MavenReactorRoot(_) => None,
                TopologyHint::NpmWorkspaceRoot(_) => None,
                TopologyHint::PixiRoot(_) => None,
            })
            .collect();

        for domain in plan_mix_umbrella_domains(files, &mix_umbrella_hints) {
            claimed_mix_dirs.insert(domain.root_dir.clone());
            claimed_mix_dirs.extend(domain.members.iter().map(|member| member.dir_path.clone()));
            domains.push(TopologyDomain::MixUmbrella(domain));
        }

        // Maven reactors intentionally do not populate a `claimed_*_dirs` set:
        // unlike Cargo/npm/Go/Pixi/Hackage, each module `pom.xml` still goes
        // through the normal per-directory sibling merge to create its own
        // package. This domain only fills in file ownership for the source
        // trees underneath each module afterwards; see
        // `apply_maven_reactor_domains`.
        let maven_reactor_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(_) => None,
                TopologyHint::GoWorkspaceRoot(_) => None,
                TopologyHint::HackageProject(_) => None,
                TopologyHint::MixUmbrellaRoot(_) => None,
                TopologyHint::MavenReactorRoot(hint) => Some(hint),
                TopologyHint::NpmWorkspaceRoot(_) => None,
                TopologyHint::PixiRoot(_) => None,
            })
            .collect();

        for domain in plan_maven_reactor_domains(files, &maven_reactor_hints) {
            domains.push(TopologyDomain::MavenReactor(domain));
        }

        let npm_workspace_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(_) => None,
                TopologyHint::GoWorkspaceRoot(_) => None,
                TopologyHint::HackageProject(_) => None,
                TopologyHint::MixUmbrellaRoot(_) => None,
                TopologyHint::MavenReactorRoot(_) => None,
                TopologyHint::NpmWorkspaceRoot(hint) => Some(hint),
                TopologyHint::PixiRoot(_) => None,
            })
            .collect();

        for domain in plan_npm_workspace_domains(files, dir_files, &npm_workspace_hints) {
            claimed_npm_dirs.insert(domain.root_dir.clone());
            claimed_npm_dirs.extend(domain.members.iter().map(|member| member.dir_path.clone()));
            domains.push(TopologyDomain::NpmWorkspace(domain));
        }

        let pixi_root_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(_) => None,
                TopologyHint::GoWorkspaceRoot(_) => None,
                TopologyHint::HackageProject(_) => None,
                TopologyHint::MixUmbrellaRoot(_) => None,
                TopologyHint::MavenReactorRoot(_) => None,
                TopologyHint::NpmWorkspaceRoot(_) => None,
                TopologyHint::PixiRoot(hint) => Some(hint),
            })
            .collect();

        for domain in plan_pixi_domains(dir_files, &pixi_root_hints) {
            claimed_pixi_dirs.insert(domain.root_dir.clone());
            domains.push(TopologyDomain::Pixi(domain));
        }

        Self {
            domains,
            claimed_cargo_dirs,
            claimed_go_dirs,
            claimed_hackage_dirs,
            claimed_mix_dirs,
            claimed_npm_dirs,
            claimed_pixi_dirs,
        }
    }

    pub(super) fn claims_directory_assembly(
        &self,
        config: &AssemblerConfig,
        file_indices: &[usize],
        files: &[FileInfo],
    ) -> bool {
        let Some(&first_idx) = file_indices.first() else {
            return false;
        };
        let Some(parent_dir) = Path::new(&files[first_idx].path).parent() else {
            return false;
        };

        if config.datasource_ids.contains(&DatasourceId::CargoToml) {
            return self.claimed_cargo_dirs.contains(parent_dir);
        }

        if config.datasource_ids.contains(&DatasourceId::GoWork) {
            return self.claimed_go_dirs.contains(parent_dir);
        }

        if config.datasource_ids.contains(&DatasourceId::PixiToml) {
            return self.claimed_pixi_dirs.contains(parent_dir);
        }

        if config.datasource_ids.contains(&DatasourceId::HackageCabal) {
            return self.claimed_hackage_dirs.contains(parent_dir);
        }

        if config.datasource_ids.contains(&DatasourceId::HexMixExs) {
            return self.claimed_mix_dirs.contains(parent_dir);
        }

        if !config
            .datasource_ids
            .contains(&DatasourceId::NpmPackageJson)
        {
            return false;
        }

        self.claimed_npm_dirs.contains(parent_dir)
    }

    pub(super) fn apply_directory_scoped_domains(
        &self,
        files: &mut [FileInfo],
        packages: &mut Vec<Package>,
        dependencies: &mut Vec<TopLevelDependency>,
    ) {
        for domain in &self.domains {
            match domain {
                TopologyDomain::GoWorkspace(domain) => {
                    let Some(result) = sibling_merge::assemble_siblings(
                        go_assembler_config(),
                        files,
                        &domain.root_dir_file_indices,
                    )
                    .into_iter()
                    .next() else {
                        continue;
                    };

                    apply_directory_merge_result(files, packages, dependencies, result);
                }
                TopologyDomain::HackageProject(domain) => {
                    let results = hackage_merge::assemble_hackage_packages(
                        files,
                        &domain.root_dir_file_indices,
                    );
                    for result in results {
                        apply_directory_merge_result(files, packages, dependencies, result);
                    }
                }
                TopologyDomain::Pixi(domain) => {
                    let Some(result) = sibling_merge::assemble_siblings(
                        pixi_assembler_config(),
                        files,
                        &domain.root_dir_file_indices,
                    )
                    .into_iter()
                    .next() else {
                        continue;
                    };

                    apply_directory_merge_result(files, packages, dependencies, result);
                }
                TopologyDomain::CargoWorkspace(_)
                | TopologyDomain::MixUmbrella(_)
                | TopologyDomain::MavenReactor(_)
                | TopologyDomain::NpmWorkspace(_) => {}
            }
        }
    }

    pub(super) fn apply_cargo_workspace_domains(
        &self,
        files: &mut [FileInfo],
        packages: &mut Vec<Package>,
        dependencies: &mut Vec<TopLevelDependency>,
    ) {
        for domain in &self.domains {
            match domain {
                TopologyDomain::CargoWorkspace(domain) => {
                    apply_cargo_workspace_domain(domain, files, packages, dependencies);
                }
                TopologyDomain::GoWorkspace(_)
                | TopologyDomain::HackageProject(_)
                | TopologyDomain::MixUmbrella(_)
                | TopologyDomain::MavenReactor(_)
                | TopologyDomain::NpmWorkspace(_)
                | TopologyDomain::Pixi(_) => {}
            }
        }
    }

    pub(super) fn apply_npm_workspace_domains(
        &self,
        files: &mut [FileInfo],
        packages: &mut Vec<Package>,
        dependencies: &mut Vec<TopLevelDependency>,
    ) {
        for domain in &self.domains {
            match domain {
                TopologyDomain::CargoWorkspace(_)
                | TopologyDomain::GoWorkspace(_)
                | TopologyDomain::HackageProject(_)
                | TopologyDomain::MixUmbrella(_)
                | TopologyDomain::MavenReactor(_)
                | TopologyDomain::Pixi(_) => {}
                TopologyDomain::NpmWorkspace(domain) => {
                    apply_npm_workspace_domain(domain, files, packages, dependencies);
                }
            }
        }
    }

    pub(super) fn apply_mix_umbrella_domains(
        &self,
        files: &mut [FileInfo],
        packages: &mut Vec<Package>,
        dependencies: &mut Vec<TopLevelDependency>,
    ) {
        for domain in &self.domains {
            match domain {
                TopologyDomain::CargoWorkspace(_)
                | TopologyDomain::GoWorkspace(_)
                | TopologyDomain::HackageProject(_)
                | TopologyDomain::MavenReactor(_)
                | TopologyDomain::NpmWorkspace(_)
                | TopologyDomain::Pixi(_) => {}
                TopologyDomain::MixUmbrella(domain) => {
                    apply_mix_umbrella_domain(domain, files, packages, dependencies);
                }
            }
        }
    }

    /// Fill in reactor-aware file ownership for every declared Maven multi-module
    /// tree, without touching package identity or dependency extraction.
    ///
    /// Every declared `pom.xml` in a reactor (root or nested module) already owns
    /// its own directory via the normal per-directory sibling merge that ran
    /// before this post-assembly pass, so it already has a non-empty
    /// `for_packages`. What is missing is ownership for files nested *underneath*
    /// each module (`src/main/java/...`, resources, …), which sibling merge never
    /// reaches because they do not sit beside a manifest.
    ///
    /// This method collects every reactor anchor directory (the root plus every
    /// resolved module) across all domains into one flat list, then attributes
    /// each currently unowned file under a declared reactor root to the *deepest*
    /// (most specific) anchor that contains it. That single "deepest wins" rule
    /// is what makes nested reactors (a module that itself declares further
    /// `<modules>`) work correctly without explicit recursion: a nested module's
    /// own `pom.xml` contributes its own anchor, which is more specific than its
    /// parent's, so files under it resolve to the nested module instead of the
    /// outer root.
    ///
    /// Ownership is filled in only for files that do not already belong to any
    /// package (additive, not corrective), and only within a directory tree that
    /// a real reactor actually declares — an unrelated `pom.xml` elsewhere in the
    /// scan is left untouched. Maven's own build output directory (`target/`) is
    /// excluded so compiled artifacts are not attributed to source packages.
    pub(super) fn apply_maven_reactor_domains(&self, files: &mut [FileInfo]) {
        let mut scope_roots: Vec<PathBuf> = Vec::new();
        let mut anchors: Vec<(PathBuf, Vec<PackageUid>)> = Vec::new();

        for domain in &self.domains {
            let TopologyDomain::MavenReactor(domain) = domain else {
                continue;
            };

            scope_roots.push(domain.root_dir.clone());
            anchors.push((
                domain.root_dir.clone(),
                files[domain.root_pom_idx].for_packages.clone(),
            ));

            for &member_idx in &domain.member_pom_indices {
                let Some(member_dir) = Path::new(&files[member_idx].path).parent() else {
                    continue;
                };
                let member_dir = member_dir.to_path_buf();
                // A declared module normally lives under the reactor root
                // directory, which already covers it via `scope_roots` above.
                // But `<module>` strings may legally escape it with a `../`
                // spelling (a true sibling directory), so each resolved
                // member also contributes its own scope root — otherwise a
                // correctly-resolved sibling module would still get no file
                // ownership benefit and remain effectively dropped from the
                // reactor.
                scope_roots.push(member_dir.clone());
                anchors.push((member_dir, files[member_idx].for_packages.clone()));
            }
        }

        if anchors.is_empty() {
            return;
        }

        for file in files.iter_mut() {
            if !file.for_packages.is_empty() {
                continue;
            }

            let file_path = Path::new(&file.path);
            let Some(file_dir) = file_path.parent() else {
                continue;
            };

            if !scope_roots.iter().any(|root| file_dir.starts_with(root)) {
                continue;
            }

            let Some((anchor_dir, package_uids)) = anchors
                .iter()
                .filter(|(anchor_dir, _)| file_dir.starts_with(anchor_dir))
                .max_by_key(|(anchor_dir, _)| anchor_dir.as_os_str().len())
            else {
                continue;
            };

            if package_uids.is_empty() || crosses_maven_build_output_dir(anchor_dir, file_dir) {
                continue;
            }

            file.for_packages.extend(package_uids.iter().cloned());
        }
    }
}

/// Maven build output (`target/classes`, `target/test-classes`, …) sits directly
/// underneath a module directory; skip it so compiled artifacts are not
/// attributed back to the source package.
fn crosses_maven_build_output_dir(anchor_dir: &Path, file_dir: &Path) -> bool {
    file_dir
        .strip_prefix(anchor_dir)
        .map(|relative| relative.components().any(|c| c.as_os_str() == "target"))
        .unwrap_or(false)
}

fn collect_go_workspace_hints(files: &[FileInfo]) -> Vec<GoWorkspaceRootHint> {
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

fn collect_pixi_root_hints(files: &[FileInfo]) -> Vec<PixiRootHint> {
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

fn collect_hackage_project_hints(files: &[FileInfo]) -> Vec<HackageProjectHint> {
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

        let Some(parent) = path.parent() else {
            continue;
        };
        let root_dir = parent.to_path_buf();
        if seen.insert(root_dir.clone()) {
            hints.push(HackageProjectHint { root_dir });
        }
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

fn collect_maven_reactor_hints(files: &[FileInfo]) -> Vec<MavenReactorRootHint> {
    let mut hints = Vec::new();

    for (idx, file) in files.iter().enumerate() {
        let path = Path::new(&file.path);
        if path.file_name().and_then(|name| name.to_str()) != Some("pom.xml") {
            continue;
        }

        for pkg_data in &file.package_data {
            if pkg_data.datasource_id != Some(DatasourceId::MavenPom) {
                continue;
            }

            let module_paths: Vec<String> = pkg_data
                .extra_data
                .as_ref()
                .and_then(|extra_data| extra_data.get("modules"))
                .and_then(|modules| modules.as_array())
                .map(|modules| {
                    modules
                        .iter()
                        .filter_map(|module| module.as_str())
                        .map(|module| module.to_string())
                        .collect()
                })
                .unwrap_or_default();

            if module_paths.is_empty() {
                continue;
            }

            let Some(parent) = path.parent() else {
                continue;
            };

            hints.push(MavenReactorRootHint {
                root_dir: parent.to_path_buf(),
                root_pom_idx: idx,
                module_paths,
            });
        }
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

fn plan_maven_reactor_domains(
    files: &[FileInfo],
    reactor_hints: &[&MavenReactorRootHint],
) -> Vec<MavenReactorDomain> {
    let mut domains = Vec::new();

    for hint in reactor_hints {
        domains.push(MavenReactorDomain {
            root_dir: hint.root_dir.clone(),
            root_pom_idx: hint.root_pom_idx,
            member_pom_indices: resolve_maven_reactor_members(files, hint),
        });
    }

    domains.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    domains
}

/// Resolve each declared `<module>` string to a scanned `pom.xml` with a real
/// package identity (a purl). A module string is a relative path from the
/// declaring POM's directory to the member's own directory (almost always a
/// bare directory name, but Maven also allows nested relative paths,
/// including `.`/`..` components such as `./module-a` or `../sibling-module`).
/// A module that does not resolve to a purl-bearing `pom.xml` on disk is
/// silently skipped — Provenant never guesses at an undeclared or missing
/// member.
fn resolve_maven_reactor_members(files: &[FileInfo], hint: &MavenReactorRootHint) -> Vec<usize> {
    let mut member_indices = Vec::new();

    for module in &hint.module_paths {
        let candidate_path = normalize_lexical_path(&hint.root_dir.join(module)).join("pom.xml");

        let Some(found_idx) = files
            .iter()
            .position(|file| Path::new(&file.path) == candidate_path)
        else {
            continue;
        };

        let has_identity = files[found_idx].package_data.iter().any(|pkg_data| {
            pkg_data.datasource_id == Some(DatasourceId::MavenPom) && pkg_data.purl.is_some()
        });

        if has_identity {
            member_indices.push(found_idx);
        }
    }

    member_indices
}

/// Lexically resolve `.` and `..` components in a path without touching the
/// filesystem, so a declared module path such as `./module-a` or
/// `../sibling-module` compares equal to the scanned path's own normalized
/// form.
fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }

    normalized
}

fn plan_go_workspace_domains(
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

fn plan_pixi_domains(
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

fn plan_hackage_project_domains(
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

fn apply_directory_merge_result(
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
    result: DirectoryMergeOutput,
) {
    let (package, deps, affected_indices) = result;

    if let Some(package) = package {
        let package_uid = package.package_uid.clone();
        for idx in &affected_indices {
            if !files[*idx].for_packages.contains(&package_uid) {
                files[*idx].for_packages.push(package_uid.clone());
            }
        }
        packages.push(package);
    }
    dependencies.extend(deps);
}

fn go_assembler_config() -> &'static AssemblerConfig {
    ASSEMBLERS
        .iter()
        .find(|config| config.datasource_ids.contains(&DatasourceId::GoWork))
        .expect("Go assembler config must exist")
}

fn pixi_assembler_config() -> &'static AssemblerConfig {
    ASSEMBLERS
        .iter()
        .find(|config| config.datasource_ids.contains(&DatasourceId::PixiToml))
        .expect("Pixi assembler config must exist")
}
