// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::io::Read;

use serde::Deserialize;

use super::schema::{EmbeddedArtifactMetadata, EmbeddedLoaderSnapshot, SCHEMA_VERSION};
use crate::license_detection::index::LicenseIndex;
use crate::license_detection::index::build_index_from_loaded;

/// Prefix of [`EmbeddedLoaderSnapshot`] covering only the leading fields needed
/// to validate the cache and surface artifact metadata.
///
/// postcard serializes struct fields in declaration order with no whole-struct
/// length framing, so deserializing this prefix from the start of the snapshot
/// yields the same `schema_version` and `metadata` as the full snapshot without
/// touching the (much larger) `rules` and `licenses` that follow. The field
/// order here MUST stay in lockstep with the leading fields of
/// [`EmbeddedLoaderSnapshot`].
#[derive(Debug, Clone, Deserialize)]
struct EmbeddedArtifactMetadataPrefix {
    schema_version: u32,
    metadata: EmbeddedArtifactMetadata,
}

/// Decompressed-prefix budget for the streaming metadata read. The metadata
/// section is a few hundred bytes; this leaves generous headroom while keeping
/// the read bounded so a malformed artifact cannot force decompressing the
/// whole payload before failing.
const METADATA_PREFIX_MAX_BYTES: usize = 64 * 1024;

/// Bytes pulled from the streaming zstd decoder per growth step.
const METADATA_PREFIX_CHUNK_BYTES: usize = 4 * 1024;

// Loader handle: fields are consumed by maintainer/index-build paths, not every routine-scan path.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LoadedEmbeddedLicenseIndex {
    pub index: LicenseIndex,
    pub metadata: EmbeddedArtifactMetadata,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("License loader artifact error: {0}")]
pub struct SerializationError(pub String);

pub fn load_loader_snapshot_from_bytes(
    bytes: &[u8],
) -> Result<EmbeddedLoaderSnapshot, SerializationError> {
    if bytes.is_empty() {
        return Err(SerializationError(
            "Embedded license index artifact is empty".to_string(),
        ));
    }

    let decompressed = zstd::decode_all(bytes).map_err(|e| {
        SerializationError(format!("Failed to decompress embedded artifact: {}", e))
    })?;

    let snapshot: EmbeddedLoaderSnapshot = postcard::from_bytes(&decompressed).map_err(|e| {
        SerializationError(format!("Failed to deserialize embedded artifact: {}", e))
    })?;

    if snapshot.schema_version != SCHEMA_VERSION {
        return Err(SerializationError(format!(
            "Embedded artifact schema version mismatch: expected {}, got {}",
            SCHEMA_VERSION, snapshot.schema_version
        )));
    }

    Ok(snapshot)
}

// Used by maintainer/index-build entry points; not every binary links this path.
#[allow(dead_code)]
pub fn load_embedded_license_index_from_bytes(
    bytes: &[u8],
) -> Result<LoadedEmbeddedLicenseIndex, SerializationError> {
    let snapshot = load_loader_snapshot_from_bytes(bytes)?;
    let index = build_index_from_loaded(snapshot.rules, snapshot.licenses, false);

    Ok(LoadedEmbeddedLicenseIndex {
        index,
        metadata: snapshot.metadata,
    })
}

