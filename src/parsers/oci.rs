// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Parser for OCI image layout `index.json` files and `docker save` tarball
//! `manifest.json` files.
//!
//! Emits `pkg:oci/<name>@sha256:<digest>` PURLs for each resolved image, where
//! the digest is the image **config** digest. See the purl-spec `oci` type:
//! <https://github.com/package-url/purl-spec/blob/main/types/oci-definition.json>
//!
//! Resolution model: for an OCI image layout (`index.json`) the parser follows
//! each manifest descriptor statically into the referenced
//! `blobs/sha256/<hex>` blob to recover the per-platform image manifest, then
//! uses that manifest's `config.digest` as the PURL version. Nested image
//! indexes (manifest lists referenced from a top-level index) are followed up
//! to a bounded depth. For `docker save` `manifest.json`, the `Config` field
//! already points at the image config blob, so its digest is used directly.
//!
//! Inherent limitation: when the referenced blob is not present on disk (for
//! example a bare `index.json` lifted out of its layout, or a sparse/lazy-pull
//! layout), the per-platform config digest cannot be recovered statically. In
//! that case the parser falls back to the manifest *descriptor* digest as the
//! version and records `digest_source = "descriptor"` in `extra_data`. This is
//! a property of the input (the blob is simply absent), not a deferred feature.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::json;

use crate::models::{DatasourceId, PackageData, PackageType};
use crate::parser_warn as warn;
use crate::parsers::utils::{
    MAX_ITERATION_COUNT, capped_iteration_limit, read_file_to_string, truncate_field,
};

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

/// Upper bound on how deeply nested image indexes are followed. Real layouts
/// nest at most index -> manifest list -> manifest; this keeps a hostile or
/// cyclic layout bounded.
const MAX_INDEX_DEPTH: usize = 8;

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
    #[serde(rename = "mediaType")]
    media_type: Option<String>,
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

/// An OCI / Docker schema-2 image manifest blob. Only the config descriptor is
/// needed to recover the image config digest.
#[derive(Debug, Deserialize)]
struct OciImageManifest {
    #[serde(default)]
    config: Option<OciConfigDescriptor>,
}

#[derive(Debug, Deserialize)]
struct OciConfigDescriptor {
    #[serde(default)]
    digest: Option<String>,
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

        // The layout root is the directory containing index.json; blobs live in
        // `<root>/blobs/sha256/<hex>`. `manifest.json` (docker save) is
        // self-contained and does not need the root.
        let layout_root = path.parent().map(Path::to_path_buf);
        parse_oci_content(&content, layout_root.as_deref())
    }
}

/// Parses the JSON content of an OCI `index.json` or Docker `manifest.json`.
///
/// `layout_root` is the directory that contains the OCI layout (the parent of
/// `index.json`); it is used to follow descriptors into `blobs/sha256/<hex>`.
/// When `None`, descriptor blobs cannot be resolved and the parser falls back
/// to descriptor digests.
///
/// Returns an empty vector when the content is valid JSON but not an OCI image
/// index or `docker save` manifest, so the parser does not claim unrelated
/// `index.json` / `manifest.json` files.
pub(crate) fn parse_oci_content(content: &str, layout_root: Option<&Path>) -> Vec<PackageData> {
    // `docker save` manifest.json is a JSON array; OCI index.json is an object.
    let trimmed = content.trim_start();
    if trimmed.starts_with('[') {
        return parse_docker_save_manifest(content);
    }

    parse_oci_image_index(content, layout_root)
}

fn parse_oci_image_index(content: &str, layout_root: Option<&Path>) -> Vec<PackageData> {
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

    let mut packages = Vec::new();
    collect_index_packages(index, layout_root, &Identity::default(), 0, &mut packages);
    packages
}

/// Image identity (name / tag / repository_url / full ref) that a parent index
/// descriptor contributes to its children. In a buildx-style layout the
/// `io.containerd.image.name` annotation lives only on the top-level
/// descriptor that points at the nested index; the leaf per-platform manifests
/// carry no name annotation, so the identity must be inherited downward.
#[derive(Default, Clone)]
struct Identity {
    name: Option<String>,
    tag: Option<String>,
    repository_url: Option<String>,
    image_ref: Option<String>,
}

