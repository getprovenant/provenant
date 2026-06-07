// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use crate::models::{DatasourceId, PackageType};
use crate::parsers::PackageParser;
use crate::parsers::oci::{OciImageLayoutParser, parse_oci_content};

#[test]
fn is_match_index_and_manifest_json() {
    assert!(OciImageLayoutParser::is_match(Path::new(
        "oci-layout/index.json"
    )));
    assert!(OciImageLayoutParser::is_match(Path::new(
        "some/dir/manifest.json"
    )));
    assert!(!OciImageLayoutParser::is_match(Path::new("package.json")));
    assert!(!OciImageLayoutParser::is_match(Path::new(
        "Package.swift.json"
    )));
}

#[test]
fn parses_oci_image_index_into_one_package_per_manifest() {
    let content = r#"{
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [
            {
                "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "platform": {"architecture": "amd64", "os": "linux"},
                "annotations": {"org.opencontainers.image.ref.name": "docker.io/library/nginx:1.27"}
            },
            {
                "digest": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                "platform": {"architecture": "arm64", "os": "linux"},
                "annotations": {"org.opencontainers.image.ref.name": "docker.io/library/nginx:1.27"}
            }
        ]
    }"#;

    let packages = parse_oci_content(content);
    assert_eq!(packages.len(), 2);

    let first = &packages[0];
    assert_eq!(first.package_type, Some(PackageType::Oci));
    assert_eq!(first.datasource_id, Some(DatasourceId::OciImageIndex));
    assert_eq!(first.name.as_deref(), Some("nginx"));
    assert_eq!(
        first.version.as_deref(),
        Some("sha256:1111111111111111111111111111111111111111111111111111111111111111")
    );
    let qualifiers = first.qualifiers.as_ref().expect("qualifiers");
    assert_eq!(qualifiers.get("tag").map(String::as_str), Some("1.27"));
    assert_eq!(qualifiers.get("arch").map(String::as_str), Some("amd64"));
    assert_eq!(
        first.purl.as_deref(),
        Some(
            "pkg:oci/nginx@sha256:1111111111111111111111111111111111111111111111111111111111111111?arch=amd64&tag=1.27"
        )
    );

    assert_eq!(
        packages[1].qualifiers.as_ref().and_then(|q| q.get("arch")),
        Some(&"arm64".to_string())
    );
}

#[test]
fn prefers_containerd_image_name_over_bare_ref_name() {
    // Mirrors a real `docker save` index.json (Docker 29): ref.name holds only
    // the tag, while the full image name lives in io.containerd.image.name.
    let content = r#"{
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [
            {
                "digest": "sha256:0e760fdfbc48ba8041e7c6db999bb40bfca508b4be580ac75d32c4e29d202ce1",
                "annotations": {
                    "io.containerd.image.name": "docker.io/library/hello-world:latest",
                    "org.opencontainers.image.ref.name": "latest"
                }
            }
        ]
    }"#;

    let packages = parse_oci_content(content);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name.as_deref(), Some("hello-world"));
    assert_eq!(
        packages[0].qualifiers.as_ref().and_then(|q| q.get("tag")),
        Some(&"latest".to_string())
    );
    assert_eq!(
        packages[0].purl.as_deref(),
        Some(
            "pkg:oci/hello-world@sha256:0e760fdfbc48ba8041e7c6db999bb40bfca508b4be580ac75d32c4e29d202ce1?tag=latest"
        )
    );
}

#[test]
fn parses_docker_save_manifest_array() {
    let content = r#"[
        {
            "Config": "blobs/sha256/aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899.json",
            "RepoTags": ["registry.example.com:5000/team/app:2.1.0"],
            "Layers": ["blobs/sha256/abc"]
        }
    ]"#;

    let packages = parse_oci_content(content);
    assert_eq!(packages.len(), 1);
    let package = &packages[0];
    assert_eq!(package.datasource_id, Some(DatasourceId::OciImageManifest));
    assert_eq!(package.name.as_deref(), Some("app"));
    assert_eq!(
        package.version.as_deref(),
        Some("sha256:aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899")
    );
    assert_eq!(
        package.qualifiers.as_ref().and_then(|q| q.get("tag")),
        Some(&"2.1.0".to_string())
    );
}

#[test]
fn docker_save_config_already_prefixed_digest() {
    let content = r#"[
        {
            "Config": "sha256:deadbeef",
            "RepoTags": ["ubuntu:22.04"]
        }
    ]"#;

    let packages = parse_oci_content(content);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].version.as_deref(), Some("sha256:deadbeef"));
    assert_eq!(packages[0].name.as_deref(), Some("ubuntu"));
    assert_eq!(
        packages[0].qualifiers.as_ref().and_then(|q| q.get("tag")),
        Some(&"22.04".to_string())
    );
}

#[test]
fn registry_port_is_not_mistaken_for_a_tag() {
    let content = r#"[
        {
            "Config": "blobs/sha256/abcdef.json",
            "RepoTags": ["localhost:5000/myimage"]
        }
    ]"#;

    let packages = parse_oci_content(content);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name.as_deref(), Some("myimage"));
    assert!(
        packages[0]
            .qualifiers
            .as_ref()
            .map(|q| !q.contains_key("tag"))
            .unwrap_or(true)
    );
}

#[test]
fn image_name_is_lowercased() {
    let content = r#"[
        {
            "Config": "sha256:abc",
            "RepoTags": ["MyOrg/MyApp:Latest"]
        }
    ]"#;

    let packages = parse_oci_content(content);
    assert_eq!(packages[0].name.as_deref(), Some("myapp"));
    assert_eq!(
        packages[0].qualifiers.as_ref().and_then(|q| q.get("tag")),
        Some(&"Latest".to_string())
    );
}

#[test]
fn non_oci_index_json_returns_no_packages() {
    // A plausible foreign index.json that is not an OCI image index.
    let content = r#"{"schemaVersion": 1, "name": "something", "entries": []}"#;
    assert!(parse_oci_content(content).is_empty());

    let arbitrary = r#"{"foo": "bar"}"#;
    assert!(parse_oci_content(arbitrary).is_empty());
}

#[test]
fn invalid_json_returns_no_packages() {
    assert!(parse_oci_content("{ not json").is_empty());
    assert!(parse_oci_content("[ not json").is_empty());
}

#[test]
fn descriptor_without_digest_is_skipped() {
    let content = r#"{
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [
            {"platform": {"architecture": "amd64"}},
            {"digest": "sha256:abc", "annotations": {"org.opencontainers.image.ref.name": "app:1"}}
        ]
    }"#;

    let packages = parse_oci_content(content);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].version.as_deref(), Some("sha256:abc"));
}
