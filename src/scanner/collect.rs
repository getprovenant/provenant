// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use glob::Pattern;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::utils::file::is_path_excluded;

pub struct CollectedPaths {
    pub files: Vec<(PathBuf, fs::Metadata)>,
    pub directories: Vec<(PathBuf, fs::Metadata)>,
    pub excluded_count: usize,
    pub total_file_bytes: u64,
    pub collection_errors: Vec<(PathBuf, String)>,
    /// Set when a configured collection limit stopped the walk before it
    /// finished. The reason is surfaced through `collection_errors`.
    pub limit_reached: bool,
}

/// Bounds applied while walking the filesystem.
///
/// Defaults are fully permissive so ordinary CLI and library scans behave
/// exactly as before. Untrusted callers (notably `provenant serve`) opt into
/// finite ceilings and an out-of-tree symlink guard so a hostile input tree
/// cannot exhaust memory, wall-clock time, or leak content from outside the
/// scan root.
#[derive(Debug, Clone, Default)]
pub struct CollectionLimits {
    /// Maximum number of regular files to collect, if any.
    pub max_file_count: Option<usize>,
    /// Maximum cumulative size of collected files in bytes, if any.
    pub max_total_bytes: Option<u64>,
    /// Wall-clock deadline for the whole collection pass, if any.
    pub deadline: Option<Instant>,
    /// When set, file symlinks whose canonicalized target escapes this root are
    /// skipped instead of being dereferenced and scanned.
    pub symlink_root_guard: Option<PathBuf>,
}

impl CollectionLimits {
    /// Permissive limits used by trusted CLI and library scans.
    pub fn unbounded() -> Self {
        Self::default()
    }

    /// Resolves the symlink guard once per walk so each encountered symlink only
    /// canonicalizes its own target instead of re-canonicalizing the scan root.
    ///
    /// When a guard is requested but its root cannot be canonicalized, the root
    /// path is left un-canonicalized so containment checks fail closed (no real
    /// canonical target will be considered contained), matching the prior
    /// per-symlink behavior.
    fn resolved_for_walk(&self) -> Self {
        let symlink_root_guard = self
            .symlink_root_guard
            .as_ref()
            .map(|root| fs::canonicalize(root).unwrap_or_else(|_| root.clone()));
        Self {
            max_file_count: self.max_file_count,
            max_total_bytes: self.max_total_bytes,
            deadline: self.deadline,
            symlink_root_guard,
        }
    }

