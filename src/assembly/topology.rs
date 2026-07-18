// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use crate::models::{DatasourceId, FileInfo, Package, PackageUid, TopLevelDependency};

use super::AssemblerConfig;
use super::cargo_workspace_merge::{
    CargoWorkspaceDomain, CargoWorkspaceRootHint, apply_cargo_workspace_domain,
    collect_cargo_workspace_hints, plan_cargo_workspace_domains,
};
use super::dart_workspace_merge::{
    DartWorkspaceDomain, DartWorkspaceRootHint, apply_dart_workspace_domain,
    collect_dart_workspace_hints, plan_dart_workspace_domains,
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

pub(super) struct GradleMultiProjectRootHint {
    root_dir: PathBuf,
    project_paths: Vec<String>,
    root_project_name: Option<String>,
}

pub(super) struct GradleMultiProjectDomain {
    root_dir: PathBuf,
    root_build_idx: Option<usize>,
    root_project_name: Option<String>,
    member_build_indices: Vec<usize>,
}

pub(super) struct UvWorkspaceRootHint {
    root_dir: PathBuf,
    root_pyproject_idx: usize,
    member_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
}

pub(super) struct UvWorkspaceDomain {
    root_dir: PathBuf,
    root_pyproject_idx: usize,
    member_pyproject_indices: Vec<usize>,
}

pub(super) enum TopologyHint {
    CargoWorkspaceRoot(CargoWorkspaceRootHint),
    DartWorkspaceRoot(DartWorkspaceRootHint),
    GoWorkspaceRoot(GoWorkspaceRootHint),
    GradleMultiProjectRoot(GradleMultiProjectRootHint),
    HackageProject(HackageProjectHint),
    MixUmbrellaRoot(MixUmbrellaRootHint),
    MavenReactorRoot(MavenReactorRootHint),
    NpmWorkspaceRoot(NpmWorkspaceRootHint),
    PixiRoot(PixiRootHint),
    UvWorkspaceRoot(UvWorkspaceRootHint),
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
    claimed_cargo_dirs: HashSet<PathBuf>,
    claimed_dart_dirs: HashSet<PathBuf>,
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
            collect_dart_workspace_hints(files)
                .into_iter()
                .map(TopologyHint::DartWorkspaceRoot),
        );
        hints.extend(
            collect_go_workspace_hints(files)
                .into_iter()
                .map(TopologyHint::GoWorkspaceRoot),
        );
        hints.extend(
            collect_gradle_multi_project_hints(files)
                .into_iter()
                .map(TopologyHint::GradleMultiProjectRoot),
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
        hints.extend(
            collect_uv_workspace_hints(files)
                .into_iter()
                .map(TopologyHint::UvWorkspaceRoot),
        );

        let mut domains = Vec::new();
        let mut claimed_cargo_dirs = HashSet::new();
        let mut claimed_dart_dirs = HashSet::new();
        let mut claimed_go_dirs = HashSet::new();
        let mut claimed_hackage_dirs = HashSet::new();
        let mut claimed_mix_dirs = HashSet::new();
        let mut claimed_npm_dirs = HashSet::new();
        let mut claimed_pixi_dirs = HashSet::new();

        let cargo_workspace_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(hint) => Some(hint),
                _ => None,
            })
            .collect();

        for domain in plan_cargo_workspace_domains(files, dir_files, &cargo_workspace_hints) {
            claimed_cargo_dirs.insert(domain.root_dir.clone());
            claimed_cargo_dirs.extend(domain.members.iter().map(|member| member.dir_path.clone()));
            domains.push(TopologyDomain::CargoWorkspace(domain));
        }

        let dart_workspace_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::DartWorkspaceRoot(hint) => Some(hint),
                _ => None,
            })
            .collect();

        for domain in plan_dart_workspace_domains(files, &dart_workspace_hints) {
            claimed_dart_dirs.insert(domain.root_dir.clone());
            claimed_dart_dirs.extend(domain.members.iter().map(|member| member.dir_path.clone()));
            domains.push(TopologyDomain::DartWorkspace(domain));
        }

        let go_workspace_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(_) => None,
                TopologyHint::GoWorkspaceRoot(hint) => Some(hint),
                _ => None,
            })
            .collect();

        for domain in plan_go_workspace_domains(dir_files, &go_workspace_hints) {
            claimed_go_dirs.insert(domain.root_dir.clone());
            domains.push(TopologyDomain::GoWorkspace(domain));
        }

        let gradle_multi_project_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::GradleMultiProjectRoot(hint) => Some(hint),
                _ => None,
            })
            .collect();

        for domain in plan_gradle_multi_project_domains(files, &gradle_multi_project_hints) {
            domains.push(TopologyDomain::GradleMultiProject(domain));
        }

        let hackage_project_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::CargoWorkspaceRoot(_) => None,
                TopologyHint::GoWorkspaceRoot(_) => None,
                TopologyHint::HackageProject(hint) => Some(hint),
                _ => None,
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
                _ => None,
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
                _ => None,
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
                _ => None,
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
                _ => None,
            })
            .collect();

        for domain in plan_pixi_domains(dir_files, &pixi_root_hints) {
            claimed_pixi_dirs.insert(domain.root_dir.clone());
            domains.push(TopologyDomain::Pixi(domain));
        }

        let uv_workspace_hints: Vec<_> = hints
            .iter()
            .filter_map(|hint| match hint {
                TopologyHint::UvWorkspaceRoot(hint) => Some(hint),
                _ => None,
            })
            .collect();

        for domain in plan_uv_workspace_domains(files, &uv_workspace_hints) {
            domains.push(TopologyDomain::UvWorkspace(domain));
        }

        Self {
            domains,
            claimed_cargo_dirs,
            claimed_dart_dirs,
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

        if config.datasource_ids.contains(&DatasourceId::PubspecYaml) {
            return self.claimed_dart_dirs.contains(parent_dir);
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
                | TopologyDomain::DartWorkspace(_)
                | TopologyDomain::GradleMultiProject(_)
                | TopologyDomain::MixUmbrella(_)
                | TopologyDomain::MavenReactor(_)
                | TopologyDomain::NpmWorkspace(_)
                | TopologyDomain::UvWorkspace(_) => {}
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
                | TopologyDomain::DartWorkspace(_)
                | TopologyDomain::GradleMultiProject(_)
                | TopologyDomain::HackageProject(_)
                | TopologyDomain::MixUmbrella(_)
                | TopologyDomain::MavenReactor(_)
                | TopologyDomain::NpmWorkspace(_)
                | TopologyDomain::Pixi(_)
                | TopologyDomain::UvWorkspace(_) => {}
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
                | TopologyDomain::DartWorkspace(_)
                | TopologyDomain::GoWorkspace(_)
                | TopologyDomain::GradleMultiProject(_)
                | TopologyDomain::HackageProject(_)
                | TopologyDomain::MixUmbrella(_)
                | TopologyDomain::MavenReactor(_)
                | TopologyDomain::Pixi(_)
                | TopologyDomain::UvWorkspace(_) => {}
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
                | TopologyDomain::DartWorkspace(_)
                | TopologyDomain::GoWorkspace(_)
                | TopologyDomain::GradleMultiProject(_)
                | TopologyDomain::HackageProject(_)
                | TopologyDomain::MavenReactor(_)
                | TopologyDomain::NpmWorkspace(_)
                | TopologyDomain::Pixi(_)
                | TopologyDomain::UvWorkspace(_) => {}
                TopologyDomain::MixUmbrella(domain) => {
                    apply_mix_umbrella_domain(domain, files, packages, dependencies);
                }
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

    pub(super) fn apply_gradle_multi_project_domains(
        &self,
        files: &mut [FileInfo],
        packages: &mut Vec<Package>,
        dependencies: &mut Vec<TopLevelDependency>,
    ) {
        let mut scope_roots = Vec::new();
        let mut anchor_indices = Vec::new();

        for domain in &self.domains {
            let TopologyDomain::GradleMultiProject(domain) = domain else {
                continue;
            };

            scope_roots.push(domain.root_dir.clone());
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
                if let Some(member_dir) = Path::new(&files[member_idx].path).parent() {
                    scope_roots.push(member_dir.to_path_buf());
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

    pub(super) fn apply_uv_workspace_domains(&self, files: &mut [FileInfo]) {
        let mut scope_roots = Vec::new();
        let mut anchor_indices = Vec::new();
        let mut protected_project_dirs = Vec::new();

        for domain in &self.domains {
            let TopologyDomain::UvWorkspace(domain) = domain else {
                continue;
            };

            scope_roots.push(domain.root_dir.clone());
            anchor_indices.push(domain.root_pyproject_idx);
            for &member_idx in &domain.member_pyproject_indices {
                if let Some(member_dir) = Path::new(&files[member_idx].path).parent() {
                    scope_roots.push(member_dir.to_path_buf());
                }
                anchor_indices.push(member_idx);
            }
        }

        let anchor_set: HashSet<usize> = anchor_indices.iter().copied().collect();
        for (idx, file) in files.iter().enumerate() {
            if anchor_set.contains(&idx)
                || Path::new(&file.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    != Some("pyproject.toml")
            {
                continue;
            }
            let Some(project_dir) = Path::new(&file.path).parent() else {
                continue;
            };
            if scope_roots.iter().any(|root| project_dir.starts_with(root)) {
                protected_project_dirs.push(project_dir.to_path_buf());
            }
        }

        assign_unowned_files_to_anchors(
            files,
            &scope_roots,
            &anchor_indices,
            &[],
            &protected_project_dirs,
        );
    }
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
    let Some(build_dir) = Path::new(&files[build_idx].path).parent() else {
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
        if path.parent() != Some(build_dir)
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
fn assign_unowned_files_to_anchors(
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

/// Maven build output (`target/classes`, `target/test-classes`, …) sits directly
/// underneath a module directory as its immediate `target/` child; skip only
/// that subtree so compiled artifacts are not attributed back to the source
/// package. A `target` path segment deeper in the tree (e.g. a source package
/// literally named `target`, as in `src/main/java/com/example/target/Foo.java`)
/// is not build output and must still receive ownership.
fn crosses_maven_build_output_dir(anchor_dir: &Path, file_dir: &Path) -> bool {
    file_dir
        .strip_prefix(anchor_dir)
        .map(|relative| {
            relative.components().next() == Some(Component::Normal(OsStr::new("target")))
        })
        .unwrap_or(false)
}

fn collect_gradle_multi_project_hints(files: &[FileInfo]) -> Vec<GradleMultiProjectRootHint> {
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
        let Some(root_dir) = path.parent() else {
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
        hints.push(GradleMultiProjectRootHint {
            root_dir: root_dir.to_path_buf(),
            project_paths,
            root_project_name,
        });
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

fn plan_gradle_multi_project_domains(
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
                let project_dir = normalize_lexical_path(&hint.root_dir.join(project));
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
        path.parent() == Some(directory)
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

fn collect_uv_workspace_hints(files: &[FileInfo]) -> Vec<UvWorkspaceRootHint> {
    let mut hints = Vec::new();

    for (idx, file) in files.iter().enumerate() {
        let path = Path::new(&file.path);
        if path.file_name().and_then(|name| name.to_str()) != Some("pyproject.toml") {
            continue;
        }
        for data in &file.package_data {
            if !matches!(
                data.datasource_id,
                Some(DatasourceId::PypiPyprojectToml | DatasourceId::PypiPoetryPyprojectToml)
            ) {
                continue;
            }
            let member_patterns = extra_string_array(data, "workspace_members");
            if member_patterns.is_empty() {
                continue;
            }
            let Some(root_dir) = path.parent() else {
                continue;
            };
            hints.push(UvWorkspaceRootHint {
                root_dir: root_dir.to_path_buf(),
                root_pyproject_idx: idx,
                member_patterns,
                exclude_patterns: extra_string_array(data, "workspace_exclude"),
            });
        }
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

fn gradle_extra_string(data: &crate::models::PackageData, key: &str) -> Option<String> {
    data.extra_data
        .as_ref()
        .and_then(|extra| extra.get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn extra_string_array(data: &crate::models::PackageData, key: &str) -> Vec<String> {
    data.extra_data
        .as_ref()
        .and_then(|extra| extra.get(key))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect()
}

fn plan_uv_workspace_domains(
    files: &[FileInfo],
    hints: &[&UvWorkspaceRootHint],
) -> Vec<UvWorkspaceDomain> {
    let mut domains = Vec::new();

    for hint in hints {
        let mut member_pyproject_indices =
            files
                .iter()
                .enumerate()
                .filter_map(|(idx, file)| {
                    if idx == hint.root_pyproject_idx {
                        return None;
                    }
                    let path = Path::new(&file.path);
                    if path.file_name().and_then(|name| name.to_str()) != Some("pyproject.toml")
                        || !file.package_data.iter().any(|data| {
                            matches!(
                                data.datasource_id,
                                Some(
                                    DatasourceId::PypiPyprojectToml
                                        | DatasourceId::PypiPoetryPyprojectToml
                                )
                            ) && data.purl.is_some()
                        })
                    {
                        return None;
                    }
                    let member_dir = path.parent()?;
                    let included = hint.member_patterns.iter().any(|pattern| {
                        workspace_pattern_matches(&hint.root_dir, member_dir, pattern)
                    });
                    let excluded = hint.exclude_patterns.iter().any(|pattern| {
                        workspace_exclude_matches(&hint.root_dir, member_dir, pattern)
                    });
                    (included && !excluded).then_some(idx)
                })
                .collect::<Vec<_>>();
        member_pyproject_indices.sort_by(|left, right| files[*left].path.cmp(&files[*right].path));

        if member_pyproject_indices.is_empty() {
            continue;
        }
        domains.push(UvWorkspaceDomain {
            root_dir: hint.root_dir.clone(),
            root_pyproject_idx: hint.root_pyproject_idx,
            member_pyproject_indices,
        });
    }

    domains.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    domains
}

fn workspace_pattern_matches(root_dir: &Path, member_dir: &Path, pattern: &str) -> bool {
    let pattern = pattern.trim().strip_prefix("./").unwrap_or(pattern.trim());
    if pattern.is_empty() {
        return false;
    }

    if !pattern
        .chars()
        .any(|character| matches!(character, '*' | '?' | '['))
    {
        return normalize_lexical_path(&root_dir.join(pattern)) == member_dir;
    }

    let Ok(relative) = member_dir.strip_prefix(root_dir) else {
        return false;
    };
    glob::Pattern::new(pattern)
        .ok()
        .is_some_and(|glob| glob.matches_path(relative))
}

/// Whether `member_dir` is excluded by a uv workspace `exclude` pattern.
///
/// Glob excludes match exactly like [`workspace_pattern_matches`]. A literal
/// (non-glob) exclude additionally rejects any member nested *under* the
/// excluded directory: `exclude = ["packages/ignored"]` must also drop
/// `packages/ignored/sub`, which a recursive `members = ["packages/**"]` glob
/// would otherwise pull back in. `Path::starts_with` is component-wise, so
/// `packages/ignored` does not spuriously exclude a sibling like
/// `packages/ignored-2`.
fn workspace_exclude_matches(root_dir: &Path, member_dir: &Path, pattern: &str) -> bool {
    let trimmed = pattern.trim().strip_prefix("./").unwrap_or(pattern.trim());
    if trimmed.is_empty() {
        return false;
    }

    if !trimmed
        .chars()
        .any(|character| matches!(character, '*' | '?' | '['))
    {
        let excluded_dir = normalize_lexical_path(&root_dir.join(trimmed));
        return member_dir.starts_with(&excluded_dir);
    }

    workspace_pattern_matches(root_dir, member_dir, pattern)
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
