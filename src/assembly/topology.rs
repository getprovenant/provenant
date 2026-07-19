// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use crate::models::{DatasourceId, FileInfo, Package, PackageUid, TopLevelDependency};
use strum::EnumIter;

use super::AssemblerConfig;
use super::DirectoryMergeOutput;
use super::cargo_workspace_merge::{
    CargoWorkspaceDomain, apply_cargo_workspace_domain, collect_cargo_workspace_hints,
    plan_cargo_workspace_domains,
};
use super::dart_workspace_merge::{
    DartWorkspaceDomain, apply_dart_workspace_domain, collect_dart_workspace_hints,
    plan_dart_workspace_domains,
};
use super::go_workspace::{
    GoWorkspaceDomain, apply_go_workspace_domain, collect_go_workspace_hints,
    plan_go_workspace_domains,
};
use super::gradle_multiproject::{
    self as gradle_multiproject, GradleMultiProjectDomain, collect_gradle_multi_project_hints,
    plan_gradle_multi_project_domains,
};
use super::hackage_merge::{
    HackageProjectDomain, apply_hackage_project_domain, collect_hackage_project_hints,
    plan_hackage_project_domains,
};
use super::maven_reactor::{
    self as maven_reactor, MavenReactorDomain, collect_maven_reactor_hints,
    plan_maven_reactor_domains,
};
use super::mix_umbrella_merge::{
    MixUmbrellaDomain, apply_mix_umbrella_domain, collect_mix_umbrella_hints,
    plan_mix_umbrella_domains,
};
use super::npm_workspace_merge::{
    NpmWorkspaceDomain, apply_npm_workspace_domain, collect_npm_workspace_hints,
    plan_npm_workspace_domains,
};
use super::pixi_topology::{
    PixiDomain, apply_pixi_domain, collect_pixi_root_hints, plan_pixi_domains,
};
use super::uv_workspace::{
    self as uv_workspace, UvWorkspaceDomain, collect_uv_workspace_hints, plan_uv_workspace_domains,
};

/// Identity of a topology family in the [`TOPOLOGY_HANDLERS`] registry.
///
/// Keys the per-family set of directories a family claims from the generic
/// per-directory assembler loop (see [`TopologyPlan::claimed_dirs`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumIter)]
pub(super) enum TopologyFamilyId {
    CargoWorkspace,
    DartWorkspace,
    GoWorkspace,
    GradleMultiProject,
    HackageProject,
    MixUmbrella,
    MavenReactor,
    NpmWorkspace,
    Pixi,
    UvWorkspace,
}

pub(super) enum TopologyDomain {
    CargoWorkspace(CargoWorkspaceDomain),
    DartWorkspace(DartWorkspaceDomain),
    GoWorkspace(GoWorkspaceDomain),
    GradleMultiProject(GradleMultiProjectDomain),
    HackageProject(HackageProjectDomain),
    MixUmbrella(MixUmbrellaDomain),
    MavenReactor(MavenReactorDomain),
    NpmWorkspace(NpmWorkspaceDomain),
    Pixi(PixiDomain),
    UvWorkspace(UvWorkspaceDomain),
}

pub(super) struct TopologyPlan {
    domains: Vec<TopologyDomain>,
    /// Directories each family claims from the generic per-directory assembler
    /// loop, keyed by family. A family that only fills in file ownership after
    /// assembly (Maven/Gradle/uv) contributes no entry here.
    claimed_dirs: HashMap<TopologyFamilyId, HashSet<PathBuf>>,
}

/// Read-only scan inputs handed to every topology family's planner.
struct TopologyInputs<'a> {
    files: &'a [FileInfo],
    dir_files: &'a HashMap<PathBuf, Vec<usize>>,
}

/// A single family's planning output: the domains it discovered plus the
/// directories it claims from the generic per-directory assembler loop.
#[derive(Default)]
struct PlannedFamily {
    domains: Vec<TopologyDomain>,
    claimed_dirs: HashSet<PathBuf>,
}

