// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared bounded ZIP-archive reading helpers.
//!
//! These helpers centralize the archive-hardening pattern used by package
//! parsers that introspect ZIP-based archives (`.nupkg`, `.whl`, `.jar`, ...).
//! They enforce the [`ADR-0004`](../../docs/adr/0004-security-first-parsing.md)
//! archive-safety limits: per-entry size caps, an overall uncompressed budget,
//! an entry-count cap, compression-ratio (zip-bomb) rejection, and path-traversal
//! sanitization. No archive is ever extracted to disk and no archive content is
//! ever executed.

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

use crate::parser_warn as warn;
use crate::parsers::utils::MAX_ITERATION_COUNT;
use zip::ZipArchive;

/// Largest compressed archive we will open at all.
pub const MAX_ARCHIVE_SIZE: u64 = 100 * 1024 * 1024;
/// Largest single uncompressed entry we will read.
pub const MAX_ENTRY_SIZE: u64 = 50 * 1024 * 1024;
/// Largest cumulative uncompressed size we will account for across all entries.
pub const MAX_TOTAL_UNCOMPRESSED_SIZE: u64 = 1024 * 1024 * 1024;
/// Highest tolerated uncompressed:compressed ratio before treating an entry as a zip bomb.
pub const MAX_COMPRESSION_RATIO: f64 = 100.0;

/// A ZIP entry that has passed the bounded-read safety checks and is safe to read.
#[derive(Clone, Debug)]
pub struct ValidatedZipEntry {
    index: usize,
    name: String,
}

impl ValidatedZipEntry {
    /// Sanitized, forward-slash-normalized entry path.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Open a ZIP archive at `path` after enforcing the compressed-size cap, then
/// collect the list of entries that pass the per-entry and cumulative safety
/// checks. Returns `None` if the archive is too large, cannot be opened, or
/// trips the cumulative uncompressed budget.
pub fn open_bounded_zip(path: &Path) -> Option<(ZipArchive<File>, Vec<ValidatedZipEntry>)> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_ARCHIVE_SIZE {
        warn!(
            "zip archive {:?} exceeds max size ({} > {})",
            path,
            metadata.len(),
            MAX_ARCHIVE_SIZE
        );
        return None;
    }

    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    let entries = collect_validated_zip_entries(&mut archive, path)?;
    Some((archive, entries))
}

fn collect_validated_zip_entries(
    archive: &mut ZipArchive<File>,
    path: &Path,
) -> Option<Vec<ValidatedZipEntry>> {
    let mut entries = Vec::new();
    let mut total_uncompressed: u64 = 0;

    for i in 0..archive.len() {
        if i >= MAX_ITERATION_COUNT {
            warn!(
                "zip archive {:?}: too many entries, stopping at {}",
                path, i
            );
            break;
        }

        let Ok(file) = archive.by_index_raw(i) else {
            continue;
        };

        let compressed_size = file.compressed_size();
        let uncompressed_size = file.size();

        let Some(entry_name) = normalize_archive_entry_path(file.name()) else {
            warn!("Skipping unsafe path in zip {:?}: {}", path, file.name());
            continue;
        };

        if compressed_size > 0 {
            let ratio = uncompressed_size as f64 / compressed_size as f64;
            if ratio > MAX_COMPRESSION_RATIO {
                warn!(
                    "zip archive {:?}: rejecting entry {} for suspicious compression ratio {:.2}:1",
                    path, entry_name, ratio
                );
                continue;
            }
        }

        if uncompressed_size > MAX_ENTRY_SIZE {
            warn!(
                "zip archive {:?}: skipping oversized entry {} ({} bytes)",
                path, entry_name, uncompressed_size
            );
            continue;
        }

        total_uncompressed = total_uncompressed.saturating_add(uncompressed_size);
        if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_SIZE {
            warn!(
                "zip archive {:?}: cumulative uncompressed size exceeds limit ({} > {})",
                path, total_uncompressed, MAX_TOTAL_UNCOMPRESSED_SIZE
            );
            return None;
        }

        entries.push(ValidatedZipEntry {
            index: i,
            name: entry_name,
        });
    }

    Some(entries)
}

/// Find the first validated entry whose normalized name equals `name`.
pub fn find_entry_by_name<'a>(
    entries: &'a [ValidatedZipEntry],
    name: &str,
) -> Option<&'a ValidatedZipEntry> {
    entries.iter().find(|entry| entry.name == name)
}

/// Find every validated entry whose normalized name ends with `suffix`.
pub fn find_entries_by_suffix<'a>(
    entries: &'a [ValidatedZipEntry],
    suffix: &str,
) -> Vec<&'a ValidatedZipEntry> {
    entries
        .iter()
        .filter(|entry| entry.name.ends_with(suffix))
        .collect()
}

/// Read a single validated entry as UTF-8 (lossy on invalid bytes), re-checking
/// the per-entry size and compression-ratio caps against the real archive index.
pub fn read_entry_to_string(
    archive: &mut ZipArchive<File>,
    entry: &ValidatedZipEntry,
    path: &Path,
) -> Result<String, String> {
    let mut file = archive
        .by_index(entry.index)
        .map_err(|e| format!("Failed to open zip entry {}: {}", entry.name, e))?;

    let compressed_size = file.compressed_size();
    let uncompressed_size = file.size();

    if compressed_size > 0 {
        let ratio = uncompressed_size as f64 / compressed_size as f64;
        if ratio > MAX_COMPRESSION_RATIO {
            return Err(format!(
                "Rejected suspicious compression ratio in zip {:?}: {:.2}:1",
                path, ratio
            ));
        }
    }

    if uncompressed_size > MAX_ENTRY_SIZE {
        return Err(format!(
            "Rejected oversized entry in zip {:?}: {} bytes",
            path, uncompressed_size
        ));
    }

    read_limited_utf8(&mut file, MAX_ENTRY_SIZE, &entry.name)
}

fn read_limited_utf8<R: Read>(
    reader: &mut R,
    max_bytes: u64,
    context: &str,
) -> Result<String, String> {
    let mut limited = reader.take(max_bytes + 1);
    let mut bytes = Vec::new();
    limited
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read zip entry {}: {}", context, e))?;

    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "zip entry {} exceeded {} byte limit while reading",
            context, max_bytes
        ));
    }

    match String::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(err) => {
            warn!(
                "Invalid UTF-8 in archive entry {}; using lossy conversion",
                context
            );
            Ok(String::from_utf8_lossy(&err.into_bytes()).into_owned())
        }
    }
}

/// Normalize a ZIP entry path to forward slashes and reject anything that would
/// escape the archive root (absolute paths, drive prefixes, `..` traversal).
pub fn normalize_archive_entry_path(entry_path: &str) -> Option<String> {
    let normalized = entry_path.replace('\\', "/");
    if normalized.len() >= 3 {
        let bytes = normalized.as_bytes();
        if bytes[1] == b':' && bytes[2] == b'/' && bytes[0].is_ascii_alphabetic() {
            return None;
        }
    }

    let mut components = Vec::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(segment) => components.push(segment.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => return None,
        }
    }

    (!components.is_empty()).then(|| components.join("/"))
}
