// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::scan_test_utils::{assert_dependency_present, scan_and_assemble};
    use crate::models::PackageType;

    #[test]
    fn huggingface_repo_emits_model_card_and_config_packages() {
        let (_files, result) =
            scan_and_assemble(Path::new("testdata/assembly-golden/huggingface-basic"));

        let hf_packages: Vec<_> = result
            .packages
            .iter()
            .filter(|package| package.package_type == Some(PackageType::Huggingface))
            .collect();

        // README.md model card and config.json each emit a standalone package
        // (they are intentionally not merged into one logical model package).
        assert_eq!(
            hf_packages.len(),
            2,
            "expected model-card and config packages, got {:?}",
            hf_packages
                .iter()
                .map(|p| p.purl.clone())
                .collect::<Vec<_>>()
        );

        let model_card = hf_packages
            .iter()
            .find(|package| {
                package
                    .datasource_ids
                    .iter()
                    .any(|id| id.as_str() == "huggingface_model_card")
            })
            .expect("model-card package");
        assert_eq!(
            model_card.purl.as_deref(),
            Some("pkg:huggingface/acme-ai/sentiment-demo")
        );
        assert_eq!(
            model_card.declared_license_expression_spdx.as_deref(),
            Some("MIT")
        );

        assert_dependency_present(
            &result.dependencies,
            "pkg:huggingface/bert-base-uncased",
            "README.md",
        );
        assert_dependency_present(&result.dependencies, "pkg:huggingface/imdb", "README.md");
    }
}
