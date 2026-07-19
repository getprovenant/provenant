// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::HashSet;

use crate::models::{
    DatasourceId, FileInfo, Package, PackageData, PackageType, TopLevelDependency,
};
use strum::EnumIter;

use super::{
    AssemblerConfig, AssemblyMode, DirectoryMergeOutput, bazel_prune, clojure_deps_assign,
    conda_rootfs_merge, debian_source_merge, file_ref_resolve, ivy_dependencies_properties_assign,
    nix_flake_compat_merge, npm_resource_assign, nuget_cpm_resolve, python_requirements_assign,
    resource_assign, swift_merge, topology,
};

// ── Bespoke per-directory mergers (see AssemblerConfig::directory_merger) ──
//
// Each wrapper adapts an ecosystem's directory merger to the uniform
// [`super::DirectoryMergeFn`] signature so it can be attached directly to its
// `AssemblerConfig` row, replacing the generic per-directory engine for that
// config's claimed directories.

/// Swift skips per-directory assembly entirely; its packages are formed by the
/// `SwiftMerge` post-assembly pass instead.
fn merge_swift_skip(
    _config: &AssemblerConfig,
    _files: &[FileInfo],
    _file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    Vec::new()
}

fn merge_cocoapods(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    super::cocoapods_merge::assemble_cocoapods_packages(config, files, file_indices)
}

fn merge_debian_source(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    debian_source_merge::assemble_debian_source_packages(config, files, file_indices)
}

fn merge_hackage(
    _config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    super::hackage_merge::assemble_hackage_packages(files, file_indices)
}

fn merge_huggingface(
    _config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    super::huggingface_merge::assemble_huggingface_packages(files, file_indices)
}

fn merge_windows_update(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    super::windows_update_merge::assemble_windows_update_packages(config, files, file_indices)
}

/// A per-file workspace/reactor marker detected while collecting
/// [`PostAssemblyInputs`]. A pass whose `should_run` needs "did any manifest in
/// this scan declare a workspace/umbrella/reactor?" consults these instead of
/// re-scanning every `PackageData`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum WorkspaceMarker {
    NpmWorkspace,
    CargoWorkspace,
    MixUmbrella,
    MavenReactor,
    GradleMultiProject,
    UvWorkspace,
    DartWorkspace,
}

/// Recognizes a [`WorkspaceMarker`] in a single `PackageData`. Registered rows
/// are applied once per `PackageData` while collecting [`PostAssemblyInputs`],
/// so adding a marker is one [`MARKER_DETECTORS`] row rather than a new bool
/// field plus a hand-copied collect branch.
struct MarkerDetector {
    marker: WorkspaceMarker,
    detect: fn(DatasourceId, &PackageData) -> bool,
}

static MARKER_DETECTORS: &[MarkerDetector] = &[
    MarkerDetector {
        marker: WorkspaceMarker::NpmWorkspace,
        detect: |datasource_id, data| {
            matches!(
                datasource_id,
                DatasourceId::NpmPackageJson | DatasourceId::PnpmWorkspaceYaml
            ) && data
                .extra_data
                .as_ref()
                .is_some_and(|extra_data| extra_data.contains_key("workspaces"))
        },
    },
    MarkerDetector {
        marker: WorkspaceMarker::CargoWorkspace,
        detect: |datasource_id, data| {
            datasource_id == DatasourceId::CargoToml
                && data
                    .extra_data
                    .as_ref()
                    .and_then(|extra_data| extra_data.get("workspace"))
                    .and_then(|workspace| workspace.get("members"))
                    .and_then(|members| members.as_array())
                    .is_some_and(|members| !members.is_empty())
        },
    },
    MarkerDetector {
        marker: WorkspaceMarker::MixUmbrella,
        detect: |datasource_id, data| {
            datasource_id == DatasourceId::HexMixExs
                && data
                    .extra_data
                    .as_ref()
                    .is_some_and(|extra_data| extra_data.contains_key("apps_path"))
        },
    },
    MarkerDetector {
        marker: WorkspaceMarker::MavenReactor,
        detect: |datasource_id, data| {
            datasource_id == DatasourceId::MavenPom
                && data
                    .extra_data
                    .as_ref()
                    .and_then(|extra_data| extra_data.get("modules"))
                    .and_then(|modules| modules.as_array())
                    .is_some_and(|modules| !modules.is_empty())
        },
    },
    MarkerDetector {
        marker: WorkspaceMarker::GradleMultiProject,
        detect: |datasource_id, data| {
            datasource_id == DatasourceId::GradleSettings
                && data
                    .extra_data
                    .as_ref()
                    .and_then(|extra_data| extra_data.get("projects"))
                    .and_then(|projects| projects.as_array())
                    .is_some_and(|projects| !projects.is_empty())
        },
    },
    MarkerDetector {
        marker: WorkspaceMarker::UvWorkspace,
        detect: |datasource_id, data| {
            matches!(
                datasource_id,
                DatasourceId::PypiPyprojectToml | DatasourceId::PypiPoetryPyprojectToml
            ) && data
                .extra_data
                .as_ref()
                .and_then(|extra_data| extra_data.get("workspace_members"))
                .and_then(|members| members.as_array())
                .is_some_and(|members| !members.is_empty())
        },
    },
    MarkerDetector {
        marker: WorkspaceMarker::DartWorkspace,
        detect: |datasource_id, data| {
            datasource_id == DatasourceId::PubspecYaml
                && data
                    .extra_data
                    .as_ref()
                    .and_then(|extra_data| extra_data.get("workspace_members"))
                    .and_then(|members| members.as_array())
                    .is_some_and(|members| !members.is_empty())
        },
    },
];

/// Stable identity of a post-assembly pass, used for registry coverage tests
/// (mirrors [`super::topology`]'s `TopologyFamilyId`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumIter)]
pub(super) enum PostAssemblyPassId {
    SwiftMerge,
    CondaRootfsMerge,
    NpmResourceAssign,
    PythonRequirementsAssign,
    IvyDependenciesPropertiesAssign,
    ClojureDepsEdnAssign,
    FileReferenceResolve,
    RpmYumdbMerge,
    NpmWorkspaceMerge,
    CargoWorkspaceMerge,
    MixUmbrellaMerge,
    MavenReactorAssign,
    GradleMultiProjectAssign,
    UvWorkspaceAssign,
    DartWorkspaceMerge,
    NugetCpmResolve,
    CargoResourceAssign,
    ComposerResourceAssign,
    RubyResourceAssign,
    NixFlakeCompatMerge,
    BazelPrune,
}

