// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(test, feature = "golden-tests"))]
mod golden_tests {
    use crate::parsers::PackageParser;
    use crate::parsers::golden_test_utils::compare_package_data_parser_only;
    use crate::parsers::vcpkg::{VcpkgConfigurationParser, VcpkgLockParser, VcpkgManifestParser};
    use std::path::PathBuf;

    #[test]
    fn test_golden_vcpkg_project_manifest() {
        let test_file = PathBuf::from("testdata/vcpkg/project/vcpkg.json");
        let expected_file = PathBuf::from("testdata/vcpkg/golden/project-vcpkg-expected.json");

        let package_data = VcpkgManifestParser::extract_first_package(&test_file);

        match compare_package_data_parser_only(&package_data, &expected_file) {
            Ok(_) => (),
            Err(e) => panic!("Golden test failed for vcpkg project manifest: {}", e),
        }
    }

    #[test]
    fn test_golden_vcpkg_port_manifest() {
        let test_file = PathBuf::from("testdata/vcpkg/port/vcpkg.json");
        let expected_file = PathBuf::from("testdata/vcpkg/golden/port-vcpkg-expected.json");

        let package_data = VcpkgManifestParser::extract_first_package(&test_file);

        match compare_package_data_parser_only(&package_data, &expected_file) {
            Ok(_) => (),
            Err(e) => panic!("Golden test failed for vcpkg port manifest: {}", e),
        }
    }

    #[test]
    fn test_golden_vcpkg_configuration() {
        let test_file = PathBuf::from("testdata/vcpkg/configuration/vcpkg-configuration.json");
        let expected_file =
            PathBuf::from("testdata/vcpkg/golden/vcpkg-configuration-expected.json");

        let package_data = VcpkgConfigurationParser::extract_first_package(&test_file);

        match compare_package_data_parser_only(&package_data, &expected_file) {
            Ok(_) => (),
            Err(e) => panic!("Golden test failed for vcpkg configuration: {}", e),
        }
    }

    #[test]
    fn test_golden_vcpkg_lock() {
        let test_file = PathBuf::from("testdata/vcpkg/lock/vcpkg-lock.json");
        let expected_file = PathBuf::from("testdata/vcpkg/golden/vcpkg-lock-expected.json");

        let package_data = VcpkgLockParser::extract_first_package(&test_file);

        match compare_package_data_parser_only(&package_data, &expected_file) {
            Ok(_) => (),
            Err(e) => panic!("Golden test failed for vcpkg lockfile: {}", e),
        }
    }
}
