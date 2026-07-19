// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Maven reactor (multi-module) topology: plan domains and fill nested file ownership.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use crate::models::{DatasourceId, FileInfo, PackageUid};

use super::topology::normalize_lexical_path;

/// A `pom.xml` that declares a non-empty `<modules>` list (stashed by the parser
/// under `extra_data.modules`). Every such POM — root or nested — contributes its
/// own reactor anchor; see [`MavenReactorDomain`] and
/// [`apply_maven_reactor_domains`] for how anchors combine.
pub(super) struct MavenReactorRootHint {
    pub(super) root_dir: PathBuf,
    pub(super) root_pom_idx: usize,
    pub(super) module_paths: Vec<String>,
}

/// A reactor root plus the module `pom.xml` files that were actually found on
/// disk. Module strings that do not resolve to a scanned, purl-bearing `pom.xml`
/// are dropped rather than guessed at.
pub(super) struct MavenReactorDomain {
    pub(super) root_dir: PathBuf,
    pub(super) root_pom_idx: usize,
    pub(super) member_pom_indices: Vec<usize>,
}

pub(super) fn collect_maven_reactor_hints(files: &[FileInfo]) -> Vec<MavenReactorRootHint> {
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

pub(super) fn plan_maven_reactor_domains(
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
pub(super) fn apply_maven_reactor_domains<'a>(
    domains: impl IntoIterator<Item = &'a MavenReactorDomain>,
    files: &mut [FileInfo],
) {
    let mut scope_roots: Vec<PathBuf> = Vec::new();
    let mut anchors: Vec<(PathBuf, Vec<PackageUid>)> = Vec::new();

    for domain in domains {
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
