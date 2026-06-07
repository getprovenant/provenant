// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::fs;
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
    // Without a layout root the blobs cannot be read, so the version falls back
    // to the descriptor digest (digest_source = "descriptor").
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

    let packages = parse_oci_content(content, None);
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
    // The ref names an explicit registry host (docker.io), so repository_url is
    // derivable and emitted.
    assert_eq!(
        qualifiers.get("repository_url").map(String::as_str),
        Some("docker.io/library/nginx")
    );
    assert_eq!(
        first.purl.as_deref(),
        Some(
            "pkg:oci/nginx@sha256:1111111111111111111111111111111111111111111111111111111111111111?arch=amd64&repository_url=docker.io/library/nginx&tag=1.27"
        )
    );
    assert_eq!(
        first
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("digest_source")),
        Some(&serde_json::json!("descriptor"))
    );

    assert_eq!(
        packages[1].qualifiers.as_ref().and_then(|q| q.get("arch")),
        Some(&"arm64".to_string())
    );
}

#[test]
fn follows_descriptor_blob_to_config_digest_per_platform() {
    // Realistic OCI image layout: an index references two per-platform image
    // manifest blobs, each pointing at a distinct config blob. The PURL version
    // must be the *config* digest, resolved per platform.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let blobs = root.join("blobs").join("sha256");
    fs::create_dir_all(&blobs).expect("create blobs dir");

    let amd64_manifest_hex = "aaaa111111111111111111111111111111111111111111111111111111111111";
    let arm64_manifest_hex = "bbbb222222222222222222222222222222222222222222222222222222222222";
    let amd64_config_hex = "1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa";
    let arm64_config_hex = "2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb";

    fs::write(
        blobs.join(amd64_manifest_hex),
        format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{{"mediaType":"application/vnd.oci.image.config.v1+json","digest":"sha256:{amd64_config_hex}","size":1}},"layers":[]}}"#
        ),
    )
    .expect("write amd64 manifest");
    fs::write(
        blobs.join(arm64_manifest_hex),
        format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{{"mediaType":"application/vnd.oci.image.config.v1+json","digest":"sha256:{arm64_config_hex}","size":1}},"layers":[]}}"#
        ),
    )
    .expect("write arm64 manifest");

    let index = format!(
        r#"{{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [
                {{
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "digest": "sha256:{amd64_manifest_hex}",
                    "platform": {{"architecture": "amd64", "os": "linux"}},
                    "annotations": {{"io.containerd.image.name": "registry.example.com/library/nginx:1.27"}}
                }},
                {{
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "digest": "sha256:{arm64_manifest_hex}",
                    "platform": {{"architecture": "arm64", "os": "linux"}},
                    "annotations": {{"io.containerd.image.name": "registry.example.com/library/nginx:1.27"}}
                }}
            ]
        }}"#
    );

    let packages = parse_oci_content(&index, Some(root));
    assert_eq!(packages.len(), 2);

    let amd64 = &packages[0];
    assert_eq!(
        amd64.version.as_deref(),
        Some(format!("sha256:{amd64_config_hex}").as_str())
    );
    let q = amd64.qualifiers.as_ref().expect("qualifiers");
    assert_eq!(q.get("arch").map(String::as_str), Some("amd64"));
    assert_eq!(q.get("tag").map(String::as_str), Some("1.27"));
    assert_eq!(
        q.get("repository_url").map(String::as_str),
        Some("registry.example.com/library/nginx")
    );
    assert_eq!(
        amd64
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("digest_source")),
        Some(&serde_json::json!("config"))
    );
    assert_eq!(
        amd64.purl.as_deref(),
        Some(
            format!(
                "pkg:oci/nginx@sha256:{amd64_config_hex}?arch=amd64&repository_url=registry.example.com/library/nginx&tag=1.27"
            )
            .as_str()
        )
    );

    let arm64 = &packages[1];
    assert_eq!(
        arm64.version.as_deref(),
        Some(format!("sha256:{arm64_config_hex}").as_str())
    );
    assert_eq!(
        arm64.qualifiers.as_ref().and_then(|q| q.get("arch")),
        Some(&"arm64".to_string())
    );
}

