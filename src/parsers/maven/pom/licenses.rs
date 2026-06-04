// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use super::properties::{PropertyResolver, resolve_option};
use crate::models::LicenseDetection;
use crate::parsers::license_normalization::{
    DeclaredLicenseMatchMetadata, NormalizedDeclaredLicense, build_declared_license_data,
    combine_normalized_licenses, empty_declared_license_data, normalize_declared_license_key,
};

#[derive(Clone, Default)]
pub(super) struct MavenLicenseEntry {
    pub(super) name: Option<String>,
    pub(super) url: Option<String>,
    pub(super) comments: Option<String>,
}

pub(super) fn resolve_license_entry(
    resolver: &mut PropertyResolver,
    license: &mut MavenLicenseEntry,
) {
    resolve_option(resolver, &mut license.name);
    resolve_option(resolver, &mut license.url);
    resolve_option(resolver, &mut license.comments);
}

pub(super) fn build_license_statement(licenses: &[MavenLicenseEntry]) -> Option<String> {
    let rendered_entries: Vec<String> = licenses
        .iter()
        .filter_map(|license| {
            let mut lines = Vec::new();

            if let Some(name) = license
                .name
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
                lines.push(format!("    name: {name}"));
            }
            if let Some(url) = license
                .url
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
                lines.push(format!("    url: {url}"));
            }
            if let Some(comments) = license
                .comments
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
                lines.push(format!("    comments: {comments}"));
            }

            (!lines.is_empty()).then(|| format!("- license:\n{}", lines.join("\n")))
        })
        .collect();

    if rendered_entries.is_empty() {
        None
    } else {
        Some(format!("{}\n", rendered_entries.join("\n")))
    }
}

pub(super) fn is_license_like_comment(comment: &str) -> bool {
    let lowered = comment.to_ascii_lowercase();
    [
        "license",
        "licensed",
        "copyright",
        "spdx",
        "apache",
        "mit",
        "bsd",
        "gpl",
        "lgpl",
        "mozilla public",
        "eclipse public",
    ]
    .iter()
    .any(|marker| lowered.contains(marker))
}

pub(super) fn build_maven_declared_license_data(
    licenses: &[MavenLicenseEntry],
    matched_text: Option<&str>,
) -> (Option<String>, Option<String>, Vec<LicenseDetection>) {
    let normalized: Vec<_> = licenses
        .iter()
        .filter_map(|license| license.name.as_deref())
        .filter_map(normalize_maven_license_name)
        .collect();

    if normalized.is_empty() {
        return empty_declared_license_data();
    }

    let Some(combined) = combine_normalized_licenses(normalized, " OR ") else {
        return empty_declared_license_data();
    };

    build_declared_license_data(
        combined,
        DeclaredLicenseMatchMetadata::single_line(matched_text.unwrap_or_default()),
    )
}

fn normalize_maven_license_name(name: &str) -> Option<NormalizedDeclaredLicense> {
    match name.trim() {
        "Public Domain" | "public domain" => Some(NormalizedDeclaredLicense::new(
            "public-domain",
            "LicenseRef-scancode-public-domain",
        )),
        other => normalize_declared_license_key(other),
    }
}
