// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::parser_warn as warn;
use packageurl::PackageUrl;
use serde_json::json;

use crate::models::{DatasourceId, Dependency, PackageData, PackageType};
use crate::parsers::utils::{MAX_ITERATION_COUNT, read_file_to_string, truncate_field};

use super::PackageParser;
use super::license_normalization::normalize_spdx_declared_license;
use super::metadata::ParserMetadata;

const PACKAGE_TYPE: PackageType = PackageType::Docker;
const OCI_LABEL_PREFIX: &str = "org.opencontainers.image.";

fn default_package_data() -> PackageData {
    PackageData {
        package_type: Some(PACKAGE_TYPE),
        primary_language: Some("Dockerfile".to_string()),
        datasource_id: Some(DatasourceId::Dockerfile),
        ..Default::default()
    }
}

pub struct DockerfileParser;

impl PackageParser for DockerfileParser {
    const PACKAGE_TYPE: PackageType = PACKAGE_TYPE;

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "Dockerfile or Containerfile OCI image metadata",
            file_patterns: &[
                "**/Dockerfile",
                "**/dockerfile",
                "**/Containerfile",
                "**/containerfile",
                "**/Containerfile.core",
                "**/containerfile.core",
            ],
            package_type: "docker",
            primary_language: "Dockerfile",
            documentation_url: Some(
                "https://github.com/opencontainers/image-spec/blob/main/annotations.md",
            ),
        }]
    }

    fn is_match(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_ascii_lowercase())
            .is_some_and(|name| {
                matches!(
                    name.as_str(),
                    "dockerfile" | "containerfile" | "containerfile.core"
                )
            })
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(content) => content,
            Err(error) => {
                warn!("Failed to read Dockerfile {:?}: {}", path, error);
                return vec![default_package_data()];
            }
        };

        vec![parse_dockerfile(&content)]
    }
}

pub(crate) fn parse_dockerfile(content: &str) -> PackageData {
    let oci_labels = extract_oci_labels(content);
    let extra_data = (!oci_labels.is_empty())
        .then(|| HashMap::from([("oci_labels".to_string(), json!(oci_labels))]));
    let extracted_license_statement = oci_labels.get("org.opencontainers.image.licenses").cloned();
    let (declared_license_expression, declared_license_expression_spdx, license_detections) =
        normalize_spdx_declared_license(extracted_license_statement.as_deref());

    let dependencies = extract_base_image_dependencies(content);

    PackageData {
        package_type: Some(PACKAGE_TYPE),
        primary_language: Some("Dockerfile".to_string()),
        datasource_id: Some(DatasourceId::Dockerfile),
        name: oci_labels
            .get("org.opencontainers.image.title")
            .map(|v| truncate_field(v.clone())),
        description: oci_labels
            .get("org.opencontainers.image.description")
            .map(|v| truncate_field(v.clone())),
        homepage_url: oci_labels
            .get("org.opencontainers.image.url")
            .map(|v| truncate_field(v.clone())),
        vcs_url: oci_labels
            .get("org.opencontainers.image.source")
            .map(|v| truncate_field(v.clone())),
        version: oci_labels
            .get("org.opencontainers.image.version")
            .map(|v| truncate_field(v.clone())),
        declared_license_expression,
        declared_license_expression_spdx,
        license_detections,
        extracted_license_statement: extracted_license_statement.map(truncate_field),
        extra_data,
        dependencies,
        ..Default::default()
    }
}

