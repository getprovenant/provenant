// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(test, feature = "golden-tests"))]
mod golden_tests {
    use std::path::PathBuf;

    use crate::parsers::PackageParser;
    use crate::parsers::golden_test_utils::compare_package_data_parser_only;
    use crate::parsers::{
        HuggingfaceConfigParser, HuggingfaceModelCardParser, HuggingfaceModelIndexParser,
    };

    #[test]
    fn test_golden_huggingface_model_card() {
        let test_file = PathBuf::from("testdata/huggingface-golden/model-card/README.md");
        let expected_file =
            PathBuf::from("testdata/huggingface-golden/model-card/README.md.expected.json");
        let package_data = HuggingfaceModelCardParser::extract_first_package(&test_file);

        if let Err(error) = compare_package_data_parser_only(&package_data, &expected_file) {
            panic!("Golden test failed: {}", error);
        }
    }

    #[test]
    fn test_golden_huggingface_config() {
        let test_file = PathBuf::from("testdata/huggingface-golden/config/config.json");
        let expected_file =
            PathBuf::from("testdata/huggingface-golden/config/config.json.expected.json");
        let package_data = HuggingfaceConfigParser::extract_first_package(&test_file);

        if let Err(error) = compare_package_data_parser_only(&package_data, &expected_file) {
            panic!("Golden test failed: {}", error);
        }
    }

    #[test]
    fn test_golden_huggingface_model_index() {
        let test_file = PathBuf::from("testdata/huggingface-golden/model-index/model_index.json");
        let expected_file =
            PathBuf::from("testdata/huggingface-golden/model-index/model_index.json.expected.json");
        let package_data = HuggingfaceModelIndexParser::extract_first_package(&test_file);

        if let Err(error) = compare_package_data_parser_only(&package_data, &expected_file) {
            panic!("Golden test failed: {}", error);
        }
    }
}
