// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Elixir Mix umbrella-project assembly.
//!
//! A Mix "umbrella" is a root `mix.exs` whose `project` config carries a
//! literal `apps_path:` (see `src/parsers/mix_exs.rs`). Its child applications
//! live one level under that directory (`<apps_path>/<app>/mix.exs`, one app
//! per directory) and are otherwise plain sibling-merge Hex packages — the
//! only thing that is topology-defined is *which* directories belong to the
//! umbrella and how the umbrella's single shared `mix.lock` distributes across
//! them. Everything else (declared license, description, …) is the member's
//! own, exactly as for a standalone `mix.exs`.
//!
//! # Contract
//!
//! - Each discovered child app under `apps_path` becomes its own
//!   `pkg:hex/<app>@<version>` package, keyed off its own `mix.exs`.
//! - A dependency declared as `{:sibling, in_umbrella: true}` is resolved to
//!   the sibling app's actual package identity (purl + version) instead of a
//!   fabricated unversioned `pkg:hex/sibling`. A dangling reference (the
//!   sibling was filtered out by `apps:`, or does not exist) is dropped rather
//!   than guessed.
//! - The umbrella normally shares one `mix.lock` at the root, and mix.lock
//!   itself has no per-app partition of *which* app needs which locked
//!   dependency — Mix resolves the whole umbrella's dependency set at once.
//!   Provenant recovers ownership the only way it can prove it: a locked entry
//!   is attributed to every app (or the root, if it has its own package
//!   identity) whose *own* direct deps list that same app/alias name. An entry
//!   that no manifest directly declares (a pure transitive dependency of some
//!   member) cannot be attributed to a specific app without guessing, so it
//!   stays hoisted (`for_package_uid: None`), the same honest fallback used
//!   for a standalone `mix.lock` with no sibling `mix.exs`.
//! - The root `mix.exs` itself commonly has no `app:` (an umbrella root is not
//!   its own OTP application) and so forms no package; its own deps and any
//!   lock entries no member claims stay hoisted in that case, matching
//!   Cargo/npm workspace roots that lack their own package manifest section.
//!   If the root manifest *does* carry an `app:` (unusual but valid), it is
//!   treated as another owning candidate, exactly like a member.
//! - A member directory is claimed for umbrella topology instead of ordinary
//!   sibling merge, but that claim does not extend to dropping a `mix.lock`
//!   that lives directly inside that member's own directory. Mix does not
//!   forbid a member from also carrying its own lock (e.g. an app that is
//!   also lockable/runnable standalone), so when one is present every one of
//!   its entries is attributed to that single member — unambiguous, unlike
//!   the shared root lock — in addition to (not instead of) the umbrella-wide
//!   root `mix.lock` attribution above.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::models::{
    DatasourceId, Dependency, FileInfo, Package, PackageData, PackageUid, TopLevelDependency,
};

pub(super) struct MixUmbrellaRootHint {
    pub(super) root_dir: PathBuf,
    pub(super) root_mix_exs_idx: usize,
    pub(super) apps_path: String,
    pub(super) apps_filter: Option<HashSet<String>>,
}

pub(super) struct MixUmbrellaMemberDomain {
    pub(super) manifest_idx: usize,
    pub(super) dir_path: PathBuf,
    /// A member directory normally has no `mix.lock` of its own — the
    /// umbrella shares one root-level lock (see `root_mix_lock_idx`). Mix
    /// does not forbid a member from carrying its own lock too (e.g. an app
    /// that is also runnable/lockable standalone), and dropping that lock
    /// silently would make its locked deps disappear. When present, it is
    /// merged for this member in addition to (not instead of) umbrella
    /// topology; see `apply_mix_umbrella_domain`.
    pub(super) local_mix_lock_idx: Option<usize>,
}

