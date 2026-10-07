// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::models::{DatasourceId, Dependency, FileInfo, Package, PackageData, TopLevelDependency};

use super::AssemblerConfig;

struct PendingDependency {
    dependency: Dependency,
    datafile_path: String,
    datasource_id: DatasourceId,
}

pub fn assemble_nested_patterns(
    files: &[FileInfo],
    config: &AssemblerConfig,
) -> Option<(Package, Vec<TopLevelDependency>, Vec<usize>)> {
    if !has_nested_patterns(config) {
        return None;
    }

    let matching_files = find_matching_files(files, config);
    if matching_files.is_empty() {
        return None;
    }

    let package_root = find_package_root(&matching_files, files)?;

    let sibling_indices = find_nested_siblings(&package_root, files, config);

    if sibling_indices.len() < 2 {
        return None;
    }

    let sibling_indices =
        if nested_maven_pom_count(&package_root, &sibling_indices, files, config) > 1 {
            select_uberjar_manifest_owner(&package_root, &sibling_indices, files)?
        } else {
            sibling_indices
        };

    assemble_from_indices(config, files, &sibling_indices)
}

fn has_nested_patterns(config: &AssemblerConfig) -> bool {
    config
        .sibling_file_patterns
        .iter()
        .any(|p| p.contains("**"))
}

fn find_matching_files(files: &[FileInfo], config: &AssemblerConfig) -> Vec<usize> {
    files
        .iter()
        .enumerate()
        .filter(|(_, file)| {
            file.package_data.iter().any(|pkg_data| {
                pkg_data
                    .datasource_id
                    .is_some_and(|dsid| config.datasource_ids.contains(&dsid))
            })
        })
        .map(|(idx, _)| idx)
        .collect()
}

const NESTED_ANCHOR_DIRS: &[&str] = &["META-INF", "debian", "data.gz-extract"];

fn find_package_root(matching_indices: &[usize], files: &[FileInfo]) -> Option<PathBuf> {
    for &idx in matching_indices {
        let file_path = Path::new(&files[idx].path);

        for &anchor in NESTED_ANCHOR_DIRS {
            if file_path.components().any(|c| c.as_os_str() == anchor) {
                let mut current = file_path;
                while let Some(parent) = current.parent() {
                    if parent
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|name| name == anchor)
                    {
                        return parent.parent().map(|p| p.to_path_buf());
                    }
                    current = parent;
                }
            }
        }

        if file_path.file_name().and_then(|n| n.to_str()) == Some("metadata.gz-extract") {
            return file_path.parent().map(|p| p.to_path_buf());
        }

        if file_path.file_name().and_then(|n| n.to_str()) == Some("pom.xml") {
            return file_path.parent().map(|p| p.to_path_buf());
        }
    }

    None
}

fn find_nested_siblings(root: &Path, files: &[FileInfo], config: &AssemblerConfig) -> Vec<usize> {
    files
        .iter()
        .enumerate()
        .filter(|(_, file)| {
            let file_path = Path::new(&file.path);

            if !file_path.starts_with(root) {
                return false;
            }

            config.sibling_file_patterns.iter().any(|pattern| {
                if pattern.contains("**") {
                    matches_nested_pattern(&file.path, pattern)
                } else {
                    file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|name| matches_simple_pattern(name, pattern))
                }
            })
        })
        .map(|(idx, _)| idx)
        .collect()
}

/// Counts Maven POMs anchored under `root`; zero for non-Maven configs.
///
/// The nested merge folds a single packaged artifact (one POM plus its sibling
/// `pom.properties` / `MANIFEST.MF`) into one package. More than one POM means
/// either a source reactor, whose module POMs the per-directory merge already
/// models, or a fat jar bundling shaded POMs; neither may be folded wholesale.
fn nested_maven_pom_count(
    root: &Path,
    indices: &[usize],
    files: &[FileInfo],
    config: &AssemblerConfig,
) -> usize {
    if !config.datasource_ids.contains(&DatasourceId::MavenPom) {
        return 0;
    }

    indices
        .iter()
        .filter(|&&idx| {
            Path::new(&files[idx].path).starts_with(root)
                && files[idx]
                    .package_data
                    .iter()
                    .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::MavenPom))
        })
        .count()
}

