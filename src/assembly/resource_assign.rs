// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared driver for containment-based package resource assignment.
//!
//! Several ecosystems (Cargo, Composer, RubyGems) attribute a package's
//! source-tree files by the same rule: find each package's root directory, then
//! give that package ownership of every file under the root, skipping a
//! per-ecosystem set of build-output / vendor / cache subtrees (and, for
//! ecosystems that can legitimately nest, deferring ownership to the deepest
//! matching root). Each ecosystem is a single [`ContainmentResourceAssigner`]
//! row in [`RESOURCE_ASSIGNERS`] rather than a bespoke, near-identical pass.
//!
//! npm intentionally does **not** use this driver: it assigns each file to its
//! single *nearest* package owner (replacing any prior ownership) rather than
//! additively to every containing root, so it keeps its own
//! [`super::npm_resource_assign`] implementation.

use std::path::{Path, PathBuf};

use crate::cache::DEFAULT_CACHE_DIR_NAME;
use crate::models::{FileInfo, Package, PackageType, PackageUid};

/// A registered containment-based resource assigner for one ecosystem.
pub(super) struct ContainmentResourceAssigner {
    /// Only packages of this type contribute roots.
    pub(super) package_type: PackageType,
    /// Locate a package's root directory (the directory whose subtree the
    /// package owns), or `None` when the package contributes no root.
    pub(super) find_root: fn(&Package) -> Option<PathBuf>,
    /// First-level child directory names under a root that are excluded from
    /// ownership (build output, vendored dependencies, internal cache).
    pub(super) excluded_root_children: &'static [&'static str],
    /// When true, a file that also sits under a more specific (nested) root of
    /// the same ecosystem is owned only by that deepest root, not the outer one.
    pub(super) skip_nested_roots: bool,
}

impl ContainmentResourceAssigner {
    fn assign(&self, files: &mut [FileInfo], packages: &[Package]) {
        let roots: Vec<(PathBuf, PackageUid)> = packages
            .iter()
            .filter(|package| package.package_type == Some(self.package_type))
            .filter_map(|package| {
                (self.find_root)(package).map(|root| (root, package.package_uid.clone()))
            })
            .collect();

        if roots.is_empty() {
            return;
        }

        for file in files.iter_mut() {
            let path = Path::new(&file.path);

            for (root, package_uid) in &roots {
                if !path.starts_with(root)
                    || is_excluded_child(path, root, self.excluded_root_children)
                {
                    continue;
                }

                if self.skip_nested_roots
                    && roots.iter().any(|(other_root, _)| {
                        other_root != root
                            && other_root.starts_with(root)
                            && path.starts_with(other_root)
                    })
                {
                    continue;
                }

                if !file.for_packages.contains(package_uid) {
                    file.for_packages.push(package_uid.clone());
                }
            }
        }
    }
}

/// Whether `path` lies under one of the root's excluded first-level children.
///
/// An excluded name matches only as the first component beneath `root`
/// (e.g. `<root>/target/...`); a directory of the same name deeper in the tree
/// is not build output and still receives ownership.
fn is_excluded_child(path: &Path, root: &Path, excluded: &[&str]) -> bool {
    if excluded.is_empty() {
        return false;
    }
    path.strip_prefix(root)
        .ok()
        .and_then(|relative| relative.components().next())
        .is_some_and(|component| excluded.iter().any(|name| component.as_os_str() == *name))
}

pub(super) static RESOURCE_ASSIGNERS: &[ContainmentResourceAssigner] = &[
    ContainmentResourceAssigner {
        package_type: PackageType::Cargo,
        find_root: super::cargo_resource_assign::cargo_package_root,
        excluded_root_children: &["target"],
        skip_nested_roots: false,
    },
    ContainmentResourceAssigner {
        package_type: PackageType::Composer,
        find_root: super::composer_resource_assign::composer_package_root,
        excluded_root_children: &["vendor", DEFAULT_CACHE_DIR_NAME],
        skip_nested_roots: true,
    },
    ContainmentResourceAssigner {
        package_type: PackageType::Gem,
        find_root: super::ruby_resource_assign::ruby_package_root,
        excluded_root_children: &[DEFAULT_CACHE_DIR_NAME],
        skip_nested_roots: true,
    },
];

/// Run the registered containment assigner for `package_type` (no-op when the
/// ecosystem has no registered assigner).
pub(super) fn assign_resources_for(
    package_type: PackageType,
    files: &mut [FileInfo],
    packages: &[Package],
) {
    for assigner in RESOURCE_ASSIGNERS {
        if assigner.package_type == package_type {
            assigner.assign(files, packages);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn each_package_type_is_registered_at_most_once() {
        let mut seen = HashSet::new();
        for assigner in RESOURCE_ASSIGNERS {
            assert!(
                seen.insert(assigner.package_type),
                "package type {:?} registered more than once in RESOURCE_ASSIGNERS",
                assigner.package_type
            );
        }
    }
}
