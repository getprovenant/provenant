// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Parser for Hugging Face model and dataset repository metadata.
//!
//! Hugging Face repositories are git repositories whose tracked files describe a
//! model or dataset. This parser statically extracts what those files prove:
//!
//! - **Model card `README.md`** — YAML frontmatter carrying `license`,
//!   `tags`, `language`, `base_model`, and `datasets` (`library_name` and
//!   `pipeline_tag` are used only as detection signals, not stored in output).
//! - **`config.json`** — Transformers model configuration (`model_type`,
//!   `architectures`, `_name_or_path`).
//! - **`model_index.json`** — Diffusers pipeline configuration (`_class_name`,
//!   `_name_or_path`).
//!
//! ## Identity is an honest unknown
//!
//! The purl-spec `huggingface` type is `pkg:huggingface/<namespace>/<name>@<revision>`,
//! where `<namespace>/<name>` is the repository id and `<revision>` is the git
//! commit hash. Neither is reliably stored in tracked files: the repo id lives in
//! the remote URL / `.git` config and the revision is git state, not a checked-in
//! artifact. The only checked-in identity hint is `_name_or_path`, which
//! `save_pretrained` writes as the `<namespace>/<name>` the weights were loaded
//! from — usually, but not guaranteed to be, the repository's own id.
//!
//! Following the project's honest-unknown guidance, this parser emits a
//! `pkg:huggingface/<namespace>/<name>` purl only when `_name_or_path` has the
//! unambiguous `<namespace>/<name>` shape. Otherwise it omits the purl and still
//! reports the provable facts (declared license, base-model and dataset
//! dependencies, architecture metadata). The revision qualifier is always omitted
//! because no tracked file proves it.

use std::collections::HashMap;
use std::path::Path;

use packageurl::PackageUrl;
use serde_json::Value as JsonValue;

use crate::models::{DatasourceId, Dependency, PackageData, PackageType};
use crate::parser_warn as warn;

use super::PackageParser;
use super::license_normalization::normalize_spdx_declared_license;
use super::utils::{CappedIterExt, read_file_to_string, truncate_field};

/// Parser for a Hugging Face model-card `README.md` (YAML frontmatter).
pub struct HuggingfaceModelCardParser;

/// Parser for a Hugging Face / Transformers `config.json`.
pub struct HuggingfaceConfigParser;

/// Parser for a Hugging Face / Diffusers `model_index.json`.
pub struct HuggingfaceModelIndexParser;

impl PackageParser for HuggingfaceModelCardParser {
    const PACKAGE_TYPE: PackageType = PackageType::Huggingface;

    fn is_match(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("README.md"))
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(content) => content,
            Err(error) => {
                warn!(
                    "Failed to read Hugging Face model card at {:?}: {}",
                    path, error
                );
                return Vec::new();
            }
        };

        let Some(frontmatter) = extract_frontmatter(&content) else {
            return Vec::new();
        };

        let yaml: yaml_serde::Value = match yaml_serde::from_str(frontmatter) {
            Ok(yaml) => yaml,
            Err(error) => {
                warn!(
                    "Failed to parse Hugging Face model-card frontmatter at {:?}: {}",
                    path, error
                );
                return Vec::new();
            }
        };

        // A generic README with frontmatter (e.g. a static-site post) is not a
        // model card. Require at least one Hugging Face-specific key before
        // claiming this file.
        if !looks_like_model_card(&yaml) {
            return Vec::new();
        }

        vec![parse_model_card(&yaml)]
    }

    fn metadata() -> Vec<super::metadata::ParserMetadata> {
        vec![super::metadata::ParserMetadata {
            description: "Hugging Face model-card README frontmatter",
            file_patterns: &["**/README.md"],
            package_type: "huggingface",
            primary_language: "Python",
            documentation_url: Some(
                "https://huggingface.co/docs/hub/model-cards#model-card-metadata",
            ),
        }]
    }
}

impl PackageParser for HuggingfaceConfigParser {
    const PACKAGE_TYPE: PackageType = PackageType::Huggingface;

    fn is_match(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "config.json")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let Some(json) = read_json(path, "config.json") else {
            return Vec::new();
        };

        if !looks_like_transformers_config(&json) {
            return Vec::new();
        }