/// Walks an image index, following descriptors into blob manifests to resolve
/// per-platform config digests, recursing through nested image indexes up to
/// [`MAX_INDEX_DEPTH`]. `inherited` carries identity contributed by ancestor
/// descriptors.
fn collect_index_packages(
    index: OciImageIndex,
    layout_root: Option<&Path>,
    inherited: &Identity,
    depth: usize,
    packages: &mut Vec<PackageData>,
) {
    let manifest_limit = capped_iteration_limit(index.manifests.len(), "OCI image index manifests");
    for descriptor in index.manifests.into_iter().take(manifest_limit) {
        if packages.len() >= MAX_ITERATION_COUNT {
            break;
        }

        // buildx emits attestation manifests (SBOM/provenance) as extra
        // descriptors with platform `unknown/unknown`; they are not container
        // images and would otherwise yield bogus `arch=unknown` packages.
        if descriptor_is_attestation(&descriptor) {
            continue;
        }

        // A descriptor's own annotations override the inherited identity.
        let resolved = resolve_identity(&descriptor, inherited);

        // A descriptor may itself reference a nested image index / manifest
        // list. Follow it (bounded) so multi-arch images expressed as nested
        // indexes still resolve per-platform, passing the resolved identity
        // down to the leaves.
        if depth < MAX_INDEX_DEPTH
            && descriptor_is_index(&descriptor)
            && let Some(nested) = read_index_blob(layout_root, descriptor.digest.as_deref())
        {
            collect_index_packages(nested, layout_root, &resolved, depth + 1, packages);
            continue;
        }

        if let Some(package) = package_from_descriptor(&descriptor, &resolved, layout_root) {
            packages.push(package);
        }
    }
}

fn descriptor_is_index(descriptor: &OciDescriptor) -> bool {
    matches!(
        descriptor.media_type.as_deref(),
        Some(OCI_INDEX_MEDIA_TYPE) | Some(DOCKER_MANIFEST_LIST_MEDIA_TYPE)
    )
}

const ATTESTATION_REF_TYPE_ANNOTATION: &str = "vnd.docker.reference.type";

/// A buildx attestation manifest carries `vnd.docker.reference.type` and an
/// `unknown/unknown` platform; it describes provenance/SBOM for a sibling
/// image, not an image itself.
fn descriptor_is_attestation(descriptor: &OciDescriptor) -> bool {
    descriptor
        .annotations
        .as_ref()
        .is_some_and(|annotations| annotations.contains_key(ATTESTATION_REF_TYPE_ANNOTATION))
}

/// Resolves the identity for a descriptor: its own name annotations win, and
/// any field it does not provide is inherited from the ancestor identity.
fn resolve_identity(descriptor: &OciDescriptor, inherited: &Identity) -> Identity {
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
    if identity_ref.is_none() {
        // No own identity: inherit wholesale from the ancestor descriptor.
        return inherited.clone();
    }

    let parsed = parse_image_ref(identity_ref);
    let mut tag = parsed.tag;
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

    Identity {
        name: parsed.name.or_else(|| inherited.name.clone()),
        tag: tag.or_else(|| inherited.tag.clone()),
        repository_url: parsed
            .repository_url
            .or_else(|| inherited.repository_url.clone()),
        image_ref: identity_ref
            .map(str::to_string)
            .or_else(|| inherited.image_ref.clone()),
    }
}

/// An OCI image index is identified by its media type, or by a schema version
/// of 2 paired with at least one manifest descriptor.
///
/// `index.json` is a very common filename, so the schema-version fallback (used
/// when no recognized `mediaType` is present) additionally requires that at
/// least one descriptor carries a non-empty `digest`. Every real OCI/Docker
/// descriptor has one, so this rejects unrelated `{ "schemaVersion": 2, ... }`
/// JSON that happens to expose a `manifests` array without descriptor digests.
fn is_oci_image_index(index: &OciImageIndex) -> bool {
    let media_type = index.media_type.as_deref();
    if media_type == Some(OCI_INDEX_MEDIA_TYPE)
        || media_type == Some(DOCKER_MANIFEST_LIST_MEDIA_TYPE)
    {
        return true;
    }

    index.schema_version == Some(2)
        && index.manifests.iter().any(|descriptor| {
            descriptor
                .digest
                .as_deref()
                .is_some_and(|d| !d.trim().is_empty())
        })
}