#[test]
fn follows_nested_image_index_blob() {
    // A top-level index whose descriptor is itself a manifest list (nested
    // index). The parser must recurse into the nested index blob and resolve
    // the leaf image manifests' config digests.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let blobs = root.join("blobs").join("sha256");
    fs::create_dir_all(&blobs).expect("create blobs dir");

    let nested_index_hex = "cccc333333333333333333333333333333333333333333333333333333333333";
    let leaf_manifest_hex = "dddd444444444444444444444444444444444444444444444444444444444444";
    let leaf_config_hex = "3333cccc3333cccc3333cccc3333cccc3333cccc3333cccc3333cccc3333cccc";

    fs::write(
        blobs.join(leaf_manifest_hex),
        format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{{"digest":"sha256:{leaf_config_hex}"}},"layers":[]}}"#
        ),
    )
    .expect("write leaf manifest");
    fs::write(
        blobs.join(nested_index_hex),
        format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:{leaf_manifest_hex}","platform":{{"architecture":"amd64","os":"linux"}},"annotations":{{"io.containerd.image.name":"ghcr.io/acme/app:1.0"}}}}]}}"#
        ),
    )
    .expect("write nested index");

    let index = format!(
        r#"{{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [
                {{
                    "mediaType": "application/vnd.oci.image.index.v1+json",
                    "digest": "sha256:{nested_index_hex}"
                }}
            ]
        }}"#
    );

    let packages = parse_oci_content(&index, Some(root));
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name.as_deref(), Some("app"));
    assert_eq!(
        packages[0].version.as_deref(),
        Some(format!("sha256:{leaf_config_hex}").as_str())
    );
    assert_eq!(
        packages[0]
            .qualifiers
            .as_ref()
            .and_then(|q| q.get("repository_url")),
        Some(&"ghcr.io/acme/app".to_string())
    );
}

#[test]
fn buildx_layout_inherits_name_and_skips_attestations() {
    // Mirrors a real `docker buildx ... -o type=oci` layout: the image name
    // lives only on the top-level descriptor that points at a nested index, the
    // per-platform leaf manifests carry no name annotation (identity must be
    // inherited), and buildx adds `unknown/unknown` attestation manifests that
    // must not become packages.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let blobs = root.join("blobs").join("sha256");
    fs::create_dir_all(&blobs).expect("create blobs dir");

    let nested_index_hex = "1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a";
    let amd64_manifest_hex = "2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b";
    let arm64_manifest_hex = "3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c";
    let attestation_hex = "4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d";
    let amd64_config_hex = "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e";
    let arm64_config_hex = "6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f";

    fs::write(
        blobs.join(amd64_manifest_hex),
        format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{{"digest":"sha256:{amd64_config_hex}"}},"layers":[]}}"#
        ),
    )
    .expect("write amd64 manifest");
    fs::write(
        blobs.join(arm64_manifest_hex),
        format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","config":{{"digest":"sha256:{arm64_config_hex}"}},"layers":[]}}"#
        ),
    )
    .expect("write arm64 manifest");
    // The nested index leaves carry platforms but NO name annotation; one entry
    // is a buildx attestation manifest with `unknown/unknown`.
    fs::write(
        blobs.join(nested_index_hex),
        format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[
                {{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:{amd64_manifest_hex}","platform":{{"architecture":"amd64","os":"linux"}}}},
                {{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:{arm64_manifest_hex}","platform":{{"architecture":"arm64","os":"linux"}}}},
                {{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:{attestation_hex}","platform":{{"architecture":"unknown","os":"unknown"}},"annotations":{{"vnd.docker.reference.type":"attestation-manifest","vnd.docker.reference.digest":"sha256:{amd64_manifest_hex}"}}}}
            ]}}"#
        ),
    )
    .expect("write nested index");

    let index = format!(
        r#"{{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [
                {{
                    "mediaType": "application/vnd.oci.image.index.v1+json",
                    "digest": "sha256:{nested_index_hex}",
                    "annotations": {{
                        "io.containerd.image.name": "docker.io/library/myapp:multi",
                        "org.opencontainers.image.ref.name": "multi"
                    }}
                }}
            ]
        }}"#
    );

    let packages = parse_oci_content(&index, Some(root));
    // Two real platforms; the attestation manifest is excluded.
    assert_eq!(packages.len(), 2);

    for package in &packages {
        // Identity is inherited from the top-level descriptor.
        assert_eq!(package.name.as_deref(), Some("myapp"));
        let q = package.qualifiers.as_ref().expect("qualifiers");
        assert_eq!(q.get("tag").map(String::as_str), Some("multi"));
        assert_eq!(
            q.get("repository_url").map(String::as_str),
            Some("docker.io/library/myapp")
        );
        // Each platform resolves its own config digest from the blob.
        assert_eq!(
            package
                .extra_data
                .as_ref()
                .and_then(|extra| extra.get("digest_source")),
            Some(&serde_json::json!("config"))
        );
    }

    assert_eq!(
        packages[0].version.as_deref(),
        Some(format!("sha256:{amd64_config_hex}").as_str())
    );
    assert_eq!(
        packages[0].qualifiers.as_ref().and_then(|q| q.get("arch")),
        Some(&"amd64".to_string())
    );
    assert_eq!(
        packages[1].version.as_deref(),
        Some(format!("sha256:{arm64_config_hex}").as_str())
    );
    assert_eq!(
        packages[1].qualifiers.as_ref().and_then(|q| q.get("arch")),
        Some(&"arm64".to_string())
    );
}