/// Parses `FROM` directives and emits each external base image as a `pkg:docker`
/// dependency. Internal multi-stage references, `scratch`, and `ARG`-templated
/// images are intentionally skipped.
fn extract_base_image_dependencies(content: &str) -> Vec<Dependency> {
    let mut stage_names: HashSet<String> = HashSet::new();
    let mut dependencies = Vec::new();
    let mut seen_purls: HashSet<String> = HashSet::new();

    for instruction in logical_lines(content) {
        let trimmed = instruction.trim_start();
        if !starts_with_instruction(trimmed, "FROM") {
            continue;
        }

        let Some((image, stage)) = parse_from_arguments(&trimmed[4..]) else {
            continue;
        };

        // Skip references to an earlier build stage rather than an external image.
        // The containment check must run before inserting the current alias so that
        // `FROM image AS image` (alias equals the untagged image name) is not
        // misclassified as an internal stage reference.
        let is_internal = stage_names.contains(&image.to_ascii_lowercase());
        if let Some(stage) = stage {
            stage_names.insert(stage.to_ascii_lowercase());
        }
        if is_internal {
            continue;
        }

        // `scratch` is the empty base, not a pullable image.
        if image.eq_ignore_ascii_case("scratch") {
            continue;
        }

        // Honest unknown: an unresolved build-arg template (`${BASE}` / `$BASE`).
        if image.contains('$') {
            continue;
        }

        let Some(purl) = build_docker_purl(image) else {
            continue;
        };

        if seen_purls.insert(purl.clone()) {
            // Only a `@sha256:…` digest is an immutable pin; a tag (or no tag)
            // is mutable, so it is not considered pinned.
            let is_pinned = image.contains('@');
            dependencies.push(Dependency {
                purl: Some(truncate_field(purl)),
                extracted_requirement: None,
                scope: None,
                is_runtime: None,
                is_optional: None,
                is_pinned: Some(is_pinned),
                is_direct: Some(true),
                resolved_package: None,
                extra_data: None,
            });
        }
    }

    dependencies
}

/// Extracts the image reference and optional `AS <stage>` name from a `FROM`
/// directive body. Returns `None` when no image token is present. Leading
/// `--platform=...` flags are ignored.
fn parse_from_arguments(rest: &str) -> Option<(&str, Option<&str>)> {
    let mut tokens = rest.split_whitespace().filter(|token| {
        // Drop build flags such as `--platform=linux/amd64`.
        !token.starts_with("--")
    });

    let image = tokens.next()?;

    let mut stage = None;
    if tokens
        .next()
        .is_some_and(|token| token.eq_ignore_ascii_case("AS"))
    {
        stage = tokens.next();
    }

    Some((image, stage))
}

/// Builds a `pkg:docker` PURL from a Docker image reference of the form
/// `[registry/]repository[:tag|@digest]`. The registry, when present, is
/// emitted as a `repository_url` qualifier; otherwise it is omitted.
fn build_docker_purl(image: &str) -> Option<String> {
    let (path, version) = split_image_version(image);
    if path.is_empty() {
        return None;
    }

    let (registry, repository) = split_registry(path);
    let (namespace, name) = split_repository(repository)?;

    let mut purl = PackageUrl::new("docker", name).ok()?;

    if let Some(namespace) = namespace {
        purl.with_namespace(namespace).ok()?;
    }

    if let Some(version) = version {
        purl.with_version(version).ok()?;
    }

    if let Some(registry) = registry {
        purl.add_qualifier("repository_url", registry).ok()?;
    }

    Some(purl.to_string())
}

/// Splits an image reference into its `[registry/]repository` path and the
/// optional tag-or-digest version. A `@` digest takes precedence; otherwise a
/// `:` after the final path segment is treated as a tag.
fn split_image_version(image: &str) -> (&str, Option<&str>) {
    if let Some((path, digest)) = image.split_once('@') {
        return (path, (!digest.is_empty()).then_some(digest));
    }

    // A colon only marks a tag when it appears in the final path segment;
    // a colon in an earlier segment denotes a registry port.
    if let Some(colon) = image.rfind(':')
        && !image[colon + 1..].contains('/')
    {
        let tag = &image[colon + 1..];
        return (&image[..colon], (!tag.is_empty()).then_some(tag));
    }

    (image, None)
}

