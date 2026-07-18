// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

mod assemblers;
#[cfg(test)]
mod assembly_test;
mod bazel_prune;
mod cargo_resource_assign;
mod cargo_workspace_merge;
mod clojure_deps_assign;
mod cocoapods_merge;
mod composer_resource_assign;
mod conda_rootfs_merge;
mod dart_workspace_merge;
mod debian_source_merge;
pub mod file_ref_resolve;
mod hackage_merge;
mod huggingface_merge;
mod ivy_dependencies_properties_assign;
mod mix_umbrella_merge;
mod nested_merge;
mod nix_flake_compat_merge;
mod npm_resource_assign;
mod npm_workspace_merge;
mod nuget_cpm_resolve;
mod project_dependency_assign;
mod python_requirements_assign;
mod ruby_resource_assign;
mod sibling_merge;
mod swift_merge;
mod topology;
mod windows_update_merge;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::models::{DatasourceId, FileInfo, Package, PackageUid, TopLevelDependency};

pub use assemblers::ASSEMBLERS;

type DirectoryMergeOutput = (Option<Package>, Vec<TopLevelDependency>, Vec<usize>);

/// Pre-computed lookup: DatasourceId → config key (first DatasourceId in config).
/// Built once on first use, avoiding HashMap allocation on every `assemble()` call.
static ASSEMBLER_LOOKUP: LazyLock<HashMap<DatasourceId, DatasourceId>> = LazyLock::new(|| {
    let mut lookup = HashMap::new();
    for config in ASSEMBLERS {
        let key = *config
            .datasource_ids
            .first()
            .expect("assembler must have at least one datasource_id");
        for &dsid in config.datasource_ids {
            lookup.insert(dsid, key);
        }
    }
    lookup
});

static ASSEMBLER_CONFIG_LOOKUP: LazyLock<HashMap<DatasourceId, &'static AssemblerConfig>> =
    LazyLock::new(|| {
        let mut lookup = HashMap::new();
        for config in ASSEMBLERS {
            let key = *config
                .datasource_ids
                .first()
                .expect("assembler must have at least one datasource_id");
            lookup.insert(key, config);
        }
        lookup
    });

/// Result of the assembly phase: top-level packages and dependencies,
/// plus updated file-to-package associations.
pub struct AssemblyResult {
    pub packages: Vec<Package>,
    pub dependencies: Vec<TopLevelDependency>,
}

/// How an assembler groups PackageData into Packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssemblyMode {
    /// Merge related files in the same directory (or nested) into one Package.
    SiblingMerge,
    /// Like [`AssemblyMode::SiblingMerge`], but a directory holding multiple
    /// manifests with distinct package identities (purls) yields one package per
    /// identity instead of collapsing them into one. A directory with a single
    /// identity (a normal one-module directory plus its purl-less supplementary
    /// siblings) falls back to the `SiblingMerge` result unchanged. Use this for
    /// ecosystems whose directories can legitimately hold several independent
    /// manifests, e.g. a flat set of standalone Maven `.pom` files.
    SiblingMergePerIdentity,
    /// Each PackageData becomes its own independent Package (e.g., database files
    /// containing many installed packages like Alpine DB, RPM DB, Debian status).
    OnePerPackageData,
}

pub struct AssemblerConfig {
    pub datasource_ids: &'static [DatasourceId],
    pub sibling_file_patterns: &'static [&'static str],
    pub mode: AssemblyMode,
}

