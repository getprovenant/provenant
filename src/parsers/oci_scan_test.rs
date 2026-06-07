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
        assert_eq!(
            package.purl.as_deref(),
            Some(
                "pkg:oci/alpine@sha256:9b1c7c0e4f4a3a1d2b5e6f7081923a4b5c6d7e8f90a1b2c3d4e5f60718293a4b?arch=amd64&tag=3.20"
            )
        );
    }
}