        vec![parse_config(&json, DatasourceId::HuggingfaceConfigJson)]
    }

    fn metadata() -> Vec<super::metadata::ParserMetadata> {
        vec![super::metadata::ParserMetadata {
            description: "Hugging Face Transformers config.json",
            file_patterns: &["**/config.json"],
            package_type: "huggingface",
            primary_language: "Python",
            documentation_url: Some(
                "https://huggingface.co/docs/transformers/main_classes/configuration",
            ),
        }]
    }
}

impl PackageParser for HuggingfaceModelIndexParser {
    const PACKAGE_TYPE: PackageType = PackageType::Huggingface;

    fn is_match(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "model_index.json")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let Some(json) = read_json(path, "model_index.json") else {
            return Vec::new();
        };

        if !looks_like_diffusers_index(&json) {
            return Vec::new();
        }

        vec![parse_config(&json, DatasourceId::HuggingfaceModelIndexJson)]
    }

    fn metadata() -> Vec<super::metadata::ParserMetadata> {
        vec![super::metadata::ParserMetadata {
            description: "Hugging Face Diffusers model_index.json",
            file_patterns: &["**/model_index.json"],
            package_type: "huggingface",
            primary_language: "Python",
            documentation_url: Some(
                "https://huggingface.co/docs/diffusers/using-diffusers/loading#diffusion-pipeline",
            ),
        }]
    }
}

/// Reads and parses a JSON file, returning `None` on any read/parse failure.
fn read_json(path: &Path, label: &str) -> Option<JsonValue> {
    let content = match read_file_to_string(path, None) {
        Ok(content) => content,
        Err(error) => {
            warn!(
                "Failed to read Hugging Face {} at {:?}: {}",
                label, path, error
            );
            return None;
        }
    };

    match serde_json::from_str::<JsonValue>(&content) {
        Ok(json) => Some(json),
        Err(error) => {
            warn!(
                "Failed to parse Hugging Face {} at {:?}: {}",
                label, path, error
            );
            None
        }
    }
}

/// Extracts the YAML frontmatter block delimited by leading `---` fences.
///
/// Hugging Face model cards open with a `---` line, the YAML body, then a closing
/// `---` line. Returns the body between the fences, or `None` when the document
/// does not start with a frontmatter block.
fn extract_frontmatter(content: &str) -> Option<&str> {
    let trimmed = content.strip_prefix('\u{feff}').unwrap_or(content);
    let after_open = trimmed
        .strip_prefix("---\n")
        .or_else(|| trimmed.strip_prefix("---\r\n"))?;

    // Find the closing fence: a line containing only `---`.
    let mut search_start = 0;
    while let Some(rel) = after_open[search_start..].find("---") {
        let idx = search_start + rel;
        let at_line_start = idx == 0 || after_open.as_bytes()[idx - 1] == b'\n';
        let after = &after_open[idx + 3..];
        let line_ends = after.is_empty() || after.starts_with('\n') || after.starts_with('\r');
        if at_line_start && line_ends {
            return Some(&after_open[..idx]);
        }
        search_start = idx + 3;
    }
    None
}

/// Hugging Face model-card frontmatter keys that are distinctive to the
/// documented model-card metadata spec. These keys do not appear in ordinary
/// static-site / docs front matter, so any one of them is a sufficient signal
/// that a `README.md` is a Hugging Face model/dataset card.
const STRONG_MODEL_CARD_KEYS: &[&str] = &[
    "library_name",
    "pipeline_tag",
    "base_model",
    "datasets",
    "model-index",
    "license_name",
    "license_link",
    "model_name",
    "widget",
    "co2_eq_emissions",
];

/// Model-card keys that also occur in generic front matter (Jekyll/Hugo posts,
/// docs pages). Each alone is too weak to claim a Hugging Face card, but the
/// combination of several is characteristic of one, so two or more are required.
const WEAK_MODEL_CARD_KEYS: &[&str] = &[
    "license",
    "tags",
    "language",
    "metrics",
    "inference",
    "thumbnail",
];

