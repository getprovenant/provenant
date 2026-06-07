// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Parser for OCI image layout `index.json` files and `docker save` tarball
//! `manifest.json` files.
//!
//! Emits `pkg:oci/<name>@sha256:<digest>` PURLs for each resolved image, where
//! the digest is the image manifest (or config) digest. See the purl-spec `oci`
//! type:
//! <https://github.com/package-url/purl-spec/blob/main/PURL-TYPES.rst#oci>
//!
//! Scope: this parser reads the well-defined `index.json` (OCI image index) and
//! the Docker `docker save` `manifest.json` entrypoints statically. It does not
//! traverse referenced blob manifests under `blobs/sha256/` to resolve
//! per-platform config digests; that broader image-layout traversal is a
//! deferred follow-up.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::json;

use crate::models::{DatasourceId, PackageData, PackageType};
use crate::parser_warn as warn;
use crate::parsers::utils::{MAX_ITERATION_COUNT, read_file_to_string, truncate_field};

use super::PackageParser;
use super::metadata::ParserMetadata;

const PACKAGE_TYPE: PackageType = PackageType::Oci;

/// OCI image index media type.
const OCI_INDEX_MEDIA_TYPE: &str = "application/vnd.oci.image.index.v1+json";
/// Docker distribution manifest list media type (Docker schema 2).
const DOCKER_MANIFEST_LIST_MEDIA_TYPE: &str =
    "application/vnd.docker.distribution.manifest.list.v2+json";

/// containerd records the full image reference (registry/name:tag) here; the
/// standard OCI `ref.name` annotation often holds only the tag, so we prefer
/// this when present.
const CONTAINERD_IMAGE_NAME_ANNOTATION: &str = "io.containerd.image.name";
const REF_NAME_ANNOTATION: &str = "org.opencontainers.image.ref.name";

/// Top-level OCI image index (`index.json`).
#[derive(Debug, Deserialize)]
struct OciImageIndex {
    #[serde(default)]
    #[serde(rename = "mediaType")]
    media_type: Option<String>,
    #[serde(default)]
    #[serde(rename = "schemaVersion")]
    schema_version: Option<u64>,
    #[serde(default)]
    manifests: Vec<OciDescriptor>,
}

/// A descriptor entry in an OCI image index `manifests` array.
#[derive(Debug, Deserialize)]
struct OciDescriptor {
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    platform: Option<OciPlatform>,
    #[serde(default)]
    annotations: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct OciPlatform {
    #[serde(default)]
    architecture: Option<String>,
    #[serde(default)]
    os: Option<String>,
}

/// A single entry in a `docker save` `manifest.json` array.
#[derive(Debug, Deserialize)]
struct DockerSaveManifestEntry {
    #[serde(default)]
    #[serde(rename = "Config")]
    config: Option<String>,
    #[serde(default)]
    #[serde(rename = "RepoTags")]
    repo_tags: Option<Vec<String>>,
}

pub struct OciImageLayoutParser;

impl PackageParser for OciImageLayoutParser {
    const PACKAGE_TYPE: PackageType = PACKAGE_TYPE;

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "OCI image layout index.json and docker save manifest.json",
            file_patterns: &["**/index.json", "**/manifest.json"],
            package_type: "oci",
            primary_language: "",
            documentation_url: Some(
                "https://github.com/opencontainers/image-spec/blob/main/image-layout.md",
            ),
        }]
    }

    fn is_match(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "index.json" || name == "manifest.json")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(content) => content,
            Err(error) => {
                warn!("Failed to read OCI manifest {:?}: {}", path, error);
                return Vec::new();
            }
        };

        parse_oci_content(&content)
    }
}

/// Parses the JSON content of an OCI `index.json` or Docker `manifest.json`.
///
/// Returns an empty vector when the content is valid JSON but not an OCI image
/// index or `docker save` manifest, so the parser does not claim unrelated
/// `index.json` / `manifest.json` files.
pub(crate) fn parse_oci_content(content: &str) -> Vec<PackageData> {
    // `docker save` manifest.json is a JSON array; OCI index.json is an object.
    let trimmed = content.trim_start();
    if trimmed.starts_with('[') {
        return parse_docker_save_manifest(content);
    }

    parse_oci_image_index(content)
}

