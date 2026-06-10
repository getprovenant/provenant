// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::scan_test_utils::{assert_dependency_present, scan_and_assemble};
    use crate::models::DatasourceId;

    #[test]
    fn test_vcpkg_scan_remains_unassembled_and_hoists_dependencies() {
        let (files, result) = scan_and_assemble(Path::new("testdata/vcpkg/project"));

        assert!(result.packages.is_empty());
        assert_dependency_present(&result.dependencies, "pkg:generic/vcpkg/fmt", "vcpkg.json");
        assert_dependency_present(
            &result.dependencies,
            "pkg:generic/vcpkg/cpprestsdk",
            "vcpkg.json",
        );
        assert!(
            result
                .dependencies
                .iter()
                .all(|dep| dep.for_package_uid.is_none())
        );
        let manifest = files
            .iter()
            .find(|file| file.path.ends_with("/vcpkg.json"))
            .expect("vcpkg.json should be scanned");
        assert!(manifest.for_packages.is_empty());
        assert!(
            manifest
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::VcpkgJson))
        );
    }

    #[test]
    fn test_vcpkg_scan_populates_declared_license_from_extracted_statement() {
        // The vcpkg parser only sets `extracted_license_statement`; the central
        // post-extraction hook must derive the declared expression end-to-end
        // through scanner dispatch, even without an active license engine.
        let (files, _result) = scan_and_assemble(Path::new("testdata/vcpkg/port"));

        let manifest = files
            .iter()
            .find(|file| file.path.ends_with("/vcpkg.json"))
            .expect("vcpkg.json should be scanned");
        let package = manifest
            .package_data
            .iter()
            .find(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::VcpkgJson))
            .expect("vcpkg package data should be present");

        assert_eq!(package.extracted_license_statement.as_deref(), Some("MIT"));
        assert_eq!(package.declared_license_expression.as_deref(), Some("mit"));
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("MIT")
        );
        assert!(!package.license_detections.is_empty());
    }
}