/// Run the assembly phase over all scanned files.
///
/// Groups files by parent directory, finds related manifests/lockfiles,
/// merges them into top-level `Package` objects, and hoists dependencies.
/// Updates each `FileInfo.for_packages` with the UIDs of packages it belongs to.
pub fn assemble(files: &mut [FileInfo]) -> AssemblyResult {
    let assembler_lookup = &*ASSEMBLER_LOOKUP;
    let assembler_config_lookup = &*ASSEMBLER_CONFIG_LOOKUP;
    let mut packages = Vec::new();
    let mut dependencies = Vec::new();

    let dir_files = group_files_by_directory(files);
    let topology_plan = topology::TopologyPlan::build(files, &dir_files);

    for file_indices in dir_files.values() {
        let groups = active_config_keys(files, file_indices, assembler_lookup);

        for &config_key in &groups {
            let config = assembler_config_lookup
                .get(&config_key)
                .copied()
                .expect("assembler config must exist");

            if topology_plan.claims_directory_assembly(config, file_indices, files) {
                continue;
            }

            if let Some(special_merger) = assemblers::special_directory_merger_for(config_key) {
                let results = special_merger.run(config, files, file_indices);
                apply_directory_merge_results(files, &mut packages, &mut dependencies, results);
                continue;
            }

            match config.mode {
                AssemblyMode::SiblingMerge => {
                    let results = sibling_merge::assemble_siblings(config, files, file_indices);
                    apply_directory_merge_results(files, &mut packages, &mut dependencies, results);
                }
                AssemblyMode::SiblingMergePerIdentity => {
                    let results =
                        sibling_merge::assemble_siblings_per_identity(config, files, file_indices);
                    apply_directory_merge_results(files, &mut packages, &mut dependencies, results);
                }
                AssemblyMode::OnePerPackageData => {
                    let results = assemble_one_per_package_data(config, files, file_indices);
                    apply_directory_merge_results(files, &mut packages, &mut dependencies, results);
                }
            }
        }
    }

    topology_plan.apply_directory_scoped_domains(files, &mut packages, &mut dependencies);

    for config in ASSEMBLERS {
        if !matches!(
            config.mode,
            AssemblyMode::SiblingMerge | AssemblyMode::SiblingMergePerIdentity
        ) {
            continue;
        }
        if let Some((pkg, deps, affected_indices)) =
            nested_merge::assemble_nested_patterns(files, config)
        {
            let package_uid = pkg.package_uid.clone();

            // The nested merge subsumes exactly the per-directory packages that were
            // previously created for the files it consumed. Those packages are recorded
            // on the affected files' `for_packages`, so key dedup off those specific UIDs
            // rather than a blanket purl match (which would also delete unrelated packages,
            // including every purl-less package scan-wide when the merged purl is `None`).
            let removed_package_uids: HashSet<PackageUid> = affected_indices
                .iter()
                .flat_map(|idx| files[*idx].for_packages.iter().cloned())
                .filter(|uid| *uid != package_uid)
                .collect();

            packages.retain(|p| !removed_package_uids.contains(&p.package_uid));
            dependencies.retain(|d| {
                d.for_package_uid
                    .as_ref()
                    .is_none_or(|old_uid| !removed_package_uids.contains(old_uid))
            });

            for idx in &affected_indices {
                files[*idx].for_packages.clear();
                files[*idx].for_packages.push(package_uid.clone());
            }

            packages.push(pkg);
            dependencies.extend(deps);
        }
    }

    assemblers::run_post_assembly_passes(files, &mut packages, &mut dependencies, &topology_plan);
    hoist_unassembled_file_dependencies(files, &mut dependencies);

    for package in &mut packages {
        package.datafile_paths.sort();
        package.datafile_paths.dedup();
        package.datasource_ids.sort_by_key(|left| left.to_string());
        package.datasource_ids.dedup();
    }

    for file in files.iter_mut() {
        file.for_packages
            .sort_by(|left, right| left.stable_key().cmp(right.stable_key()));
        file.for_packages.dedup();
    }

    packages
        .sort_by(|left, right| stable_package_sort_key(left).cmp(&stable_package_sort_key(right)));
    dependencies.sort_by(|left, right| {
        left.purl
            .as_deref()
            .cmp(&right.purl.as_deref())
            .then_with(|| {
                left.extracted_requirement
                    .as_deref()
                    .cmp(&right.extracted_requirement.as_deref())
            })
            .then_with(|| left.scope.as_deref().cmp(&right.scope.as_deref()))
            .then_with(|| left.datafile_path.cmp(&right.datafile_path))
            .then_with(|| {
                left.datasource_id
                    .to_string()
                    .cmp(&right.datasource_id.to_string())
            })
            .then_with(|| {
                left.for_package_uid
                    .as_ref()
                    .map(|uid| uid.stable_key())
                    .cmp(&right.for_package_uid.as_ref().map(|uid| uid.stable_key()))
            })
    });

    AssemblyResult {
        packages,
        dependencies,
    }
}

