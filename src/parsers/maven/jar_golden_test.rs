// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(test, feature = "golden-tests"))]
mod golden_tests {
    use crate::parsers::PackageParser;
    use crate::parsers::golden_test_utils::compare_package_data_parser_only;
    use crate::parsers::{AndroidLibraryRecognizer, JavaJarRecognizer, JavaWarRecognizer};
    use std::path::{Path, PathBuf};

    fn assert_fixture_exists(path: &Path) {
        assert!(path.exists(), "missing fixture: {}", path.display());
    }

    #[test]
    fn test_golden_jar_manifest_and_pom_properties() {
        let test_file = PathBuf::from("testdata/jvm-archive-golden/demo-lib-1.2.3.jar");
        let expected_file =
            PathBuf::from("testdata/jvm-archive-golden/demo-lib-1.2.3.jar.expected");

        assert_fixture_exists(&test_file);
        assert_fixture_exists(&expected_file);

        let package_data = JavaJarRecognizer::extract_first_package(&test_file);

        if let Err(e) = compare_package_data_parser_only(&package_data, &expected_file) {
            panic!("Golden test failed for jar: {}", e);
        }
    }

    #[test]
    fn test_golden_war_archive() {
        let test_file = PathBuf::from("testdata/jvm-archive-golden/web-app-3.4.5.war");
        let expected_file = PathBuf::from("testdata/jvm-archive-golden/web-app-3.4.5.war.expected");

        assert_fixture_exists(&test_file);
        assert_fixture_exists(&expected_file);

        let package_data = JavaWarRecognizer::extract_first_package(&test_file);

        if let Err(e) = compare_package_data_parser_only(&package_data, &expected_file) {
            panic!("Golden test failed for war: {}", e);
        }
    }

    #[test]
    fn test_golden_aar_archive() {
        let test_file = PathBuf::from("testdata/jvm-archive-golden/ui-lib-0.9.0.aar");
        let expected_file = PathBuf::from("testdata/jvm-archive-golden/ui-lib-0.9.0.aar.expected");

        assert_fixture_exists(&test_file);
        assert_fixture_exists(&expected_file);

        let package_data = AndroidLibraryRecognizer::extract_first_package(&test_file);

        if let Err(e) = compare_package_data_parser_only(&package_data, &expected_file) {
            panic!("Golden test failed for aar: {}", e);
        }
    }
}