/// Decide whether a `README.md` frontmatter mapping is a Hugging Face model
/// card. A single strong key is decisive; otherwise at least two weak keys must
/// be present together so an arbitrary post carrying just `license` or just
/// `tags` is not over-claimed, while a minimal real card
/// (e.g. `license` + `tags`) is still recognized.
fn looks_like_model_card(yaml: &yaml_serde::Value) -> bool {
    if yaml.as_mapping().is_none() {
        return false;
    }

    if STRONG_MODEL_CARD_KEYS
        .iter()
        .any(|key| yaml.get(*key).is_some())
    {
        return true;
    }

    let weak_hits = WEAK_MODEL_CARD_KEYS
        .iter()
        .filter(|key| yaml.get(**key).is_some())
        .count();
    weak_hits >= 2
}

/// Transformers `config.json` signal keys. Newer configs carry `model_type` /
/// `architectures` / `transformers_version`; older configs may carry only the
/// architecture hyperparameters, so accept those too. The combination of these
/// keys is highly specific to a model configuration.
const CONFIG_KEYS: &[&str] = &[
    "model_type",
    "architectures",
    "transformers_version",
    "hidden_size",
    "num_attention_heads",
    "num_hidden_layers",
    "vocab_size",
    "max_position_embeddings",
    "intermediate_size",
];

fn looks_like_transformers_config(json: &JsonValue) -> bool {
    if !json.is_object() {
        return false;
    }
    CONFIG_KEYS.iter().any(|key| json.get(*key).is_some())
}

fn looks_like_diffusers_index(json: &JsonValue) -> bool {
    json.get("_class_name").is_some() || json.get("_diffusers_version").is_some()
}

fn default_package(datasource_id: DatasourceId) -> PackageData {
    PackageData {
        package_type: Some(PackageType::Huggingface),
        datasource_id: Some(datasource_id),
        primary_language: Some("Python".to_string()),
        ..Default::default()
    }
}

/// Builds a `pkg:huggingface/<namespace>/<name>` purl when `repo_id` has the
/// unambiguous `<namespace>/<name>` shape. Returns `(namespace, name, purl)`.
fn identity_from_repo_id(repo_id: &str) -> Option<(String, String, String)> {
    let repo_id = repo_id.trim();
    let mut parts = repo_id.splitn(2, '/');
    let namespace = parts.next()?.trim();
    let name = parts.next()?.trim();
    if namespace.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }

    let mut purl = match PackageUrl::new(PackageType::Huggingface.as_str(), name) {
        Ok(purl) => purl,
        Err(error) => {
            warn!(
                "Failed to build Hugging Face purl for '{}': {}",
                repo_id, error
            );
            return None;
        }
    };
    if let Err(error) = purl.with_namespace(namespace) {
        warn!(
            "Failed to set namespace '{}' on Hugging Face purl: {}",
            namespace, error
        );
        return None;
    }

    Some((namespace.to_string(), name.to_string(), purl.to_string()))
}

/// Builds a bare `pkg:huggingface/<namespace>/<name>` dependency purl, or a
/// single-segment `pkg:huggingface/<name>` purl when no namespace is present.
fn dependency_purl(reference: &str) -> Option<String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }
    if let Some((_, _, purl)) = identity_from_repo_id(reference) {
        return Some(purl);
    }
    match PackageUrl::new(PackageType::Huggingface.as_str(), reference) {
        Ok(purl) => Some(purl.to_string()),
        Err(error) => {
            warn!(
                "Failed to build Hugging Face dependency purl for '{}': {}",
                reference, error
            );
            None
        }
    }
}

