// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! uv workspace topology: member discovery, shared-lock attribution, and nested
//! file ownership.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use packageurl::PackageUrl;

use crate::models::{
    DatasourceId, Dependency, FileInfo, PackageData, PackageUid, TopLevelDependency,
};

use super::path_identity::{normalize_lexical_path, scanned_file_dir, strip_declared_dot_slash};
use super::topology::assign_unowned_files_to_anchors;

pub(super) struct UvWorkspaceRootHint {
    pub(super) root_dir: PathBuf,
    pub(super) root_pyproject_idx: usize,
    pub(super) member_patterns: Vec<String>,
    pub(super) exclude_patterns: Vec<String>,
}

pub(super) struct UvWorkspaceDomain {
    pub(super) root_dir: PathBuf,
    pub(super) root_pyproject_idx: usize,
    pub(super) member_pyproject_indices: Vec<usize>,
}

/// A uv workspace member (or the workspace root) that can own shared-lock
/// entries: its package identity plus the distribution names its own
/// `pyproject.toml` declares as direct dependencies.
struct UvLockCandidate {
    package_uid: PackageUid,
    direct_dependency_names: HashSet<String>,
}

pub(super) fn collect_uv_workspace_hints(files: &[FileInfo]) -> Vec<UvWorkspaceRootHint> {
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
            let Some(root_dir) = scanned_file_dir(&file.path) else {
                continue;
            };
            hints.push(UvWorkspaceRootHint {
                root_dir,
                root_pyproject_idx: idx,
                member_patterns,
                exclude_patterns: extra_string_array(data, "workspace_exclude"),
            });
        }
    }

    hints.sort_by(|left, right| left.root_dir.cmp(&right.root_dir));
    hints
}

