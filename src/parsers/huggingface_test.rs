// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use crate::models::{DatasourceId, PackageType};
    use crate::parsers::{
        HuggingfaceConfigParser, HuggingfaceModelCardParser, HuggingfaceModelIndexParser,
        PackageParser,
    };

    fn write_temp(name: &str, content: &str) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().expect("tempdir");
        let path = temp_dir.path().join(name);
        fs::write(&path, content).expect("write fixture");
        (temp_dir, path)
    }

    #[test]
    fn model_card_is_match() {
        assert!(HuggingfaceModelCardParser::is_match(Path::new("README.md")));
        assert!(HuggingfaceModelCardParser::is_match(Path::new("readme.md")));
        assert!(!HuggingfaceModelCardParser::is_match(Path::new(
            "README.android"
        )));
        assert!(!HuggingfaceModelCardParser::is_match(Path::new(
            "config.json"
        )));
    }

    #[test]
    fn config_and_index_is_match() {
        assert!(HuggingfaceConfigParser::is_match(Path::new("config.json")));
        assert!(!HuggingfaceConfigParser::is_match(Path::new(
            "model_index.json"
        )));
        assert!(HuggingfaceModelIndexParser::is_match(Path::new(
            "model_index.json"
        )));
    }

    #[test]
    fn model_card_extracts_identity_license_and_dependencies() {
        let (_tmp, path) = write_temp(
            "README.md",
            "---\nlicense: apache-2.0\nlibrary_name: transformers\ntags:\n  - text-generation\nmodel_name: acme-ai/tiny-llama-demo\nbase_model:\n  - meta-llama/Llama-2-7b-hf\ndatasets:\n  - wikitext\n  - acme-ai/demo-corpus\n---\n\n# Tiny Llama Demo\n",
        );

        let package = HuggingfaceModelCardParser::extract_first_package(&path);

        assert_eq!(package.package_type, Some(PackageType::Huggingface));
        assert_eq!(
            package.datasource_id,
            Some(DatasourceId::HuggingfaceModelCard)
        );
        assert_eq!(package.namespace.as_deref(), Some("acme-ai"));
        assert_eq!(package.name.as_deref(), Some("tiny-llama-demo"));
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:huggingface/acme-ai/tiny-llama-demo")
        );
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("Apache-2.0")
        );
        assert_eq!(
            package.extracted_license_statement.as_deref(),
            Some("apache-2.0")
        );
        assert!(package.keywords.contains(&"text-generation".to_string()));

        let base = package
            .dependencies
            .iter()
            .find(|dep| dep.scope.as_deref() == Some("base_model"))
            .expect("base_model dependency");
        assert_eq!(
            base.purl.as_deref(),
            Some("pkg:huggingface/meta-llama/Llama-2-7b-hf")
        );

        let datasets: Vec<_> = package
            .dependencies
            .iter()
            .filter(|dep| dep.scope.as_deref() == Some("dataset"))
            .filter_map(|dep| dep.purl.as_deref())
            .collect();
        assert!(datasets.contains(&"pkg:huggingface/wikitext"));
        assert!(datasets.contains(&"pkg:huggingface/acme-ai/demo-corpus"));
    }

    #[test]
    fn model_card_without_huggingface_keys_is_ignored() {
        // Generic front matter (e.g. a static-site post) must not be claimed.
        let (_tmp, path) = write_temp(
            "README.md",
            "---\ntitle: My Blog Post\ndate: 2024-01-01\n---\n\nHello world.\n",
        );
        assert!(HuggingfaceModelCardParser::extract_packages(&path).is_empty());
    }

    #[test]
    fn model_card_without_frontmatter_is_ignored() {
        let (_tmp, path) = write_temp("README.md", "# Just a heading\n\nNo frontmatter here.\n");
        assert!(HuggingfaceModelCardParser::extract_packages(&path).is_empty());
    }

    #[test]
    fn model_card_with_unparseable_repo_id_omits_purl() {
        // Honest unknown: a single-segment model_name has no namespace, so we do
        // not guess an identity purl, but still record the license.
        let (_tmp, path) = write_temp(
            "README.md",
            "---\nlicense: mit\nlibrary_name: transformers\nmodel_name: bert-base-uncased\n---\n",
        );
        let package = HuggingfaceModelCardParser::extract_first_package(&path);
        assert_eq!(package.purl, None);
        assert_eq!(package.namespace, None);
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("MIT")
        );
    }

    #[test]
    fn config_extracts_identity_and_architecture() {
        let (_tmp, path) = write_temp(
            "config.json",
            r#"{"_name_or_path": "acme-ai/tiny-llama-demo", "architectures": ["LlamaForCausalLM"], "model_type": "llama", "transformers_version": "4.40.0"}"#,
        );
        let package = HuggingfaceConfigParser::extract_first_package(&path);
        assert_eq!(package.package_type, Some(PackageType::Huggingface));
        assert_eq!(
            package.datasource_id,
            Some(DatasourceId::HuggingfaceConfigJson)
        );
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:huggingface/acme-ai/tiny-llama-demo")
        );
        let extra = package.extra_data.expect("extra_data");
        assert_eq!(
            extra.get("model_type").and_then(|v| v.as_str()),
            Some("llama")
        );
        assert!(extra.contains_key("architectures"));
    }

    #[test]
    fn config_without_huggingface_signal_is_ignored() {
        let (_tmp, path) = write_temp("config.json", r#"{"compilerOptions": {"strict": true}}"#);
        assert!(HuggingfaceConfigParser::extract_packages(&path).is_empty());
    }

    #[test]
    fn config_with_local_name_or_path_omits_purl() {
        // A local checkpoint path is not a namespace/name repo id.
        let (_tmp, path) = write_temp(
            "config.json",
            r#"{"_name_or_path": "/tmp/checkpoints/run1", "model_type": "bert"}"#,
        );
        let package = HuggingfaceConfigParser::extract_first_package(&path);
        assert_eq!(package.purl, None);
        assert_eq!(package.namespace, None);
        assert_eq!(
            package.datasource_id,
            Some(DatasourceId::HuggingfaceConfigJson)
        );
    }

    #[test]
    fn model_index_extracts_identity_and_class() {
        let (_tmp, path) = write_temp(
            "model_index.json",
            r#"{"_class_name": "StableDiffusionPipeline", "_diffusers_version": "0.27.0", "_name_or_path": "acme-ai/tiny-diffusion-demo"}"#,
        );
        let package = HuggingfaceModelIndexParser::extract_first_package(&path);
        assert_eq!(
            package.datasource_id,
            Some(DatasourceId::HuggingfaceModelIndexJson)
        );
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:huggingface/acme-ai/tiny-diffusion-demo")
        );
        let extra = package.extra_data.expect("extra_data");
        assert_eq!(
            extra.get("_class_name").and_then(|v| v.as_str()),
            Some("StableDiffusionPipeline")
        );
    }

    #[test]
    fn malformed_json_is_ignored() {
        let (_tmp, path) = write_temp("config.json", "{not valid json");
        assert!(HuggingfaceConfigParser::extract_packages(&path).is_empty());
    }

    #[test]
    fn model_card_license_as_sequence_is_extracted() {
        // Real model cards (e.g. prajjwal1/bert-tiny) sometimes write `license`
        // as a single-element list; only `license` + `tags` + `language` keys.
        let (_tmp, path) = write_temp(
            "README.md",
            "---\nlanguage:\n  - en\nlicense:\n  - mit\ntags:\n  - BERT\n  - NLI\n---\n\nReadme body.\n",
        );
        let package = HuggingfaceModelCardParser::extract_first_package(&path);
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("MIT")
        );
        assert!(package.keywords.contains(&"BERT".to_string()));
        // No checked-in identity, so no purl is guessed.
        assert_eq!(package.purl, None);
    }

    #[test]
    fn older_config_without_model_type_is_recognized() {
        // Legacy configs (e.g. prajjwal1/bert-tiny) carry only architecture
        // hyperparameters, no `model_type`/`architectures`.
        let (_tmp, path) = write_temp(
            "config.json",
            r#"{"hidden_size": 128, "num_attention_heads": 2, "num_hidden_layers": 2, "vocab_size": 30522}"#,
        );
        let packages = HuggingfaceConfigParser::extract_packages(&path);
        assert_eq!(packages.len(), 1);
        assert_eq!(
            packages[0].datasource_id,
            Some(DatasourceId::HuggingfaceConfigJson)
        );
        assert_eq!(packages[0].purl, None);
    }
}