pub(super) struct MixUmbrellaDomain {
    pub(super) root_dir: PathBuf,
    pub(super) apps_dir: PathBuf,
    pub(super) root_mix_exs_idx: usize,
    pub(super) root_mix_lock_idx: Option<usize>,
    pub(super) members: Vec<MixUmbrellaMemberDomain>,
}

/// Owning candidate for umbrella-shared lock entries: the umbrella root itself
/// (only when it has its own package identity) or one of its member apps.
struct Candidate {
    package: Package,
    /// `app`/alias names this candidate declares directly in its own `deps()`
    /// (excluding `in_umbrella:` entries, which are never real Hex packages).
    direct_dep_app_names: HashSet<String>,
}

pub(super) fn collect_mix_umbrella_hints(files: &[FileInfo]) -> Vec<MixUmbrellaRootHint> {
    let mut hints = Vec::new();

    for (idx, file) in files.iter().enumerate() {
        let path = Path::new(&file.path);
        if path.file_name().and_then(|name| name.to_str()) != Some("mix.exs") {
            continue;
        }

        for pkg_data in &file.package_data {
            if pkg_data.datasource_id != Some(DatasourceId::HexMixExs) {
                continue;
            }

            let Some(extra_data) = &pkg_data.extra_data else {
                continue;
            };
            let Some(apps_path) = extra_data.get("apps_path").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(parent) = path.parent() else {
                continue;
            };

            let apps_filter = extra_data
                .get("apps")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect::<HashSet<_>>()
                });

            hints.push(MixUmbrellaRootHint {
                root_dir: parent.to_path_buf(),
                root_mix_exs_idx: idx,
                apps_path: apps_path.to_string(),
                apps_filter,
            });
        }
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

pub(super) fn plan_mix_umbrella_domains(
    files: &[FileInfo],
    root_hints: &[&MixUmbrellaRootHint],
) -> Vec<MixUmbrellaDomain> {
    let mut domains = Vec::new();

    for hint in root_hints {
        let apps_dir = hint.root_dir.join(&hint.apps_path);
        let members = discover_members(files, &apps_dir, hint.apps_filter.as_ref());

        if members.is_empty() {
            continue;
        }

        domains.push(MixUmbrellaDomain {
            root_dir: hint.root_dir.clone(),
            apps_dir,
            root_mix_exs_idx: hint.root_mix_exs_idx,
            root_mix_lock_idx: find_mix_lock_index(files, &hint.root_dir),
            members,
        });
    }

    domains.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    domains
}

fn discover_members(
    files: &[FileInfo],
    apps_dir: &Path,
    apps_filter: Option<&HashSet<String>>,
) -> Vec<MixUmbrellaMemberDomain> {
    let mut members = Vec::new();

    for (idx, file) in files.iter().enumerate() {
        let path = Path::new(&file.path);
        if path.file_name().and_then(|name| name.to_str()) != Some("mix.exs") {
            continue;
        }

        let Some(parent) = path.parent() else {
            continue;
        };
        if parent.parent() != Some(apps_dir) {
            continue;
        }

        let has_valid_package = file
            .package_data
            .iter()
            .any(|pkg| pkg.datasource_id == Some(DatasourceId::HexMixExs) && pkg.purl.is_some());
        if !has_valid_package {
            continue;
        }

        if let Some(filter) = apps_filter {
            let dir_name = parent
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !filter.contains(dir_name) {
                continue;
            }
        }

        members.push(MixUmbrellaMemberDomain {
            manifest_idx: idx,
            dir_path: parent.to_path_buf(),
            local_mix_lock_idx: find_mix_lock_index(files, parent),
        });
    }

    members.sort_by(|left, right| {
        files[left.manifest_idx]
            .path
            .cmp(&files[right.manifest_idx].path)
    });
    members
}

fn find_mix_lock_index(files: &[FileInfo], dir: &Path) -> Option<usize> {
    files.iter().position(|file| {
        let path = Path::new(&file.path);
        path.parent() == Some(dir)
            && path.file_name().and_then(|name| name.to_str()) == Some("mix.lock")
    })
}