/// For a fat jar bundling several `META-INF/maven/<group>/<artifact>/` POMs,
/// selects the jar's root `META-INF/MANIFEST.MF` plus the one POM directory whose
/// identity matches it. Returns `None` (manifest stays unowned) when the manifest
/// has no identity or zero or several POM directories match; shaded POMs keep
/// their own per-directory packages either way.
fn select_uberjar_manifest_owner(
    root: &Path,
    indices: &[usize],
    files: &[FileInfo],
) -> Option<Vec<usize>> {
    let manifest_path = root.join("META-INF").join("MANIFEST.MF");
    let maven_dir = root.join("META-INF").join("maven");

    let manifest_idx = indices
        .iter()
        .copied()
        .find(|&idx| Path::new(&files[idx].path) == manifest_path)?;
    let manifest_identities: Vec<&PackageData> = files[manifest_idx]
        .package_data
        .iter()
        .filter(|pkg_data| {
            matches!(
                pkg_data.datasource_id,
                Some(DatasourceId::JavaJarManifest | DatasourceId::JavaOsgiManifest)
            )
        })
        .collect();

    let pom_dir_of = |idx: usize| -> Option<&Path> {
        let dir = Path::new(&files[idx].path).parent()?;
        (dir.parent()?.parent()? == maven_dir).then_some(dir)
    };

    let mut matched_dirs: Vec<&Path> = indices
        .iter()
        .filter_map(|&idx| {
            let dir = pom_dir_of(idx)?;
            files[idx]
                .package_data
                .iter()
                .filter(|pkg_data| {
                    matches!(
                        pkg_data.datasource_id,
                        Some(DatasourceId::MavenPom | DatasourceId::MavenPomProperties)
                    )
                })
                .any(|pom| {
                    manifest_identities
                        .iter()
                        .any(|manifest| manifest_identity_matches(manifest, pom))
                })
                .then_some(dir)
        })
        .collect();
    matched_dirs.sort_unstable();
    matched_dirs.dedup();

    let [owner_dir] = matched_dirs.as_slice() else {
        return None;
    };

    let mut selected: Vec<usize> = indices
        .iter()
        .copied()
        .filter(|&idx| pom_dir_of(idx) == Some(*owner_dir))
        .collect();
    selected.push(manifest_idx);
    Some(selected)
}

/// Name and version must be present and equal; a manifest namespace, when known,
/// must equal the POM group.
fn manifest_identity_matches(manifest: &PackageData, pom: &PackageData) -> bool {
    manifest.name.is_some()
        && manifest.version.is_some()
        && manifest.name == pom.name
        && manifest.version == pom.version
        && (manifest.namespace.is_none() || manifest.namespace == pom.namespace)
}

fn should_dedupe_ruby_extracted_dependencies(config: &AssemblerConfig) -> bool {
    config
        .datasource_ids
        .contains(&crate::models::DatasourceId::GemArchiveExtracted)
}

fn dependency_identity(
    dep: &TopLevelDependency,
) -> (Option<String>, Option<String>, Option<String>) {
    (
        dep.purl.clone(),
        dep.extracted_requirement.clone(),
        dep.scope.clone(),
    )
}

fn matches_nested_pattern(file_path: &str, pattern: &str) -> bool {
    let pattern_without_prefix = pattern.strip_prefix("**/").unwrap_or(pattern);

    file_path.contains(pattern_without_prefix)
}

fn matches_simple_pattern(file_name: &str, pattern: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix('*') {
        file_name.ends_with(suffix)
            || file_name
                .to_ascii_lowercase()
                .ends_with(&suffix.to_ascii_lowercase())
    } else {
        file_name == pattern || file_name.eq_ignore_ascii_case(pattern)
    }
}

fn assemble_from_indices(
    config: &AssemblerConfig,
    files: &[FileInfo],
    indices: &[usize],
) -> Option<(Package, Vec<TopLevelDependency>, Vec<usize>)> {
    let mut package: Option<Package> = None;
    let mut pending_dependencies = Vec::new();
    let mut affected_indices = Vec::new();

    for &pattern in config.sibling_file_patterns {
        for &idx in indices {
            let file = &files[idx];
            let file_path = Path::new(&file.path);
            let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            let matches = if pattern.contains("**") {
                matches_nested_pattern(&file.path, pattern)
            } else {
                matches_simple_pattern(file_name, pattern)
            };

            if !matches {
                continue;
            }

            if file.package_data.is_empty() {
                continue;
            }

            affected_indices.push(idx);

            for pkg_data in &file.package_data {
                if !is_handled_by(pkg_data, config) {
                    continue;
                }

                let datafile_path = file.path.clone();
                let Some(datasource_id) = pkg_data.datasource_id else {
                    continue;
                };

                match &mut package {
                    None => {
                        if pkg_data.purl.is_some() {
                            package =
                                Some(Package::from_package_data(pkg_data, datafile_path.clone()));
                        }
                    }
                    Some(pkg) => {
                        pkg.update(pkg_data, datafile_path.clone());
                    }
                }

                for dep in &pkg_data.dependencies {
                    if super::is_reportable_dependency(dep) {
                        pending_dependencies.push(PendingDependency {
                            dependency: dep.clone(),
                            datafile_path: datafile_path.clone(),
                            datasource_id,
                        });
                    }
                }
            }
        }
    }

    package.map(|pkg| {
        let for_package_uid = Some(pkg.package_uid.clone());
        let mut dependencies = Vec::new();
        let mut seen_dependency_keys: HashSet<(Option<String>, Option<String>, Option<String>)> =
            HashSet::new();

        for pending in pending_dependencies {
            let candidate = TopLevelDependency::from_dependency(
                &pending.dependency,
                pending.datafile_path,
                pending.datasource_id,
                for_package_uid.clone(),
            );

            if should_dedupe_ruby_extracted_dependencies(config) {
                let key = dependency_identity(&candidate);
                if seen_dependency_keys.insert(key) {
                    dependencies.push(candidate);
                }
            } else {
                dependencies.push(candidate);
            }
        }

        (pkg, dependencies, affected_indices)
    })
}

fn is_handled_by(pkg_data: &PackageData, config: &AssemblerConfig) -> bool {
    pkg_data
        .datasource_id
        .is_some_and(|dsid| config.datasource_ids.contains(&dsid))
}

#[cfg(test)]
#[path = "nested_merge_test.rs"]
mod tests;
