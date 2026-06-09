// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::license_detection::models::{LoadedLicense, LoadedRule};
use crate::models::LicenseIndexProvenance;

pub const SCHEMA_VERSION: u32 = 6;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddedArtifactMetadata {
    pub spdx_license_list_version: String,
    pub license_index_provenance: LicenseIndexProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedLoaderSnapshot {
    // WARNING: `schema_version` and `metadata` MUST stay the first two fields,
    // in this order. The warm-startup fast path in
    // `embedded::index::load_embedded_artifact_metadata_from_bytes` postcard-
    // decodes only this `{schema_version, metadata}` prefix (via
    // `EmbeddedArtifactMetadataPrefix`) without reading `rules`/`licenses`.
    // Inserting or reordering a field before/between these two silently breaks
    // that prefix decode.
    pub schema_version: u32,
    pub metadata: EmbeddedArtifactMetadata,
    pub rules: Vec<LoadedRule>,
    pub licenses: Vec<LoadedLicense>,
}
