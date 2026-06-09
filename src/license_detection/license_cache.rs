// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::cache::{CacheConfig, write_bytes_atomically};
use crate::license_detection::index::LicenseIndex;
use crate::license_detection::models::{LoadedLicense, LoadedRule};

const CACHE_ROOT_DIR_NAME: &str = "license-index";
const CACHE_FILE_EXTENSION: &str = "rkyv";

/// On-disk cache layout: `[32-byte rules fingerprint][32-byte payload digest][rkyv payload]`.
///
/// The fingerprint identifies which rules/licenses the payload was built from
/// (a derivable constant, used to select and validate the cache file). The
/// payload digest is a SHA-256 over the rkyv payload bytes and is the actual
/// integrity check: it is verified before any deserialization so tampered or
/// truncated payloads are rejected before reaching the rkyv/automaton decoders.
const FINGERPRINT_LEN: usize = 32;
const PAYLOAD_DIGEST_LEN: usize = 32;
const HEADER_LEN: usize = FINGERPRINT_LEN + PAYLOAD_DIGEST_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseCacheNamespace {
    Embedded,
    CustomRules,
}

impl LicenseCacheNamespace {
    fn directory_name(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::CustomRules => "custom",
        }
    }
}

pub struct LicenseCacheConfig {
    pub root_dir: PathBuf,
    pub reindex: bool,
    pub enabled: bool,
}

impl LicenseCacheConfig {
    pub fn new(root_dir: PathBuf, reindex: bool, enabled: bool) -> Self {
        Self {
            root_dir,
            reindex,
            enabled,
        }
    }

    pub fn default_root_dir() -> PathBuf {
        CacheConfig::default_root_dir_without_scan_root()
    }

    fn namespace_dir(&self, namespace: LicenseCacheNamespace) -> PathBuf {
        self.root_dir
            .join(CACHE_ROOT_DIR_NAME)
            .join(namespace.directory_name())
    }

    fn cache_file_path(&self, namespace: LicenseCacheNamespace, fingerprint: &[u8; 32]) -> PathBuf {
        self.namespace_dir(namespace).join(format!(
            "{}.{}",
            fingerprint_hex(fingerprint),
            CACHE_FILE_EXTENSION
        ))
    }
}