#[test]
fn missing_blob_falls_back_to_descriptor_digest() {
    // Layout root is provided but the referenced blob is absent (sparse layout
    // / bare index): the parser falls back to the descriptor digest and records
    // digest_source = "descriptor".
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    fs::create_dir_all(root.join("blobs").join("sha256")).expect("create blobs dir");

    let descriptor_hex = "eeee555555555555555555555555555555555555555555555555555555555555";
    let index = format!(
        r#"{{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [
                {{
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "digest": "sha256:{descriptor_hex}",
                    "platform": {{"architecture": "amd64"}},
                    "annotations": {{"io.containerd.image.name": "docker.io/library/nginx:1.27"}}
                }}
            ]
        }}"#
    );

    let packages = parse_oci_content(&index, Some(root));
    assert_eq!(packages.len(), 1);
    assert_eq!(
        packages[0].version.as_deref(),
        Some(format!("sha256:{descriptor_hex}").as_str())
    );
    assert_eq!(
        packages[0]
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("digest_source")),
        Some(&serde_json::json!("descriptor"))
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

    let packages = parse_oci_content(content, None);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name.as_deref(), Some("hello-world"));
    assert_eq!(
        packages[0].qualifiers.as_ref().and_then(|q| q.get("tag")),
        Some(&"latest".to_string())
    );
    assert_eq!(
        packages[0].purl.as_deref(),
        Some(
            "pkg:oci/hello-world@sha256:0e760fdfbc48ba8041e7c6db999bb40bfca508b4be580ac75d32c4e29d202ce1?repository_url=docker.io/library/hello-world&tag=latest"
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

    let packages = parse_oci_content(content, None);
    assert_eq!(packages.len(), 1);
    let package = &packages[0];
    assert_eq!(package.datasource_id, Some(DatasourceId::OciImageManifest));
    assert_eq!(package.name.as_deref(), Some("app"));
    assert_eq!(
        package.version.as_deref(),
        Some("sha256:aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899")
    );
    let q = package.qualifiers.as_ref().expect("qualifiers");
    assert_eq!(q.get("tag").map(String::as_str), Some("2.1.0"));
    assert_eq!(
        q.get("repository_url").map(String::as_str),
        Some("registry.example.com:5000/team/app")
    );
    assert_eq!(
        package
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("digest_source")),
        Some(&serde_json::json!("config"))
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

    let packages = parse_oci_content(content, None);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].version.as_deref(), Some("sha256:deadbeef"));
    assert_eq!(packages[0].name.as_deref(), Some("ubuntu"));
    assert_eq!(
        packages[0].qualifiers.as_ref().and_then(|q| q.get("tag")),
        Some(&"22.04".to_string())
    );
    // Bare image name with no registry host: repository_url is an honest unknown.
    assert!(
        packages[0]
            .qualifiers
            .as_ref()
            .map(|q| !q.contains_key("repository_url"))
            .unwrap_or(true)
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

    let packages = parse_oci_content(content, None);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name.as_deref(), Some("myimage"));
    assert!(
        packages[0]
            .qualifiers
            .as_ref()
            .map(|q| !q.contains_key("tag"))
            .unwrap_or(true)
    );
    // localhost:5000 is an explicit registry host, so repository_url is set.
    assert_eq!(
        packages[0]
            .qualifiers
            .as_ref()
            .and_then(|q| q.get("repository_url")),
        Some(&"localhost:5000/myimage".to_string())
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

    let packages = parse_oci_content(content, None);
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
    assert!(parse_oci_content(content, None).is_empty());

    let arbitrary = r#"{"foo": "bar"}"#;
    assert!(parse_oci_content(arbitrary, None).is_empty());
}

#[test]
fn invalid_json_returns_no_packages() {
    assert!(parse_oci_content("{ not json", None).is_empty());
    assert!(parse_oci_content("[ not json", None).is_empty());
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

    let packages = parse_oci_content(content, None);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].version.as_deref(), Some("sha256:abc"));
}

#[test]
fn malformed_digest_never_escapes_blobs_dir() {
    // A path-bearing or malformed digest must not be turned into a blob path;
    // the parser falls back to the descriptor value rather than reading
    // arbitrary files.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    fs::create_dir_all(root.join("blobs").join("sha256")).expect("create blobs dir");

    let content = r#"{
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [
            {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": "sha256:../../../etc/passwd",
                "annotations": {"io.containerd.image.name": "registry.example.com/app:1"}
            }
        ]
    }"#;

    let packages = parse_oci_content(content, Some(root));
    assert_eq!(packages.len(), 1);
    assert_eq!(
        packages[0].version.as_deref(),
        Some("sha256:../../../etc/passwd")
    );
    assert_eq!(
        packages[0]
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("digest_source")),
        Some(&serde_json::json!("descriptor"))
    );
}