/// A registered topology family: its identity, the datasource IDs whose
/// per-directory assembly it claims, and the planner that turns scan inputs into
/// domains and claimed directories.
///
/// Families differ deliberately. Some claim their directories outright so the
/// generic loop skips them (Cargo/npm/Mix/Dart claim root and members;
/// Go/Pixi/Hackage claim their single root), while others claim nothing and only
/// attribute nested file ownership after assembly (Maven/Gradle/uv). That
/// difference lives entirely in each planner and its `claim_datasource_ids`,
/// not in a shared algorithm.
struct TopologyHandler {
    family: TopologyFamilyId,
    /// The datasource IDs whose per-directory assembly this family claims. Empty
    /// means ownership-only: the family never claims a directory.
    claim_datasource_ids: &'static [DatasourceId],
    plan: fn(&TopologyInputs) -> PlannedFamily,
}

static TOPOLOGY_HANDLERS: &[TopologyHandler] = &[
    TopologyHandler {
        family: TopologyFamilyId::CargoWorkspace,
        claim_datasource_ids: &[DatasourceId::CargoToml],
        plan: plan_cargo_family,
    },
    TopologyHandler {
        family: TopologyFamilyId::DartWorkspace,
        claim_datasource_ids: &[DatasourceId::PubspecYaml],
        plan: plan_dart_family,
    },
    TopologyHandler {
        family: TopologyFamilyId::GoWorkspace,
        claim_datasource_ids: &[DatasourceId::GoWork],
        plan: plan_go_family,
    },
    TopologyHandler {
        family: TopologyFamilyId::GradleMultiProject,
        claim_datasource_ids: &[],
        plan: plan_gradle_family,
    },
    TopologyHandler {
        family: TopologyFamilyId::HackageProject,
        claim_datasource_ids: &[DatasourceId::HackageCabal],
        plan: plan_hackage_family,
    },
    TopologyHandler {
        family: TopologyFamilyId::MixUmbrella,
        claim_datasource_ids: &[DatasourceId::HexMixExs],
        plan: plan_mix_family,
    },
    TopologyHandler {
        family: TopologyFamilyId::MavenReactor,
        claim_datasource_ids: &[],
        plan: plan_maven_family,
    },
    TopologyHandler {
        family: TopologyFamilyId::NpmWorkspace,
        claim_datasource_ids: &[DatasourceId::NpmPackageJson],
        plan: plan_npm_family,
    },
    TopologyHandler {
        family: TopologyFamilyId::Pixi,
        claim_datasource_ids: &[DatasourceId::PixiToml],
        plan: plan_pixi_family,
    },
    TopologyHandler {
        family: TopologyFamilyId::UvWorkspace,
        claim_datasource_ids: &[],
        plan: plan_uv_family,
    },
];

fn plan_cargo_family(inputs: &TopologyInputs) -> PlannedFamily {
    let hints = collect_cargo_workspace_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_cargo_workspace_domains(inputs.files, inputs.dir_files, &hint_refs) {
        planned.claimed_dirs.insert(domain.root_dir.clone());
        planned
            .claimed_dirs
            .extend(domain.members.iter().map(|member| member.dir_path.clone()));
        planned.domains.push(TopologyDomain::CargoWorkspace(domain));
    }
    planned
}

fn plan_dart_family(inputs: &TopologyInputs) -> PlannedFamily {
    let hints = collect_dart_workspace_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_dart_workspace_domains(inputs.files, &hint_refs) {
        planned.claimed_dirs.insert(domain.root_dir.clone());
        planned
            .claimed_dirs
            .extend(domain.members.iter().map(|member| member.dir_path.clone()));
        planned.domains.push(TopologyDomain::DartWorkspace(domain));
    }
    planned
}