/// Read just the artifact metadata without materializing the full snapshot.
///
/// This streams the zstd payload and postcard-decodes only the
/// `{schema_version, metadata}` prefix, so warm-cache startup no longer pays
/// for decompressing and deserializing all rules and licenses just to validate
/// the cache. The schema-version check matches
/// [`load_loader_snapshot_from_bytes`] so a stale or mismatched artifact is
/// still rejected here.
pub fn load_embedded_artifact_metadata_from_bytes(
    bytes: &[u8],
) -> Result<EmbeddedArtifactMetadata, SerializationError> {
    if bytes.is_empty() {
        return Err(SerializationError(
            "Embedded license index artifact is empty".to_string(),
        ));
    }

    let mut decoder =
        zstd::stream::read::Decoder::new(std::io::Cursor::new(bytes)).map_err(|e| {
            SerializationError(format!("Failed to decompress embedded artifact: {}", e))
        })?;

    // Grow a decompressed prefix until the metadata prefix decodes, the decoder
    // hits EOF, or we exceed the bounded budget. postcard reports a recoverable
    // "ran out of bytes" error while the prefix is incomplete; any other error
    // is a genuine deserialization failure and must propagate.
    let mut prefix: Vec<u8> = Vec::with_capacity(METADATA_PREFIX_CHUNK_BYTES);
    loop {
        match postcard::take_from_bytes::<EmbeddedArtifactMetadataPrefix>(&prefix) {
            Ok((decoded, _rest)) => {
                if decoded.schema_version != SCHEMA_VERSION {
                    return Err(SerializationError(format!(
                        "Embedded artifact schema version mismatch: expected {}, got {}",
                        SCHEMA_VERSION, decoded.schema_version
                    )));
                }
                return Ok(decoded.metadata);
            }
            Err(postcard::Error::DeserializeUnexpectedEnd) => {}
            Err(e) => {
                return Err(SerializationError(format!(
                    "Failed to deserialize embedded artifact metadata: {}",
                    e
                )));
            }
        }

        if prefix.len() >= METADATA_PREFIX_MAX_BYTES {
            return Err(SerializationError(format!(
                "Embedded artifact metadata exceeds {} byte prefix budget",
                METADATA_PREFIX_MAX_BYTES
            )));
        }

        let mut chunk = [0u8; METADATA_PREFIX_CHUNK_BYTES];
        let read = decoder.read(&mut chunk).map_err(|e| {
            SerializationError(format!("Failed to decompress embedded artifact: {}", e))
        })?;
        if read == 0 {
            // Decoder is exhausted but the prefix still did not decode.
            return Err(SerializationError(
                "Failed to deserialize embedded artifact metadata: artifact ended before metadata"
                    .to_string(),
            ));
        }
        prefix.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_detection::models::{LoadedLicense, LoadedRule};

    fn create_test_metadata() -> EmbeddedArtifactMetadata {
        EmbeddedArtifactMetadata {
            spdx_license_list_version: "3.27".to_string(),
            license_index_provenance: crate::models::LicenseIndexProvenance {
                source: "embedded-artifact".to_string(),
                dataset_fingerprint: "test".to_string(),
                ignored_rules: vec![],
                ignored_licenses: vec![],
                ignored_rules_due_to_licenses: vec![],
                added_rules: vec![],
                replaced_rules: vec![],
                added_licenses: vec![],
                replaced_licenses: vec![],
            },
        }
    }

    fn serialize_loader_snapshot_to_bytes(
        rules: Vec<LoadedRule>,
        licenses: Vec<LoadedLicense>,
    ) -> Result<Vec<u8>, SerializationError> {
        let snapshot = EmbeddedLoaderSnapshot {
            schema_version: SCHEMA_VERSION,
            metadata: create_test_metadata(),
            rules,
            licenses,
        };

        let postcard_bytes = postcard::to_allocvec(&snapshot).map_err(|e| {
            SerializationError(format!("Failed to serialize embedded artifact: {}", e))
        })?;

        zstd::encode_all(&postcard_bytes[..], 0)
            .map_err(|e| SerializationError(format!("Failed to compress embedded artifact: {}", e)))
    }

    fn create_test_loaded_rule() -> LoadedRule {
        LoadedRule {
            identifier: "test.RULE".to_string(),
            license_expression: "mit".to_string(),
            text: "MIT License text".to_string(),
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
        }
    }

    fn create_test_loaded_license() -> LoadedLicense {
        LoadedLicense {
            key: "mit".to_string(),
            short_name: Some("MIT".to_string()),
            name: "MIT License".to_string(),
            language: Some("en".to_string()),
            spdx_license_key: Some("MIT".to_string()),
            other_spdx_license_keys: vec![],
            category: Some("Permissive".to_string()),
            owner: None,
            homepage_url: None,
            text: "MIT License text".to_string(),
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
        }
    }

    #[test]
    fn test_load_license_index_from_bytes_roundtrip() {
        let bytes = serialize_loader_snapshot_to_bytes(
            vec![create_test_loaded_rule()],
            vec![create_test_loaded_license()],
        )
        .expect("Should serialize");

        let index = load_embedded_license_index_from_bytes(&bytes)
            .expect("Should deserialize")
            .index;

        assert_eq!(index.licenses_by_key.len(), 1);
        assert!(
            index
                .rules_by_rid
                .iter()
                .any(|rule| rule.identifier == "test.RULE"),
            "runtime index should retain the serialized rule"
        );
        assert!(
            index
                .rules_by_rid
                .iter()
                .any(|rule| rule.identifier == "mit.LICENSE"),
            "runtime index should synthesize a license-derived rule"
        );
    }

    #[test]
    fn test_load_embedded_artifact_metadata_from_bytes_roundtrip() {
        let bytes = serialize_loader_snapshot_to_bytes(
            vec![create_test_loaded_rule()],
            vec![create_test_loaded_license()],
        )
        .expect("Should serialize");

        let metadata = load_embedded_artifact_metadata_from_bytes(&bytes)
            .expect("Should deserialize metadata");

        assert_eq!(metadata.spdx_license_list_version, "3.27");
        assert_eq!(
            metadata.license_index_provenance.source,
            "embedded-artifact"
        );
    }

    #[test]
    fn test_load_license_index_from_bytes_rejects_empty() {
        let error = load_embedded_license_index_from_bytes(&[]).unwrap_err();
        assert!(error.to_string().contains("artifact is empty"));
    }

    #[test]
    fn test_metadata_prefix_matches_full_decode() {
        // Use enough rules/licenses that the trailing sections clearly dwarf the
        // metadata, so the prefix read cannot accidentally consume everything.
        let rules: Vec<LoadedRule> = (0..256).map(|_| create_test_loaded_rule()).collect();
        let licenses: Vec<LoadedLicense> = (0..256).map(|_| create_test_loaded_license()).collect();
        let bytes = serialize_loader_snapshot_to_bytes(rules, licenses).expect("Should serialize");

        let prefix_metadata =
            load_embedded_artifact_metadata_from_bytes(&bytes).expect("prefix read should succeed");
        let full_metadata = load_loader_snapshot_from_bytes(&bytes)
            .expect("full decode should succeed")
            .metadata;

        assert_eq!(prefix_metadata, full_metadata);
    }

    #[test]
    fn test_metadata_prefix_rejects_empty() {
        let error = load_embedded_artifact_metadata_from_bytes(&[]).unwrap_err();
        assert!(error.to_string().contains("artifact is empty"));
    }

    #[test]
    fn test_metadata_prefix_rejects_schema_version_mismatch() {
        let snapshot = EmbeddedLoaderSnapshot {
            schema_version: SCHEMA_VERSION + 1,
            metadata: create_test_metadata(),
            rules: vec![create_test_loaded_rule()],
            licenses: vec![create_test_loaded_license()],
        };
        let postcard_bytes = postcard::to_allocvec(&snapshot).expect("Should serialize");
        let bytes = zstd::encode_all(&postcard_bytes[..], 0).expect("Should compress");

        let error = load_embedded_artifact_metadata_from_bytes(&bytes).unwrap_err();
        assert!(
            error.to_string().contains("schema version mismatch"),
            "expected schema mismatch, got: {error}"
        );
    }

    #[test]
    fn test_metadata_prefix_rejects_garbage() {
        let bytes =
            zstd::encode_all(&b"not a valid postcard snapshot"[..], 0).expect("Should compress");
        let error = load_embedded_artifact_metadata_from_bytes(&bytes).unwrap_err();
        assert!(
            error.to_string().contains("deserialize"),
            "expected deserialize failure, got: {error}"
        );
    }
}