fn package_from_descriptor(
    descriptor: &OciDescriptor,
    identity: &Identity,
    layout_root: Option<&Path>,
) -> Option<PackageData> {
    let descriptor_digest = descriptor
        .digest
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    let name = identity.name.clone()?;

    // Follow the descriptor into its manifest blob to recover the config
    // digest. When the blob is absent (inherent limitation), fall back to the
    // descriptor digest.
    let (version, digest_source) = match read_config_digest(layout_root, descriptor_digest) {
        Some(config_digest) => (config_digest, "config"),
        None => (descriptor_digest.to_string(), "descriptor"),
    };

    let mut qualifiers: HashMap<String, String> = HashMap::new();
    if let Some(tag) = identity.tag.clone() {
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
    if let Some(repository_url) = identity.repository_url.as_deref() {
        qualifiers.insert(
            "repository_url".to_string(),
            truncate_field(repository_url.to_string()),
        );
    }

    let mut extra_data: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(identity_ref) = identity.image_ref.as_deref() {
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
    extra_data.insert("digest_source".to_string(), json!(digest_source));

    Some(build_package(
        Some(name),
        version,
        qualifiers,
        extra_data,
        DatasourceId::OciImageIndex,
    ))
}

/// Reads `blobs/sha256/<hex>` for `digest` and parses it as an image manifest,
/// returning its `config.digest`. Returns `None` when the layout root is
/// unknown, the blob is missing/unreadable, or the manifest has no config
/// digest.
fn read_config_digest(layout_root: Option<&Path>, digest: &str) -> Option<String> {
    let content = read_blob(layout_root, digest)?;
    let manifest: OciImageManifest = serde_json::from_str(&content).ok()?;
    let config_digest = manifest
        .config?
        .digest
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    Some(config_digest)
}

/// Reads `blobs/sha256/<hex>` for `digest` and parses it as a nested image
/// index. Returns `None` when the layout root is unknown or the blob is not a
/// readable image index.
fn read_index_blob(layout_root: Option<&Path>, digest: Option<&str>) -> Option<OciImageIndex> {
    let content = read_blob(layout_root, digest?)?;
    let index: OciImageIndex = serde_json::from_str(&content).ok()?;
    is_oci_image_index(&index).then_some(index)
}

/// Resolves and reads the blob file for a `sha256:<hex>` digest within the
/// layout. The digest must be a well-formed `sha256:` followed by exactly 64
/// hex characters, so a malformed or path-bearing digest can never escape the
/// `blobs/sha256/` directory.
fn read_blob(layout_root: Option<&Path>, digest: &str) -> Option<String> {
    let layout_root = layout_root?;
    let hex = digest.strip_prefix("sha256:")?;
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }

    let blob_path: PathBuf = layout_root.join("blobs").join("sha256").join(hex);
    read_file_to_string(&blob_path, None).ok()
}

fn parse_docker_save_manifest(content: &str) -> Vec<PackageData> {
    let entries: Vec<DockerSaveManifestEntry> = match serde_json::from_str(content) {
        Ok(entries) => entries,
        Err(error) => {
            warn!("Failed to parse docker save manifest JSON: {}", error);
            return Vec::new();
        }
    };

    let entry_limit = capped_iteration_limit(entries.len(), "docker save manifest entries");
    entries
        .into_iter()
        .take(entry_limit)
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

    let parsed = parse_image_ref(ref_name);
    let name = parsed.name.clone()?;

    let mut qualifiers: HashMap<String, String> = HashMap::new();
    if let Some(tag) = parsed.tag.clone() {
        qualifiers.insert("tag".to_string(), truncate_field(tag));
    }
    if let Some(repository_url) = parsed.repository_url.as_deref() {
        qualifiers.insert(
            "repository_url".to_string(),
            truncate_field(repository_url.to_string()),
        );
    }

    let mut extra_data: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(ref_name) = ref_name {
        extra_data.insert("image_ref_name".to_string(), json!(ref_name));
    }
    // docker save Config points straight at the image config blob.
    extra_data.insert("digest_source".to_string(), json!("config"));

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

/// The image-name components recovered from an OCI ref name / docker repo tag.
struct ImageRef {
    /// Lowercased last fragment of the repository name (the purl `name`).
    name: Option<String>,
    /// Tag that was associated with the digest, when present.
    tag: Option<String>,
    /// `repository_url` qualifier: registry host (and port) plus the full
    /// repository path, with any tag/digest stripped. Only set when the ref
    /// carries an explicit registry host so it is genuinely derivable.
    repository_url: Option<String>,
}

/// Splits an OCI ref name / docker repo tag into a lowercased image name, an
/// optional tag, and a `repository_url` qualifier. A ref of
/// `registry.example.com/library/nginx:1.27` yields name `nginx`, tag `1.27`,
/// and repository_url `registry.example.com/library/nginx`; the
/// registry/namespace prefix is preserved in `repository_url` rather than the
/// PURL name to keep the identity portable.
fn parse_image_ref(ref_name: Option<&str>) -> ImageRef {
    let Some(ref_name) = ref_name else {
        return ImageRef {
            name: None,
            tag: None,
            repository_url: None,
        };
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

    ImageRef {
        name,
        tag: tag.filter(|value| !value.trim().is_empty()),
        repository_url: repository_url_from_repository(repository),
    }
}

/// Derives a `repository_url` qualifier from the (tag-stripped) repository
/// reference, but only when the reference carries an explicit registry host.
///
/// A reference is treated as having an explicit registry only when its first
/// path segment looks like a host: it contains a `.` (domain), a `:` (port),
/// or is exactly `localhost`. Bare references like `library/nginx` or `ubuntu`
/// have an implicit Docker Hub registry that is a convention rather than a
/// stated fact, so no `repository_url` is emitted for them (honest unknown).
fn repository_url_from_repository(repository: &str) -> Option<String> {
    let repository = repository.trim();
    if repository.is_empty() {
        return None;
    }

    let first_segment = repository.split('/').next().unwrap_or(repository);
    let looks_like_registry =
        first_segment == "localhost" || first_segment.contains('.') || first_segment.contains(':');

    looks_like_registry.then(|| repository.to_string())
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

/// Builds a `pkg:oci/<name>@sha256:<digest>` PURL with `arch` / `repository_url`
/// / `tag` qualifiers, per the purl-spec `oci` type. The digest is carried in
/// the version component.
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