fn fingerprint_hex(fingerprint: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(fingerprint.len() * 2);
    for byte in fingerprint {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn prune_namespace_dir(namespace_dir: &Path, active_path: &Path) -> Result<()> {
    if !namespace_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(namespace_dir)
        .with_context(|| format!("Failed to read license cache namespace {namespace_dir:?}"))?
    {
        let path = entry?.path();
        if path == active_path
            || path.extension().and_then(|ext| ext.to_str()) != Some(CACHE_FILE_EXTENSION)
        {
            continue;
        }
        fs::remove_file(&path)
            .with_context(|| format!("Failed to prune stale license cache file {path:?}"))?;
    }

    Ok(())
}

pub fn compute_rules_fingerprint(
    rules: &[LoadedRule],
    licenses: &[LoadedLicense],
) -> Result<[u8; 32]> {
    let mut sorted_rules: Vec<_> = rules.iter().collect();
    sorted_rules.sort_by_key(|r| &r.identifier);
    let mut sorted_licenses: Vec<_> = licenses.iter().collect();
    sorted_licenses.sort_by_key(|l| &l.key);

    let serialized = postcard::to_allocvec(&(sorted_rules, sorted_licenses))
        .context("Failed to serialize effective rules/licenses for cache fingerprinting")?;

    Ok(Sha256::digest(serialized).into())
}

pub fn compute_artifact_fingerprint(artifact_bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(artifact_bytes).into()
}

pub fn load_cached_index(
    config: &LicenseCacheConfig,
    namespace: LicenseCacheNamespace,
    fingerprint: &[u8; 32],
) -> Result<Option<LicenseIndex>> {
    if !config.enabled {
        return Ok(None);
    }

    let cache_path = config.cache_file_path(namespace, fingerprint);

    if !cache_path.exists() {
        return Ok(None);
    }

    let bytes = match fs::read(&cache_path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    };

    // A header is required for the fingerprint + payload digest. Files shorter
    // than the header (including the old fingerprint-only layout) are treated as
    // a miss so they are transparently rebuilt rather than erroring.
    if bytes.len() < HEADER_LEN {
        return Ok(None);
    }

    let stored_fingerprint = &bytes[..FINGERPRINT_LEN];
    if stored_fingerprint != fingerprint.as_slice() {
        return Ok(None);
    }

    let stored_digest = &bytes[FINGERPRINT_LEN..HEADER_LEN];
    let payload = &bytes[HEADER_LEN..];

    // Integrity check over the actual payload bytes, performed before any
    // deserialization. A mismatch (tamper, truncation, partial write, or a
    // stale layout) is treated as a miss so the index is rebuilt safely.
    let computed_digest: [u8; PAYLOAD_DIGEST_LEN] = Sha256::digest(payload).into();
    if stored_digest != computed_digest.as_slice() {
        return Ok(None);
    }

    let archived = match rkyv::access::<rkyv::Archived<LicenseIndex>, rkyv::rancor::Error>(payload)
    {
        Ok(archived) => archived,
        Err(_) => return Ok(None),
    };

    // Use the fallible strategy so a payload whose automaton bytes fail the
    // checked daachorse deserializer surfaces as a miss instead of a panic.
    match rkyv::deserialize::<LicenseIndex, rkyv::rancor::Error>(archived) {
        Ok(cached) => Ok(Some(cached)),
        Err(_) => Ok(None),
    }
}

pub fn save_cached_index(
    config: &LicenseCacheConfig,
    namespace: LicenseCacheNamespace,
    cached: &LicenseIndex,
    fingerprint: &[u8; 32],
) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(cached)
        .map_err(|e| anyhow::anyhow!("Failed to serialize license index cache: {}", e))?;

    // Layout: [fingerprint][SHA-256(payload)][payload]. The digest is verified
    // on load before any deserialization (see `load_cached_index`).
    let payload_digest: [u8; PAYLOAD_DIGEST_LEN] = Sha256::digest(&rkyv_bytes).into();

    let mut file_bytes = Vec::with_capacity(HEADER_LEN + rkyv_bytes.len());
    file_bytes.extend_from_slice(fingerprint);
    file_bytes.extend_from_slice(&payload_digest);
    file_bytes.extend_from_slice(&rkyv_bytes);

    let namespace_dir = config.namespace_dir(namespace);
    let cache_path = config.cache_file_path(namespace, fingerprint);

    crate::cache::locking::with_exclusive_cache_lock(&config.root_dir, || {
        // Cache entries can hold license/copyright text and file paths from
        // private repositories, so restrict the directory tree to the owner.
        crate::cache::create_dir_all_private(&namespace_dir)
            .with_context(|| "Failed to create license index cache directory")?;
        prune_namespace_dir(&namespace_dir, &cache_path)?;
        write_bytes_atomically(&cache_path, &file_bytes)
            .with_context(|| "Failed to persist license index cache file")
    })?;

    Ok(())
}

pub fn delete_cache(
    config: &LicenseCacheConfig,
    namespace: LicenseCacheNamespace,
    fingerprint: &[u8; 32],
) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    let cache_path = config.cache_file_path(namespace, fingerprint);
    crate::cache::locking::with_exclusive_cache_lock(&config.root_dir, || -> Result<()> {
        if cache_path.exists() {
            fs::remove_file(&cache_path).context("Failed to delete license index cache file")?;
        }
        Ok(())
    })?;

    Ok(())
}

