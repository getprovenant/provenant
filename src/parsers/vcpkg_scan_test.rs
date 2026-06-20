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

    #[test]
    fn test_vcpkg_lock_scan_remains_unassembled_and_preserves_registry_locks() {
        let (files, result) = scan_and_assemble(Path::new("testdata/vcpkg/lock"));

        assert!(result.packages.is_empty());
        let lockfile = files
            .iter()
            .find(|file| file.path.ends_with("/vcpkg-lock.json"))
            .expect("vcpkg-lock.json should be scanned");
        assert!(lockfile.for_packages.is_empty());

        let package = lockfile
            .package_data
            .iter()
            .find(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::VcpkgLockJson))
            .expect("vcpkg lock package data should be present");
        assert!(package.is_private);
        let registry_locks = package
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("registry_locks"))
            .and_then(serde_json::Value::as_array)
            .expect("registry_locks should be preserved");

        assert_eq!(registry_locks.len(), 2);
    }

    #[test]
    fn test_vcpkg_configuration_scan_remains_unassembled_and_preserves_provenance() {
        let (files, result) = scan_and_assemble(Path::new("testdata/vcpkg/configuration"));

        assert!(result.packages.is_empty());
        let config = files
            .iter()
            .find(|file| file.path.ends_with("/vcpkg-configuration.json"))
            .expect("vcpkg-configuration.json should be scanned");
        assert!(config.for_packages.is_empty());

        let package = config
            .package_data
            .iter()
            .find(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::VcpkgConfigurationJson))
            .expect("vcpkg configuration package data should be present");
        assert!(package.is_private);
        let registries = package
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("registries"))
            .and_then(serde_json::Value::as_array)
            .expect("registries should be preserved");

        assert_eq!(registries.len(), 1);
    }

    #[test]
    fn test_vcpkg_colocated_manifest_and_configuration_both_surface() {
        // A `vcpkg-configuration.json` sitting next to a manifest with no embedded
        // configuration is read twice on purpose: the manifest parser ingests it as a
        // sibling into its own `extra_data["configuration"]`, and the standalone
        // configuration parser independently emits its own private package_data record.
        // Both representations must coexist without suppressing each other.
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        std::fs::write(
            temp_dir.path().join("vcpkg.json"),
            r#"{"name":"colocated-sample","version-string":"1.0.0","dependencies":["fmt"]}"#,
        )
        .expect("Failed to write vcpkg.json");
        std::fs::write(
            temp_dir.path().join("vcpkg-configuration.json"),
            r#"{"default-registry":{"kind":"git","repository":"https://github.com/microsoft/vcpkg","baseline":"3426db05b996481ca31e95fff3734cf23e0f51bc"},"registries":[{"kind":"git","repository":"https://example.com/registry","baseline":"0000000000000000000000000000000000000000","packages":["foo"]}]}"#,
        )
        .expect("Failed to write vcpkg-configuration.json");

        let (files, result) = scan_and_assemble(temp_dir.path());

        // vcpkg datasources stay unassembled, so neither file produces a top-level package.
        assert!(result.packages.is_empty());

        let manifest = files
            .iter()
            .find(|file| file.path.ends_with("/vcpkg.json"))
            .expect("vcpkg.json should be scanned");
        let manifest_package = manifest
            .package_data
            .iter()
            .find(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::VcpkgJson))
            .expect("vcpkg manifest package data should be present");
        assert!(
            manifest_package
                .extra_data
                .as_ref()
                .and_then(|extra| extra.get("configuration"))
                .is_some(),
            "manifest should ingest the sibling configuration"
        );

        let config = files
            .iter()
            .find(|file| file.path.ends_with("/vcpkg-configuration.json"))
            .expect("vcpkg-configuration.json should be scanned");
        assert!(config.for_packages.is_empty());
        let config_package = config
            .package_data
            .iter()
            .find(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::VcpkgConfigurationJson))
            .expect("standalone configuration package data should be present");
        assert!(config_package.is_private);
        let registries = config_package
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("registries"))
            .and_then(serde_json::Value::as_array)
            .expect("registries should be preserved");
        assert_eq!(registries.len(), 1);
    }
}
