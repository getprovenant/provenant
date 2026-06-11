// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use super::io::write_bytes_atomically;
use super::locking::with_exclusive_cache_lock;
use crate::models::{FileInfo, Sha256Digest};
use crate::utils::hash::calculate_sha256;

const INCREMENTAL_MANIFEST_VERSION: u32 = 5;
const MANIFEST_FILE_NAME: &str = "manifest.postcard";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStateFingerprint {
    pub size: u64,
    pub modified_seconds: u64,
    pub modified_nanos: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalManifestEntry {
    pub state: FileStateFingerprint,
    pub content_sha256: Sha256Digest,
    pub file_info: FileInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalManifest {
    pub version: u32,
    pub options_fingerprint: String,
    pub entries: BTreeMap<String, IncrementalManifestEntry>,
}

impl IncrementalManifest {
    pub fn new(
        options_fingerprint: String,
        entries: BTreeMap<String, IncrementalManifestEntry>,
    ) -> Self {
        Self {
            version: INCREMENTAL_MANIFEST_VERSION,
            options_fingerprint,
            entries,
        }
    }

    pub fn entry(&self, relative_path: &str) -> Option<&IncrementalManifestEntry> {
        self.entries.get(relative_path)
    }

    pub fn is_compatible_with(&self, options_fingerprint: &str) -> bool {
        self.version == INCREMENTAL_MANIFEST_VERSION
            && self.options_fingerprint == options_fingerprint
    }
}

pub fn incremental_manifest_path(cache_root: &Path, manifest_key: &str) -> PathBuf {
    cache_root
        .join("incremental")
        .join(manifest_key)
        .join(MANIFEST_FILE_NAME)
}

pub fn metadata_fingerprint(metadata: &fs::Metadata) -> Option<FileStateFingerprint> {
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;

    Some(FileStateFingerprint {
        size: metadata.len(),
        modified_seconds: duration.as_secs(),
        modified_nanos: duration.subsec_nanos(),
    })
}

/// Decides whether a cached manifest entry can be reused for `path`.
///
/// Both modes first require the size + nanosecond-mtime fingerprint to match.
///
/// When `trust_mtime` is `false` (the default, paranoid mode) the file is
/// re-read and SHA-256 hashed, so a content change that preserves both size and
/// mtime is still detected. When `trust_mtime` is `true` the fingerprint match
/// alone is accepted and the read + hash are skipped, trading the rare
/// same-tick, same-size silent edit for warm-rescan speed.
pub fn manifest_entry_matches_path(
    entry: &IncrementalManifestEntry,
    path: &Path,
    metadata: &fs::Metadata,
    trust_mtime: bool,
) -> io::Result<bool> {
    if !metadata_fingerprint(metadata).is_some_and(|fingerprint| fingerprint == entry.state) {
        return Ok(false);
    }

    if trust_mtime {
        return Ok(true);
    }

    let bytes = fs::read(path)?;
    Ok(calculate_sha256(&bytes) == entry.content_sha256)
}

pub fn load_incremental_manifest(
    manifest_path: &Path,
    options_fingerprint: &str,
) -> io::Result<Option<IncrementalManifest>> {
    let bytes = match fs::read(manifest_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    let manifest: IncrementalManifest = match postcard::from_bytes(&bytes) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(None),
    };

    if !manifest.is_compatible_with(options_fingerprint) {
        return Ok(None);
    }

    Ok(Some(manifest))
}

pub fn write_incremental_manifest(
    cache_root: &Path,
    manifest_path: &Path,
    manifest: &IncrementalManifest,
) -> io::Result<()> {
    let bytes = postcard::to_allocvec(manifest)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

    with_exclusive_cache_lock(cache_root, || write_bytes_atomically(manifest_path, &bytes))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::models::{DiagnosticSeverity, FileInfo, FileType, ScanDiagnostic};

    fn sample_manifest(options_fingerprint: &str) -> IncrementalManifest {
        let mut entries = BTreeMap::new();
        entries.insert(
            "src/main.rs".to_string(),
            IncrementalManifestEntry {
                state: FileStateFingerprint {
                    size: 12,
                    modified_seconds: 10,
                    modified_nanos: 20,
                },
                content_sha256: Sha256Digest::from_hex(
                    "f2ca1bb6c7e907d06dafe4687e579fce9f2b2c8a179a4e7c1f6c5052d4f7d070",
                )
                .unwrap(),
                file_info: FileInfo::new(
                    "main.rs".to_string(),
                    "main".to_string(),
                    ".rs".to_string(),
                    "/tmp/project/src/main.rs".to_string(),
                    FileType::File,
                    None,
                    None,
                    12,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Vec::new(),
                    None,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
            },
        );

        IncrementalManifest::new(options_fingerprint.to_string(), entries)
    }

    #[test]
    fn test_load_incremental_manifest_returns_none_for_incompatible_options() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let manifest_path = incremental_manifest_path(temp_dir.path(), "abc123");
        let manifest = sample_manifest("options-v1");

        write_incremental_manifest(temp_dir.path(), &manifest_path, &manifest)
            .expect("write manifest");

        let loaded =
            load_incremental_manifest(&manifest_path, "options-v2").expect("load manifest");

        assert!(loaded.is_none());
    }

    #[test]
    fn test_load_incremental_manifest_returns_none_for_older_manifest_version() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let manifest_path = incremental_manifest_path(temp_dir.path(), "abc123");
        let mut manifest = sample_manifest("options-v1");
        manifest.version = 1;

        write_incremental_manifest(temp_dir.path(), &manifest_path, &manifest)
            .expect("write manifest");

        let loaded =
            load_incremental_manifest(&manifest_path, "options-v1").expect("load manifest");

        assert!(loaded.is_none());
    }

    #[test]
    fn test_write_and_load_incremental_manifest_round_trip() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let manifest_path = incremental_manifest_path(temp_dir.path(), "abc123");
        let manifest = sample_manifest("options-v1");

        write_incremental_manifest(temp_dir.path(), &manifest_path, &manifest)
            .expect("write manifest");

        let loaded = load_incremental_manifest(&manifest_path, "options-v1")
            .expect("load manifest")
            .expect("expected manifest");

        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entry("src/main.rs").is_some());
    }

    #[test]
    fn test_incremental_manifest_preserves_scan_diagnostic_severity() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let manifest_path = incremental_manifest_path(temp_dir.path(), "diag");
        let mut manifest = sample_manifest("options-v1");
        let entry = manifest
            .entries
            .get_mut("src/main.rs")
            .expect("manifest entry");
        entry.file_info.scan_diagnostics =
            vec![ScanDiagnostic::warning("custom recoverable warning")];

        write_incremental_manifest(temp_dir.path(), &manifest_path, &manifest)
            .expect("write manifest");

        let loaded = load_incremental_manifest(&manifest_path, "options-v1")
            .expect("load manifest")
            .expect("expected manifest");

        let loaded_entry = loaded.entry("src/main.rs").expect("loaded entry");
        assert_eq!(loaded_entry.file_info.scan_diagnostics.len(), 1);
        assert_eq!(
            loaded_entry.file_info.scan_diagnostics[0].severity,
            DiagnosticSeverity::Warning
        );
    }

    #[test]
    fn test_manifest_entry_matches_path_detects_content_changes() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("src/main.rs");
        fs::create_dir_all(file_path.parent().expect("parent")).expect("create parent");
        fs::write(&file_path, "fn main() {}\n").expect("write file");
        let metadata = fs::metadata(&file_path).expect("metadata");

        let entry = IncrementalManifestEntry {
            state: metadata_fingerprint(&metadata).expect("fingerprint"),
            content_sha256: Sha256Digest::from_hex("not-the-real-hash")
                .unwrap_or(Sha256Digest::EMPTY),
            file_info: FileInfo::new(
                "main.rs".to_string(),
                "main".to_string(),
                ".rs".to_string(),
                file_path.to_string_lossy().to_string(),
                FileType::File,
                None,
                None,
                metadata.len(),
                None,
                None,
                None,
                None,
                None,
                Vec::new(),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        };

        assert!(
            !manifest_entry_matches_path(&entry, &file_path, &metadata, false)
                .expect("compare path")
        );
    }

    /// Builds a manifest entry whose recorded SHA-256 deliberately does NOT
    /// match the file's real contents, so any code path that re-hashes the file
    /// returns `false` while a path that trusts the fingerprint returns `true`.
    fn entry_with_wrong_hash(
        file_path: &Path,
        metadata: &fs::Metadata,
    ) -> IncrementalManifestEntry {
        IncrementalManifestEntry {
            state: metadata_fingerprint(metadata).expect("fingerprint"),
            content_sha256: Sha256Digest::EMPTY,
            file_info: FileInfo::new(
                "main.rs".to_string(),
                "main".to_string(),
                ".rs".to_string(),
                file_path.to_string_lossy().to_string(),
                FileType::File,
                None,
                None,
                metadata.len(),
                None,
                None,
                None,
                None,
                None,
                Vec::new(),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        }
    }

    #[test]
    fn test_trust_mtime_accepts_fingerprint_match_without_rehashing() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("src/main.rs");
        fs::create_dir_all(file_path.parent().expect("parent")).expect("create parent");
        fs::write(&file_path, "fn main() {}\n").expect("write file");
        let metadata = fs::metadata(&file_path).expect("metadata");

        // The recorded hash is wrong, so the only way this returns `true` is by
        // trusting the fingerprint and skipping the read + hash.
        let entry = entry_with_wrong_hash(&file_path, &metadata);

        assert!(
            manifest_entry_matches_path(&entry, &file_path, &metadata, true).expect("compare path"),
            "trust-mtime mode must accept a size+mtime match without re-hashing"
        );
    }

    #[test]
    fn test_default_mode_rehashes_on_fingerprint_match() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("src/main.rs");
        fs::create_dir_all(file_path.parent().expect("parent")).expect("create parent");
        fs::write(&file_path, "fn main() {}\n").expect("write file");
        let metadata = fs::metadata(&file_path).expect("metadata");

        // Same wrong-hash entry: default (paranoid) mode must re-hash and reject.
        let entry = entry_with_wrong_hash(&file_path, &metadata);

        assert!(
            !manifest_entry_matches_path(&entry, &file_path, &metadata, false)
                .expect("compare path"),
            "default mode must re-hash and reject a fingerprint match whose content differs"
        );
    }

    #[test]
    fn test_trust_mtime_still_detects_changed_fingerprint() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("src/main.rs");
        fs::create_dir_all(file_path.parent().expect("parent")).expect("create parent");
        fs::write(&file_path, "fn main() {}\n").expect("write file");
        let metadata = fs::metadata(&file_path).expect("metadata");

        // Record a fingerprint for a different size so the size check alone fails.
        let mut state = metadata_fingerprint(&metadata).expect("fingerprint");
        state.size += 1;
        let mut entry = entry_with_wrong_hash(&file_path, &metadata);
        entry.state = state;

        assert!(
            !manifest_entry_matches_path(&entry, &file_path, &metadata, true)
                .expect("compare path"),
            "trust-mtime mode must still treat a changed size/mtime fingerprint as changed"
        );
    }
}
