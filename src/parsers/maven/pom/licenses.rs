// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use super::properties::{PropertyResolver, resolve_option};
use crate::models::LicenseDetection;
use crate::parsers::license_normalization::{
    DeclaredLicenseMatchMetadata, NormalizedDeclaredLicense, build_declared_license_data,
    combine_normalized_licenses, detect_declared_license_name, empty_declared_license_data,
    normalize_declared_license_key,
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
    // Each `<license>` is a distinct declared license; a POM that lists several
    // means all of them apply, so they AND-combine (matching ScanCode).
    let declared: Vec<&MavenLicenseEntry> = licenses
        .iter()
        .filter(|license| {
            entry_field(&license.name).is_some() || entry_field(&license.url).is_some()
        })
        .collect();

    if declared.is_empty() {
        return empty_declared_license_data();
    }

    // Resolve every declared entry, mapping any entry that does not resolve to
    // `unknown-license-reference` rather than dropping it. Dropping an operand
    // would silently understate the declared licensing, and nulling the whole
    // expression would discard the entries that *did* resolve; an explicit
    // unknown operand keeps the result honest and complete.
    let normalized: Vec<_> = declared
        .into_iter()
        .map(normalize_maven_license_entry)
        .collect();

    let Some(combined) = combine_normalized_licenses(normalized, " AND ") else {
        return empty_declared_license_data();
    };

    build_declared_license_data(
        combined,
        DeclaredLicenseMatchMetadata::single_line(matched_text.unwrap_or_default()),
    )
}

fn entry_field(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// Resolves a single Maven `<license>` entry into a normalized declared license.
///
/// Resolution order, stopping at the first confident match:
/// 1. exact SPDX-key/identifier lookup on the `<name>`;
/// 2. free-text detection over the `<name>` alone;
/// 3. free-text detection over the combined `<name>` and `<url>`.
///
/// Step 3 recovers descriptive names that resolve only when paired with their
/// `<url>`: e.g. "GNU General Lesser Public License (LGPL) version 3.0" with
/// `http://www.gnu.org/licenses/lgpl.html` resolves to `lgpl-3.0`, where the
/// name alone matches nothing. It is only used when the name alone fails, so a
/// name that already resolves (e.g. "GNU Lesser General Public License" ->
/// `lgpl-2.1-plus`) is not muddied by a second, redundant operand the URL line
/// would otherwise contribute. The POM `<licenses>` block is trustworthy
/// declared manifest metadata, so detecting from these bounded fields is
/// declared normalization, not file-content scanning.
///
/// An entry that resolves to nothing maps to `unknown-license-reference` (the
/// existing convention for an unresolved but declared license reference) so it
/// is preserved as an explicit operand rather than silently dropped.
fn normalize_maven_license_entry(license: &MavenLicenseEntry) -> NormalizedDeclaredLicense {
    let name = entry_field(&license.name);

    if let Some(name) = name {
        match name {
            "Public Domain" | "public domain" => {
                return NormalizedDeclaredLicense::new(
                    "public-domain",
                    "LicenseRef-scancode-public-domain",
                );
            }
            other => {
                if let Some(normalized) = normalize_declared_license_key(other) {
                    return normalized;
                }
                if let Some(normalized) = detect_declared_license_name(other) {
                    return normalized;
                }
            }
        }
    }

    let detection_text = [name, entry_field(&license.url)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");

    detect_declared_license_name(&detection_text).unwrap_or_else(|| {
        NormalizedDeclaredLicense::new(
            "unknown-license-reference",
            "LicenseRef-scancode-unknown-license-reference",
        )
    })
}
