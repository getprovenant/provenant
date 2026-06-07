// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use super::super::PackageParser;
    use super::super::docker::{DockerfileParser, parse_dockerfile};
    use crate::models::{DatasourceId, PackageType};
    use std::path::PathBuf;

    #[test]
    fn test_is_match_dockerfile_and_containerfile_variants() {
        assert!(DockerfileParser::is_match(&PathBuf::from("Dockerfile")));
        assert!(DockerfileParser::is_match(&PathBuf::from("dockerfile")));
        assert!(DockerfileParser::is_match(&PathBuf::from("Containerfile")));
        assert!(DockerfileParser::is_match(&PathBuf::from(
            "containerfile.core"
        )));

        assert!(!DockerfileParser::is_match(&PathBuf::from(
            "Dockerfile.dev"
        )));
        assert!(!DockerfileParser::is_match(&PathBuf::from(
            "docker-compose.yml"
        )));
        assert!(!DockerfileParser::is_match(&PathBuf::from(
            "my-Containerfile.txt"
        )));
    }

    #[test]
    fn test_parse_oci_labels_from_dockerfile() {
        let content = r#"
FROM docker.io/library/debian:bookworm-slim

LABEL org.opencontainers.image.title="Jitsi Broadcasting Infrastructure (jibri)" \
      org.opencontainers.image.description="Components for recording and/or streaming a conference." \
      org.opencontainers.image.url="https://github.com/jitsi/jibri" \
      org.opencontainers.image.source="https://github.com/jitsi/docker-jitsi-meet" \
      org.opencontainers.image.documentation="https://jitsi.github.io/handbook/" \
      org.opencontainers.image.version="stable-8960-1" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.revision="abcdef123456"
"#;

        let package = parse_dockerfile(content);

        assert_eq!(package.package_type, Some(PackageType::Docker));
        assert_eq!(package.primary_language, Some("Dockerfile".to_string()));
        assert_eq!(package.datasource_id, Some(DatasourceId::Dockerfile));
        assert_eq!(
            package.name.as_deref(),
            Some("Jitsi Broadcasting Infrastructure (jibri)")
        );
        assert_eq!(
            package.description.as_deref(),
            Some("Components for recording and/or streaming a conference.")
        );
        assert_eq!(
            package.homepage_url.as_deref(),
            Some("https://github.com/jitsi/jibri")
        );
        assert_eq!(
            package.vcs_url.as_deref(),
            Some("https://github.com/jitsi/docker-jitsi-meet")
        );
        assert_eq!(package.version.as_deref(), Some("stable-8960-1"));
        assert_eq!(
            package.extracted_license_statement.as_deref(),
            Some("Apache-2.0")
        );
        assert_eq!(
            package.declared_license_expression.as_deref(),
            Some("apache-2.0")
        );
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("Apache-2.0")
        );
        assert_eq!(package.license_detections.len(), 1);
        assert_eq!(
            package.license_detections[0].license_expression_spdx,
            "Apache-2.0"
        );

        let oci_labels = package
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("oci_labels"))
            .and_then(|value| value.as_object())
            .expect("oci_labels should be collected");

        assert_eq!(
            oci_labels
                .get("org.opencontainers.image.documentation")
                .and_then(|value| value.as_str()),
            Some("https://jitsi.github.io/handbook/")
        );
        assert_eq!(
            oci_labels
                .get("org.opencontainers.image.revision")
                .and_then(|value| value.as_str()),
            Some("abcdef123456")
        );
    }

    #[test]
    fn test_parse_old_style_label_value() {
        let package =
            parse_dockerfile("LABEL org.opencontainers.image.title \"Example Container\"\n");

        assert_eq!(package.name.as_deref(), Some("Example Container"));
    }

    #[test]
    fn test_parse_old_style_label_value_with_equals_sign() {
        let package = parse_dockerfile(
            "LABEL org.opencontainers.image.description \"mode=a=b compatibility\"\n",
        );

        assert_eq!(
            package.description.as_deref(),
            Some("mode=a=b compatibility")
        );
    }

    #[test]
    fn test_parse_repeated_labels_override_previous_values() {
        let package = parse_dockerfile(
            "LABEL org.opencontainers.image.title=\"First\"\nLABEL org.opencontainers.image.title=\"Second\"\n",
        );

        assert_eq!(package.name.as_deref(), Some("Second"));
    }

    #[test]
    fn test_parse_dockerfile_without_oci_labels_still_returns_package_data() {
        let package = parse_dockerfile("FROM scratch\nRUN echo hello\n");

        assert_eq!(package.package_type, Some(PackageType::Docker));
        assert_eq!(package.primary_language, Some("Dockerfile".to_string()));
        assert_eq!(package.datasource_id, Some(DatasourceId::Dockerfile));
        assert!(package.extra_data.is_none());
    }

    fn purls(content: &str) -> Vec<String> {
        parse_dockerfile(content)
            .dependencies
            .iter()
            .filter_map(|dep| dep.purl.clone())
            .collect()
    }

    #[test]
    fn test_from_tag_emits_docker_purl() {
        assert_eq!(
            purls("FROM python:3.12-slim\n"),
            vec!["pkg:docker/python@3.12-slim".to_string()]
        );
    }

    #[test]
    fn test_from_registry_namespace_and_digest() {
        assert_eq!(
            purls("FROM ghcr.io/org/img@sha256:abc123\n"),
            vec!["pkg:docker/org/img@sha256:abc123?repository_url=ghcr.io".to_string()]
        );
    }

    #[test]
    fn test_from_registry_with_tag() {
        assert_eq!(
            purls("FROM quay.io/pulp/pulp-base:latest\n"),
            vec!["pkg:docker/pulp/pulp-base@latest?repository_url=quay.io".to_string()]
        );
    }

    #[test]
    fn test_from_docker_io_library_namespace() {
        assert_eq!(
            purls("FROM docker.io/library/debian:bookworm-slim\n"),
            vec!["pkg:docker/library/debian@bookworm-slim?repository_url=docker.io".to_string()]
        );
    }

    #[test]
    fn test_from_registry_with_port() {
        assert_eq!(
            purls("FROM registry.local:5000/team/app:1.0\n"),
            vec!["pkg:docker/team/app@1.0?repository_url=registry.local:5000".to_string()]
        );
    }

    #[test]
    fn test_from_without_tag_has_no_version() {
        assert_eq!(
            purls("FROM ubuntu\n"),
            vec!["pkg:docker/ubuntu".to_string()]
        );
    }

    #[test]
    fn test_is_pinned_only_for_digest_refs() {
        // A `@sha256:…` digest is an immutable pin.
        let digest = parse_dockerfile("FROM ghcr.io/org/img@sha256:abc123\n");
        assert_eq!(digest.dependencies[0].is_pinned, Some(true));
        // A tag is mutable, so not pinned.
        let tag = parse_dockerfile("FROM python:3.12-slim\n");
        assert_eq!(tag.dependencies[0].is_pinned, Some(false));
        // No tag is also not pinned.
        let untagged = parse_dockerfile("FROM ubuntu\n");
        assert_eq!(untagged.dependencies[0].is_pinned, Some(false));
    }

    #[test]
    fn test_scratch_emits_no_purl() {
        assert!(purls("FROM scratch\nRUN echo hi\n").is_empty());
    }

    #[test]
    fn test_arg_templated_image_is_skipped() {
        assert!(purls("ARG BASE=python:3.12\nFROM ${BASE}\n").is_empty());
        assert!(purls("FROM $BASE\n").is_empty());
    }

    #[test]
    fn test_platform_flag_is_ignored() {
        assert_eq!(
            purls("FROM --platform=linux/amd64 alpine:3.20\n"),
            vec!["pkg:docker/alpine@3.20".to_string()]
        );
    }

    #[test]
    fn test_multistage_skips_internal_stage_reference() {
        let content = "\
FROM golang:1.22 AS build
RUN go build
FROM build
COPY --from=build /app /app
";
        assert_eq!(purls(content), vec!["pkg:docker/golang@1.22".to_string()]);
    }

    #[test]
    fn test_multistage_external_bases_both_emitted() {
        let content = "\
FROM node:20 AS frontend
FROM nginx:1.27-alpine
";
        assert_eq!(
            purls(content),
            vec![
                "pkg:docker/node@20".to_string(),
                "pkg:docker/nginx@1.27-alpine".to_string(),
            ]
        );
    }

    #[test]
    fn test_duplicate_base_images_are_deduplicated() {
        let content = "FROM alpine:3.20\nFROM alpine:3.20\n";
        assert_eq!(purls(content), vec!["pkg:docker/alpine@3.20".to_string()]);
    }

    #[test]
    fn test_stage_name_matching_is_case_insensitive() {
        let content = "FROM golang:1.22 AS Build\nFROM build\n";
        assert_eq!(purls(content), vec!["pkg:docker/golang@1.22".to_string()]);
    }

    #[test]
    fn test_base_image_dependency_is_direct() {
        let package = parse_dockerfile("FROM python:3.12-slim\n");
        assert_eq!(package.dependencies.len(), 1);
        assert_eq!(package.dependencies[0].is_direct, Some(true));
        assert_eq!(package.dependencies[0].extracted_requirement, None);
    }

    #[test]
    fn test_stage_alias_same_as_image_name_emits_purl() {
        // `FROM node AS node` -- the alias exactly equals the untagged image name.
        // The image is external, so a PURL must be emitted. A previous bug inserted
        // the alias before the internal-reference check, silently dropping this case.
        assert_eq!(
            purls(
                "FROM node AS node
"
            ),
            vec!["pkg:docker/node".to_string()]
        );
    }
}