fn parse_model_card(yaml: &yaml_serde::Value) -> PackageData {
    let mut package = default_package(DatasourceId::HuggingfaceModelCard);

    if let Some((namespace, name, purl)) = yaml
        .get("model_name")
        .and_then(yaml_serde::Value::as_str)
        .and_then(identity_from_repo_id)
    {
        package.namespace = Some(truncate_field(namespace));
        package.name = Some(truncate_field(name));
        package.purl = Some(purl);
    }

    package.keywords = collect_string_seq(yaml.get("tags"));

    let languages = collect_string_seq(yaml.get("language"));
    if !languages.is_empty() {
        let mut extra = HashMap::new();
        extra.insert(
            "language".to_string(),
            serde_json::Value::Array(
                languages
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
        package.extra_data = Some(extra);
    }

    apply_declared_license(&mut package, model_card_license(yaml));
    package.dependencies = model_card_dependencies(yaml);

    package
}

/// Resolves the declared license from a model card. Hugging Face uses `license`
/// for SPDX-style identifiers and `license_name` for custom licenses. The
/// `license` field is occasionally written as a single-element list, so accept a
/// scalar or the first sequence entry.
fn model_card_license(yaml: &yaml_serde::Value) -> Option<String> {
    first_scalar_or_seq(yaml.get("license"))
        .or_else(|| first_scalar_or_seq(yaml.get("license_name")))
}

/// Returns a scalar string value, or the first string of a sequence value.
fn first_scalar_or_seq(value: Option<&yaml_serde::Value>) -> Option<String> {
    collect_string_seq(value).into_iter().next()
}

fn model_card_dependencies(yaml: &yaml_serde::Value) -> Vec<Dependency> {
    let mut dependencies = Vec::new();

    for base_model in collect_string_seq(yaml.get("base_model")) {
        if let Some(purl) = dependency_purl(&base_model) {
            dependencies.push(Dependency {
                purl: Some(purl),
                extracted_requirement: Some(truncate_field(base_model)),
                scope: Some("base_model".to_string()),
                is_runtime: None,
                is_optional: None,
                is_pinned: None,
                is_direct: Some(true),
                resolved_package: None,
                extra_data: None,
            });
        }
    }

    for dataset in collect_string_seq(yaml.get("datasets")) {
        if let Some(purl) = dependency_purl(&dataset) {
            dependencies.push(Dependency {
                purl: Some(purl),
                extracted_requirement: Some(truncate_field(dataset)),
                scope: Some("dataset".to_string()),
                is_runtime: None,
                is_optional: None,
                is_pinned: None,
                is_direct: Some(true),
                resolved_package: None,
                extra_data: None,
            });
        }
    }

    dependencies
}

fn parse_config(json: &JsonValue, datasource_id: DatasourceId) -> PackageData {
    let mut package = default_package(datasource_id);

    if let Some((namespace, name, purl)) = json
        .get("_name_or_path")
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .and_then(identity_from_repo_id)
    {
        package.namespace = Some(truncate_field(namespace));
        package.name = Some(truncate_field(name));
        package.purl = Some(purl);
    }

    let mut extra = HashMap::new();
    for key in [
        "model_type",
        "transformers_version",
        "_class_name",
        "_diffusers_version",
    ] {
        if let Some(value) = json.get(key).and_then(JsonValue::as_str) {
            extra.insert(
                key.to_string(),
                serde_json::Value::String(truncate_field(value.to_string())),
            );
        }
    }
    if let Some(architectures) = json.get("architectures").and_then(JsonValue::as_array) {
        let values: Vec<serde_json::Value> = architectures
            .iter()
            .capped("config.json architectures")
            .filter_map(|value| value.as_str())
            .map(|value| serde_json::Value::String(truncate_field(value.to_string())))
            .collect();
        if !values.is_empty() {
            extra.insert(
                "architectures".to_string(),
                serde_json::Value::Array(values),
            );
        }
    }
    if !extra.is_empty() {
        package.extra_data = Some(extra);
    }

    package
}

fn apply_declared_license(package: &mut PackageData, license: Option<String>) {
    let Some(license) = license else {
        return;
    };
    let license = truncate_field(license);
    if license.is_empty() {
        return;
    }
    package.extracted_license_statement = Some(license.clone());
    let (declared, declared_spdx, detections) = normalize_spdx_declared_license(Some(&license));
    package.declared_license_expression = declared;
    package.declared_license_expression_spdx = declared_spdx;
    package.license_detections = detections;
}

/// Collects a YAML value that may be a single string or a sequence of strings
/// into a `Vec<String>`, skipping non-string and empty entries.
fn collect_string_seq(value: Option<&yaml_serde::Value>) -> Vec<String> {
    match value {
        Some(yaml_serde::Value::String(single)) => {
            let single = single.trim();
            if single.is_empty() {
                Vec::new()
            } else {
                vec![truncate_field(single.to_string())]
            }
        }
        Some(seq) => seq
            .as_sequence()
            .into_iter()
            .flatten()
            .capped("model card string sequence")
            .filter_map(yaml_serde::Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| truncate_field(entry.to_string()))
            .collect(),
        None => Vec::new(),
    }
}
