// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::scan_test_utils::{assert_dependency_present, scan_and_assemble};
    use crate::models::PackageType;

    #[test]
    fn huggingface_repo_assembles_one_merged_model_package() {
        let (_files, result) =
            scan_and_assemble(Path::new("testdata/assembly-golden/huggingface-basic"));

        let hf_packages: Vec<_> = result
            .packages
            .iter()
            .filter(|package| package.package_type == Some(PackageType::Huggingface))
            .collect();

        // The model-card README.md and config.json in one repository directory
        // describe a single model, so they merge into exactly one package.
        assert_eq!(
            hf_packages.len(),
            1,
            "expected one merged model package, got {:?}",
            hf_packages
                .iter()
                .map(|p| p.purl.clone())
                .collect::<Vec<_>>()
        );

        let model = hf_packages[0];
        assert_eq!(
            model.purl.as_deref(),
            Some("pkg:huggingface/acme-ai/sentiment-demo")
        );
        // License comes from the model card; architecture/model_type from config.
        assert_eq!(
            model.declared_license_expression_spdx.as_deref(),
            Some("MIT")
        );
        let extra = model.extra_data.as_ref().expect("merged extra_data");
        assert_eq!(
            extra.get("model_type").and_then(|value| value.as_str()),
            Some("bert"),
            "config.json model_type should be merged into the model package"
        );
        assert!(
            extra.contains_key("architectures"),
            "config.json architectures should be merged into the model package"
        );
        // Both datafiles are attributed to the one package.
        assert!(
            model
                .datasource_ids
                .iter()
                .any(|id| id.as_str() == "huggingface_model_card")
        );
        assert!(
            model
                .datasource_ids
                .iter()
                .any(|id| id.as_str() == "huggingface_config_json")
        );

        assert_dependency_present(
            &result.dependencies,
            "pkg:huggingface/bert-base-uncased",
            "README.md",
        );
        assert_dependency_present(&result.dependencies, "pkg:huggingface/imdb", "README.md");
    }

    #[test]
    fn huggingface_repo_without_identity_still_merges_into_one_package() {
        // Honest-unknown case: no _name_or_path / model_name, so no identity PURL
        // is derivable. The card and config must still merge into one package so
        // license/tags/architecture and dependencies are reported once.
        let (_files, result) = scan_and_assemble(Path::new(
            "testdata/assembly-golden/huggingface-no-identity",
        ));

        let hf_packages: Vec<_> = result
            .packages
            .iter()
            .filter(|package| package.package_type == Some(PackageType::Huggingface))
            .collect();

        assert_eq!(
            hf_packages.len(),
            1,
            "expected one merged package, got {:?}",
            hf_packages
                .iter()
                .map(|p| p.purl.clone())
                .collect::<Vec<_>>()
        );

        let model = hf_packages[0];
        assert_eq!(model.purl, None, "identity is an honest unknown");
        assert_eq!(
            model.declared_license_expression_spdx.as_deref(),
            Some("Apache-2.0")
        );
        let extra = model.extra_data.as_ref().expect("merged extra_data");
        assert_eq!(
            extra.get("model_type").and_then(|value| value.as_str()),
            Some("bert")
        );

        assert_dependency_present(
            &result.dependencies,
            "pkg:huggingface/bert-base-uncased",
            "README.md",
        );
    }
}