fn plan_go_family(inputs: &TopologyInputs) -> PlannedFamily {
    let hints = collect_go_workspace_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_go_workspace_domains(inputs.dir_files, &hint_refs) {
        planned.claimed_dirs.insert(domain.root_dir.clone());
        planned.domains.push(TopologyDomain::GoWorkspace(domain));
    }
    planned
}

fn plan_gradle_family(inputs: &TopologyInputs) -> PlannedFamily {
    let hints = collect_gradle_multi_project_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_gradle_multi_project_domains(inputs.files, &hint_refs) {
        planned
            .domains
            .push(TopologyDomain::GradleMultiProject(domain));
    }
    planned
}

fn plan_hackage_family(inputs: &TopologyInputs) -> PlannedFamily {
    let hints = collect_hackage_project_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_hackage_project_domains(inputs.dir_files, &hint_refs) {
        planned.claimed_dirs.insert(domain.root_dir.clone());
        planned.domains.push(TopologyDomain::HackageProject(domain));
    }
    planned
}

fn plan_mix_family(inputs: &TopologyInputs) -> PlannedFamily {
    let hints = collect_mix_umbrella_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_mix_umbrella_domains(inputs.files, &hint_refs) {
        planned.claimed_dirs.insert(domain.root_dir.clone());
        planned
            .claimed_dirs
            .extend(domain.members.iter().map(|member| member.dir_path.clone()));
        planned.domains.push(TopologyDomain::MixUmbrella(domain));
    }
    planned
}

fn plan_maven_family(inputs: &TopologyInputs) -> PlannedFamily {
    // Maven reactors intentionally claim no directories: unlike
    // Cargo/npm/Go/Pixi/Hackage, each module `pom.xml` still goes through the
    // normal per-directory sibling merge to create its own package. This domain
    // only fills in file ownership for the source trees underneath each module
    // afterwards; see `apply_maven_reactor_domains`.
    let hints = collect_maven_reactor_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_maven_reactor_domains(inputs.files, &hint_refs) {
        planned.domains.push(TopologyDomain::MavenReactor(domain));
    }
    planned
}

fn plan_npm_family(inputs: &TopologyInputs) -> PlannedFamily {
    let hints = collect_npm_workspace_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_npm_workspace_domains(inputs.files, inputs.dir_files, &hint_refs) {
        planned.claimed_dirs.insert(domain.root_dir.clone());
        planned
            .claimed_dirs
            .extend(domain.members.iter().map(|member| member.dir_path.clone()));
        planned.domains.push(TopologyDomain::NpmWorkspace(domain));
    }
    planned
}

fn plan_pixi_family(inputs: &TopologyInputs) -> PlannedFamily {
    let hints = collect_pixi_root_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_pixi_domains(inputs.dir_files, &hint_refs) {
        planned.claimed_dirs.insert(domain.root_dir.clone());
        planned.domains.push(TopologyDomain::Pixi(domain));
    }
    planned
}

fn plan_uv_family(inputs: &TopologyInputs) -> PlannedFamily {
    let hints = collect_uv_workspace_hints(inputs.files);
    let hint_refs: Vec<_> = hints.iter().collect();
    let mut planned = PlannedFamily::default();
    for domain in plan_uv_workspace_domains(inputs.files, &hint_refs) {
        planned.domains.push(TopologyDomain::UvWorkspace(domain));
    }
    planned
}