/// Separates a leading registry host from the repository path. The first
/// segment is a registry when it contains a `.` or `:` (port) or equals
/// `localhost`, matching Docker reference resolution.
fn split_registry(path: &str) -> (Option<&str>, &str) {
    if let Some((first, rest)) = path.split_once('/')
        && (first.contains('.') || first.contains(':') || first == "localhost")
    {
        return (Some(first), rest);
    }

    (None, path)
}

/// Splits a repository path into an optional namespace and a name. The final
/// path segment is the name; any preceding segments form the namespace.
fn split_repository(repository: &str) -> Option<(Option<&str>, &str)> {
    if repository.is_empty() {
        return None;
    }

    match repository.rsplit_once('/') {
        Some((namespace, name)) if !name.is_empty() => Some((Some(namespace), name)),
        Some(_) => None,
        None => Some((None, repository)),
    }
}

fn extract_oci_labels(content: &str) -> HashMap<String, String> {
    let mut labels = HashMap::new();

    for instruction in logical_lines(content) {
        let trimmed = instruction.trim_start();
        if !starts_with_instruction(trimmed, "LABEL") {
            continue;
        }

        parse_label_instruction(trimmed[5..].trim_start(), &mut labels);
    }

    labels
}

fn logical_lines(content: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut iterations = 0usize;

    for raw_line in content.lines() {
        iterations += 1;
        if iterations > MAX_ITERATION_COUNT {
            warn!("logical_lines: exceeded MAX_ITERATION_COUNT, truncating");
            break;
        }
        let line = raw_line.trim_end();
        let trimmed = line.trim();

        if current.is_empty() && (trimmed.is_empty() || trimmed.starts_with('#')) {
            continue;
        }

        let has_continuation = ends_with_unescaped_backslash(line);
        let segment = if has_continuation {
            let mut without_backslash = line.trim_end().to_string();
            without_backslash.pop();
            without_backslash.trim().to_string()
        } else {
            trimmed.to_string()
        };

        if !segment.is_empty() {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(&segment);
        }

        if !has_continuation && !current.is_empty() {
            lines.push(current.trim().to_string());
            current.clear();
        }
    }

    if !current.is_empty() {
        lines.push(current.trim().to_string());
    }

    lines
}

fn ends_with_unescaped_backslash(line: &str) -> bool {
    let trailing = line.chars().rev().take_while(|char| *char == '\\').count();
    trailing % 2 == 1
}

fn starts_with_instruction(line: &str, instruction: &str) -> bool {
    if line.len() < instruction.len()
        || !line[..instruction.len()].eq_ignore_ascii_case(instruction)
    {
        return false;
    }

    line.chars()
        .nth(instruction.len())
        .is_none_or(|next| next.is_whitespace())
}

fn parse_label_instruction(rest: &str, labels: &mut HashMap<String, String>) {
    let tokens = tokenize_label_arguments(rest);
    if tokens.is_empty() {
        return;
    }

    if tokens.first().is_some_and(|token| token.contains('=')) {
        for (i, token) in tokens.into_iter().enumerate() {
            if i >= MAX_ITERATION_COUNT {
                warn!("parse_label_instruction: exceeded MAX_ITERATION_COUNT, truncating");
                break;
            }
            let Some((key, value)) = token.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key.starts_with(OCI_LABEL_PREFIX) {
                labels.insert(key.to_string(), truncate_field(value.trim().to_string()));
            }
        }
        return;
    }

    if let Some((key, values)) = tokens.split_first()
        && key.starts_with(OCI_LABEL_PREFIX)
    {
        labels.insert(
            key.to_string(),
            truncate_field(values.join(" ").trim().to_string()),
        );
    }
}

fn tokenize_label_arguments(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote: Option<char> = None;
    let mut iterations = 0usize;

    while let Some(ch) = chars.next() {
        iterations += 1;
        if iterations > MAX_ITERATION_COUNT {
            warn!("tokenize_label_arguments: exceeded MAX_ITERATION_COUNT, truncating");
            break;
        }
        match quote {
            Some(current_quote) => {
                if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if ch == current_quote {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                whitespace if whitespace.is_whitespace() => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(ch),
            },
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}