pub(super) fn plan_uv_workspace_domains(
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
                    let member_dir = scanned_file_dir(&file.path)?;
                    let included = hint.member_patterns.iter().any(|pattern| {
                        workspace_pattern_matches(&hint.root_dir, &member_dir, pattern)
                    });
                    let excluded = hint.exclude_patterns.iter().any(|pattern| {
                        workspace_exclude_matches(&hint.root_dir, &member_dir, pattern)
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

fn extra_string_array(data: &PackageData, key: &str) -> Vec<String> {
    data.extra_data
        .as_ref()
        .and_then(|extra| extra.get(key))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect()
}

fn workspace_pattern_matches(root_dir: &Path, member_dir: &Path, pattern: &str) -> bool {
    let pattern = strip_declared_dot_slash(pattern);
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
    let trimmed = strip_declared_dot_slash(pattern);
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

/// Fill in workspace-aware file ownership and shared-lock attribution for
/// every declared uv workspace.
///
/// A uv workspace shares one root `uv.lock` that resolves every member's
/// dependency set at once, with no per-member partition of which locked
/// entry belongs to which member (`src/parsers/uv_lock.rs` therefore hoists
/// the whole set onto the root package during ordinary sibling merge).
/// Provenant recovers ownership the only way it can prove it statically,
/// mirroring the Mix umbrella and Dart workspace rule: a locked entry is
/// attributed to every workspace member (or the root, when the root
/// `pyproject.toml` is itself a package) whose *own* `[project.dependencies]`
/// directly declare that same distribution name. An entry that no member
/// declares directly (a pure transitive dependency of the resolved set)
/// cannot be attributed without guessing, so it stays hoisted
/// (`for_package_uid: None`) — the same honest fallback used for a
/// standalone `uv.lock` with no workspace.
///
/// This is only attempted when the shared lock actually exists on disk; a
/// workspace without a checked-in `uv.lock` keeps whatever attribution
/// ordinary assembly already produced.
pub(super) fn apply_uv_workspace_domains<'a>(
    domains: impl IntoIterator<Item = &'a UvWorkspaceDomain>,
    files: &mut [FileInfo],
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let domains: Vec<&UvWorkspaceDomain> = domains.into_iter().collect();

    for domain in &domains {
        attribute_uv_shared_lock(domain, files, dependencies);
    }

    let mut scope_roots = Vec::new();
    let mut anchor_indices = Vec::new();
    let mut protected_project_dirs = Vec::new();

    for domain in &domains {
        scope_roots.push(normalize_lexical_path(&domain.root_dir));
        anchor_indices.push(domain.root_pyproject_idx);
        for &member_idx in &domain.member_pyproject_indices {
            if let Some(member_dir) = scanned_file_dir(&files[member_idx].path) {
                scope_roots.push(member_dir);
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
        let Some(project_dir) = scanned_file_dir(&file.path) else {
            continue;
        };
        if scope_roots.iter().any(|root| project_dir.starts_with(root)) {
            protected_project_dirs.push(project_dir);
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

/// Re-attribute a uv workspace's shared root `uv.lock` entries to the members
/// (or root) whose own manifests declare each locked distribution directly;
/// entries no member declares stay hoisted. See [`apply_uv_workspace_domains`]
/// for the full contract.
fn attribute_uv_shared_lock(
    domain: &UvWorkspaceDomain,
    files: &mut [FileInfo],
    dependencies: &mut Vec<TopLevelDependency>,
) {
    let Some(lock_idx) = find_uv_lock_index(files, &domain.root_dir) else {
        return;
    };

    let mut candidates = Vec::new();
    if let Some(candidate) = uv_lock_candidate(files, domain.root_pyproject_idx) {
        candidates.push(candidate);
    }
    for &member_idx in &domain.member_pyproject_indices {
        if let Some(candidate) = uv_lock_candidate(files, member_idx) {
            candidates.push(candidate);
        }
    }

    let lock_path = files[lock_idx].path.clone();
    let lock_deps: Vec<Dependency> = files[lock_idx]
        .package_data
        .iter()
        .find(|data| data.datasource_id == Some(DatasourceId::PypiUvLock))
        .map(|data| data.dependencies.clone())
        .unwrap_or_default();

    if lock_deps.is_empty() {
        return;
    }

    // Ordinary sibling merge already hoisted every `uv.lock` entry onto the
    // workspace root package (or left them unowned). Drop that thin attribution
    // and re-emit each entry with the member-aware ownership below.
    dependencies.retain(|dependency| dependency.datafile_path != lock_path);

    for dep in &lock_deps {
        let locked_name = dep.purl.as_deref().and_then(pypi_purl_name);
        let owners: Vec<&UvLockCandidate> = candidates
            .iter()
            .filter(|candidate| {
                locked_name
                    .as_ref()
                    .is_some_and(|name| candidate.direct_dependency_names.contains(name))
            })
            .collect();

        if owners.is_empty() {
            dependencies.push(TopLevelDependency::from_dependency(
                dep,
                lock_path.clone(),
                DatasourceId::PypiUvLock,
                None,
            ));
            continue;
        }

        for owner in owners {
            dependencies.push(TopLevelDependency::from_dependency(
                dep,
                lock_path.clone(),
                DatasourceId::PypiUvLock,
                Some(owner.package_uid.clone()),
            ));
        }
    }
}

fn uv_lock_candidate(files: &[FileInfo], pyproject_idx: usize) -> Option<UvLockCandidate> {
    let package_uid = files[pyproject_idx].for_packages.first().cloned()?;
    let direct_dependency_names = files[pyproject_idx]
        .package_data
        .iter()
        .find(|data| {
            matches!(
                data.datasource_id,
                Some(DatasourceId::PypiPyprojectToml | DatasourceId::PypiPoetryPyprojectToml)
            )
        })
        .map(|data| {
            data.dependencies
                .iter()
                .filter(|dep| dep.is_direct == Some(true))
                .filter_map(|dep| dep.purl.as_deref().and_then(pypi_purl_name))
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();

    Some(UvLockCandidate {
        package_uid,
        direct_dependency_names,
    })
}

fn find_uv_lock_index(files: &[FileInfo], root_dir: &Path) -> Option<usize> {
    let root_dir = normalize_lexical_path(root_dir);
    files.iter().position(|file| {
        let path = Path::new(&file.path);
        scanned_file_dir(&file.path).as_deref() == Some(root_dir.as_path())
            && path.file_name().and_then(|name| name.to_str()) == Some("uv.lock")
            && file
                .package_data
                .iter()
                .any(|data| data.datasource_id == Some(DatasourceId::PypiUvLock))
    })
}

/// Extract a PyPI distribution name from a purl and normalize it per PEP 503
/// so that a `uv.lock` entry (`normalize_pypi_name`, lower/trim only) compares
/// equal to a `pyproject.toml` direct dependency (which is dash-normalized).
fn pypi_purl_name(purl: &str) -> Option<String> {
    PackageUrl::from_str(purl)
        .ok()
        .map(|parsed| normalize_pypi_distribution_name(parsed.name()))
}

fn normalize_pypi_distribution_name(name: &str) -> String {
    // Ownership is keyed by distribution identity, which never includes extras.
    // A direct dependency spelled with extras (`requests[socks]`) normally has
    // them split off before the purl is built, but strip any surviving
    // `[extras]` suffix here so a stray extras spelling still matches the base
    // `uv.lock` entry rather than being missed and left hoisted.
    let base = name.split('[').next().unwrap_or(name);
    let lowered = base.trim().to_ascii_lowercase();
    let mut normalized = String::with_capacity(lowered.len());
    let mut last_was_separator = false;
    for character in lowered.chars() {
        if matches!(character, '-' | '_' | '.') {
            if !last_was_separator {
                normalized.push('-');
                last_was_separator = true;
            }
        } else {
            normalized.push(character);
            last_was_separator = false;
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pypi_distribution_name_normalization_is_pep503_and_extras_free() {
        // PEP 503 dash-folding so a `uv.lock` entry (lower/trim only) matches a
        // dash-normalized `pyproject.toml` dependency name.
        assert_eq!(
            normalize_pypi_distribution_name("Flask_Login"),
            "flask-login"
        );
        assert_eq!(
            normalize_pypi_distribution_name("ruamel.yaml"),
            "ruamel-yaml"
        );
        // Extras never change distribution identity for ownership.
        assert_eq!(
            normalize_pypi_distribution_name("requests[socks]"),
            "requests"
        );
        // Extraction from a purl folds the same way.
        assert_eq!(
            pypi_purl_name("pkg:pypi/Flask_Login@0.6.3").as_deref(),
            Some("flask-login")
        );
    }
}
