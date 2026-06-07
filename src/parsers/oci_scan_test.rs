// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::scan_test_utils::scan_and_assemble;
    use crate::models::{DatasourceId, PackageType};

    #[test]
    fn test_oci_image_layout_scan_emits_oci_purl_unassembled() {
        let (files, result) = scan_and_assemble(Path::new("testdata/oci-scan/image-layout"));

        // OCI image identities are standalone; nothing is assembled into a
        // top-level package.
        assert!(result.packages.is_empty());

        let index = files
            .iter()
            .find(|file| file.path.ends_with("index.json"))
            .expect("index.json should be scanned");

        assert!(index.for_packages.is_empty());
        assert_eq!(index.package_data.len(), 1);

        let package = &index.package_data[0];
        assert_eq!(package.package_type, Some(PackageType::Oci));
        assert_eq!(package.datasource_id, Some(DatasourceId::OciImageIndex));
        assert_eq!(package.name.as_deref(), Some("alpine"));
        // The version is the per-platform image *config* digest, resolved by
        // following the manifest descriptor into blobs/sha256/<hex>, not the
        // manifest descriptor digest.
        assert_eq!(
            package.purl.as_deref(),
            Some(
                "pkg:oci/alpine@sha256:c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00?arch=amd64&repository_url=docker.io/library/alpine&tag=3.20"
            )
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
    fn test_oci_buildx_nested_index_scan_resolves_per_platform() {
        // A buildx-style OCI layout: the top-level index.json descriptor points
        // at a nested index whose per-platform leaves carry no name annotation
        // (identity is inherited) plus a buildx attestation manifest that must
        // be excluded. Each surviving platform resolves its own config digest.
        let (files, result) = scan_and_assemble(Path::new("testdata/oci-scan/buildx-layout"));

        assert!(result.packages.is_empty());

        let index = files
            .iter()
            .find(|file| file.path.ends_with("index.json"))
            .expect("index.json should be scanned");

        assert!(index.for_packages.is_empty());
        assert_eq!(index.package_data.len(), 2);

        for package in &index.package_data {
            assert_eq!(package.package_type, Some(PackageType::Oci));
            assert_eq!(package.datasource_id, Some(DatasourceId::OciImageIndex));
            assert_eq!(package.name.as_deref(), Some("demo"));
            let qualifiers = package.qualifiers.as_ref().expect("qualifiers");
            assert_eq!(qualifiers.get("tag").map(String::as_str), Some("multi"));
            assert_eq!(
                qualifiers.get("repository_url").map(String::as_str),
                Some("docker.io/library/demo")
            );
            assert_eq!(
                package
                    .extra_data
                    .as_ref()
                    .and_then(|extra| extra.get("digest_source")),
                Some(&serde_json::json!("config"))
            );
        }

        let purls: Vec<&str> = index
            .package_data
            .iter()
            .filter_map(|package| package.purl.as_deref())
            .collect();
        assert!(purls.contains(
            &"pkg:oci/demo@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?arch=amd64&repository_url=docker.io/library/demo&tag=multi"
        ));
        assert!(purls.contains(
            &"pkg:oci/demo@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb?arch=arm64&repository_url=docker.io/library/demo&tag=multi"
        ));
    }
}
