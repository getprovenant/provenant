// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Rendering a license expression's SPDX form so it *mirrors* an already-combined
//! key (scancode-key) expression's `AND`/`OR`/`WITH` structure, rather than
//! independently re-combining the per-detection `license_expression_spdx` strings.
//!
//! The latter loses structure: AND-combining individual SPDX operands tightens a
//! file-level `OR` into an `AND`, and skipping detections that lack an SPDX id can
//! leak scancode-key text into the SPDX field. Deriving the SPDX form from the key
//! expression via a per-detection key<->SPDX token correspondence keeps the exact
//! operator structure and yields an absent field (never key-form text) when any
//! operand has no SPDX id.

use std::collections::HashMap;

use crate::models::LicenseDetection;

/// Renders the SPDX form of `key_expression` by mapping each license-key token through
/// the key->SPDX correspondence built from `detections`, preserving the key
/// expression's `AND`/`OR`/`WITH` structure. Returns `None` (an absent SPDX field,
/// never key-form text) when any operand lacks an SPDX id — e.g. a custom/unmapped
/// license — matching the strict rendering used by declared-license promotion.
pub(super) fn spdx_expression_mirroring_key(
    key_expression: &str,
    detections: &[LicenseDetection],
) -> Option<String> {
    let (_, token_to_spdx) = detection_token_maps(detections);
    render_license_expression(key_expression, &token_to_spdx, true)
}

/// Builds case-insensitive token maps from a set of detections: license-key token →
/// canonical key form, and license-key token → SPDX id. Each detection's
/// `license_expression` and `license_expression_spdx` share the same operator
/// structure, so their license tokens align positionally. Both the key and the SPDX
/// spelling of every token are indexed, so an expression in *either* form resolves.
pub(super) fn detection_token_maps(
    detections: &[LicenseDetection],
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut token_to_key: HashMap<String, String> = HashMap::new();
    let mut token_to_spdx: HashMap<String, String> = HashMap::new();
    for detection in detections {
        let pairs = std::iter::once((
            detection.license_expression.as_str(),
            detection.license_expression_spdx.as_str(),
        ))
        .chain(detection.matches.iter().map(|match_item| {
            (
                match_item.license_expression.as_str(),
                match_item.license_expression_spdx.as_str(),
            )
        }));
        for (key_expression, spdx_expression) in pairs {
            let keys = license_tokens(key_expression);
            let spdxes = license_tokens(spdx_expression);
            if keys.len() != spdxes.len() {
                continue;
            }
            for (key, spdx) in keys.into_iter().zip(spdxes) {
                if key.is_empty() || spdx.is_empty() {
                    continue;
                }
                token_to_key
                    .entry(key.to_ascii_lowercase())
                    .or_insert_with(|| key.to_string());
                token_to_key
                    .entry(spdx.to_ascii_lowercase())
                    .or_insert_with(|| key.to_string());
                token_to_spdx
                    .entry(key.to_ascii_lowercase())
                    .or_insert_with(|| spdx.to_string());
                token_to_spdx
                    .entry(spdx.to_ascii_lowercase())
                    .or_insert_with(|| spdx.to_string());
            }
        }
    }
    (token_to_key, token_to_spdx)
}

/// Re-renders a license expression by mapping each license-key token (case-insensitively)
/// through `token_map`, leaving `AND`/`OR`/`WITH` operators and parentheses untouched so
/// the operator structure is preserved. With `strict`, returns `None` if any license
/// token is unmapped — so an SPDX rendering that cannot fully resolve yields an absent
/// field rather than leaking key-form text. Without `strict`, an unmapped token passes
/// through unchanged.
pub(super) fn render_license_expression(
    expression: &str,
    token_map: &HashMap<String, String>,
    strict: bool,
) -> Option<String> {
    let mut rendered = String::with_capacity(expression.len());
    for token in tokenize_license_expression(expression) {
        match token {
            ExpressionToken::Operator(text) => rendered.push_str(text),
            ExpressionToken::License(key) => match token_map.get(&key.to_ascii_lowercase()) {
                Some(mapped) => rendered.push_str(mapped),
                None if strict => return None,
                None => rendered.push_str(key),
            },
        }
    }
    Some(rendered)
}

