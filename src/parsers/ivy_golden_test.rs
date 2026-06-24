// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(test, feature = "golden-tests"))]
mod golden_tests {
    use crate::parsers::PackageParser;
    use crate::parsers::golden_test_utils::compare_package_data_parser_only;
    use crate::parsers::ivy::{IvyDependenciesPropertiesParser, IvyXmlParser};
    use std::path::{Path, PathBuf};

    fn assert_fixture_exists(path: &Path) {
        assert!(path.exists(), "missing fixture: {}", path.display());
    }

    #[test]
    fn test_golden_basic() {
        let test_file = PathBuf::from("testdata/ivy-golden/basic/ivy.xml");
        let expected_file = PathBuf::from("testdata/ivy-golden/basic/ivy.xml.expected");

        assert_fixture_exists(&test_file);
        assert_fixture_exists(&expected_file);

        let package_data = IvyXmlParser::extract_first_package(&test_file);

        if let Err(e) = compare_package_data_parser_only(&package_data, &expected_file) {
            panic!("Golden test failed for ivy basic: {}", e);
        }
    }

    #[test]
    fn test_golden_dependencies_properties() {
        let test_file = PathBuf::from("testdata/ivy-golden/dependencies/dependencies.properties");
        let expected_file =
            PathBuf::from("testdata/ivy-golden/dependencies/dependencies.properties.expected.json");

        assert_fixture_exists(&test_file);
        assert_fixture_exists(&expected_file);

        let package_data = IvyDependenciesPropertiesParser::extract_first_package(&test_file);

        if let Err(e) = compare_package_data_parser_only(&package_data, &expected_file) {
            panic!("Golden test failed for ivy dependencies.properties: {}", e);
        }
    }
}