fn parse_oci_image_index(content: &str) -> Vec<PackageData> {
    let index: OciImageIndex = match serde_json::from_str(content) {
        Ok(index) => index,
        Err(error) => {
            warn!("Failed to parse OCI image index JSON: {}", error);
            return Vec::new();
        }
    };

    if !is_oci_image_index(&index) {
        return Vec::new();
    }

    index
        .manifests
        .into_iter()
        .take(MAX_ITERATION_COUNT)
        .filter_map(package_from_descriptor)
        .collect()
}

/// An OCI image index is identified by its media type, or by a schema version
/// of 2 paired with at least one manifest descriptor.
fn is_oci_image_index(index: &OciImageIndex) -> bool {
    let media_type = index.media_type.as_deref();
    if media_type == Some(OCI_INDEX_MEDIA_TYPE)
        || media_type == Some(DOCKER_MANIFEST_LIST_MEDIA_TYPE)
    {
        return true;
    }

    index.schema_version == Some(2) && !index.manifests.is_empty()
}

fn package_from_descriptor(descriptor: OciDescriptor) -> Option<PackageData> {
    let digest = descriptor
        .digest
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    let annotations = descriptor.annotations.as_ref();
    let containerd_name = annotations
        .and_then(|annotations| annotations.get(CONTAINERD_IMAGE_NAME_ANNOTATION))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let ref_name = annotations
        .and_then(|annotations| annotations.get(REF_NAME_ANNOTATION))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());

    // Prefer the containerd full image reference for identity; the standard OCI
    // ref.name often carries only the tag.
    let identity_ref = containerd_name.or(ref_name);
    let (name, mut tag) = image_name_and_tag(identity_ref);
    let name = name?;
    // If identity came from containerd (full ref) but lacked a tag, fall back to
    // the bare ref.name annotation as the tag.
    if tag.is_none()
        && containerd_name.is_some()
        && let Some(bare) = ref_name
        && !bare.contains('/')
        && !bare.contains(':')
    {
        tag = Some(bare.to_string());
    }

    let mut qualifiers: HashMap<String, String> = HashMap::new();
    if let Some(tag) = tag {
        qualifiers.insert("tag".to_string(), truncate_field(tag));
    }
    if let Some(platform) = descriptor.platform.as_ref()
        && let Some(arch) = platform
            .architecture
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        qualifiers.insert("arch".to_string(), arch.to_string());
    }

    let mut extra_data: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(identity_ref) = identity_ref {
        extra_data.insert("image_ref_name".to_string(), json!(identity_ref));
    }
    if let Some(platform) = descriptor.platform.as_ref()
        && let Some(os) = platform
            .os
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        extra_data.insert("os".to_string(), json!(os));
    }

    Some(build_package(
        Some(name),
        digest.to_string(),
        qualifiers,
        extra_data,
        DatasourceId::OciImageIndex,
    ))
}

fn parse_docker_save_manifest(content: &str) -> Vec<PackageData> {
    let entries: Vec<DockerSaveManifestEntry> = match serde_json::from_str(content) {
        Ok(entries) => entries,
        Err(error) => {
            warn!("Failed to parse docker save manifest JSON: {}", error);
            return Vec::new();
        }
    };

    entries
        .into_iter()
        .take(MAX_ITERATION_COUNT)
        .filter_map(package_from_docker_save_entry)
        .collect()
}