fn apply_directory_merge_results(
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
    results: Vec<DirectoryMergeOutput>,
) {
    for (package, deps, affected_indices) in results {
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
}

fn hoist_unassembled_file_dependencies(
    files: &[FileInfo],
    dependencies: &mut Vec<TopLevelDependency>,
) {
    for file in files {
        if !file.for_packages.is_empty() {
            continue;
        }

        for pkg_data in &file.package_data {
            let Some(datasource_id) = pkg_data.datasource_id else {
                continue;
            };

            if !should_hoist_unassembled_dependencies(datasource_id) {
                continue;
            }

            dependencies.extend(pkg_data.dependencies.iter().map(|dep| {
                TopLevelDependency::from_dependency(dep, file.path.clone(), datasource_id, None)
            }));
        }
    }
}

const HOIST_IF_UNOWNED_DATASOURCE_IDS: &[DatasourceId] = &[DatasourceId::PipRequirements];

fn should_hoist_unassembled_dependencies(datasource_id: DatasourceId) -> bool {
    if HOIST_IF_UNOWNED_DATASOURCE_IDS.contains(&datasource_id) {
        return true;
    }

    if !assemblers::is_unassembled_datasource(datasource_id) {
        return false;
    }

    !matches!(
        datasource_id,
        DatasourceId::NugetDirectoryBuildProps | DatasourceId::NugetDirectoryPackagesProps
    )
}

fn stable_package_sort_key(package: &Package) -> (Option<&str>, Option<&str>, Option<&str>, &str) {
    (
        package.purl.as_deref(),
        package.name.as_deref(),
        package.version.as_deref(),
        package
            .datafile_paths
            .first()
            .map(String::as_str)
            .unwrap_or(""),
    )
}

fn assemble_one_per_package_data(
    config: &AssemblerConfig,
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    let mut results = Vec::new();

    for &idx in file_indices {
        let file = &files[idx];
        for pkg_data in &file.package_data {
            let dsid_matches = pkg_data
                .datasource_id
                .is_some_and(|dsid| config.datasource_ids.contains(&dsid));

            if !dsid_matches || should_skip_placeholder_only_cocoapods_podspec(pkg_data) {
                continue;
            }

            let Some(datasource_id) = pkg_data.datasource_id else {
                continue;
            };
            let datafile_path = file.path.clone();

            // A record carrying an identity becomes its own package that owns its
            // dependencies. A purl-less record cannot be a package, but its
            // dependencies are still hoisted (unowned) rather than dropped — the
            // same visibility they had before this datasource was assembled.
            let (package, affected) = if pkg_data.purl.is_some() {
                (
                    Some(Package::from_package_data(pkg_data, datafile_path.clone())),
                    vec![idx],
                )
            } else {
                (None, Vec::new())
            };
            let for_package_uid = package.as_ref().map(|pkg| pkg.package_uid.clone());

            let deps: Vec<TopLevelDependency> = pkg_data
                .dependencies
                .iter()
                .filter(|dep| dep.purl.is_some() || dep.extracted_requirement.is_some())
                .map(|dep| {
                    TopLevelDependency::from_dependency(
                        dep,
                        datafile_path.clone(),
                        datasource_id,
                        for_package_uid.clone(),
                    )
                })
                .collect();

            if package.is_none() && deps.is_empty() {
                continue;
            }

            results.push((package, deps, affected));
        }
    }

    results
}

pub(super) fn should_skip_placeholder_only_cocoapods_podspec(
    pkg_data: &crate::models::PackageData,
) -> bool {
    pkg_data.datasource_id == Some(DatasourceId::CocoapodsPodspec)
        && pkg_data
            .extra_data
            .as_ref()
            .and_then(|data| data.get("dynamic_identity_placeholders"))
            .and_then(|value| value.as_bool())
            == Some(true)
}

/// Collect the assembler config keys active in a directory in a deterministic
/// order.
///
/// The keys are gathered into a `BTreeSet` so iteration order is stable across
/// runs and processes. A `HashSet` here made per-directory assembler execution
/// order depend on hash seeding, which produced run-to-run nondeterministic
/// output in polyglot directories (issue #1026).
fn active_config_keys(
    files: &[FileInfo],
    file_indices: &[usize],
    assembler_lookup: &HashMap<DatasourceId, DatasourceId>,
) -> BTreeSet<DatasourceId> {
    let mut groups: BTreeSet<DatasourceId> = BTreeSet::new();
    for &idx in file_indices {
        for pkg_data in &files[idx].package_data {
            if let Some(dsid) = pkg_data.datasource_id
                && let Some(&config_key) = assembler_lookup.get(&dsid)
            {
                groups.insert(config_key);
            }
        }
    }
    groups
}

/// Group file indices by their parent directory path.
fn group_files_by_directory(files: &[FileInfo]) -> HashMap<PathBuf, Vec<usize>> {
    let mut groups: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (idx, file) in files.iter().enumerate() {
        if let Some(parent) = std::path::Path::new(&file.path).parent() {
            groups.entry(parent.to_path_buf()).or_default().push(idx);
        }
    }
    groups
}
