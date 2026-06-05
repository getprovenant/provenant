// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use glob::Pattern;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::utils::file::is_path_excluded;

pub struct CollectedPaths {
    pub files: Vec<(PathBuf, fs::Metadata)>,
    pub directories: Vec<(PathBuf, fs::Metadata)>,
    pub excluded_count: usize,
    pub total_file_bytes: u64,
    pub collection_errors: Vec<(PathBuf, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionFrontier {
    pub path: PathBuf,
    pub recurse: bool,
}

struct CollectionAccumulator {
    files: Vec<(PathBuf, fs::Metadata)>,
    directories: Vec<(PathBuf, fs::Metadata)>,
    file_seen: HashSet<PathBuf>,
    dir_seen: HashSet<PathBuf>,
    excluded_count: usize,
    total_file_bytes: u64,
    collection_errors: Vec<(PathBuf, String)>,
}

enum TraversalMetadata {
    File(fs::Metadata),
    Directory {
        metadata: fs::Metadata,
        can_recurse: bool,
    },
    Other,
}

impl CollectedPaths {
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn directory_count(&self) -> usize {
        self.directories.len()
    }

    pub fn scan_root(&self) -> Option<&Path> {
        self.directories
            .first()
            .map(|(path, _)| path.as_path())
            .or_else(|| {
                self.files
                    .first()
                    .and_then(|(path, _)| path.parent().or(Some(path.as_path())))
            })
    }
}

pub fn collect_paths<P: AsRef<Path>>(
    root: P,
    max_depth: usize,
    exclude_patterns: &[Pattern],
) -> CollectedPaths {
    let depth_limit = depth_limit_from_cli(max_depth);
    let root = root.as_ref();

    if is_path_excluded(root, exclude_patterns) {
        return CollectedPaths {
            files: Vec::new(),
            directories: Vec::new(),
            excluded_count: 1,
            total_file_bytes: 0,
            collection_errors: Vec::new(),
        };
    }

    let traversal_metadata = match classify_for_traversal(root, true) {
        Ok(traversal_metadata) => traversal_metadata,
        Err(error) => {
            return CollectedPaths {
                files: Vec::new(),
                directories: Vec::new(),
                excluded_count: 0,
                total_file_bytes: 0,
                collection_errors: vec![(root.to_path_buf(), error.to_string())],
            };
        }
    };

    match traversal_metadata {
        TraversalMetadata::File(metadata) => CollectedPaths {
            total_file_bytes: metadata.len(),
            files: vec![(root.to_path_buf(), metadata)],
            directories: Vec::new(),
            excluded_count: 0,
            collection_errors: Vec::new(),
        },
        TraversalMetadata::Directory {
            metadata,
            can_recurse,
        } if can_recurse => collect_all_paths(root, &metadata, depth_limit, exclude_patterns),
        TraversalMetadata::Directory { metadata, .. } => CollectedPaths {
            files: Vec::new(),
            directories: vec![(root.to_path_buf(), metadata)],
            excluded_count: 0,
            total_file_bytes: 0,
            collection_errors: Vec::new(),
        },
        TraversalMetadata::Other => CollectedPaths {
            files: Vec::new(),
            directories: Vec::new(),
            excluded_count: 0,
            total_file_bytes: 0,
            collection_errors: Vec::new(),
        },
    }
}

pub fn collect_selected_paths(
    root: &Path,
    selected: &[CollectionFrontier],
    max_depth: usize,
    exclude_patterns: &[Pattern],
) -> CollectedPaths {
    let depth_limit = depth_limit_from_cli(max_depth);

    if is_path_excluded(root, exclude_patterns) {
        return CollectedPaths {
            files: Vec::new(),
            directories: Vec::new(),
            excluded_count: 1,
            total_file_bytes: 0,
            collection_errors: Vec::new(),
        };
    }

    let root_metadata = match classify_for_traversal(root, true) {
        Ok(TraversalMetadata::Directory { metadata, .. }) => metadata,
        Ok(TraversalMetadata::File(metadata)) => metadata,
        Ok(TraversalMetadata::Other) => {
            return CollectedPaths {
                files: Vec::new(),
                directories: Vec::new(),
                excluded_count: 0,
                total_file_bytes: 0,
                collection_errors: Vec::new(),
            };
        }
        Err(error) => {
            return CollectedPaths {
                files: Vec::new(),
                directories: Vec::new(),
                excluded_count: 0,
                total_file_bytes: 0,
                collection_errors: vec![(root.to_path_buf(), error.to_string())],
            };
        }
    };

    let mut accumulator = CollectionAccumulator {
        files: Vec::new(),
        directories: vec![(root.to_path_buf(), root_metadata)],
        file_seen: HashSet::new(),
        dir_seen: HashSet::from([root.to_path_buf()]),
        excluded_count: 0,
        total_file_bytes: 0,
        collection_errors: Vec::new(),
    };

    for frontier in minimize_frontier(selected) {
        let relative_depth = frontier.path.components().count();
        if depth_limit.is_some_and(|limit| relative_depth > limit) {
            continue;
        }

        let absolute = root.join(&frontier.path);
        if is_path_or_any_ancestor_excluded(root, &absolute, exclude_patterns) {
            accumulator.excluded_count += 1;
            continue;
        }

        let traversal_metadata = match classify_for_traversal(&absolute, false) {
            Ok(traversal_metadata) => traversal_metadata,
            Err(error) => {
                accumulator
                    .collection_errors
                    .push((absolute, error.to_string()));
                continue;
            }
        };

        add_ancestor_directories(root, &absolute, &mut accumulator);

        let collected = match traversal_metadata {
            TraversalMetadata::File(metadata) => {
                insert_file(&mut accumulator, absolute, metadata);
                continue;
            }
            TraversalMetadata::Directory {
                metadata,
                can_recurse,
            } if frontier.recurse && can_recurse => {
                let subtree_depth_limit =
                    depth_limit.map(|limit| limit.saturating_sub(relative_depth));
                collect_all_paths(&absolute, &metadata, subtree_depth_limit, exclude_patterns)
            }
            TraversalMetadata::Directory { metadata, .. } => CollectedPaths {
                files: Vec::new(),
                directories: vec![(absolute, metadata)],
                excluded_count: 0,
                total_file_bytes: 0,
                collection_errors: Vec::new(),
            },
            TraversalMetadata::Other => continue,
        };
        merge_collected(&mut accumulator, collected);
    }

    CollectedPaths {
        files: accumulator.files,
        directories: accumulator.directories,
        excluded_count: accumulator.excluded_count,
        total_file_bytes: accumulator.total_file_bytes,
        collection_errors: accumulator.collection_errors,
    }
}

fn collect_all_paths(
    root: &Path,
    root_metadata: &fs::Metadata,
    depth_limit: Option<usize>,
    exclude_patterns: &[Pattern],
) -> CollectedPaths {
    let mut files = Vec::new();
    let mut directories = vec![(root.to_path_buf(), root_metadata.clone())];
    let mut excluded_count = 0;
    let mut total_file_bytes = 0_u64;
    let mut collection_errors = Vec::new();

    let mut pending_dirs: Vec<(PathBuf, Option<usize>)> = vec![(root.to_path_buf(), depth_limit)];

    while let Some((dir_path, current_depth)) = pending_dirs.pop() {
        let entries: Vec<_> = match fs::read_dir(&dir_path) {
            Ok(entries) => entries.filter_map(Result::ok).collect(),
            Err(e) => {
                collection_errors.push((dir_path.clone(), e.to_string()));
                continue;
            }
        };

        for entry in entries {
            let path = entry.path();

            if is_path_excluded(&path, exclude_patterns) {
                excluded_count += 1;
                continue;
            }

            match classify_for_traversal(&path, false) {
                Ok(TraversalMetadata::File(metadata)) => {
                    total_file_bytes += metadata.len();
                    files.push((path, metadata));
                }
                Ok(TraversalMetadata::Directory {
                    metadata,
                    can_recurse,
                }) => {
                    directories.push((path.clone(), metadata));
                    let should_recurse = can_recurse && current_depth.is_none_or(|d| d > 0);
                    if should_recurse {
                        let next_depth = current_depth.map(|d| d - 1);
                        pending_dirs.push((path, next_depth));
                    }
                }
                _ => continue,
            }
        }
    }

    CollectedPaths {
        files,
        directories,
        excluded_count,
        total_file_bytes,
        collection_errors,
    }
}

fn classify_for_traversal(
    path: &Path,
    recurse_into_symlinked_directories: bool,
) -> io::Result<TraversalMetadata> {
    let link_metadata = fs::symlink_metadata(path)?;
    if link_metadata.file_type().is_symlink() {
        let target_metadata = fs::metadata(path)?;
        return Ok(classify_resolved_metadata(
            target_metadata,
            recurse_into_symlinked_directories,
        ));
    }

    Ok(classify_resolved_metadata(link_metadata, true))
}

fn classify_resolved_metadata(metadata: fs::Metadata, can_recurse: bool) -> TraversalMetadata {
    if metadata.is_file() {
        TraversalMetadata::File(metadata)
    } else if metadata.is_dir() {
        TraversalMetadata::Directory {
            metadata,
            can_recurse,
        }
    } else {
        TraversalMetadata::Other
    }
}

fn depth_limit_from_cli(max_depth: usize) -> Option<usize> {
    if max_depth == 0 {
        None
    } else {
        Some(max_depth)
    }
}

fn is_path_or_any_ancestor_excluded(
    path_root: &Path,
    path: &Path,
    exclude_patterns: &[Pattern],
) -> bool {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if is_path_excluded(candidate, exclude_patterns) {
            return true;
        }
        if candidate == path_root {
            break;
        }
        current = candidate.parent();
    }
    false
}

fn minimize_frontier(selected: &[CollectionFrontier]) -> Vec<CollectionFrontier> {
    let mut ordered = selected.to_vec();
    ordered.sort_by_key(|entry| (entry.path.components().count(), !entry.recurse));

    let mut minimized = Vec::new();
    for entry in ordered {
        let covered = minimized.iter().any(|existing: &CollectionFrontier| {
            existing.recurse
                && (entry.path == existing.path || entry.path.starts_with(&existing.path))
        });
        if !covered {
            minimized.push(entry);
        }
    }
    minimized
}

fn add_ancestor_directories(root: &Path, path: &Path, accumulator: &mut CollectionAccumulator) {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir == root {
            break;
        }
        if accumulator.dir_seen.insert(dir.to_path_buf()) {
            match classify_for_traversal(dir, false) {
                Ok(TraversalMetadata::Directory { metadata, .. }) => {
                    accumulator.directories.push((dir.to_path_buf(), metadata))
                }
                Ok(_) => {}
                Err(error) => accumulator
                    .collection_errors
                    .push((dir.to_path_buf(), error.to_string())),
            }
        }
        current = dir.parent();
    }
}