/// Apply a planned umbrella domain: build one package per member (plus a root
/// package when the root manifest has its own identity), resolve
/// `in_umbrella:` edges to the sibling's real identity, attribute the shared
/// `mix.lock` to whichever manifests directly declare each locked app name,
/// and assign `for_packages` across the umbrella tree.
pub(super) fn apply_mix_umbrella_domain(
    domain: &MixUmbrellaDomain,
    files: &mut [FileInfo],
    packages: &mut Vec<Package>,
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let member_pkg_data: Vec<PackageData> = domain
        .members
        .iter()
        .filter_map(|member| {
            files[member.manifest_idx]
                .package_data
                .iter()
                .find(|pkg| {
                    pkg.datasource_id == Some(DatasourceId::HexMixExs) && pkg.purl.is_some()
                })
                .cloned()
        })
        .collect();

    if member_pkg_data.len() != domain.members.len() {
        // A member lost its identity between planning and here (should not
        // happen given `discover_members` already filtered on purl); bail out
        // rather than build a partial, inconsistent domain.
        return;
    }

    // Fetched independently of purl: an umbrella root commonly has no `app:`
    // (no identity, so no package), but it can still declare its own direct
    // deps, which must stay honestly hoisted rather than silently dropped.
    let root_pkg_data = files[domain.root_mix_exs_idx]
        .package_data
        .iter()
        .find(|pkg| pkg.datasource_id == Some(DatasourceId::HexMixExs))
        .cloned();
    let root_has_identity = root_pkg_data
        .as_ref()
        .is_some_and(|pkg_data| pkg_data.purl.is_some());

    let mut candidates: Vec<Candidate> = Vec::new();
    if root_has_identity {
        let root_pkg_data = root_pkg_data.as_ref().expect("checked above");
        candidates.push(Candidate {
            package: Package::from_package_data(
                root_pkg_data,
                files[domain.root_mix_exs_idx].path.clone(),
            ),
            direct_dep_app_names: direct_dep_app_names(root_pkg_data),
        });
    }
    let root_candidate_len = candidates.len();
    for (member, pkg_data) in domain.members.iter().zip(member_pkg_data.iter()) {
        candidates.push(Candidate {
            package: Package::from_package_data(pkg_data, files[member.manifest_idx].path.clone()),
            direct_dep_app_names: direct_dep_app_names(pkg_data),
        });
    }

    // Map each member's own app/package name to its resolved identity, so
    // `in_umbrella: true` deps can be rewritten to the sibling's real purl.
    let identity_by_app_name: HashMap<String, (String, String)> = candidates
        .iter()
        .filter_map(|candidate| {
            let name = candidate.package.name.clone()?;
            let purl = candidate.package.purl.clone()?;
            let version = candidate.package.version.clone().unwrap_or_default();
            Some((name, (purl, version)))
        })
        .collect();

    // Emit each candidate's own direct dependencies (mix.exs deps()), resolving
    // in_umbrella edges to the sibling's real identity. Root and members are
    // walked explicitly (rather than looping over `candidates` generically) so
    // each is paired with its own source `PackageData` and manifest path. A
    // root with no identity still contributes its own deps, honestly hoisted
    // (`owner_uid: None`) rather than dropped.
    if let Some(root_pkg_data) = &root_pkg_data {
        let root_owner_uid = root_has_identity.then(|| candidates[0].package.package_uid.clone());
        emit_direct_dependencies(
            root_pkg_data,
            &files[domain.root_mix_exs_idx].path.clone(),
            root_owner_uid,
            &identity_by_app_name,
            dependencies,
        );
    }
    for (offset, member) in domain.members.iter().enumerate() {
        let candidate = &candidates[root_candidate_len + offset];
        emit_direct_dependencies(
            &member_pkg_data[offset],
            &files[member.manifest_idx].path.clone(),
            Some(candidate.package.package_uid.clone()),
            &identity_by_app_name,
            dependencies,
        );
    }

    // Attribute the shared mix.lock to every candidate that directly declares
    // the locked app/alias name; hoist unowned when nobody does. File-level
    // ownership (`for_packages`) for mix.lock itself is filled in below by
    // `assign_files_to_umbrella_packages`, the same shared-file rule used for
    // every other file directly in the umbrella tree.
    if let Some(lock_idx) = domain.root_mix_lock_idx {
        let lock_path = files[lock_idx].path.clone();
        let lock_deps: Vec<Dependency> = files[lock_idx]
            .package_data
            .iter()
            .find(|pkg| pkg.datasource_id == Some(DatasourceId::HexMixLock))
            .map(|pkg| pkg.dependencies.clone())
            .unwrap_or_default();

        for dep in &lock_deps {
            let Some(app_name) = dep_app_name(dep) else {
                continue;
            };

            let owners: Vec<&Candidate> = candidates
                .iter()
                .filter(|candidate| candidate.direct_dep_app_names.contains(&app_name))
                .collect();

            if owners.is_empty() {
                dependencies.push(TopLevelDependency::from_dependency(
                    dep,
                    lock_path.clone(),
                    DatasourceId::HexMixLock,
                    None,
                ));
                continue;
            }

            for owner in owners {
                dependencies.push(TopLevelDependency::from_dependency(
                    dep,
                    lock_path.clone(),
                    DatasourceId::HexMixLock,
                    Some(owner.package.package_uid.clone()),
                ));
            }
        }
    }

    // A member's own local mix.lock (unusual, but not forbidden by Mix) is
    // unambiguous: unlike the shared root lock, every entry in it belongs
    // entirely to that one member, exactly as for a standalone
    // mix.exs+mix.lock sibling pair outside any umbrella. Merge it in
    // addition to the umbrella-wide root lock rather than dropping it just
    // because this directory was claimed for umbrella topology instead of
    // ordinary sibling merge.
    for (offset, member) in domain.members.iter().enumerate() {
        let Some(lock_idx) = member.local_mix_lock_idx else {
            continue;
        };
        let lock_path = files[lock_idx].path.clone();
        let lock_deps: Vec<Dependency> = files[lock_idx]
            .package_data
            .iter()
            .find(|pkg| pkg.datasource_id == Some(DatasourceId::HexMixLock))
            .map(|pkg| pkg.dependencies.clone())
            .unwrap_or_default();

        let owner_uid = candidates[root_candidate_len + offset]
            .package
            .package_uid
            .clone();
        for dep in &lock_deps {
            dependencies.push(TopLevelDependency::from_dependency(
                dep,
                lock_path.clone(),
                DatasourceId::HexMixLock,
                Some(owner_uid.clone()),
            ));
        }
    }

    let root_package_uid = root_has_identity.then(|| candidates[0].package.package_uid.clone());
    let member_uids: Vec<PackageUid> = (0..domain.members.len())
        .map(|offset| {
            candidates[root_candidate_len + offset]
                .package
                .package_uid
                .clone()
        })
        .collect();

    // `candidates` is ordered [root?, member0, member1, ...]; push every
    // candidate's package (root, if present, then each member) exactly once.
    for candidate in candidates {
        packages.push(candidate.package);
    }

    assign_files_to_umbrella_packages(
        files,
        &domain.root_dir,
        &domain.apps_dir,
        &domain.members,
        &member_uids,
        root_package_uid.as_ref(),
    );
}

