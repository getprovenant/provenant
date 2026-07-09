// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(test, feature = "golden-tests"))]
mod golden_tests {
    use crate::parsers::PackageParser;
    use crate::parsers::golden_test_utils::compare_package_data_parser_only;
    use crate::parsers::vscode_extension::VscodeExtensionManifestParser;
    use std::path::{Path, PathBuf};

    fn assert_fixture_exists(path: &Path) {
        assert!(path.exists(), "missing fixture: {}", path.display());
    }

    #[test]
    fn test_golden_vscode_extension_manifest() {
        let test_file =
            PathBuf::from("testdata/vscode-extension-golden/basic/extension.vsixmanifest");
        let expected_file = PathBuf::from(
            "testdata/vscode-extension-golden/basic/extension.vsixmanifest.expected.json",
        );

        assert_fixture_exists(&test_file);
        assert_fixture_exists(&expected_file);

        let package_data = VscodeExtensionManifestParser::extract_first_package(&test_file);

        if let Err(error) = compare_package_data_parser_only(&package_data, &expected_file) {
            panic!("Golden test failed for VS Code extension manifest: {error}");
        }
    }
}