impl TopologyPlan {
    pub(super) fn build(files: &[FileInfo], dir_files: &HashMap<PathBuf, Vec<usize>>) -> Self {
        let inputs = TopologyInputs { files, dir_files };
        let mut domains = Vec::new();
        let mut claimed_dirs: HashMap<TopologyFamilyId, HashSet<PathBuf>> = HashMap::new();

        for handler in TOPOLOGY_HANDLERS {
            let planned = (handler.plan)(&inputs);
            if !planned.claimed_dirs.is_empty() {
                claimed_dirs.insert(handler.family, planned.claimed_dirs);
            }
            domains.extend(planned.domains);
        }

        Self {
            domains,
            claimed_dirs,
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

        for handler in TOPOLOGY_HANDLERS {
            if handler.claim_datasource_ids.is_empty() {
                continue;
            }
            if handler
                .claim_datasource_ids
                .iter()
                .any(|dsid| config.datasource_ids.contains(dsid))
            {
                return self
                    .claimed_dirs
                    .get(&handler.family)
                    .is_some_and(|dirs| dirs.contains(parent_dir));
            }
        }

        false
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
                    apply_go_workspace_domain(domain, files, packages, dependencies);
                }
                TopologyDomain::HackageProject(domain) => {
                    apply_hackage_project_domain(domain, files, packages, dependencies);
                }
                TopologyDomain::Pixi(domain) => {
                    apply_pixi_domain(domain, files, packages, dependencies);
                }
                _ => {}
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
            if let TopologyDomain::CargoWorkspace(domain) = domain {
                apply_cargo_workspace_domain(domain, files, packages, dependencies);
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
            if let TopologyDomain::NpmWorkspace(domain) = domain {
                apply_npm_workspace_domain(domain, files, packages, dependencies);
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
            if let TopologyDomain::MixUmbrella(domain) = domain {
                apply_mix_umbrella_domain(domain, files, packages, dependencies);
            }
        }
    }

    pub(super) fn apply_dart_workspace_domains(
        &self,
        files: &mut [FileInfo],
        packages: &mut Vec<Package>,
        dependencies: &mut Vec<TopLevelDependency>,
    ) {
        for domain in &self.domains {
            if let TopologyDomain::DartWorkspace(domain) = domain {
                apply_dart_workspace_domain(domain, files, packages, dependencies);
            }
        }
    }

    pub(super) fn apply_maven_reactor_domains(&self, files: &mut [FileInfo]) {
        maven_reactor::apply_maven_reactor_domains(
            self.domains.iter().filter_map(|domain| match domain {
                TopologyDomain::MavenReactor(domain) => Some(domain),
                _ => None,
            }),
            files,
        );
    }

    pub(super) fn apply_gradle_multi_project_domains(
        &self,
        files: &mut [FileInfo],
        packages: &mut Vec<Package>,
        dependencies: &mut Vec<TopLevelDependency>,
    ) {
        gradle_multiproject::apply_gradle_multi_project_domains(
            self.domains.iter().filter_map(|domain| match domain {
                TopologyDomain::GradleMultiProject(domain) => Some(domain),
                _ => None,
            }),
            files,
            packages,
            dependencies,
        );
    }

    pub(super) fn apply_uv_workspace_domains(
        &self,
        files: &mut [FileInfo],
        dependencies: &mut Vec<TopLevelDependency>,
    ) {
        uv_workspace::apply_uv_workspace_domains(
            self.domains.iter().filter_map(|domain| match domain {
                TopologyDomain::UvWorkspace(domain) => Some(domain),
                _ => None,
            }),
            files,
            dependencies,
        );
    }
}

/// Attribute currently-unowned files to the deepest (most specific) anchor
/// package that contains them, within the declared workspace scope.
///
/// This is the shared "reactor-style" file-ownership rule used by the Maven
/// reactor, Gradle multi-project, and uv workspace
/// topologies. Ownership is additive (only files with no package yet), bounded to
/// files under a declared `scope_root`, and skips a build-output subtree that
/// sits as an anchor's *immediate* child (e.g. Maven `target/`, Gradle `build/`,
/// Dart `.dart_tool/`) so compiled artifacts are not attributed to source
/// packages. "Deepest wins" is what makes a nested member (its own manifest
/// contributes a more specific anchor) claim its files over an outer root.
pub(super) fn assign_unowned_files_to_anchors(
    files: &mut [FileInfo],
    scope_roots: &[PathBuf],
    anchor_indices: &[usize],
    excluded_immediate_children: &[&OsStr],
    protected_dirs: &[PathBuf],
) {
    let anchors: Vec<(PathBuf, Vec<PackageUid>)> = anchor_indices
        .iter()
        .filter_map(|idx| {
            Path::new(&files[*idx].path)
                .parent()
                .map(|dir| (dir.to_path_buf(), files[*idx].for_packages.clone()))
        })
        .collect();

    for file in files.iter_mut() {
        if !file.for_packages.is_empty() {
            continue;
        }
        let Some(file_dir) = Path::new(&file.path).parent() else {
            continue;
        };
        if !scope_roots.iter().any(|root| file_dir.starts_with(root)) {
            continue;
        }
        if protected_dirs
            .iter()
            .any(|protected| file_dir.starts_with(protected))
        {
            continue;
        }

        let Some((anchor_dir, package_uids)) = anchors
            .iter()
            .filter(|(anchor_dir, _)| file_dir.starts_with(anchor_dir))
            .max_by_key(|(anchor_dir, _)| anchor_dir.as_os_str().len())
        else {
            continue;
        };
        if package_uids.is_empty()
            || crosses_excluded_build_output(anchor_dir, file_dir, excluded_immediate_children)
        {
            continue;
        }

        file.for_packages.extend(package_uids.iter().cloned());
    }
}

/// Whether `file_dir` lies under one of the anchor's excluded immediate
/// build-output children. An excluded name matches only as the first path
/// component beneath the anchor (e.g. `<module>/target/...`); a directory of the
/// same name deeper in the tree (a real source package literally named `target`)
/// is not build output and still receives ownership.
fn crosses_excluded_build_output(
    anchor_dir: &Path,
    file_dir: &Path,
    excluded_immediate_children: &[&OsStr],
) -> bool {
    if excluded_immediate_children.is_empty() {
        return false;
    }
    file_dir
        .strip_prefix(anchor_dir)
        .ok()
        .and_then(|relative| relative.components().next())
        .is_some_and(|first| {
            excluded_immediate_children
                .iter()
                .any(|excluded| first == Component::Normal(excluded))
        })
}

/// Lexically resolve `.` and `..` components in a path without touching the
/// filesystem, so a declared module path such as `./module-a` or
/// `../sibling-module` compares equal to the scanned path's own normalized
/// form.
///
/// Unresolved parent components are kept rather than discarded. Dropping them
/// would let an over-escaped module like `../../../module-a` collapse onto an
/// unrelated in-scan `module-a/` and incorrectly accept it as a reactor member.
pub(super) fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => match normalized.components().next_back() {
                Some(std::path::Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(std::path::Component::ParentDir) | None => {
                    normalized.push(std::path::Component::ParentDir.as_os_str());
                }
                // Never escape a filesystem root / Windows prefix.
                Some(std::path::Component::RootDir) | Some(std::path::Component::Prefix(_)) => {}
                // CurDir is skipped above, so it never accumulates here.
                Some(std::path::Component::CurDir) => unreachable!(),
            },
            other => normalized.push(other.as_os_str()),
        }
    }

    normalized
}

pub(super) fn apply_directory_merge_result(
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

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn every_topology_family_is_registered_exactly_once() {
        for family in TopologyFamilyId::iter() {
            let count = TOPOLOGY_HANDLERS
                .iter()
                .filter(|handler| handler.family == family)
                .count();
            assert_eq!(
                count, 1,
                "topology family {family:?} must be registered exactly once"
            );
        }
        assert_eq!(TOPOLOGY_HANDLERS.len(), TopologyFamilyId::iter().count());
    }

    #[test]
    fn claim_datasource_ids_map_to_a_single_family() {
        let mut seen: HashMap<DatasourceId, TopologyFamilyId> = HashMap::new();
        for handler in TOPOLOGY_HANDLERS {
            for &dsid in handler.claim_datasource_ids {
                if let Some(existing) = seen.insert(dsid, handler.family) {
                    panic!(
                        "datasource {dsid:?} claimed by both {existing:?} and {:?}",
                        handler.family
                    );
                }
            }
        }
    }
}