fn direct_dep_app_names(pkg_data: &PackageData) -> HashSet<String> {
    pkg_data
        .dependencies
        .iter()
        .filter(|dep| !is_in_umbrella(dep))
        .filter_map(dep_app_name)
        .collect()
}

fn is_in_umbrella(dep: &Dependency) -> bool {
    dep.extra_data
        .as_ref()
        .and_then(|extra| extra.get("in_umbrella"))
        .and_then(|value| value.as_bool())
        == Some(true)
}

fn dep_app_name(dep: &Dependency) -> Option<String> {
    dep.extra_data
        .as_ref()?
        .get("app")?
        .as_str()
        .map(str::to_string)
}

fn emit_direct_dependencies(
    pkg_data: &PackageData,
    manifest_path: &str,
    owner_uid: Option<PackageUid>,
    identity_by_app_name: &HashMap<String, (String, String)>,
    dependencies: &mut Vec<TopLevelDependency>,
) {
    for dep in &pkg_data.dependencies {
        if is_in_umbrella(dep) {
            let Some(app_name) = dep_app_name(dep) else {
                continue;
            };
            let Some((purl, version)) = identity_by_app_name.get(&app_name) else {
                // Dangling in_umbrella reference (filtered by `apps:`, or the
                // sibling does not exist): drop rather than guess.
                continue;
            };

            let mut resolved = dep.clone();
            resolved.purl = Some(purl.clone());
            resolved.extracted_requirement = if version.is_empty() {
                None
            } else {
                Some(version.clone())
            };

            dependencies.push(TopLevelDependency::from_dependency(
                &resolved,
                manifest_path.to_string(),
                DatasourceId::HexMixExs,
                owner_uid.clone(),
            ));
            continue;
        }

        if dep.purl.is_some() {
            dependencies.push(TopLevelDependency::from_dependency(
                dep,
                manifest_path.to_string(),
                DatasourceId::HexMixExs,
                owner_uid.clone(),
            ));
        }
    }
}