enum ExpressionToken<'a> {
    /// Operators, parentheses, and whitespace — emitted verbatim.
    Operator(&'a str),
    /// A license-key token to be mapped through a token map.
    License(&'a str),
}

/// Splits a license expression into license-key tokens and the operator/punctuation
/// runs between them, preserving every character so the input can be reconstructed.
fn tokenize_license_expression(expression: &str) -> Vec<ExpressionToken<'_>> {
    let is_license_char = |c: char| c.is_alphanumeric() || matches!(c, '-' | '.' | '_' | '+' | ':');
    let mut tokens = Vec::new();
    let mut rest = expression;
    while !rest.is_empty() {
        let boundary = rest.find(is_license_char).unwrap_or(rest.len());
        if boundary > 0 {
            tokens.push(ExpressionToken::Operator(&rest[..boundary]));
            rest = &rest[boundary..];
            continue;
        }
        let end = rest
            .find(|c: char| !is_license_char(c))
            .unwrap_or(rest.len());
        let word = &rest[..end];
        if is_expression_operator(word) {
            tokens.push(ExpressionToken::Operator(word));
        } else {
            tokens.push(ExpressionToken::License(word));
        }
        rest = &rest[end..];
    }
    tokens
}

fn is_expression_operator(word: &str) -> bool {
    matches!(word.to_ascii_uppercase().as_str(), "AND" | "OR" | "WITH")
}

/// The license-key tokens of an expression in order, dropping operators and punctuation.
fn license_tokens(expression: &str) -> Vec<&str> {
    tokenize_license_expression(expression)
        .into_iter()
        .filter_map(|token| match token {
            ExpressionToken::License(license) => Some(license),
            ExpressionToken::Operator(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detection(key: &str, spdx: &str) -> LicenseDetection {
        LicenseDetection {
            license_expression: key.to_string(),
            license_expression_spdx: spdx.to_string(),
            matches: Vec::new(),
            detection_log: Vec::new(),
            identifier: String::new(),
        }
    }

    #[test]
    fn mirrors_or_choice_without_collapsing_to_and() {
        let detections = vec![detection("mit OR apache-2.0", "MIT OR Apache-2.0")];
        assert_eq!(
            spdx_expression_mirroring_key("mit OR apache-2.0", &detections).as_deref(),
            Some("MIT OR Apache-2.0"),
        );
    }

    #[test]
    fn mirrors_key_structure_when_combined_from_separate_detections() {
        let detections = vec![
            detection("mit OR apache-2.0", "MIT OR Apache-2.0"),
            detection("bsd-new", "BSD-3-Clause"),
        ];
        assert_eq!(
            spdx_expression_mirroring_key("(mit OR apache-2.0) AND bsd-new", &detections)
                .as_deref(),
            Some("(MIT OR Apache-2.0) AND BSD-3-Clause"),
        );
    }

    #[test]
    fn yields_absent_field_rather_than_leaking_key_form_text() {
        // `proprietary-license` is a scancode key with no SPDX id (its detection carries
        // an empty SPDX form). Strict rendering must abstain — never emit the key token
        // into the SPDX field.
        let detections = vec![
            detection("mit", "MIT"),
            detection("proprietary-license", ""),
        ];
        assert_eq!(
            spdx_expression_mirroring_key("mit AND proprietary-license", &detections),
            None,
        );
    }

    #[test]
    fn resolves_a_key_already_spelled_in_spdx_form() {
        // The `detected_license_expression` fallback can hand this helper a key argument
        // that is itself SPDX-spelled; the token map indexes both spellings so it resolves.
        let detections = vec![detection("bsl-1.1 AND mpl-2.0", "BUSL-1.1 AND MPL-2.0")];
        assert_eq!(
            spdx_expression_mirroring_key("BUSL-1.1 AND MPL-2.0", &detections).as_deref(),
            Some("BUSL-1.1 AND MPL-2.0"),
        );
    }
}