/// A registered post-assembly pass: its identity, the gate deciding whether the
/// scan's inputs make it relevant, and the mutation it applies. Adding a pass is
/// one [`POST_ASSEMBLY_PASSES`] row (plus a [`MARKER_DETECTORS`] row when it
/// gates on a workspace marker) rather than an enum variant plus two match arms.
struct PostAssemblyPass {
    id: PostAssemblyPassId,
    should_run: fn(&PostAssemblyInputs) -> bool,
    run: fn(
        &mut [FileInfo],
        &mut Vec<Package>,
        &mut Vec<TopLevelDependency>,
        &topology::TopologyPlan,
    ),
}

static POST_ASSEMBLY_PASSES: &[PostAssemblyPass] = &[
    PostAssemblyPass {
        id: PostAssemblyPassId::SwiftMerge,
        should_run: |inputs| inputs.has_any_file_datasource(SWIFT_POST_ASSEMBLY_DATASOURCE_IDS),
        run: |files, packages, dependencies, _plan| {
            swift_merge::assemble_swift_packages(files, packages, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::CondaRootfsMerge,
        should_run: |inputs| {
            inputs.has_all_file_datasources(CONDA_ROOTFS_POST_ASSEMBLY_DATASOURCE_IDS)
        },
        run: |files, packages, dependencies, _plan| {
            conda_rootfs_merge::merge_conda_rootfs_metadata(files, packages, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::NpmResourceAssign,
        should_run: |inputs| inputs.has_package_type(PackageType::Npm),
        run: |files, packages, _dependencies, _plan| {
            npm_resource_assign::assign_npm_package_resources(files, packages)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::PythonRequirementsAssign,
        should_run: |inputs| {
            inputs.has_package_type(PackageType::Pypi)
                && inputs.has_any_file_datasource(&[DatasourceId::PipRequirements])
        },
        run: |files, packages, dependencies, _plan| {
            python_requirements_assign::assign_python_requirements_to_projects(
                files,
                packages,
                dependencies,
            )
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::IvyDependenciesPropertiesAssign,
        should_run: |inputs| {
            inputs.has_package_type(PackageType::Ivy)
                && inputs.has_any_file_datasource(&[DatasourceId::AntIvyDependenciesProperties])
        },
        run: |files, packages, dependencies, _plan| {
            ivy_dependencies_properties_assign::assign_ivy_dependencies_properties_to_projects(
                files,
                packages,
                dependencies,
            )
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::ClojureDepsEdnAssign,
        should_run: |inputs| {
            inputs.has_package_type(PackageType::Maven)
                && inputs.has_all_file_datasources(&[
                    DatasourceId::ClojureProjectClj,
                    DatasourceId::ClojureDepsEdn,
                ])
        },
        run: |files, packages, dependencies, _plan| {
            clojure_deps_assign::assign_clojure_deps_edn_to_projects(files, packages, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::FileReferenceResolve,
        should_run: |inputs| {
            file_ref_resolve::has_relevant_file_reference_datasource_ids(
                &inputs.file_datasource_ids,
            )
        },
        run: |files, packages, dependencies, _plan| {
            file_ref_resolve::resolve_file_references(files, packages, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::RpmYumdbMerge,
        should_run: |inputs| {
            inputs.has_any_file_datasource(&[DatasourceId::RpmYumdb])
                && inputs.has_any_file_datasource(RPM_INSTALLED_DATABASE_DATASOURCE_IDS)
        },
        run: |files, packages, _dependencies, _plan| {
            file_ref_resolve::merge_rpm_yumdb_metadata(files, packages)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::NpmWorkspaceMerge,
        should_run: |inputs| inputs.has_marker(WorkspaceMarker::NpmWorkspace),
        run: |files, packages, dependencies, plan| {
            plan.apply_npm_workspace_domains(files, packages, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::CargoWorkspaceMerge,
        should_run: |inputs| inputs.has_marker(WorkspaceMarker::CargoWorkspace),
        run: |files, packages, dependencies, plan| {
            plan.apply_cargo_workspace_domains(files, packages, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::MixUmbrellaMerge,
        should_run: |inputs| inputs.has_marker(WorkspaceMarker::MixUmbrella),
        run: |files, packages, dependencies, plan| {
            plan.apply_mix_umbrella_domains(files, packages, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::MavenReactorAssign,
        should_run: |inputs| inputs.has_marker(WorkspaceMarker::MavenReactor),
        run: |files, _packages, _dependencies, plan| plan.apply_maven_reactor_domains(files),
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::GradleMultiProjectAssign,
        should_run: |inputs| inputs.has_marker(WorkspaceMarker::GradleMultiProject),
        run: |files, packages, dependencies, plan| {
            plan.apply_gradle_multi_project_domains(files, packages, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::UvWorkspaceAssign,
        should_run: |inputs| inputs.has_marker(WorkspaceMarker::UvWorkspace),
        run: |files, _packages, dependencies, plan| {
            plan.apply_uv_workspace_domains(files, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::DartWorkspaceMerge,
        should_run: |inputs| inputs.has_marker(WorkspaceMarker::DartWorkspace),
        run: |files, packages, dependencies, plan| {
            plan.apply_dart_workspace_domains(files, packages, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::NugetCpmResolve,
        should_run: |inputs| {
            inputs.has_any_file_datasource(NUGET_CPM_CONFIG_DATASOURCE_IDS)
                && inputs.has_any_file_datasource(NUGET_CPM_PROJECT_DATASOURCE_IDS)
        },
        run: |files, _packages, dependencies, _plan| {
            nuget_cpm_resolve::resolve_nuget_cpm_versions(files, dependencies)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::CargoResourceAssign,
        should_run: |inputs| inputs.has_package_type(PackageType::Cargo),
        run: |files, packages, _dependencies, _plan| {
            resource_assign::assign_resources_for(PackageType::Cargo, files, packages)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::ComposerResourceAssign,
        should_run: |inputs| inputs.has_package_type(PackageType::Composer),
        run: |files, packages, _dependencies, _plan| {
            resource_assign::assign_resources_for(PackageType::Composer, files, packages)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::RubyResourceAssign,
        should_run: |inputs| inputs.has_package_type(PackageType::Gem),
        run: |files, packages, _dependencies, _plan| {
            resource_assign::assign_resources_for(PackageType::Gem, files, packages)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::NixFlakeCompatMerge,
        should_run: |inputs| {
            inputs.has_any_file_datasource(&[DatasourceId::NixDefaultNix])
                && inputs.has_any_file_datasource(&[
                    DatasourceId::NixFlakeNix,
                    DatasourceId::NixFlakeLock,
                ])
        },
        run: |files, packages, _dependencies, _plan| {
            nix_flake_compat_merge::attach_flake_compat_default_files(files, packages)
        },
    },
    PostAssemblyPass {
        id: PostAssemblyPassId::BazelPrune,
        should_run: |inputs| inputs.has_package_type(PackageType::Bazel),
        run: |files, packages, dependencies, _plan| {
            bazel_prune::prune_unused_bazel_packages(files, packages, dependencies)
        },
    },
];

const SWIFT_POST_ASSEMBLY_DATASOURCE_IDS: &[DatasourceId] = &[
    DatasourceId::SwiftPackageManifestJson,
    DatasourceId::SwiftPackageResolved,
    DatasourceId::SwiftPackageShowDependencies,
];

const CONDA_ROOTFS_POST_ASSEMBLY_DATASOURCE_IDS: &[DatasourceId] =
    &[DatasourceId::CondaMetaJson, DatasourceId::CondaMetaYaml];

const RPM_INSTALLED_DATABASE_DATASOURCE_IDS: &[DatasourceId] = &[
    DatasourceId::RpmInstalledDatabaseBdb,
    DatasourceId::RpmInstalledDatabaseNdb,
    DatasourceId::RpmInstalledDatabaseSqlite,
];

const NUGET_CPM_CONFIG_DATASOURCE_IDS: &[DatasourceId] = &[
    DatasourceId::NugetDirectoryBuildProps,
    DatasourceId::NugetDirectoryPackagesProps,
];

const NUGET_CPM_PROJECT_DATASOURCE_IDS: &[DatasourceId] = &[
    DatasourceId::NugetCsproj,
    DatasourceId::NugetFsproj,
    DatasourceId::NugetVbproj,
];

#[derive(Default)]
struct PostAssemblyInputs {
    package_types: HashSet<PackageType>,
    file_datasource_ids: HashSet<DatasourceId>,
    markers: HashSet<WorkspaceMarker>,
}

pub(super) fn run_post_assembly_passes(
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
    topology_plan: &topology::TopologyPlan,
) {
    let inputs = PostAssemblyInputs::collect(files, packages);

    for pass in POST_ASSEMBLY_PASSES {
        if !(pass.should_run)(&inputs) {
            continue;
        }

        log::trace!("running post-assembly pass {:?}", pass.id);
        (pass.run)(files, packages, dependencies, topology_plan);
    }
}

impl PostAssemblyInputs {
    fn collect(files: &[FileInfo], packages: &[Package]) -> Self {
        let mut inputs = Self {
            package_types: packages
                .iter()
                .filter_map(|package| package.package_type)
                .collect(),
            ..Self::default()
        };

        for file in files {
            for package_data in &file.package_data {
                let Some(datasource_id) = package_data.datasource_id else {
                    continue;
                };

                inputs.file_datasource_ids.insert(datasource_id);

                for detector in MARKER_DETECTORS {
                    if !inputs.markers.contains(&detector.marker)
                        && (detector.detect)(datasource_id, package_data)
                    {
                        inputs.markers.insert(detector.marker);
                    }
                }
            }
        }

        inputs
    }

    fn has_package_type(&self, package_type: PackageType) -> bool {
        self.package_types.contains(&package_type)
    }

    fn has_marker(&self, marker: WorkspaceMarker) -> bool {
        self.markers.contains(&marker)
    }

    fn has_any_file_datasource(&self, datasource_ids: &[DatasourceId]) -> bool {
        datasource_ids
            .iter()
            .any(|datasource_id| self.file_datasource_ids.contains(datasource_id))
    }

    fn has_all_file_datasources(&self, datasource_ids: &[DatasourceId]) -> bool {
        datasource_ids
            .iter()
            .all(|datasource_id| self.file_datasource_ids.contains(datasource_id))
    }
}

pub static ASSEMBLERS: &[AssemblerConfig] = &[
    // ── Sibling-merge assemblers ──
    //
    // npm ecosystem: package.json + lockfiles in same directory.
    // NOTE: npm-shrinkwrap.json emits "npm_package_lock_json" as its datasource_id,
    // so "npm_shrinkwrap_json" is NOT a real datasource_id.
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::BunLock,
            DatasourceId::BunLockb,
            DatasourceId::NpmPackageJson,
            DatasourceId::NpmPackageLockJson,
            DatasourceId::YarnLock,
            DatasourceId::YarnLockV1,
            DatasourceId::YarnLockV2,
            DatasourceId::YarnPnpCjs,
            DatasourceId::PnpmLockYaml,
            DatasourceId::PnpmWorkspaceYaml,
        ],
        sibling_file_patterns: &[
            "package.json",
            "bun.lock",
            "bun.lockb",
            ".package-lock.json",
            "package-lock.json",
            ".npm-shrinkwrap.json",
            "npm-shrinkwrap.json",
            "yarn.lock",
            ".pnp.cjs",
            "pnpm-lock.yaml",
            "shrinkwrap.yaml",
            "pnpm-workspace.yaml",
        ],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Rust/Cargo ecosystem
    AssemblerConfig {
        datasource_ids: &[DatasourceId::CargoToml, DatasourceId::CargoLock],
        sibling_file_patterns: &["Cargo.toml", "Cargo.lock"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Julia ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::JuliaProjectToml,
            DatasourceId::JuliaManifestToml,
        ],
        sibling_file_patterns: &["Project.toml", "Manifest.toml"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Erlang/OTP Rebar ecosystem
    AssemblerConfig {
        datasource_ids: &[DatasourceId::RebarConfig, DatasourceId::RebarLock],
        sibling_file_patterns: &["rebar.config", "rebar.lock"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Elixir/Hex ecosystem: `mix.exs` carries the project identity (app + version)
    // and its direct deps; `mix.lock` contributes the resolved locked deps. They
    // sibling-merge into one `pkg:hex/<app>` package, the same shape as
    // Cargo.toml + Cargo.lock. A standalone `mix.lock` with no sibling `mix.exs`
    // has no identity, so its deps stay hoisted (no package is formed).
    AssemblerConfig {
        datasource_ids: &[DatasourceId::HexMixExs, DatasourceId::HexMixLock],
        sibling_file_patterns: &["mix.exs", "mix.lock"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Erlang OTP application resource files (`src/<app>.app.src`). The app name
    // and version live in the `{application, <name>, [{vsn, ...}]}` tuple, so the
    // `.app.src` is the app's identity source (`pkg:hex/<app>`); one package per
    // record. (`rebar.config` carries build config/deps, not the app identity.)
    AssemblerConfig {
        datasource_ids: &[DatasourceId::ErlangOtpAppSrc],
        sibling_file_patterns: &["*.app.src"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Carthage ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::CarthageCartfile,
            DatasourceId::CarthageCartfileResolved,
        ],
        sibling_file_patterns: &["Cartfile", "Cartfile.private", "Cartfile.resolved"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // CocoaPods ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::CocoapodsPodspec,
            DatasourceId::CocoapodsPodspecJson,
            DatasourceId::CocoapodsPodfile,
            DatasourceId::CocoapodsPodfileLock,
        ],
        sibling_file_patterns: &["*.podspec", "*.podspec.json", "Podfile", "Podfile.lock"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: Some(merge_cocoapods),
    },
    // PHP Composer ecosystem
    AssemblerConfig {
        datasource_ids: &[DatasourceId::PhpComposerJson, DatasourceId::PhpComposerLock],
        sibling_file_patterns: &[
            "*composer.json",
            "composer.*.json",
            "*composer.lock",
            "composer.*.lock",
        ],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Go ecosystem (includes legacy Godeps)
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::GoMod,
            DatasourceId::GoModGraph,
            DatasourceId::GoSum,
            DatasourceId::GoWork,
            DatasourceId::Godeps,
        ],
        sibling_file_patterns: &[
            "go.mod",
            "go.work",
            "go.mod.graph",
            "go.modgraph",
            "go.sum",
            "Godeps.json",
        ],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Dart/Flutter ecosystem
    AssemblerConfig {
        datasource_ids: &[DatasourceId::PubspecYaml, DatasourceId::PubspecLock],
        sibling_file_patterns: &["pubspec.yaml", "pubspec.lock"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Pixi ecosystem
    AssemblerConfig {
        datasource_ids: &[DatasourceId::PixiToml, DatasourceId::PixiLock],
        sibling_file_patterns: &["pixi.toml", "pixi.lock"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::NixFlakeNix, DatasourceId::NixFlakeLock],
        sibling_file_patterns: &["flake.nix", "flake.lock"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::NixDefaultNix],
        sibling_file_patterns: &["default.nix"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Helm chart ecosystem
    AssemblerConfig {
        datasource_ids: &[DatasourceId::HelmChartYaml, DatasourceId::HelmChartLock],
        sibling_file_patterns: &["Chart.yaml", "Chart.lock"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::HackageCabal,
            DatasourceId::HackageCabalProject,
            DatasourceId::HackageStackYaml,
        ],
        sibling_file_patterns: &["*.cabal", "cabal.project", "stack.yaml"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: Some(merge_hackage),
    },
    // Chef ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::ChefCookbookMetadataJson,
            DatasourceId::ChefCookbookMetadataRb,
        ],
        sibling_file_patterns: &["metadata.json", "metadata.rb"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Conan (C/C++) ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::ConanConanFilePy,
            DatasourceId::ConanConanFileTxt,
            DatasourceId::ConanLock,
            DatasourceId::ConanConanDataYml,
        ],
        sibling_file_patterns: &[
            "conanfile.py",
            "conanfile.txt",
            "conan.lock",
            "conandata.yml",
        ],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // vcpkg (C/C++) ports and manifests. Each `CONTROL` or `vcpkg.json` that
    // names a port/project is an independent package, so one package per record:
    // a ports tree holds hundreds of distinct `CONTROL`/`vcpkg.json` files that
    // must each surface (and own their `Build-Depends`/`dependencies`) rather
    // than being dropped. `vcpkg-configuration.json` and `vcpkg-lock.json` carry
    // no package identity and stay unassembled.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::VcpkgControl, DatasourceId::VcpkgJson],
        sibling_file_patterns: &["CONTROL", "vcpkg.json"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Maven/Java ecosystem (nested merge via META-INF)
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::MavenPom,
            DatasourceId::MavenPomProperties,
            DatasourceId::JavaJarManifest,
            DatasourceId::JavaOsgiManifest,
        ],
        sibling_file_patterns: &[
            "pom.xml",
            "*.pom",
            "pom.properties",
            "**/META-INF/MANIFEST.MF",
        ],
        // A directory can hold one module (pom.xml + supplementary siblings) or
        // many standalone `.pom` files with distinct GAVs; split per identity.
        mode: AssemblyMode::SiblingMergePerIdentity,
        directory_merger: None,
    },
    // Leiningen `project.clj` declares a `defproject` with Maven coordinates, so
    // each is an independent package that owns its `:dependencies`. One package
    // per record; a co-located `deps.edn` is attached by a post-assembly pass,
    // while standalone `deps.edn` manifests keep their unowned hoisted deps.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::ClojureProjectClj],
        sibling_file_patterns: &["project.clj"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // sbt `build.sbt` declares a project with Maven coordinates and owns its
    // `libraryDependencies`. One package per record so the project surfaces and
    // owns its deps instead of orphaning them.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::SbtBuildSbt],
        sibling_file_patterns: &["build.sbt"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::PypiWheel, DatasourceId::PypiPipOriginJson],
        sibling_file_patterns: &["*.whl", "origin.json"],
        // A wheelhouse (`pip download -d wheelhouse/`) holds many distinct wheels
        // in one directory, each a distinct `pkg:pypi/<name>@<v>` identity, so one
        // package per identity. A pip-cache leaf directory (one wheel plus its
        // `origin.json`, sharing the wheel's identity) falls back to one package.
        mode: AssemblyMode::SiblingMergePerIdentity,
        directory_merger: None,
    },
    // Python/PyPI ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::PypiPyprojectToml,
            DatasourceId::PypiPoetryPyprojectToml,
            DatasourceId::PypiSetupPy,
            DatasourceId::PypiSetupCfg,
            DatasourceId::PypiWheelMetadata,
            DatasourceId::PypiEgg,
            DatasourceId::PypiEggPkginfo,
            DatasourceId::PypiEditableEggPkginfo,
            DatasourceId::PypiJson,
            DatasourceId::PypiSdist,
            DatasourceId::PypiSdistPkginfo,
            DatasourceId::PypiInspectDeplock,
            DatasourceId::PipRequirements,
            DatasourceId::PypiPoetryLock,
            DatasourceId::PypiPylockToml,
            DatasourceId::PypiUvLock,
            DatasourceId::Pipfile,
            DatasourceId::PipfileLock,
        ],
        sibling_file_patterns: &[
            "pyproject.toml",
            "setup.py",
            "setup.cfg",
            "PKG-INFO",
            "METADATA",
            "pypi.json",
            "pip-inspect.deplock",
            "*.tar.gz",
            "*.tgz",
            "*.tar.bz2",
            "*.tar.xz",
            "*.zip",
            "requirements*.txt",
            "Pipfile",
            "Pipfile.lock",
            "poetry.lock",
            "pylock.toml",
            "pylock.*.toml",
            "uv.lock",
        ],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::DenoJson, DatasourceId::DenoLock],
        sibling_file_patterns: &["deno.json", "deno.jsonc", "deno.lock"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Ruby/RubyGems ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::GemArchiveExtracted,
            DatasourceId::Gemspec,
            DatasourceId::GemspecExtracted,
            DatasourceId::Gemfile,
            DatasourceId::GemfileExtracted,
            DatasourceId::GemfileLock,
            DatasourceId::GemfileLockExtracted,
        ],
        sibling_file_patterns: &[
            "metadata.gz-extract",
            "**/data.gz-extract/*.gemspec",
            "**/data.gz-extract/Gemfile",
            "**/data.gz-extract/Gemfile.lock",
            "*.gemspec",
            "Gemfile",
            "Gemfile.lock",
        ],
        // A directory usually holds one gem (its `.gemspec` plus a `Gemfile`/
        // `Gemfile.lock`), but can hold several independent `.gemspec` files with
        // distinct identities; collapsing the latter into one package loses the
        // others, so split per identity. A single-gem directory (one purled
        // gemspec plus purl-less `Gemfile`/lock siblings) falls back unchanged.
        mode: AssemblyMode::SiblingMergePerIdentity,
        directory_merger: None,
    },
    // Installed RubyGems specifications (`specifications/*.gemspec`). A
    // `vendor/bundle` / gem-home `specifications` directory holds one gemspec
    // per installed gem, each a distinct `pkg:gem/<name>@<v>` identity, so one
    // package per record.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::GemGemspecInstalledSpecifications],
        sibling_file_patterns: &["**/specifications/*.gemspec"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::GemArchive],
        sibling_file_patterns: &["*.gem"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Conda ecosystem: recipes and environment files describe one package per
    // directory, so they sibling-merge.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::CondaMetaYaml, DatasourceId::CondaYaml],
        sibling_file_patterns: &[
            "meta.yaml",
            "meta.yml",
            "recipe.yaml",
            "recipe.yml",
            "environment.yml",
            "environment.yaml",
            "conda.yaml",
            "conda.yml",
            "*conda*.yaml",
            "*conda*.yml",
            "env.yaml",
            "env.yml",
            "*env*.yaml",
            "*env*.yml",
            "*environment*.yaml",
            "*environment*.yml",
            "*.json",
        ],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Conda installed-environment records: each `conda-meta/<pkg>.json` is one
    // installed package, like the Alpine/RPM/Debian installed databases below,
    // so every record becomes its own package rather than collapsing the whole
    // `conda-meta/` directory into a single sibling-merged package.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::CondaMetaJson],
        sibling_file_patterns: &["*.json"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // RPM specfile (source packages)
    AssemblerConfig {
        datasource_ids: &[DatasourceId::RpmSpecfile],
        sibling_file_patterns: &["*.spec"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Debian source packages (nested merge via debian/ directory)
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::DebianControlInSource,
            DatasourceId::DebianCopyrightInSource,
        ],
        sibling_file_patterns: &["control", "copyright"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: Some(merge_debian_source),
    },
    // Gradle/Android ecosystem
    AssemblerConfig {
        datasource_ids: &[DatasourceId::BuildGradle, DatasourceId::GradleLockfile],
        sibling_file_patterns: &["build.gradle", "build.gradle.kts", "gradle.lockfile"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::GradleModule],
        sibling_file_patterns: &["*.module"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Hugging Face model/dataset metadata. The model-card README.md, Transformers
    // config.json, and Diffusers model_index.json in one repository directory
    // describe a single logical model, so they are merged into one package by a
    // dedicated directory merger (see huggingface_merge). Only files the Hugging
    // Face parsers actually claim (carrying a huggingface datasource id)
    // participate, so a generic README.md/config.json never triggers a merge.
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::HuggingfaceModelCard,
            DatasourceId::HuggingfaceConfigJson,
            DatasourceId::HuggingfaceModelIndexJson,
        ],
        sibling_file_patterns: &["README.md", "config.json", "model_index.json"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: Some(merge_huggingface),
    },
    // CPAN/Perl ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::CpanMetaJson,
            DatasourceId::CpanMetaYml,
            DatasourceId::CpanManifest,
            DatasourceId::CpanDistIni,
            DatasourceId::CpanMakefile,
        ],
        sibling_file_patterns: &[
            "META.json",
            "META.yml",
            "MANIFEST",
            "dist.ini",
            "Makefile.PL",
        ],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // NuGet/.NET ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::NugetCsproj,
            DatasourceId::NugetFsproj,
            DatasourceId::NugetNuspec,
            DatasourceId::NugetNupkg,
            DatasourceId::NugetProjectJson,
            DatasourceId::NugetProjectLockJson,
            DatasourceId::NugetPackagesConfig,
            DatasourceId::NugetPackagesLock,
            DatasourceId::NugetVbproj,
        ],
        sibling_file_patterns: &[
            "*.csproj",
            "*.fsproj",
            "*.nuspec",
            "*.nupkg",
            "project.json",
            "project.lock.json",
            "packages.config",
            "packages.lock.json",
            "*.packages.lock.json",
            "*.vbproj",
        ],
        // A directory can hold one project (its `.csproj`/`.nuspec` plus
        // supplementary `packages.config`/lock siblings) or several independent
        // projects / `.nuspec` packaging outputs with distinct identities;
        // collapsing the latter into one package loses the others, so split per
        // identity. A single-identity directory falls back to one package.
        mode: AssemblyMode::SiblingMergePerIdentity,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::NugetDepsJson],
        sibling_file_patterns: &["*.deps.json"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Swift/SPM ecosystem
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::SwiftPackageManifestJson,
            DatasourceId::SwiftPackageResolved,
            DatasourceId::SwiftPackageShowDependencies,
        ],
        sibling_file_patterns: &[
            "Package.swift.json",
            "Package.swift.deplock",
            "Package.resolved",
            ".package.resolved",
            "swift-show-dependencies.deplock",
        ],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: Some(merge_swift_skip),
    },
    // VS Code extension VSIX manifests. An extracted `extension.vsixmanifest`
    // carries the extension identity (`Publisher`, `Id`, `Version`) and maps
    // directly to one `pkg:vscode-extension/<publisher>/<id>` package.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::VscodeExtensionVsixManifest],
        sibling_file_patterns: &["extension.vsixmanifest"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // ── Standalone assemblers (single file → single package) ──
    //
    // These ecosystems have only one manifest file type with no sibling merging.
    // They still need configs so their datasource_ids are recognized by the assembler.
    //
    // Bower (JavaScript)
    AssemblerConfig {
        datasource_ids: &[DatasourceId::BowerJson],
        sibling_file_patterns: &["bower.json"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // CRAN (R language)
    AssemblerConfig {
        datasource_ids: &[DatasourceId::CranDescription],
        sibling_file_patterns: &["DESCRIPTION"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // FreeBSD packages
    AssemblerConfig {
        datasource_ids: &[DatasourceId::FreebsdCompactManifest],
        sibling_file_patterns: &["+COMPACT_MANIFEST"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Haxe ecosystem
    AssemblerConfig {
        datasource_ids: &[DatasourceId::HaxelibJson],
        sibling_file_patterns: &["haxelib.json"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::Gitmodules],
        sibling_file_patterns: &[".gitmodules"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // OCaml/opam ecosystem. A multi-package opam project ships several
    // `<name>.opam` files at its root, each a distinct `pkg:opam/<name>`
    // identity; collapsing the directory into one package loses the others, so
    // split per identity. A single-package directory falls back unchanged.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::OpamFile],
        sibling_file_patterns: &["opam", "*.opam"],
        mode: AssemblyMode::SiblingMergePerIdentity,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::RpmYumdb],
        sibling_file_patterns: &["**/var/lib/yum/yumdb/*/*/from_repo"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Microsoft Update Manifest
    AssemblerConfig {
        datasource_ids: &[DatasourceId::MicrosoftUpdateManifestMum],
        sibling_file_patterns: &["*.mum"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: Some(merge_windows_update),
    },
    // Autotools (C/C++ build system)
    AssemblerConfig {
        datasource_ids: &[DatasourceId::AutotoolsConfigure],
        sibling_file_patterns: &["configure", "configure.ac"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Bazel (build system). BUILD targets sibling-merge into one component per
    // build directory rather than one package per target: internal build targets
    // carry no license/dependency/version metadata, so per-target emission only
    // floods the inventory with name-only shells. Kept consistent with Buck below.
    // See docs/improvements/bazel-buck-build-targets.md.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::BazelBuild],
        sibling_file_patterns: &["BUILD"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::BazelModule],
        sibling_file_patterns: &["MODULE.bazel"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Buck (build system)
    AssemblerConfig {
        datasource_ids: &[DatasourceId::BuckFile, DatasourceId::BuckMetadata],
        sibling_file_patterns: &["BUCK", "METADATA.bzl", ".buckconfig"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Meson (build system). Each `meson.build` that declares a `project()` is an
    // independent package rooted at its directory, so one package per identity
    // rather than a sibling merge: nested `meson.build` files in a project's
    // subdirectories carry no `project()` (hence no purl) and are skipped, so the
    // top-level package list is not flooded with subdir build files. Using
    // `OnePerPackageData` also keeps independent sibling projects from collapsing
    // into one another via nested merge.
    AssemblerConfig {
        datasource_ids: &[DatasourceId::MesonBuild],
        sibling_file_patterns: &["meson.build"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Ant/Ivy (Java dependency management)
    AssemblerConfig {
        datasource_ids: &[DatasourceId::AntIvyXml],
        sibling_file_patterns: &["ivy.xml"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // JVM archives introspected in place (one archive == one package).
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::JavaJar,
            DatasourceId::JavaWarArchive,
            DatasourceId::AndroidAarLibrary,
        ],
        sibling_file_patterns: &["*.jar", "*.war", "*.aar"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Meteor (JavaScript platform)
    AssemblerConfig {
        datasource_ids: &[DatasourceId::MeteorPackage],
        sibling_file_patterns: &["package.js"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // ── One-per-PackageData assemblers (database files with many packages) ──
    //
    // Alpine installed package database
    AssemblerConfig {
        datasource_ids: &[DatasourceId::AlpineInstalledDb],
        sibling_file_patterns: &["installed"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::AlpineApkbuild],
        sibling_file_patterns: &["APKBUILD"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    // Arch Linux package metadata. A `.SRCINFO`/`.PKGINFO`/`.AURINFO` names one
    // or more `pkg:alpm/*` packages (a split recipe emits several subpackages),
    // each owning its `depends`/`makedepends`. One package per record.
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::ArchSrcinfo,
            DatasourceId::ArchPkginfo,
            DatasourceId::ArchAurinfo,
        ],
        sibling_file_patterns: &[".SRCINFO", ".PKGINFO", ".AURINFO"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // RPM installed package databases (BDB, NDB, SQLite)
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::RpmInstalledDatabaseBdb,
            DatasourceId::RpmInstalledDatabaseNdb,
            DatasourceId::RpmInstalledDatabaseSqlite,
            DatasourceId::RpmMarinerManifest,
        ],
        sibling_file_patterns: &[
            "Packages",
            "Packages.db",
            "rpmdb.sqlite",
            "container-manifest-2",
        ],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::RpmArchive],
        sibling_file_patterns: &["*.rpm", "*.srpm"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    // Debian installed package databases
    AssemblerConfig {
        datasource_ids: &[DatasourceId::DebianDeb],
        sibling_file_patterns: &["*.deb"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::DebianInstalledStatusDb,
            DatasourceId::DebianDistrolessInstalledDb,
        ],
        sibling_file_patterns: &["status"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::DebianControlExtractedDeb,
            DatasourceId::DebianMd5SumsInExtractedDeb,
        ],
        sibling_file_patterns: &["control", "md5sums"],
        mode: AssemblyMode::SiblingMerge,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::DebianSourceControlDsc],
        sibling_file_patterns: &["*.dsc"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[DatasourceId::AboutFile],
        sibling_file_patterns: &["*.ABOUT"],
        mode: AssemblyMode::OnePerPackageData,
        directory_merger: None,
    },
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::BitbakeRecipe,
            DatasourceId::BitbakeRecipeAppend,
        ],
        sibling_file_patterns: &["*.bb", "*.bbappend"],
        // A recipe directory routinely holds several distinct `.bb` recipes
        // (e.g. per-version or sibling components) each with its own
        // `pkg:yocto/<pn>@<pv>` identity; collapsing the directory into one
        // package loses the others, so split per identity. A single-recipe
        // directory (recipe + its `.bbappend`/files) falls back to the
        // one-package result unchanged.
        mode: AssemblyMode::SiblingMergePerIdentity,
        directory_merger: None,
    },
];

// Datasource IDs intentionally excluded from package assembly.
//
// This list is runtime-significant: files with these datasource IDs may remain
// unowned by any Package, while their dependencies are still eligible for
// top-level hoisting. Tests also use it to enforce explicit assembly accounting.
/// Why a `DatasourceId` is intentionally not assembled into a top-level package.
///
/// Every entry in [`UNASSEMBLED_DATASOURCE_IDS`] must state one of these reasons.
/// There is deliberately **no** "deferred"/"TODO" variant: a datasource whose
/// parser emits a `purl`-bearing package identity with dependencies must be
/// assembled (an `OnePerPackageData` config for standalone manifests), not
/// parked here. See `docs/HOW_TO_ADD_A_PARSER.md` and ADR 0006.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnassembledReason {
    /// The file does not describe a package (README, OS-release, deployment- or
    /// image-descriptor fragment, Dockerfile, …). There is no identity to assemble.
    NotAPackage,
    /// A compiled binary or binary archive whose contents are scanned or extracted
    /// elsewhere (ExtractCode), not a source manifest with its own assembled identity.
    BinaryArtifact,
    /// Metadata that enriches another datasource's package, or is consumed by a
    /// dedicated post-assembly pass, rather than defining a package on its own.
    SupplementaryMetadata,
    /// A dependency or lock list with no package identity of its own; its
    /// dependencies are hoisted, but it cannot become a package.
    DependenciesOnlyNoIdentity,
}

pub(super) static UNASSEMBLED_DATASOURCE_IDS: &[(DatasourceId, UnassembledReason)] = &[
    (DatasourceId::Readme, UnassembledReason::NotAPackage),
    (DatasourceId::EtcOsRelease, UnassembledReason::NotAPackage),
    (
        DatasourceId::AndroidManifestXml,
        UnassembledReason::NotAPackage,
    ),
    (
        DatasourceId::AndroidSoongMetadata,
        UnassembledReason::NotAPackage,
    ),
    (DatasourceId::Dockerfile, UnassembledReason::NotAPackage),
    // A Gradle settings file declares multi-project structure (subprojects,
    // root project name), not a package. Its declared subprojects drive
    // `gradle_multiproject_merge` topology; it carries no package identity or
    // dependency list of its own.
    (DatasourceId::GradleSettings, UnassembledReason::NotAPackage),
    (DatasourceId::OciImageIndex, UnassembledReason::NotAPackage),
    (
        DatasourceId::OciImageManifest,
        UnassembledReason::NotAPackage,
    ),
    (DatasourceId::Axis2ModuleXml, UnassembledReason::NotAPackage),
    (
        DatasourceId::JavaEarApplicationXml,
        UnassembledReason::NotAPackage,
    ),
    (DatasourceId::JavaWarWebXml, UnassembledReason::NotAPackage),
    (
        DatasourceId::JbossServiceXml,
        UnassembledReason::NotAPackage,
    ),
    // Compiled binaries and binary archives (extracted/scanned elsewhere).
    (
        DatasourceId::AlpineApkArchive,
        UnassembledReason::BinaryArtifact,
    ),
    (DatasourceId::AndroidAab, UnassembledReason::BinaryArtifact),
    (DatasourceId::AndroidApk, UnassembledReason::BinaryArtifact),
    (DatasourceId::AppleDmg, UnassembledReason::BinaryArtifact),
    (DatasourceId::Axis2Mar, UnassembledReason::BinaryArtifact),
    (DatasourceId::ChromeCrx, UnassembledReason::BinaryArtifact),
    (
        DatasourceId::DebianOriginalSourceTarball,
        UnassembledReason::BinaryArtifact,
    ),
    (
        DatasourceId::DebianSourceMetadataTarball,
        UnassembledReason::BinaryArtifact,
    ),
    (
        DatasourceId::InstallshieldInstaller,
        UnassembledReason::BinaryArtifact,
    ),
    (DatasourceId::IosIpa, UnassembledReason::BinaryArtifact),
    (
        DatasourceId::IsoDiskImage,
        UnassembledReason::BinaryArtifact,
    ),
    (
        DatasourceId::JavaEarArchive,
        UnassembledReason::BinaryArtifact,
    ),
    (DatasourceId::JbossSar, UnassembledReason::BinaryArtifact),
    (
        DatasourceId::MicrosoftCabinet,
        UnassembledReason::BinaryArtifact,
    ),
    (DatasourceId::MozillaXpi, UnassembledReason::BinaryArtifact),
    (
        DatasourceId::NsisInstaller,
        UnassembledReason::BinaryArtifact,
    ),
    (
        DatasourceId::SharShellArchive,
        UnassembledReason::BinaryArtifact,
    ),
    (
        DatasourceId::SquashfsDiskImage,
        UnassembledReason::BinaryArtifact,
    ),
    (DatasourceId::GoBinary, UnassembledReason::BinaryArtifact),
    (
        DatasourceId::WindowsExecutable,
        UnassembledReason::BinaryArtifact,
    ),
    (DatasourceId::RustBinary, UnassembledReason::BinaryArtifact),
    // Metadata merged into another datasource's package or a post-assembly pass.
    (
        DatasourceId::DebianInstalledFilesList,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::DebianInstalledMd5Sums,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::DebianCopyright,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::DebianCopyrightInPackage,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::DebianCopyrightStandalone,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::AntIvyDependenciesProperties,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::NugetDirectoryBuildProps,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::NugetDirectoryPackagesProps,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::CitationCff,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::PubliccodeYaml,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::RpmPackageLicenses,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::VcpkgConfigurationJson,
        UnassembledReason::SupplementaryMetadata,
    ),
    (
        DatasourceId::VcpkgLockJson,
        UnassembledReason::SupplementaryMetadata,
    ),
    // Dependency/lock lists with no package identity of their own.
    (
        DatasourceId::ClojureDepsEdn,
        UnassembledReason::DependenciesOnlyNoIdentity,
    ),
];

/// Whether `datasource_id` is intentionally left unassembled (see
/// [`UNASSEMBLED_DATASOURCE_IDS`]).
pub(super) fn is_unassembled_datasource(datasource_id: DatasourceId) -> bool {
    UNASSEMBLED_DATASOURCE_IDS
        .iter()
        .any(|(dsid, _)| *dsid == datasource_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use strum::IntoEnumIterator;

    #[test]
    fn test_every_datasource_id_is_accounted_for() {
        let mut assembled: HashSet<DatasourceId> = HashSet::new();
        for config in ASSEMBLERS {
            for &dsid in config.datasource_ids {
                assembled.insert(dsid);
            }
        }

        let unassembled: HashSet<DatasourceId> =
            UNASSEMBLED_DATASOURCE_IDS.iter().map(|(d, _)| *d).collect();
        assert_eq!(
            unassembled.len(),
            UNASSEMBLED_DATASOURCE_IDS.len(),
            "UNASSEMBLED_DATASOURCE_IDS lists a datasource more than once"
        );

        let overlap: Vec<_> = assembled.intersection(&unassembled).collect();
        assert!(
            overlap.is_empty(),
            "Datasource IDs in BOTH ASSEMBLERS and UNASSEMBLED: {overlap:?}"
        );

        let missing: Vec<_> = DatasourceId::iter()
            .filter(|dsid| !assembled.contains(dsid) && !unassembled.contains(dsid))
            .collect();

        assert!(
            missing.is_empty(),
            "Datasource IDs in NEITHER ASSEMBLERS nor UNASSEMBLED: {missing:?}\n\
             Add each to an AssemblerConfig in ASSEMBLERS, or to UNASSEMBLED_DATASOURCE_IDS."
        );
    }

    fn pass(id: PostAssemblyPassId) -> &'static PostAssemblyPass {
        POST_ASSEMBLY_PASSES
            .iter()
            .find(|pass| pass.id == id)
            .expect("every PostAssemblyPassId is registered")
    }

    #[test]
    fn test_every_post_assembly_pass_id_is_registered_exactly_once() {
        for id in PostAssemblyPassId::iter() {
            let count = POST_ASSEMBLY_PASSES
                .iter()
                .filter(|pass| pass.id == id)
                .count();
            assert_eq!(
                count, 1,
                "post-assembly pass {id:?} should be registered exactly once"
            );
        }
        assert_eq!(
            POST_ASSEMBLY_PASSES.len(),
            PostAssemblyPassId::iter().count(),
            "POST_ASSEMBLY_PASSES contains passes not in PostAssemblyPassId"
        );
    }

    #[test]
    fn test_every_workspace_marker_has_exactly_one_detector() {
        for marker in [
            WorkspaceMarker::NpmWorkspace,
            WorkspaceMarker::CargoWorkspace,
            WorkspaceMarker::MixUmbrella,
            WorkspaceMarker::MavenReactor,
            WorkspaceMarker::GradleMultiProject,
            WorkspaceMarker::UvWorkspace,
            WorkspaceMarker::DartWorkspace,
        ] {
            let count = MARKER_DETECTORS
                .iter()
                .filter(|detector| detector.marker == marker)
                .count();
            assert_eq!(
                count, 1,
                "workspace marker {marker:?} should have exactly one detector"
            );
        }
    }

    #[test]
    fn test_post_assembly_passes_skip_irrelevant_inputs() {
        let inputs = PostAssemblyInputs::default();

        for post_assembly_pass in POST_ASSEMBLY_PASSES {
            assert!(
                !(post_assembly_pass.should_run)(&inputs),
                "{:?} should skip when no relevant inputs are present",
                post_assembly_pass.id
            );
        }
    }

    #[test]
    fn test_npm_workspace_inputs_only_run_npm_passes() {
        let inputs = PostAssemblyInputs {
            package_types: HashSet::from([PackageType::Npm]),
            file_datasource_ids: HashSet::from([DatasourceId::NpmPackageJson]),
            markers: HashSet::from([WorkspaceMarker::NpmWorkspace]),
        };

        let runnable: HashSet<_> = POST_ASSEMBLY_PASSES
            .iter()
            .filter(|post_assembly_pass| (post_assembly_pass.should_run)(&inputs))
            .map(|post_assembly_pass| post_assembly_pass.id)
            .collect();

        assert_eq!(
            runnable,
            HashSet::from([
                PostAssemblyPassId::NpmResourceAssign,
                PostAssemblyPassId::NpmWorkspaceMerge,
            ])
        );
    }

    #[test]
    fn test_cargo_workspace_merge_requires_workspace_markers() {
        let without_markers = PostAssemblyInputs {
            package_types: HashSet::from([PackageType::Cargo]),
            file_datasource_ids: HashSet::from([DatasourceId::CargoToml]),
            markers: HashSet::new(),
        };

        assert!(!(pass(PostAssemblyPassId::CargoWorkspaceMerge).should_run)(
            &without_markers
        ));

        let with_markers = PostAssemblyInputs {
            markers: HashSet::from([WorkspaceMarker::CargoWorkspace]),
            ..without_markers
        };

        assert!((pass(PostAssemblyPassId::CargoWorkspaceMerge).should_run)(
            &with_markers
        ));
    }

    #[test]
    fn test_mix_umbrella_merge_requires_umbrella_markers() {
        let without_markers = PostAssemblyInputs {
            package_types: HashSet::new(),
            file_datasource_ids: HashSet::from([DatasourceId::HexMixExs]),
            markers: HashSet::new(),
        };

        assert!(!(pass(PostAssemblyPassId::MixUmbrellaMerge).should_run)(
            &without_markers
        ));

        let with_markers = PostAssemblyInputs {
            markers: HashSet::from([WorkspaceMarker::MixUmbrella]),
            ..without_markers
        };

        assert!((pass(PostAssemblyPassId::MixUmbrellaMerge).should_run)(
            &with_markers
        ));
    }
}