/// Attribute every file under the umbrella root to its owning package: files
/// under a member's directory go to that member; other files directly in the
/// umbrella tree (shared docs, root config, …) go to the root package, or to
/// every member when there is no root package, mirroring the Cargo/npm
/// workspace fallback for a workspace without its own root package. Mix build
/// output (`_build/`, `deps/`) is excluded. A file under `apps_dir` that is
/// not inside a recognized member's directory (for example an app filtered
/// out by `apps:`, or a directory with no valid `mix.exs`) is left alone
/// entirely rather than folded into the root/all-members fallback: it is not
/// part of this domain, so it keeps whatever ownership its own (non-umbrella)
/// assembly already gave it.
fn assign_files_to_umbrella_packages(
    files: &mut [FileInfo],
    root_dir: &Path,
    apps_dir: &Path,
    members: &[MixUmbrellaMemberDomain],
    member_uids: &[PackageUid],
    root_package_uid: Option<&PackageUid>,
) {
    const BUILD_OUTPUT_DIRS: [&str; 2] = ["_build", "deps"];

    for file in files.iter_mut() {
        let path = Path::new(&file.path);
        if !path.starts_with(root_dir) {
            continue;
        }

        let mut assigned = false;
        for (member, uid) in members.iter().zip(member_uids.iter()) {
            if path.starts_with(&member.dir_path) {
                if !file.for_packages.contains(uid) {
                    file.for_packages.push(uid.clone());
                }
                assigned = true;
                break;
            }
        }
        if assigned {
            continue;
        }

        if path.starts_with(apps_dir) {
            continue;
        }

        if let Ok(relative) = path.strip_prefix(root_dir)
            && let Some(first_component) = relative.components().next()
            && BUILD_OUTPUT_DIRS.contains(&first_component.as_os_str().to_string_lossy().as_ref())
        {
            continue;
        }

        if let Some(root_uid) = root_package_uid {
            if !file.for_packages.contains(root_uid) {
                file.for_packages.push(root_uid.clone());
            }
        } else {
            for uid in member_uids {
                if !file.for_packages.contains(uid) {
                    file.for_packages.push(uid.clone());
                }
            }
        }
    }
}