fn package_from_docker_save_entry(entry: DockerSaveManifestEntry) -> Option<PackageData> {
    // The Config field is `blobs/sha256/<hex>.json` (OCI layout) or `<hex>.json`
    // (legacy docker save). The hex is the image config digest.
    let config = entry
        .config
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    let digest = config_to_digest(config)?;

    let ref_name = entry
        .repo_tags
        .as_ref()
        .and_then(|tags| tags.iter().find(|tag| !tag.trim().is_empty()))
        .map(|tag| tag.trim());

    let (name, tag) = image_name_and_tag(ref_name);
    let name = name?;

    let mut qualifiers: HashMap<String, String> = HashMap::new();
    if let Some(tag) = tag {
        qualifiers.insert("tag".to_string(), truncate_field(tag));
    }

    let mut extra_data: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(ref_name) = ref_name {
        extra_data.insert("image_ref_name".to_string(), json!(ref_name));
    }

    Some(build_package(
        Some(name),
        digest,
        qualifiers,
        extra_data,
        DatasourceId::OciImageManifest,
    ))
}

/// Extracts the `sha256:<hex>` config digest from a docker save `Config` path.
fn config_to_digest(config: &str) -> Option<String> {
    if config.starts_with("sha256:") {
        return Some(config.to_string());
    }

    let file_name = Path::new(config)
        .file_name()
        .and_then(|name| name.to_str())?;
    let hex = file_name.strip_suffix(".json").unwrap_or(file_name);
    if hex.is_empty() {
        return None;
    }

    Some(format!("sha256:{hex}"))
}

/// Splits an OCI ref name / docker repo tag into a lowercased image name and an
/// optional tag. A ref of `registry.example.com/library/nginx:1.27` yields name
/// `nginx` and tag `1.27`; the registry/namespace prefix is preserved in
/// `extra_data` rather than the PURL name to keep the identity portable.
fn image_name_and_tag(ref_name: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(ref_name) = ref_name else {
        return (None, None);
    };

    // Strip a digest pin if the ref carries one (`name@sha256:...`).
    let without_digest = ref_name.split_once('@').map_or(ref_name, |(name, _)| name);

    // A colon after the last `/` is the tag separator; a colon before it is a
    // registry port and must not be treated as a tag.
    let last_segment_start = without_digest.rfind('/').map_or(0, |index| index + 1);
    let (repository, tag) = match without_digest[last_segment_start..].split_once(':') {
        Some((repo_segment, tag)) => (
            &without_digest[..last_segment_start + repo_segment.len()],
            Some(tag.to_string()),
        ),
        None => (without_digest, None),
    };

    let name = repository
        .rsplit('/')
        .next()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    (name, tag.filter(|value| !value.trim().is_empty()))
}

fn build_package(
    name: Option<String>,
    digest: String,
    qualifiers: HashMap<String, String>,
    extra_data: HashMap<String, serde_json::Value>,
    datasource_id: DatasourceId,
) -> PackageData {
    let name = name.map(truncate_field);
    let version = truncate_field(digest);

    // OCI packages are standalone (unassembled), so the parser is responsible
    // for the PURL identity rather than the assembly pass.
    let purl = name
        .as_deref()
        .and_then(|name| build_oci_purl(name, &version, &qualifiers));

    PackageData {
        package_type: Some(PACKAGE_TYPE),
        datasource_id: Some(datasource_id),
        name,
        version: Some(version),
        qualifiers: (!qualifiers.is_empty()).then_some(qualifiers),
        extra_data: (!extra_data.is_empty()).then_some(extra_data),
        purl,
        ..Default::default()
    }
}

/// Builds a `pkg:oci/<name>@sha256:<digest>` PURL with `tag` / `arch`
/// qualifiers, per the purl-spec `oci` type. The digest is carried in the
/// version component.
fn build_oci_purl(
    name: &str,
    version: &str,
    qualifiers: &HashMap<String, String>,
) -> Option<String> {
    use packageurl::PackageUrl;

    let mut purl = PackageUrl::new(PACKAGE_TYPE.as_str(), name).ok()?;
    purl.with_version(version).ok()?;

    // Deterministic qualifier order keeps the rendered PURL stable.
    let mut keys: Vec<&String> = qualifiers.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(value) = qualifiers.get(key) {
            let _ = purl.add_qualifier(key.as_str(), value.as_str());
        }
    }

    Some(purl.to_string())
}