fn insert_file(accumulator: &mut CollectionAccumulator, path: PathBuf, metadata: fs::Metadata) {
    if accumulator.file_seen.insert(path.clone()) {
        accumulator.total_file_bytes += metadata.len();
        accumulator.files.push((path, metadata));
    }
}

fn merge_collected(accumulator: &mut CollectionAccumulator, collected: CollectedPaths) {
    accumulator.excluded_count += collected.excluded_count;
    accumulator
        .collection_errors
        .extend(collected.collection_errors);

    for (path, metadata) in collected.files {
        insert_file(accumulator, path, metadata);
    }
    for (path, metadata) in collected.directories {
        if accumulator.dir_seen.insert(path.clone()) {
            accumulator.directories.push((path, metadata));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CollectionFrontier, collect_paths, collect_selected_paths};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn file_scan_root_uses_parent_directory() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let file_path = temp_dir.path().join("Directory.Packages.props");
        fs::write(&file_path, "<Project />").expect("write props file");

        let collected = collect_paths(&file_path, 0, &[]);
        assert_eq!(collected.file_count(), 1);
        assert_eq!(collected.directory_count(), 0);
        assert_eq!(collected.scan_root(), Some(temp_dir.path()));
    }

    #[test]
    fn collect_paths_recurses_regular_directories() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let nested = temp_dir.path().join("src/bin");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::write(nested.join("main.rs"), "fn main() {}\n").expect("write nested file");

        let collected = collect_paths(temp_dir.path(), 0, &[]);

        assert!(
            collected
                .files
                .iter()
                .any(|(path, _)| path == &temp_dir.path().join("src/bin/main.rs"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn collect_paths_does_not_recurse_into_symlinked_directory_cycle() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path();
        let real = root.join("real");
        fs::create_dir_all(&real).expect("create real directory");
        fs::write(real.join("file.txt"), "content\n").expect("write file");
        symlink(root, real.join("back")).expect("create symlink cycle");

        let collected = collect_paths(root, 0, &[]);

        assert!(collected.collection_errors.is_empty());
        assert_eq!(collected.file_count(), 1);
        assert!(
            collected
                .files
                .iter()
                .all(|(path, _)| !path.starts_with(real.join("back")))
        );
    }

    #[cfg(unix)]
    #[test]
    fn collect_paths_recurses_explicit_symlinked_scan_root() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let target = temp_dir.path().join("target");
        fs::create_dir_all(&target).expect("create target directory");
        fs::write(target.join("inside.txt"), "content\n").expect("write file");
        let root_link = temp_dir.path().join("root-link");
        symlink(&target, &root_link).expect("create root symlink");

        let collected = collect_paths(&root_link, 0, &[]);

        assert!(collected.collection_errors.is_empty());
        assert!(
            collected
                .files
                .iter()
                .any(|(path, _)| path == &root_link.join("inside.txt"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn collect_paths_keeps_symlinked_regular_files_scannable() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path();
        let target = root.join("target.txt");
        fs::write(&target, "content\n").expect("write target file");
        let link = root.join("link.txt");
        symlink(&target, &link).expect("create file symlink");

        let collected = collect_paths(root, 0, &[]);

        assert!(collected.collection_errors.is_empty());
        assert!(
            collected
                .files
                .iter()
                .any(|(path, _)| path == &root.join("link.txt"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn collect_selected_paths_does_not_recurse_into_symlinked_directory() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path();
        let target = root.join("target");
        fs::create_dir_all(&target).expect("create target directory");
        fs::write(target.join("inside.txt"), "content\n").expect("write file");
        symlink(&target, root.join("link")).expect("create directory symlink");

        let collected = collect_selected_paths(
            root,
            &[CollectionFrontier {
                path: PathBuf::from("link"),
                recurse: true,
            }],
            0,
            &[],
        );

        assert!(collected.collection_errors.is_empty());
        assert!(collected.files.is_empty());
        assert!(
            collected
                .directories
                .iter()
                .any(|(path, _)| path == &root.join("link"))
        );
    }
}