pub fn cache_file_size(
    config: &LicenseCacheConfig,
    namespace: LicenseCacheNamespace,
    fingerprint: &[u8; 32],
) -> Option<u64> {
    if !config.enabled {
        return None;
    }

    fs::metadata(config.cache_file_path(namespace, fingerprint))
        .ok()
        .map(|m| m.len())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::license_detection::automaton::Automaton;
    use crate::license_detection::index::dictionary::TokenDictionary;

    fn sample_cached_index() -> LicenseIndex {
        LicenseIndex {
            dictionary: TokenDictionary::default(),
            len_legalese: 0,
            rid_by_hash: Default::default(),
            rules_by_rid: Default::default(),
            tids_by_rid: Default::default(),
            rules_automaton: Automaton::empty(),
            unknown_automaton: Automaton::empty(),
            sets_by_rid: Default::default(),
            rule_metadata_by_identifier: Default::default(),
            msets_by_rid: Default::default(),
            high_sets_by_rid: Default::default(),
            high_postings_by_rid: Default::default(),
            licenses_by_key: Default::default(),
            rid_by_spdx_key: Default::default(),
            unknown_spdx_rid: None,
            rids_by_high_tid: Default::default(),
            spdx_license_list_version: Some("test".to_string()),
        }
    }

    #[test]
    fn test_cache_file_path_uses_namespace_and_fingerprint() {
        let config = LicenseCacheConfig::new(PathBuf::from("/tmp/cache-root"), false, true);
        let fingerprint = [0xAB; 32];

        assert_eq!(
            config.cache_file_path(LicenseCacheNamespace::Embedded, &fingerprint),
            PathBuf::from(format!(
                "/tmp/cache-root/license-index/embedded/{}.rkyv",
                "ab".repeat(32)
            ))
        );
        assert_eq!(
            config.cache_file_path(LicenseCacheNamespace::CustomRules, &fingerprint),
            PathBuf::from(format!(
                "/tmp/cache-root/license-index/custom/{}.rkyv",
                "ab".repeat(32)
            ))
        );
    }

    #[test]
    fn test_save_cached_index_prunes_stale_namespace_entries() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = LicenseCacheConfig::new(temp_dir.path().to_path_buf(), false, true);
        let fingerprint = [0x11; 32];
        let namespace_dir = config.namespace_dir(LicenseCacheNamespace::Embedded);
        fs::create_dir_all(&namespace_dir).expect("create namespace dir");
        fs::write(namespace_dir.join("stale.rkyv"), b"old").expect("write stale cache file");

        let cached = sample_cached_index();
        save_cached_index(
            &config,
            LicenseCacheNamespace::Embedded,
            &cached,
            &fingerprint,
        )
        .expect("save cache");

        let entries = fs::read_dir(&namespace_dir)
            .expect("read namespace dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();

        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            config.cache_file_path(LicenseCacheNamespace::Embedded, &fingerprint)
        );
    }

    #[test]
    fn test_save_then_load_round_trip() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = LicenseCacheConfig::new(temp_dir.path().to_path_buf(), false, true);
        let fingerprint = [0x33; 32];

        save_cached_index(
            &config,
            LicenseCacheNamespace::Embedded,
            &sample_cached_index(),
            &fingerprint,
        )
        .expect("save cache");

        let loaded = load_cached_index(&config, LicenseCacheNamespace::Embedded, &fingerprint)
            .expect("load cache")
            .expect("cache hit");
        assert_eq!(loaded.spdx_license_list_version.as_deref(), Some("test"));
    }

    #[test]
    fn test_tampered_payload_is_rejected_before_deserialization() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = LicenseCacheConfig::new(temp_dir.path().to_path_buf(), false, true);
        let fingerprint = [0x44; 32];

        save_cached_index(
            &config,
            LicenseCacheNamespace::Embedded,
            &sample_cached_index(),
            &fingerprint,
        )
        .expect("save cache");

        let cache_path = config.cache_file_path(LicenseCacheNamespace::Embedded, &fingerprint);
        let mut bytes = fs::read(&cache_path).expect("read cache file");
        // Flip a payload byte (past the fingerprint + digest header) so the
        // stored digest no longer matches; the load must treat this as a miss
        // rather than feeding the tampered bytes into any deserializer.
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        fs::write(&cache_path, &bytes).expect("rewrite tampered cache file");

        let loaded = load_cached_index(&config, LicenseCacheNamespace::Embedded, &fingerprint)
            .expect("load should not error on tamper");
        assert!(
            loaded.is_none(),
            "tampered payload must be rejected as a cache miss"
        );
    }

    #[test]
    fn test_legacy_fingerprint_only_layout_is_treated_as_miss() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = LicenseCacheConfig::new(temp_dir.path().to_path_buf(), false, true);
        let fingerprint = [0x55; 32];

        // Simulate an old cache file written before the payload-digest header:
        // [fingerprint][rkyv payload] with no digest. It must be a miss, not an
        // error, so it is rebuilt transparently.
        let rkyv_bytes =
            rkyv::to_bytes::<rkyv::rancor::Error>(&sample_cached_index()).expect("serialize index");
        let mut legacy = Vec::new();
        legacy.extend_from_slice(&fingerprint);
        legacy.extend_from_slice(&rkyv_bytes);

        let namespace_dir = config.namespace_dir(LicenseCacheNamespace::Embedded);
        fs::create_dir_all(&namespace_dir).expect("create namespace dir");
        let cache_path = config.cache_file_path(LicenseCacheNamespace::Embedded, &fingerprint);
        fs::write(&cache_path, &legacy).expect("write legacy cache file");

        let loaded = load_cached_index(&config, LicenseCacheNamespace::Embedded, &fingerprint)
            .expect("load should not error on legacy layout");
        assert!(loaded.is_none(), "legacy layout must be treated as a miss");
    }

    #[cfg(unix)]
    #[test]
    fn test_saved_cache_file_and_dirs_use_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().expect("create temp dir");
        let config = LicenseCacheConfig::new(temp_dir.path().to_path_buf(), false, true);
        let fingerprint = [0x66; 32];

        save_cached_index(
            &config,
            LicenseCacheNamespace::Embedded,
            &sample_cached_index(),
            &fingerprint,
        )
        .expect("save cache");

        let cache_path = config.cache_file_path(LicenseCacheNamespace::Embedded, &fingerprint);
        let file_mode = fs::metadata(&cache_path)
            .expect("file metadata")
            .permissions()
            .mode();
        assert_eq!(file_mode & 0o777, 0o600, "cache file must be owner-only");

        let namespace_dir = config.namespace_dir(LicenseCacheNamespace::Embedded);
        let dir_mode = fs::metadata(&namespace_dir)
            .expect("dir metadata")
            .permissions()
            .mode();
        assert_eq!(dir_mode & 0o777, 0o700, "cache dir must be owner-only");
    }

    #[test]
    fn test_disabled_cache_skips_persistence() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = LicenseCacheConfig::new(temp_dir.path().to_path_buf(), false, false);
        let fingerprint = [0x22; 32];

        save_cached_index(
            &config,
            LicenseCacheNamespace::Embedded,
            &sample_cached_index(),
            &fingerprint,
        )
        .expect("disabled save should succeed");

        assert!(
            !config
                .cache_file_path(LicenseCacheNamespace::Embedded, &fingerprint)
                .exists()
        );
        assert!(
            load_cached_index(&config, LicenseCacheNamespace::Embedded, &fingerprint)
                .expect("disabled load should succeed")
                .is_none()
        );
    }

    #[test]
    fn test_compute_rules_fingerprint_changes_when_rule_metadata_changes() {
        let rule_a = LoadedRule {
            identifier: "example.RULE".to_string(),
            license_expression: "mit".to_string(),
            text: "example text".to_string(),
            rule_kind: crate::license_detection::models::RuleKind::Text,
            is_false_positive: false,
            is_required_phrase: false,
            skip_for_required_phrase_generation: false,
            relevance: Some(100),
            minimum_coverage: None,
            has_stored_minimum_coverage: false,
            is_continuous: false,
            referenced_filenames: None,
            ignorable_urls: None,
            ignorable_emails: None,
            ignorable_copyrights: None,
            ignorable_holders: None,
            ignorable_authors: None,
            language: None,
            notes: None,
            is_deprecated: false,
            replaced_by: vec![],
        };
        let mut rule_b = rule_a.clone();
        rule_b.referenced_filenames = Some(vec!["LICENSE".to_string()]);

        let license = LoadedLicense {
            key: "mit".to_string(),
            short_name: Some("MIT".to_string()),
            name: "MIT License".to_string(),
            language: Some("en".to_string()),
            spdx_license_key: Some("MIT".to_string()),
            other_spdx_license_keys: vec![],
            category: Some("Permissive".to_string()),
            owner: None,
            homepage_url: None,
            text: "MIT text".to_string(),
            reference_urls: vec![],
            osi_license_key: None,
            text_urls: vec![],
            osi_url: None,
            faq_url: None,
            other_urls: vec![],
            notes: None,
            is_deprecated: false,
            is_exception: false,
            is_unknown: false,
            is_generic: false,
            replaced_by: vec![],
            minimum_coverage: None,
            standard_notice: None,
            ignorable_copyrights: None,
            ignorable_holders: None,
            ignorable_authors: None,
            ignorable_urls: None,
            ignorable_emails: None,
        };

        let fingerprint_a = compute_rules_fingerprint(&[rule_a], std::slice::from_ref(&license))
            .expect("fingerprint A");
        let fingerprint_b =
            compute_rules_fingerprint(&[rule_b], &[license]).expect("fingerprint B");

        assert_ne!(fingerprint_a, fingerprint_b);
    }
}
