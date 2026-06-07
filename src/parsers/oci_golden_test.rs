// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(test, feature = "golden-tests"))]
mod golden_tests {
    use crate::parsers::PackageParser;
    use crate::parsers::golden_test_utils::compare_package_data_parser_only;
    use crate::parsers::oci::OciImageLayoutParser;
    use std::path::PathBuf;

    #[test]
    fn test_golden_oci_image_index() {
        let test_file = PathBuf::from("testdata/oci-golden/image-index/index.json");
        let expected_file =
            PathBuf::from("testdata/oci-golden/image-index/index.json.expected.json");

        let package_data = OciImageLayoutParser::extract_first_package(&test_file);

        match compare_package_data_parser_only(&package_data, &expected_file) {
            Ok(_) => (),
            Err(error) => panic!("Golden test failed: {}", error),
        }
    }

    #[test]
    fn test_golden_docker_save_manifest() {
        let test_file = PathBuf::from("testdata/oci-golden/docker-save/manifest.json");
        let expected_file =
            PathBuf::from("testdata/oci-golden/docker-save/manifest.json.expected.json");

        let package_data = OciImageLayoutParser::extract_first_package(&test_file);

        match compare_package_data_parser_only(&package_data, &expected_file) {
            Ok(_) => (),
            Err(error) => panic!("Golden test failed: {}", error),
        }
    }
}