    fn deadline_exceeded(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn file_count_reached(&self, collected_files: usize) -> bool {
        self.max_file_count
            .is_some_and(|limit| collected_files >= limit)
    }

    fn total_bytes_exceeded(&self, collected_bytes: u64, next_file_bytes: u64) -> bool {
        self.max_total_bytes
            .is_some_and(|limit| collected_bytes.saturating_add(next_file_bytes) > limit)
    }
}

/// Distinguishes a clean walk from one stopped by a configured limit.
enum WalkBudget {
    Continue,
    Stop(&'static str),
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
    limit_reached: bool,
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
    collect_paths_with_limits(
        root,
        max_depth,
        exclude_patterns,
        &CollectionLimits::unbounded(),
    )
}

pub fn collect_paths_with_limits<P: AsRef<Path>>(
    root: P,
    max_depth: usize,
    exclude_patterns: &[Pattern],
    limits: &CollectionLimits,
) -> CollectedPaths {
    let limits = &limits.resolved_for_walk();
    let depth_limit = depth_limit_from_cli(max_depth);
    let root = root.as_ref();

    if is_path_excluded(root, exclude_patterns) {
        return CollectedPaths {
            files: Vec::new(),
            directories: Vec::new(),
            excluded_count: 1,
            total_file_bytes: 0,
            collection_errors: Vec::new(),
            limit_reached: false,
        };
    }

    let traversal_metadata = match classify_for_traversal(root, true, limits) {
        Ok(traversal_metadata) => traversal_metadata,
        Err(error) => {
            return CollectedPaths {
                files: Vec::new(),
                directories: Vec::new(),
                excluded_count: 0,
                total_file_bytes: 0,
                collection_errors: vec![(root.to_path_buf(), error.to_string())],
                limit_reached: false,
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
            limit_reached: false,
        },
        TraversalMetadata::Directory {
            metadata,
            can_recurse,
        } if can_recurse => {
            collect_all_paths(root, &metadata, depth_limit, exclude_patterns, limits)
        }
        TraversalMetadata::Directory { metadata, .. } => CollectedPaths {
            files: Vec::new(),
            directories: vec![(root.to_path_buf(), metadata)],
            excluded_count: 0,
            total_file_bytes: 0,
            collection_errors: Vec::new(),
            limit_reached: false,
        },
        TraversalMetadata::Other => CollectedPaths {
            files: Vec::new(),
            directories: Vec::new(),
            excluded_count: 0,
            total_file_bytes: 0,
            collection_errors: Vec::new(),
            limit_reached: false,
        },
    }
}

pub fn collect_selected_paths(
    root: &Path,
    selected: &[CollectionFrontier],
    max_depth: usize,
    exclude_patterns: &[Pattern],
) -> CollectedPaths {
    collect_selected_paths_with_limits(
        root,
        selected,
        max_depth,
        exclude_patterns,
        &CollectionLimits::unbounded(),
    )
}

pub fn collect_selected_paths_with_limits(
    root: &Path,
    selected: &[CollectionFrontier],
    max_depth: usize,
    exclude_patterns: &[Pattern],
    limits: &CollectionLimits,
) -> CollectedPaths {
    let limits = &limits.resolved_for_walk();
    let depth_limit = depth_limit_from_cli(max_depth);

    if is_path_excluded(root, exclude_patterns) {
        return CollectedPaths {
            files: Vec::new(),
            directories: Vec::new(),
            excluded_count: 1,
            total_file_bytes: 0,
            collection_errors: Vec::new(),
            limit_reached: false,
        };
    }

    let root_metadata = match classify_for_traversal(root, true, limits) {
        Ok(TraversalMetadata::Directory { metadata, .. }) => metadata,
        Ok(TraversalMetadata::File(metadata)) => metadata,
        Ok(TraversalMetadata::Other) => {
            return CollectedPaths {
                files: Vec::new(),
                directories: Vec::new(),
                excluded_count: 0,
                total_file_bytes: 0,
                collection_errors: Vec::new(),
                limit_reached: false,
            };
        }
        Err(error) => {
            return CollectedPaths {
                files: Vec::new(),
                directories: Vec::new(),
                excluded_count: 0,
                total_file_bytes: 0,
                collection_errors: vec![(root.to_path_buf(), error.to_string())],
                limit_reached: false,
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
        limit_reached: false,
    };

    for frontier in minimize_frontier(selected) {
        if accumulator.limit_reached {
            break;
        }
        let relative_depth = frontier.path.components().count();
        if depth_limit.is_some_and(|limit| relative_depth > limit) {
            continue;
        }

        let absolute = root.join(&frontier.path);
        if is_path_or_any_ancestor_excluded(root, &absolute, exclude_patterns) {
            accumulator.excluded_count += 1;
            continue;
        }

        let traversal_metadata = match classify_for_traversal(&absolute, false, limits) {
            Ok(traversal_metadata) => traversal_metadata,
            Err(error) => {
                accumulator
                    .collection_errors
                    .push((absolute, error.to_string()));
                continue;
            }
        };

        add_ancestor_directories(root, &absolute, &mut accumulator, limits);

        let collected = match traversal_metadata {
            TraversalMetadata::File(metadata) => {
                insert_file(&mut accumulator, absolute, metadata, limits);
                continue;
            }
            TraversalMetadata::Directory {
                metadata,
                can_recurse,
            } if frontier.recurse && can_recurse => {
                let subtree_depth_limit =
                    depth_limit.map(|limit| limit.saturating_sub(relative_depth));
                collect_all_paths(
                    &absolute,
                    &metadata,
                    subtree_depth_limit,
                    exclude_patterns,
                    &subtree_limits(limits, &accumulator),
                )
            }
            TraversalMetadata::Directory { metadata, .. } => CollectedPaths {
                files: Vec::new(),
                directories: vec![(absolute, metadata)],
                excluded_count: 0,
                total_file_bytes: 0,
                collection_errors: Vec::new(),
                limit_reached: false,
            },
            TraversalMetadata::Other => continue,
        };
        merge_collected(&mut accumulator, collected, limits);
    }

    CollectedPaths {
        files: accumulator.files,
        directories: accumulator.directories,
        excluded_count: accumulator.excluded_count,
        total_file_bytes: accumulator.total_file_bytes,
        collection_errors: accumulator.collection_errors,
        limit_reached: accumulator.limit_reached,
    }
}

/// Adjusts the file-count and total-bytes ceilings for a subtree walk so the
/// nested walk accounts for what the parent walk has already collected.
fn subtree_limits(
    limits: &CollectionLimits,
    accumulator: &CollectionAccumulator,
) -> CollectionLimits {
    CollectionLimits {
        max_file_count: limits
            .max_file_count
            .map(|limit| limit.saturating_sub(accumulator.files.len())),
        max_total_bytes: limits
            .max_total_bytes
            .map(|limit| limit.saturating_sub(accumulator.total_file_bytes)),
        deadline: limits.deadline,
        symlink_root_guard: limits.symlink_root_guard.clone(),
    }
}

fn collect_all_paths(
    root: &Path,
    root_metadata: &fs::Metadata,
    depth_limit: Option<usize>,
    exclude_patterns: &[Pattern],
    limits: &CollectionLimits,
) -> CollectedPaths {
    let mut files = Vec::new();
    let mut directories = vec![(root.to_path_buf(), root_metadata.clone())];
    let mut excluded_count = 0;
    let mut total_file_bytes = 0_u64;
    let mut collection_errors = Vec::new();
    let mut limit_reached = false;

    let mut pending_dirs: Vec<(PathBuf, Option<usize>)> = vec![(root.to_path_buf(), depth_limit)];

    'walk: while let Some((dir_path, current_depth)) = pending_dirs.pop() {
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

            match classify_for_traversal(&path, false, limits) {
                Ok(TraversalMetadata::File(metadata)) => {
                    if let WalkBudget::Stop(reason) =
                        enforce_file_budget(limits, files.len(), total_file_bytes, metadata.len())
                    {
                        collection_errors.push((path, reason.to_string()));
                        limit_reached = true;
                        break 'walk;
                    }
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
        limit_reached,
    }
}

/// Decides whether the next file would breach a configured ceiling or deadline.
fn enforce_file_budget(
    limits: &CollectionLimits,
    collected_files: usize,
    collected_bytes: u64,
    next_file_bytes: u64,
) -> WalkBudget {
    if limits.deadline_exceeded() {
        return WalkBudget::Stop("scan exceeded its overall time budget");
    }
    if limits.file_count_reached(collected_files) {
        return WalkBudget::Stop("scan exceeded its maximum file count");
    }
    if limits.total_bytes_exceeded(collected_bytes, next_file_bytes) {
        return WalkBudget::Stop("scan exceeded its maximum total byte size");
    }
    WalkBudget::Continue
}

fn classify_for_traversal(
    path: &Path,
    recurse_into_symlinked_directories: bool,
    limits: &CollectionLimits,
) -> io::Result<TraversalMetadata> {
    let link_metadata = fs::symlink_metadata(path)?;
    if link_metadata.file_type().is_symlink() {
        if let Some(canonical_root) = &limits.symlink_root_guard
            && symlink_target_escapes_root(path, canonical_root)
        {
            // An untrusted input tree points a symlink at content outside the
            // scan root; refuse to dereference it so we never read or emit
            // out-of-tree data.
            return Ok(TraversalMetadata::Other);
        }
        let target_metadata = fs::metadata(path)?;
        return Ok(classify_resolved_metadata(
            target_metadata,
            recurse_into_symlinked_directories,
        ));
    }

    Ok(classify_resolved_metadata(link_metadata, true))
}

/// Returns `true` when the symlink at `path` resolves outside `canonical_root`,
/// or when the target cannot be canonicalized (treated as an escape so we fail
/// closed). `canonical_root` is canonicalized once per walk by the caller.
fn symlink_target_escapes_root(path: &Path, canonical_root: &Path) -> bool {
    match fs::canonicalize(path) {
        Ok(canonical_target) => !canonical_target.starts_with(canonical_root),
        Err(_) => true,
    }
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

fn add_ancestor_directories(
    root: &Path,
    path: &Path,
    accumulator: &mut CollectionAccumulator,
    limits: &CollectionLimits,
) {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir == root {
            break;
        }
        if accumulator.dir_seen.insert(dir.to_path_buf()) {
            match classify_for_traversal(dir, false, limits) {
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

fn insert_file(
    accumulator: &mut CollectionAccumulator,
    path: PathBuf,
    metadata: fs::Metadata,
    limits: &CollectionLimits,
) {
    if accumulator.limit_reached || accumulator.file_seen.contains(&path) {
        return;
    }
    if let WalkBudget::Stop(reason) = enforce_file_budget(
        limits,
        accumulator.files.len(),
        accumulator.total_file_bytes,
        metadata.len(),
    ) {
        accumulator
            .collection_errors
            .push((path, reason.to_string()));
        accumulator.limit_reached = true;
        return;
    }
    accumulator.file_seen.insert(path.clone());
    accumulator.total_file_bytes += metadata.len();
    accumulator.files.push((path, metadata));
}

fn merge_collected(
    accumulator: &mut CollectionAccumulator,
    collected: CollectedPaths,
    limits: &CollectionLimits,
) {
    accumulator.excluded_count += collected.excluded_count;
    accumulator
        .collection_errors
        .extend(collected.collection_errors);

    // Insert the subtree's already-collected files before propagating its
    // limit flag. `insert_file` early-returns once `limit_reached` is set, so
    // flipping the flag first would silently drop valid files the subtree
    // gathered within its own sub-budget.
    for (path, metadata) in collected.files {
        insert_file(accumulator, path, metadata, limits);
    }
    accumulator.limit_reached |= collected.limit_reached;

    for (path, metadata) in collected.directories {
        if accumulator.dir_seen.insert(path.clone()) {
            accumulator.directories.push((path, metadata));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CollectionFrontier, CollectionLimits, collect_paths, collect_paths_with_limits,
        collect_selected_paths, collect_selected_paths_with_limits,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

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

    #[cfg(unix)]
    #[test]
    fn symlink_root_guard_skips_file_symlink_pointing_outside_root() {
        use std::os::unix::fs::symlink;

        let outside_dir = tempfile::tempdir().expect("outside temp dir");
        let secret = outside_dir.path().join("secret.txt");
        fs::write(&secret, "TOP SECRET\n").expect("write out-of-tree secret");

        let scan_dir = tempfile::tempdir().expect("scan temp dir");
        let root = scan_dir.path();
        fs::write(root.join("inside.txt"), "ordinary\n").expect("write inside file");
        symlink(&secret, root.join("creds")).expect("create escaping symlink");

        let limits = CollectionLimits {
            symlink_root_guard: Some(root.to_path_buf()),
            ..CollectionLimits::unbounded()
        };
        let collected = collect_paths_with_limits(root, 0, &[], &limits);

        // The escaping symlink must never appear in the collected file set.
        assert!(
            collected
                .files
                .iter()
                .all(|(path, _)| path != &root.join("creds"))
        );
        // The legitimate in-tree file is still collected.
        assert!(
            collected
                .files
                .iter()
                .any(|(path, _)| path == &root.join("inside.txt"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_root_guard_keeps_in_tree_file_symlinks() {
        use std::os::unix::fs::symlink;

        let scan_dir = tempfile::tempdir().expect("scan temp dir");
        let root = scan_dir.path();
        let target = root.join("target.txt");
        fs::write(&target, "content\n").expect("write target file");
        symlink(&target, root.join("link.txt")).expect("create in-tree symlink");

        let limits = CollectionLimits {
            symlink_root_guard: Some(root.to_path_buf()),
            ..CollectionLimits::unbounded()
        };
        let collected = collect_paths_with_limits(root, 0, &[], &limits);

        assert!(
            collected
                .files
                .iter()
                .any(|(path, _)| path == &root.join("link.txt"))
        );
    }

    #[test]
    fn max_file_count_limit_stops_collection() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path();
        for index in 0..5 {
            fs::write(root.join(format!("file{index}.txt")), "x\n").expect("write file");
        }

        let limits = CollectionLimits {
            max_file_count: Some(2),
            ..CollectionLimits::unbounded()
        };
        let collected = collect_paths_with_limits(root, 0, &[], &limits);

        assert!(collected.limit_reached);
        assert!(collected.file_count() <= 2);
        assert!(
            collected
                .collection_errors
                .iter()
                .any(|(_, reason)| reason.contains("maximum file count"))
        );
    }

    #[test]
    fn max_total_bytes_limit_stops_collection() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path();
        for index in 0..5 {
            fs::write(root.join(format!("file{index}.txt")), vec![b'x'; 100]).expect("write file");
        }

        let limits = CollectionLimits {
            max_total_bytes: Some(150),
            ..CollectionLimits::unbounded()
        };
        let collected = collect_paths_with_limits(root, 0, &[], &limits);

        assert!(collected.limit_reached);
        assert!(collected.total_file_bytes <= 150);
        assert!(
            collected
                .collection_errors
                .iter()
                .any(|(_, reason)| reason.contains("maximum total byte size"))
        );
    }

    #[test]
    fn deadline_already_passed_stops_collection() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path();
        fs::write(root.join("file.txt"), "x\n").expect("write file");

        let limits = CollectionLimits {
            deadline: Some(Instant::now() - Duration::from_secs(1)),
            ..CollectionLimits::unbounded()
        };
        let collected = collect_paths_with_limits(root, 0, &[], &limits);

        assert!(collected.limit_reached);
        assert!(
            collected
                .collection_errors
                .iter()
                .any(|(_, reason)| reason.contains("overall time budget"))
        );
    }

    #[test]
    fn unbounded_limits_preserve_full_collection() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path();
        for index in 0..5 {
            fs::write(root.join(format!("file{index}.txt")), "x\n").expect("write file");
        }

        let collected = collect_paths_with_limits(root, 0, &[], &CollectionLimits::unbounded());

        assert!(!collected.limit_reached);
        assert_eq!(collected.file_count(), 5);
    }

    #[test]
    fn truncated_subtree_keeps_files_collected_within_its_budget() {
        // Regression: when a recursing subtree collects valid files and then
        // trips the file-count limit, those already-collected files must still
        // appear in the merged result instead of being silently dropped.
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path();

        let first = root.join("first");
        fs::create_dir_all(&first).expect("create first dir");
        for index in 0..3 {
            fs::write(first.join(format!("f{index}.txt")), "x\n").expect("write file");
        }

        let second = root.join("second");
        fs::create_dir_all(&second).expect("create second dir");
        for index in 0..5 {
            fs::write(second.join(format!("s{index}.txt")), "x\n").expect("write file");
        }

        let limits = CollectionLimits {
            max_file_count: Some(5),
            ..CollectionLimits::unbounded()
        };
        let collected = collect_selected_paths_with_limits(
            root,
            &[
                CollectionFrontier {
                    path: PathBuf::from("first"),
                    recurse: true,
                },
                CollectionFrontier {
                    path: PathBuf::from("second"),
                    recurse: true,
                },
            ],
            0,
            &[],
            &limits,
        );

        // 3 from `first` plus 2 collected from `second` before it tripped.
        assert!(collected.limit_reached);
        assert_eq!(collected.file_count(), 5);
        assert_eq!(
            collected
                .files
                .iter()
                .filter(|(path, _)| path.starts_with(&first))
                .count(),
            3
        );
        assert_eq!(
            collected
                .files
                .iter()
                .filter(|(path, _)| path.starts_with(&second))
                .count(),
            2
        );
    }
}
