// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, LazyLock};

#[cfg(test)]
use std::cell::Cell;

use crate::parser_warn as warn;
use crate::parsers::active_parser_license_engine;
use crate::parsers::utils::{MAX_ITERATION_COUNT, RecursionGuard};

use crate::license_detection::LicenseDetectionEngine;
use crate::license_detection::expression::{
    LicenseExpression, parse_expression, simplify_expression,
};
use crate::license_detection::index::LicenseIndex;
use crate::license_detection::license_cache::LicenseCacheConfig;
use crate::models::{LicenseDetection, LineNumber, Match, MatchScore, PackageData};
use crate::utils::spdx::{
    ExpressionRelation, combine_license_expressions, combine_license_expressions_with_relation,
};

pub(crate) const PARSER_DECLARED_MATCHER: &str = "parser-declared-license";

static PARSER_LICENSE_ENGINE: LazyLock<Option<Arc<LicenseDetectionEngine>>> = LazyLock::new(|| {
    let cache_config =
        LicenseCacheConfig::new(LicenseCacheConfig::default_root_dir(), false, false);
    match LicenseDetectionEngine::from_embedded_with_cache(&cache_config) {
        Ok(engine) => Some(Arc::new(engine)),
        Err(error) => {
            warn!(
                "Failed to initialize embedded license engine for parser declared-license normalization: {}",
                error
            );
            None
        }
    }
});

#[cfg(test)]
thread_local! {
    static LAST_PARSER_LICENSE_ENGINE_PTR: Cell<usize> = const { Cell::new(0) };
}

fn parser_license_engine() -> Option<Arc<LicenseDetectionEngine>> {
    let engine = active_parser_license_engine().or_else(|| PARSER_LICENSE_ENGINE.as_ref().cloned());
    #[cfg(test)]
    if let Some(active_engine) = engine.as_ref() {
        LAST_PARSER_LICENSE_ENGINE_PTR.with(|slot| {
            slot.set(Arc::as_ptr(active_engine) as usize);
        });
    }
    engine
}

#[cfg(test)]
pub(crate) fn clear_last_parser_license_engine_ptr() {
    LAST_PARSER_LICENSE_ENGINE_PTR.with(|slot| slot.set(0));
}

#[cfg(test)]
pub(crate) fn last_parser_license_engine_ptr() -> Option<usize> {
    LAST_PARSER_LICENSE_ENGINE_PTR.with(|slot| {
        let ptr = slot.get();
        (ptr != 0).then_some(ptr)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedDeclaredLicense {
    pub(crate) declared_license_expression: String,
    pub(crate) declared_license_expression_spdx: String,
}

impl NormalizedDeclaredLicense {
    pub(crate) fn new(
        declared_license_expression: impl Into<String>,
        declared_license_expression_spdx: impl Into<String>,
    ) -> Self {
        Self {
            declared_license_expression: declared_license_expression.into(),
            declared_license_expression_spdx: declared_license_expression_spdx.into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeclaredLicenseMatchMetadata<'a> {
    pub(crate) matched_text: &'a str,
    pub(crate) start_line: LineNumber,
    pub(crate) end_line: LineNumber,
    pub(crate) referenced_filenames: Option<&'a [&'a str]>,
}

impl<'a> DeclaredLicenseMatchMetadata<'a> {
    pub(crate) fn new(matched_text: &'a str, start_line: LineNumber, end_line: LineNumber) -> Self {
        Self {
            matched_text,
            start_line,
            end_line,
            referenced_filenames: None,
        }
    }

    pub(crate) fn with_referenced_filenames(mut self, referenced_filenames: &'a [&'a str]) -> Self {
        self.referenced_filenames = Some(referenced_filenames);
        self
    }

    pub(crate) fn single_line(matched_text: &'a str) -> Self {
        Self::new(matched_text, LineNumber::ONE, LineNumber::ONE)
    }
}

pub(crate) fn empty_declared_license_data()
-> (Option<String>, Option<String>, Vec<LicenseDetection>) {
    (None, None, Vec::new())
}

pub(crate) fn normalize_spdx_declared_license(
    statement: Option<&str>,
) -> (Option<String>, Option<String>, Vec<LicenseDetection>) {
    let Some(statement) = statement.map(str::trim).filter(|value| !value.is_empty()) else {
        return empty_declared_license_data();
    };

    let Some(normalized) = normalize_spdx_expression(statement) else {
        return empty_declared_license_data();
    };

    build_declared_license_data(
        normalized,
        DeclaredLicenseMatchMetadata::single_line(statement),
    )
}

/// Runs free-text license detection over `text` and shapes the result into
/// declared-license data.
///
/// `referenced_filename` records the file the statement was read from when the
/// license came from a *referenced* file (so the detection carries that
/// provenance). Pass `None` when the statement is the manifest's own declared
/// value, since there is no separate referenced file in that case.
pub(crate) fn detect_declared_license_from_text(
    text: &str,
    referenced_filename: Option<&str>,
) -> (Option<String>, Option<String>, Vec<LicenseDetection>) {
    let text = text.trim();
    if text.is_empty() {
        return empty_declared_license_data();
    }

    let Some(engine) = parser_license_engine() else {
        return empty_declared_license_data();
    };
    let Ok(detections) = engine.detect_with_kind(text, false, false) else {
        return empty_declared_license_data();
    };
    if detections.is_empty() {
        return empty_declared_license_data();
    }

    let declared_license_expression = combine_license_expressions(
        detections
            .iter()
            .filter_map(|detection| detection.license_expression.clone()),
    );
    let declared_license_expression_spdx = combine_license_expressions(
        detections
            .iter()
            .filter_map(|detection| detection.license_expression_spdx.clone()),
    );

    match (
        declared_license_expression,
        declared_license_expression_spdx,
    ) {
        (Some(declared), Some(declared_spdx)) => {
            let references: Vec<&str> = referenced_filename.into_iter().collect();
            build_declared_license_data_from_pair(
                declared,
                declared_spdx,
                DeclaredLicenseMatchMetadata::single_line(text)
                    .with_referenced_filenames(&references),
            )
        }
        _ => empty_declared_license_data(),
    }
}

pub(crate) fn normalize_spdx_expression(statement: &str) -> Option<NormalizedDeclaredLicense> {
    let statement = statement.trim();
    if statement.is_empty() {
        return None;
    }

    let engine = parser_license_engine()?;
    let expression = parse_expression(statement).ok()?;
    let (declared_ast, declared_spdx_ast) = normalize_expression_ast(
        &expression,
        engine.index(),
        &mut RecursionGuard::depth_only(),
    )?;
    let declared_ast = simplify_expression(&declared_ast);
    let declared_spdx_ast = simplify_expression(&declared_spdx_ast);

    Some(NormalizedDeclaredLicense::new(
        render_canonical_expression(&declared_ast),
        render_canonical_spdx_expression(&declared_spdx_ast),
    ))
}

pub(crate) fn normalize_declared_license_key(key: &str) -> Option<NormalizedDeclaredLicense> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }

    let engine = parser_license_engine()?;
    normalize_license_key(key, engine.index())
}

pub(crate) fn combine_normalized_licenses(
    licenses: Vec<NormalizedDeclaredLicense>,
    separator: &str,
) -> Option<NormalizedDeclaredLicense> {
    if licenses.is_empty() {
        return None;
    }

    if licenses.len() == 1 {
        return licenses.into_iter().next();
    }

    let relation = match separator {
        " AND " => ExpressionRelation::And,
        " OR " => ExpressionRelation::Or,
        _ => {
            let declared_expression = licenses
                .iter()
                .map(|license| license.declared_license_expression.clone())
                .collect::<Vec<_>>()
                .join(separator);
            let declared_spdx_expression = licenses
                .iter()
                .map(|license| license.declared_license_expression_spdx.clone())
                .collect::<Vec<_>>()
                .join(separator);

            return Some(NormalizedDeclaredLicense::new(
                declared_expression,
                declared_spdx_expression,
            ));
        }
    };

    let declared_expression = combine_license_expressions_with_relation(
        licenses
            .iter()
            .map(|license| license.declared_license_expression.clone()),
        relation,
    )?;
    let declared_spdx_expression = combine_license_expressions_with_relation(
        licenses
            .iter()
            .map(|license| license.declared_license_expression_spdx.clone()),
        relation,
    )?;

    Some(NormalizedDeclaredLicense::new(
        declared_expression,
        declared_spdx_expression,
    ))
}

pub(crate) fn build_declared_license_data(
    normalized: NormalizedDeclaredLicense,
    metadata: DeclaredLicenseMatchMetadata<'_>,
) -> (Option<String>, Option<String>, Vec<LicenseDetection>) {
    let detection = build_declared_license_detection(&normalized, metadata);

    (
        Some(normalized.declared_license_expression),
        Some(normalized.declared_license_expression_spdx),
        vec![detection],
    )
}

pub(crate) fn build_declared_license_data_from_pair(
    declared_license_expression: impl Into<String>,
    declared_license_expression_spdx: impl Into<String>,
    metadata: DeclaredLicenseMatchMetadata<'_>,
) -> (Option<String>, Option<String>, Vec<LicenseDetection>) {
    build_declared_license_data(
        NormalizedDeclaredLicense::new(
            declared_license_expression,
            declared_license_expression_spdx,
        ),
        metadata,
    )
}

pub(crate) fn build_declared_license_detection(
    normalized: &NormalizedDeclaredLicense,
    metadata: DeclaredLicenseMatchMetadata<'_>,
) -> LicenseDetection {
    let (rule_identifier, rule_url) =
        derive_declared_rule_metadata(normalized, metadata.matched_text)
            .unwrap_or_else(|| (PARSER_DECLARED_MATCHER.to_string(), None));

    LicenseDetection {
        license_expression: normalized.declared_license_expression.clone(),
        license_expression_spdx: normalized.declared_license_expression_spdx.clone(),
        matches: vec![Match {
            license_expression: normalized.declared_license_expression.clone(),
            license_expression_spdx: normalized.declared_license_expression_spdx.clone(),
            from_file: None,
            start_line: metadata.start_line,
            end_line: metadata.end_line,
            matcher: crate::license_detection::MatcherKind::Declared,
            score: MatchScore::MAX,
            matched_length: Some(metadata.matched_text.split_whitespace().count()),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier,
            rule_url,
            matched_text: Some(metadata.matched_text.to_string()),
            referenced_filenames: metadata
                .referenced_filenames
                .map(|filenames| filenames.iter().map(|name| (*name).to_string()).collect()),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: String::new(),
    }
}

fn derive_declared_rule_metadata(
    normalized: &NormalizedDeclaredLicense,
    matched_text: &str,
) -> Option<(String, Option<String>)> {
    let matched_text = matched_text.trim();
    if matched_text.is_empty() {
        return None;
    }

    let engine = parser_license_engine()?;
    let detections = engine.detect_with_kind(matched_text, false, false).ok()?;
    let license_match = detections
        .into_iter()
        .find(|detection| detection_matches_normalized_expression(detection, normalized))?
        .matches
        .into_iter()
        .next()?;

    Some((
        license_match.rule_identifier,
        (!license_match.rule_url.is_empty()).then_some(license_match.rule_url),
    ))
}

fn detection_matches_normalized_expression(
    detection: &crate::license_detection::LicenseDetection,
    normalized: &NormalizedDeclaredLicense,
) -> bool {
    detection.license_expression.as_deref() == Some(normalized.declared_license_expression.as_str())
        || detection.license_expression_spdx.as_deref()
            == Some(normalized.declared_license_expression_spdx.as_str())
}

/// Central post-extraction population step, mirroring ScanCode's
/// `populate_license_fields` / `populate_holder_field` contract.
///
/// Runs only as a fallback: it fills `declared_license_expression` (and the
/// SPDX form plus `license_detections`) from `extracted_license_statement` when
/// a parser left them unset, and derives `holder` from `copyright` when the
/// parser left `holder` unset. It never overwrites values a parser already set,
/// and leaves fields untouched when nothing confident can be derived (for the
/// license half, the detection engine may be unavailable, in which case the
/// fields stay `None`).
pub(crate) fn populate_declared_license_and_holder(package_data: &mut PackageData) {
    populate_declared_license_fields(package_data);
    populate_holder_field(package_data);
}

fn populate_declared_license_fields(package_data: &mut PackageData) {
    if package_data.declared_license_expression.is_some() {
        return;
    }
    // A parser may leave `declared_license_expression` unset yet still emit
    // `license_detections` (for example a detection that references license
    // files beside the manifest, resolved later by
    // `finalize_package_declared_license_references`). Never clobber those.
    if !package_data.license_detections.is_empty() {
        return;
    }
    // When the parser declared license/notice file references, those files own
    // the declared expression via `finalize_package_declared_license_references`.
    // Defer to it instead of detecting over the inline statement.
    if !collect_declared_license_reference_filenames(package_data).is_empty() {
        return;
    }
    let Some(statement) = package_data
        .extracted_license_statement
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };

    // Prefer cheap SPDX-expression normalization, then fall back to running the
    // detection engine over the raw statement (e.g. "GPL (>= 2)", "Apache 2.0").
    //
    // The engine fallback is skipped for statements that look like multi-license
    // composites or structured/multi-line dumps: running free-text detection over
    // them can silently drop operands it cannot match (e.g.
    // "MIT AND Apache 2.0 AND BSD-3-Clause" loses MIT) or latch onto an arbitrary
    // fragment of a YAML/object dump, which is worse than an honest unset. When
    // SPDX parsing already failed on such a statement, leaving the field unset is
    // the safer contract.
    let (declared, declared_spdx, detections) = {
        let normalized = normalize_spdx_declared_license(Some(statement));
        if normalized.0.is_some() {
            normalized
        } else if looks_like_multi_license_composite(statement) {
            empty_declared_license_data()
        } else {
            // The statement is the manifest's own declared value, not a
            // referenced file, so it must not be recorded as a referenced
            // filename in the resulting detection.
            detect_declared_license_from_text(statement, None)
        }
    };

    if declared.is_some() {
        package_data.declared_license_expression = declared;
        package_data.declared_license_expression_spdx = declared_spdx;
        package_data.license_detections = detections;
    }
}

fn populate_holder_field(package_data: &mut PackageData) {
    if package_data
        .holder
        .as_deref()
        .is_some_and(|holder| !holder.trim().is_empty())
    {
        return;
    }
    let Some(copyright) = package_data
        .copyright
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };

    let derived = derive_holder_from_copyright(copyright);
    if !derived.trim().is_empty() {
        package_data.holder = Some(derived);
    }
}

/// Derives package holders from a copyright statement using the copyright
/// detector, mirroring ScanCode's `populate_holder_field`: detect holders, and
/// if none are found, retry with a `Copyright ` prefix on each line, then fall
/// back to the raw copyright text.
fn derive_holder_from_copyright(copyright: &str) -> String {
    let holders = detect_holders(copyright);
    if !holders.is_empty() {
        return holders.join("\n");
    }

    let prefixed = copyright
        .lines()
        .map(|line| format!("Copyright {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let holders = detect_holders(&prefixed);
    if !holders.is_empty() {
        return holders.join("\n");
    }

    copyright.to_string()
}

/// Returns true when a statement looks like it carries several licenses. Such
/// statements are unsafe to run through free-text detection as a
/// declared-license fallback because unmatched operands are silently dropped
/// (e.g. "MIT AND Apache 2.0 AND BSD-3-Clause" loses MIT, and a YAML dump of two
/// `licenses` entries detects only the last one).
///
/// A single-license multi-line dump (one license name plus its URL) is NOT
/// treated as composite: free-text detection over it still yields the correct
/// single expression.
fn looks_like_multi_license_composite(statement: &str) -> bool {
    if statement.contains('\n') {
        return counts_multiple_license_entries(statement);
    }

    let upper = format!(" {} ", statement.to_ascii_uppercase());
    if upper.contains(" AND ") || upper.contains(" OR ") {
        return true;
    }
    // Separators that commonly join distinct license names (e.g. "MIT/BSD",
    // "GPL | LGPL", "MIT, Apache-2.0").
    if statement.contains('|') || statement.contains(';') || statement.contains(',') {
        return true;
    }
    // A bare "/" separates license names, but a "/" inside a URL (e.g.
    // "http://x") must not count. Inspect each whitespace token and ignore the
    // ones that look like URLs, so "MIT/BSD see http://x" is still composite
    // while "https://example.com/LICENSE" is not.
    statement
        .split_whitespace()
        .filter(|token| !token.contains("://"))
        .any(|token| token.contains('/'))
}

/// Estimates whether a multi-line statement carries more than one license.
///
/// Parsers serialize structured `license`/`licenses` metadata to a YAML-style
/// dump, so two or more list entries (lines starting with `-`) means several
/// licenses. For bare multi-line statements with no list markers (e.g.
/// "BSD-2-Clause\nGPL-2.0-or-later"), two or more non-empty lines means the
/// same.
fn counts_multiple_license_entries(statement: &str) -> bool {
    let list_entries = statement
        .lines()
        .filter(|line| line.trim_start().starts_with('-'))
        .count();
    if list_entries > 0 {
        return list_entries > 1;
    }

    statement
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
        > 1
}

fn detect_holders(text: &str) -> Vec<String> {
    let (_copyrights, holders, _authors) = crate::copyright::detect_copyrights(text, None);
    holders
        .into_iter()
        .map(|detection| detection.holder)
        .filter(|holder| !holder.trim().is_empty())
        .collect()
}

pub(crate) fn finalize_package_declared_license_references(package_data: &mut PackageData) {
    let referenced_filenames = collect_declared_license_reference_filenames(package_data);
    if referenced_filenames.is_empty() {
        return;
    }

    if attach_referenced_filenames_to_detections(
        &mut package_data.license_detections,
        &referenced_filenames,
    ) || attach_referenced_filenames_to_detections(
        &mut package_data.other_license_detections,
        &referenced_filenames,
    ) {
        return;
    }

    let referenced_filename_slices: Vec<&str> =
        referenced_filenames.iter().map(String::as_str).collect();

    if let (Some(declared), Some(declared_spdx)) = (
        package_data.declared_license_expression.clone(),
        package_data.declared_license_expression_spdx.clone(),
    ) {
        let metadata = DeclaredLicenseMatchMetadata::single_line(
            package_data
                .extracted_license_statement
                .as_deref()
                .unwrap_or_default(),
        )
        .with_referenced_filenames(&referenced_filename_slices);
        let (_, _, detections) =
            build_declared_license_data_from_pair(declared, declared_spdx, metadata);
        package_data.license_detections = detections;
        return;
    }

    if let Some(statement) = package_data.extracted_license_statement.as_deref() {
        if let Some(normalized) = normalize_spdx_expression(statement) {
            let (_, _, detections) = build_declared_license_data(
                normalized,
                DeclaredLicenseMatchMetadata::single_line(statement)
                    .with_referenced_filenames(&referenced_filename_slices),
            );
            package_data.license_detections = detections;
            package_data.declared_license_expression = package_data
                .license_detections
                .first()
                .map(|detection| detection.license_expression.clone());
            package_data.declared_license_expression_spdx = package_data
                .license_detections
                .first()
                .map(|detection| detection.license_expression_spdx.clone());
            return;
        }

        package_data.declared_license_expression = Some("unknown-license-reference".to_string());
        package_data.declared_license_expression_spdx =
            Some("LicenseRef-scancode-unknown-license-reference".to_string());
        package_data.license_detections = vec![build_declared_license_detection(
            &NormalizedDeclaredLicense::new(
                "unknown-license-reference",
                "LicenseRef-scancode-unknown-license-reference",
            ),
            DeclaredLicenseMatchMetadata::single_line(statement)
                .with_referenced_filenames(&referenced_filename_slices),
        )];
    }
}

fn attach_referenced_filenames_to_detections(
    detections: &mut [LicenseDetection],
    referenced_filenames: &[String],
) -> bool {
    if detections.is_empty() {
        return false;
    }

    for detection in detections {
        for detection_match in &mut detection.matches {
            if detection_match.referenced_filenames.is_none() {
                detection_match.referenced_filenames = Some(referenced_filenames.to_vec());
            }
        }
    }
    true
}

fn collect_declared_license_reference_filenames(package_data: &PackageData) -> Vec<String> {
    let mut references = Vec::new();

    if let Some(extra_data) = package_data.extra_data.as_ref() {
        collect_reference_strings(extra_data.get("license_file"), &mut references);
        collect_reference_strings(extra_data.get("notice_file"), &mut references);
        collect_reference_strings(extra_data.get("license_files"), &mut references);
        collect_reference_strings(extra_data.get("notice_files"), &mut references);
    }

    let mut seen = std::collections::HashSet::new();
    references
        .into_iter()
        .filter(|reference| seen.insert(reference.clone()))
        .collect()
}

fn collect_reference_strings(value: Option<&serde_json::Value>, references: &mut Vec<String>) {
    let Some(value) = value else {
        return;
    };

    match value {
        serde_json::Value::String(value) if !value.trim().is_empty() => {
            references.push(value.trim().to_string());
        }
        serde_json::Value::String(_) => {}
        serde_json::Value::Array(values) => {
            for value in values.iter().take(MAX_ITERATION_COUNT) {
                if let Some(value) = value.as_str().filter(|value| !value.trim().is_empty()) {
                    references.push(value.trim().to_string());
                }
            }
        }
        _ => {}
    }
}

fn normalize_expression_ast(
    expression: &LicenseExpression,
    index: &LicenseIndex,
    guard: &mut RecursionGuard<()>,
) -> Option<(LicenseExpression, LicenseExpression)> {
    if guard.descend() {
        warn!("normalize_expression_ast: recursion depth exceeded limit, returning None");
        return None;
    }

    let result = match expression {
        LicenseExpression::License(key) => normalize_license_key(key, index).map(|normalized| {
            (
                LicenseExpression::License(normalized.declared_license_expression),
                LicenseExpression::License(normalized.declared_license_expression_spdx),
            )
        }),
        LicenseExpression::LicenseRef(key) => Some((
            LicenseExpression::LicenseRef(key.clone()),
            LicenseExpression::LicenseRef(key.clone()),
        )),
        LicenseExpression::And { left, right } => {
            let (left_declared, left_spdx) = normalize_expression_ast(left, index, guard)?;
            let (right_declared, right_spdx) = normalize_expression_ast(right, index, guard)?;

            Some((
                LicenseExpression::And {
                    left: Box::new(left_declared),
                    right: Box::new(right_declared),
                },
                LicenseExpression::And {
                    left: Box::new(left_spdx),
                    right: Box::new(right_spdx),
                },
            ))
        }
        LicenseExpression::Or { left, right } => {
            let (left_declared, left_spdx) = normalize_expression_ast(left, index, guard)?;
            let (right_declared, right_spdx) = normalize_expression_ast(right, index, guard)?;

            Some((
                LicenseExpression::Or {
                    left: Box::new(left_declared),
                    right: Box::new(right_declared),
                },
                LicenseExpression::Or {
                    left: Box::new(left_spdx),
                    right: Box::new(right_spdx),
                },
            ))
        }
        LicenseExpression::With { left, right } => {
            let (left_declared, left_spdx) = normalize_expression_ast(left, index, guard)?;
            let (right_declared, right_spdx) = normalize_expression_ast(right, index, guard)?;

            Some((
                LicenseExpression::With {
                    left: Box::new(left_declared),
                    right: Box::new(right_declared),
                },
                LicenseExpression::With {
                    left: Box::new(left_spdx),
                    right: Box::new(right_spdx),
                },
            ))
        }
    };
    guard.ascend();
    result
}

fn normalize_license_key(key: &str, index: &LicenseIndex) -> Option<NormalizedDeclaredLicense> {
    let normalized_key = key.trim();
    if normalized_key.is_empty() {
        return None;
    }

    if let Some(rid) = index
        .rid_by_spdx_key
        .get(&normalized_key.to_ascii_lowercase())
    {
        let rule_license_expression = index
            .rule(*rid)
            .expect("rid from spdx key lookup must be valid")
            .license_expression
            .clone();
        if rule_license_expression.contains("unknown-spdx") {
            return None;
        }

        let canonical_spdx_key = index
            .licenses_by_key
            .get(&rule_license_expression)
            .and_then(|license| license.spdx_license_key.clone())
            .unwrap_or_else(|| normalized_key.to_string());

        let declared_license_expression =
            if normalized_key.eq_ignore_ascii_case(&canonical_spdx_key) {
                normalized_key.to_ascii_lowercase()
            } else {
                rule_license_expression
            };

        let declared_license_expression_spdx = index
            .licenses_by_key
            .get(&declared_license_expression)
            .and_then(|license| license.spdx_license_key.clone())
            .unwrap_or(canonical_spdx_key);

        return Some(NormalizedDeclaredLicense::new(
            declared_license_expression,
            declared_license_expression_spdx,
        ));
    }

    let normalized_scancode_key = normalized_key.to_ascii_lowercase();
    let license = index.licenses_by_key.get(&normalized_scancode_key)?;
    let declared_license_expression = license.key.clone();
    let declared_license_expression_spdx = license
        .spdx_license_key
        .clone()
        .unwrap_or_else(|| format!("LicenseRef-scancode-{}", declared_license_expression));

    Some(NormalizedDeclaredLicense::new(
        declared_license_expression,
        declared_license_expression_spdx,
    ))
}

#[derive(Clone, Copy)]
enum BooleanOperator {
    And,
    Or,
}

fn render_canonical_expression(expression: &LicenseExpression) -> String {
    match expression {
        LicenseExpression::License(key) => key.clone(),
        LicenseExpression::LicenseRef(key) => key.clone(),
        LicenseExpression::With { left, right } => format!(
            "{} WITH {}",
            render_canonical_expression(left),
            render_canonical_expression(right)
        ),
        LicenseExpression::And { .. } => {
            render_flat_boolean_chain(expression, BooleanOperator::And)
        }
        LicenseExpression::Or { .. } => render_flat_boolean_chain(expression, BooleanOperator::Or),
    }
}

fn render_canonical_spdx_expression(expression: &LicenseExpression) -> String {
    match expression {
        LicenseExpression::License(key) => key.clone(),
        LicenseExpression::LicenseRef(key) => render_spdx_license_ref(key),
        LicenseExpression::With { left, right } => format!(
            "{} WITH {}",
            render_canonical_spdx_expression(left),
            render_canonical_spdx_expression(right)
        ),
        LicenseExpression::And { .. } => {
            render_flat_boolean_chain_spdx(expression, BooleanOperator::And)
        }
        LicenseExpression::Or { .. } => {
            render_flat_boolean_chain_spdx(expression, BooleanOperator::Or)
        }
    }
}

fn render_spdx_license_ref(key: &str) -> String {
    const LICENSE_REF_PREFIX_LEN: usize = "licenseref-".len();

    if key.len() >= LICENSE_REF_PREFIX_LEN
        && key[..LICENSE_REF_PREFIX_LEN].eq_ignore_ascii_case("licenseref-")
    {
        format!("LicenseRef-{}", &key[LICENSE_REF_PREFIX_LEN..])
    } else {
        key.to_string()
    }
}

fn render_flat_boolean_chain(expression: &LicenseExpression, operator: BooleanOperator) -> String {
    let mut parts = Vec::new();
    collect_boolean_chain(
        expression,
        operator,
        &mut parts,
        &mut RecursionGuard::depth_only(),
    );

    let separator = match operator {
        BooleanOperator::And => " AND ",
        BooleanOperator::Or => " OR ",
    };

    parts
        .into_iter()
        .map(|part| render_boolean_operand(part, operator))
        .collect::<Vec<_>>()
        .join(separator)
}

fn collect_boolean_chain<'a>(
    expression: &'a LicenseExpression,
    operator: BooleanOperator,
    parts: &mut Vec<&'a LicenseExpression>,
    guard: &mut RecursionGuard<()>,
) {
    if guard.descend() {
        warn!("collect_boolean_chain: recursion depth exceeded limit, truncating chain");
        parts.push(expression);
        return;
    }

    match (operator, expression) {
        (BooleanOperator::And, LicenseExpression::And { left, right })
        | (BooleanOperator::Or, LicenseExpression::Or { left, right }) => {
            collect_boolean_chain(left, operator, parts, guard);
            collect_boolean_chain(right, operator, parts, guard);
        }
        _ => parts.push(expression),
    }
    guard.ascend();
}

fn render_boolean_operand(
    expression: &LicenseExpression,
    parent_operator: BooleanOperator,
) -> String {
    match expression {
        LicenseExpression::And { .. } => match parent_operator {
            BooleanOperator::And => render_canonical_expression(expression),
            BooleanOperator::Or => format!("({})", render_canonical_expression(expression)),
        },
        LicenseExpression::Or { .. } => match parent_operator {
            BooleanOperator::Or => render_canonical_expression(expression),
            BooleanOperator::And => format!("({})", render_canonical_expression(expression)),
        },
        _ => render_canonical_expression(expression),
    }
}

fn render_flat_boolean_chain_spdx(
    expression: &LicenseExpression,
    operator: BooleanOperator,
) -> String {
    let mut parts = Vec::new();
    collect_boolean_chain(
        expression,
        operator,
        &mut parts,
        &mut RecursionGuard::depth_only(),
    );

    let separator = match operator {
        BooleanOperator::And => " AND ",
        BooleanOperator::Or => " OR ",
    };

    parts
        .into_iter()
        .map(|part| render_boolean_operand_spdx(part, operator))
        .collect::<Vec<_>>()
        .join(separator)
}

fn render_boolean_operand_spdx(
    expression: &LicenseExpression,
    parent_operator: BooleanOperator,
) -> String {
    match expression {
        LicenseExpression::And { .. } => match parent_operator {
            BooleanOperator::And => render_canonical_spdx_expression(expression),
            BooleanOperator::Or => format!("({})", render_canonical_spdx_expression(expression)),
        },
        LicenseExpression::Or { .. } => match parent_operator {
            BooleanOperator::Or => render_canonical_spdx_expression(expression),
            BooleanOperator::And => format!("({})", render_canonical_spdx_expression(expression)),
        },
        _ => render_canonical_spdx_expression(expression),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_spdx_declared_license_identifier() {
        let (declared, declared_spdx, detections) = normalize_spdx_declared_license(Some("MIT"));

        assert_eq!(declared.as_deref(), Some("mit"));
        assert_eq!(declared_spdx.as_deref(), Some("MIT"));
        assert_eq!(detections.len(), 1);
        assert_eq!(
            detections[0].matches[0].matcher,
            crate::license_detection::MatcherKind::Declared
        );
    }

    #[test]
    fn test_normalize_spdx_declared_license_expression() {
        let (declared, declared_spdx, detections) =
            normalize_spdx_declared_license(Some("MIT OR Apache-2.0"));

        assert_eq!(declared.as_deref(), Some("apache-2.0 OR mit"));
        assert_eq!(declared_spdx.as_deref(), Some("Apache-2.0 OR MIT"));
        assert_eq!(detections.len(), 1);
    }

    #[test]
    fn test_normalize_spdx_declared_license_simplifies_absorbed_expression() {
        let (declared, declared_spdx, detections) =
            normalize_spdx_declared_license(Some("MIT AND (MIT OR Apache-2.0)"));

        assert_eq!(declared.as_deref(), Some("mit"));
        assert_eq!(declared_spdx.as_deref(), Some("MIT"));
        assert_eq!(detections.len(), 1);
    }

    #[test]
    fn test_normalize_declared_license_key_scancode() {
        let normalized = normalize_declared_license_key("mit").expect("normalized key");

        assert_eq!(normalized.declared_license_expression, "mit");
        assert_eq!(normalized.declared_license_expression_spdx, "MIT");
    }

    #[test]
    fn test_combine_normalized_licenses_with_or() {
        let combined = combine_normalized_licenses(
            vec![
                NormalizedDeclaredLicense::new("mit", "MIT"),
                NormalizedDeclaredLicense::new("apache-2.0", "Apache-2.0"),
            ],
            " OR ",
        )
        .expect("combined expression");

        assert_eq!(combined.declared_license_expression, "apache-2.0 OR mit");
        assert_eq!(
            combined.declared_license_expression_spdx,
            "Apache-2.0 OR MIT"
        );
    }

    #[test]
    fn test_combine_normalized_licenses_simplifies_absorbed_and_expression() {
        let combined = combine_normalized_licenses(
            vec![
                NormalizedDeclaredLicense::new("mit", "MIT"),
                NormalizedDeclaredLicense::new("mit OR apache-2.0", "MIT OR Apache-2.0"),
            ],
            " AND ",
        )
        .expect("combined expression");

        assert_eq!(combined.declared_license_expression, "mit");
        assert_eq!(combined.declared_license_expression_spdx, "MIT");
    }

    #[test]
    fn test_normalize_spdx_declared_license_preserves_licenseref_prefix_case() {
        let (declared, declared_spdx, detections) =
            normalize_spdx_declared_license(Some("LicenseRef-scancode-custom-1 OR MIT"));

        assert_eq!(
            declared.as_deref(),
            Some("licenseref-scancode-custom-1 OR mit")
        );
        assert_eq!(
            declared_spdx.as_deref(),
            Some("LicenseRef-scancode-custom-1 OR MIT")
        );
        assert_eq!(detections.len(), 1);
    }

    #[test]
    fn test_build_declared_license_detection_uses_parser_matcher() {
        let detection = build_declared_license_detection(
            &NormalizedDeclaredLicense::new("mit", "MIT"),
            DeclaredLicenseMatchMetadata::new(
                "MIT",
                LineNumber::new(4).unwrap(),
                LineNumber::new(4).unwrap(),
            ),
        );

        assert_eq!(
            detection.matches[0].matcher,
            crate::license_detection::MatcherKind::Declared
        );
        assert_eq!(
            detection.matches[0].start_line,
            LineNumber::new(4).expect("valid")
        );
        assert_eq!(detection.matches[0].matched_text.as_deref(), Some("MIT"));
        assert!(!detection.matches[0].rule_identifier.is_empty());
    }

    #[test]
    fn test_build_declared_license_detection_preserves_engine_rule_metadata() {
        let mit_license_text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testdata/license-golden/single-license/mit.txt"
        ));
        let normalized = NormalizedDeclaredLicense::new("mit", "MIT");
        let (expected_rule_identifier, expected_rule_url) =
            derive_declared_rule_metadata(&normalized, mit_license_text)
                .expect("full MIT license text should derive rule metadata");

        let detection = build_declared_license_detection(
            &normalized,
            DeclaredLicenseMatchMetadata::new(
                mit_license_text,
                LineNumber::new(10).unwrap(),
                LineNumber::new(31).unwrap(),
            ),
        );

        assert_ne!(expected_rule_identifier, PARSER_DECLARED_MATCHER);
        assert_eq!(
            detection.matches[0].rule_identifier.as_str(),
            expected_rule_identifier.as_str()
        );
        assert_eq!(detection.matches[0].rule_url, expected_rule_url);
    }

    fn package_with(
        extracted: Option<&str>,
        copyright: Option<&str>,
        holder: Option<&str>,
    ) -> PackageData {
        PackageData {
            extracted_license_statement: extracted.map(str::to_string),
            copyright: copyright.map(str::to_string),
            holder: holder.map(str::to_string),
            ..PackageData::default()
        }
    }

    #[test]
    fn test_populate_declared_license_from_spdx_statement() {
        let mut package = package_with(Some("MIT"), None, None);
        populate_declared_license_and_holder(&mut package);

        assert_eq!(package.declared_license_expression.as_deref(), Some("mit"));
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("MIT")
        );
        assert_eq!(package.license_detections.len(), 1);
    }

    #[test]
    fn test_populate_declared_license_via_engine_fallback() {
        // "Apache 2.0" is not a valid SPDX identifier, so this exercises the
        // free-text detection fallback rather than SPDX normalization.
        let mut package = package_with(Some("Apache 2.0"), None, None);
        populate_declared_license_and_holder(&mut package);

        assert_eq!(
            package.declared_license_expression.as_deref(),
            Some("apache-2.0")
        );
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("Apache-2.0")
        );
        assert!(!package.license_detections.is_empty());
    }

    #[test]
    fn test_populate_declared_license_skips_lossy_multi_license_statement() {
        // Free-text detection silently drops the leading MIT here, so the hook
        // must leave the fields unset rather than emit a partial expression.
        let mut package = package_with(Some("MIT AND Apache 2.0 AND BSD-3-Clause"), None, None);
        populate_declared_license_and_holder(&mut package);

        assert!(package.declared_license_expression.is_none());
        assert!(package.declared_license_expression_spdx.is_none());
        assert!(package.license_detections.is_empty());
    }

    #[test]
    fn test_populate_declared_license_via_engine_fallback_has_no_referenced_filenames() {
        // A declared license derived from the manifest's own statement (here via
        // the free-text engine fallback) has no separate referenced file, so the
        // detection must not record the statement text as a referenced filename.
        let mut package = package_with(Some("Apache 2.0"), None, None);
        populate_declared_license_and_holder(&mut package);

        assert!(!package.license_detections.is_empty());
        for detection in &package.license_detections {
            for detection_match in &detection.matches {
                assert!(
                    detection_match
                        .referenced_filenames
                        .as_ref()
                        .is_none_or(|filenames| filenames.is_empty()),
                    "engine-fallback declared license must not record referenced filenames, got {:?}",
                    detection_match.referenced_filenames
                );
            }
        }
    }

    #[test]
    fn test_detect_declared_license_from_text_without_referenced_filename() {
        let (declared, _declared_spdx, detections) =
            detect_declared_license_from_text("Apache 2.0", None);

        assert_eq!(declared.as_deref(), Some("apache-2.0"));
        assert_eq!(detections.len(), 1);
        assert!(
            detections[0].matches[0]
                .referenced_filenames
                .as_ref()
                .is_none_or(|filenames| filenames.is_empty())
        );
    }

    #[test]
    fn test_detect_declared_license_from_text_with_referenced_filename() {
        let (declared, _declared_spdx, detections) =
            detect_declared_license_from_text("Apache 2.0", Some("LICENSE"));

        assert_eq!(declared.as_deref(), Some("apache-2.0"));
        assert_eq!(detections.len(), 1);
        assert_eq!(
            detections[0].matches[0].referenced_filenames.as_deref(),
            Some(["LICENSE".to_string()].as_slice())
        );
    }

    #[test]
    fn test_populate_holder_handles_crlf_like_lf() {
        let crlf = package_with(
            None,
            Some("Copyright (c) 2024 Foo Corp\r\nCopyright (c) 2024 Bar Inc"),
            None,
        );
        let lf = package_with(
            None,
            Some("Copyright (c) 2024 Foo Corp\nCopyright (c) 2024 Bar Inc"),
            None,
        );

        let mut crlf_package = crlf;
        let mut lf_package = lf;
        populate_declared_license_and_holder(&mut crlf_package);
        populate_declared_license_and_holder(&mut lf_package);

        assert_eq!(crlf_package.holder, lf_package.holder);
        assert!(
            !crlf_package
                .holder
                .as_deref()
                .unwrap_or_default()
                .contains('\r'),
            "CRLF copyright must not leak a trailing carriage return into the holder"
        );
    }

    #[test]
    fn test_looks_like_multi_license_composite_slash_with_url() {
        // A bare slash separating license names is composite even when a URL is
        // also present.
        assert!(looks_like_multi_license_composite("MIT/BSD see http://x"));
        // A slash that only appears inside a URL is not a separator.
        assert!(!looks_like_multi_license_composite(
            "Apache 2.0 http://www.apache.org/licenses/LICENSE-2.0"
        ));
    }

    #[test]
    fn test_populate_declared_license_does_not_overwrite_existing() {
        let mut package = package_with(Some("MIT"), None, None);
        package.declared_license_expression = Some("apache-2.0".to_string());
        package.declared_license_expression_spdx = Some("Apache-2.0".to_string());
        populate_declared_license_and_holder(&mut package);

        assert_eq!(
            package.declared_license_expression.as_deref(),
            Some("apache-2.0")
        );
        assert!(package.license_detections.is_empty());
    }

    #[test]
    fn test_populate_declared_license_from_single_multiline_license_dump() {
        // A serialized single `license` entry (name + url) is not composite and
        // should yield the correct single expression.
        let mut package = package_with(
            Some(
                "- license:\n    name: Apache-2.0\n    url: https://www.apache.org/licenses/LICENSE-2.0.txt\n",
            ),
            None,
            None,
        );
        populate_declared_license_and_holder(&mut package);

        assert_eq!(
            package.declared_license_expression.as_deref(),
            Some("apache-2.0")
        );
        assert_eq!(package.license_detections.len(), 1);
    }

    #[test]
    fn test_populate_declared_license_skips_multi_entry_license_dump() {
        // A serialized list of several `licenses` entries is lossy under
        // free-text detection (only the last is matched), so leave it unset.
        let mut package = package_with(
            Some("- type: MIT\n  url: x\n- type: Apache-2.0\n  url: y\n"),
            None,
            None,
        );
        populate_declared_license_and_holder(&mut package);

        assert!(package.declared_license_expression.is_none());
        assert!(package.license_detections.is_empty());
    }

    #[test]
    fn test_populate_declared_license_preserves_existing_detections() {
        // A parser that already built `license_detections` (e.g. referencing
        // license files) must not have them clobbered by the fallback.
        let mut package = package_with(Some("MIT"), None, None);
        package.license_detections = vec![build_declared_license_detection(
            &NormalizedDeclaredLicense::new("mit", "MIT"),
            DeclaredLicenseMatchMetadata::single_line("LICENSE"),
        )];
        let original = package.license_detections.clone();
        populate_declared_license_and_holder(&mut package);

        assert!(package.declared_license_expression.is_none());
        assert_eq!(package.license_detections, original);
    }

    #[test]
    fn test_populate_declared_license_defers_to_license_file_references() {
        // When the parser declared license-file references, those files own the
        // declared expression via finalize; the fallback must not pre-empt them.
        let mut package = package_with(Some("MIT"), None, None);
        let mut extra_data = std::collections::HashMap::new();
        extra_data.insert("license_files".to_string(), serde_json::json!(["LICENSE"]));
        package.extra_data = Some(extra_data);
        populate_declared_license_and_holder(&mut package);

        assert!(package.declared_license_expression.is_none());
        assert!(package.license_detections.is_empty());
    }

    #[test]
    fn test_populate_holder_from_copyright() {
        let mut package = package_with(None, Some("Copyright (c) 2024 Example Corporation"), None);
        populate_declared_license_and_holder(&mut package);

        assert_eq!(package.holder.as_deref(), Some("Example Corporation"));
    }

    #[test]
    fn test_populate_holder_falls_back_to_raw_copyright_when_no_holder_detected() {
        let mut package = package_with(None, Some("2015"), None);
        populate_declared_license_and_holder(&mut package);

        assert_eq!(package.holder.as_deref(), Some("2015"));
    }

    #[test]
    fn test_populate_holder_does_not_overwrite_existing() {
        let mut package = package_with(
            None,
            Some("Copyright (c) 2024 Example Corporation"),
            Some("Existing Holder"),
        );
        populate_declared_license_and_holder(&mut package);

        assert_eq!(package.holder.as_deref(), Some("Existing Holder"));
    }

    #[test]
    fn test_populate_leaves_fields_unset_without_inputs() {
        let mut package = package_with(None, None, None);
        populate_declared_license_and_holder(&mut package);

        assert!(package.declared_license_expression.is_none());
        assert!(package.holder.is_none());
    }

    #[test]
    fn test_looks_like_multi_license_composite() {
        assert!(looks_like_multi_license_composite("MIT AND Apache-2.0"));
        assert!(looks_like_multi_license_composite("GPL | LGPL"));
        assert!(looks_like_multi_license_composite("MIT/BSD"));
        assert!(looks_like_multi_license_composite("MIT, Apache-2.0"));
        assert!(!looks_like_multi_license_composite("GPL (>= 2)"));
        assert!(!looks_like_multi_license_composite("Apache 2.0"));
        assert!(!looks_like_multi_license_composite(
            "https://example.com/LICENSE"
        ));
        // Single-license multi-line dumps are not composite.
        assert!(!looks_like_multi_license_composite(
            "- license:\n    name: Apache-2.0\n    url: https://example.com\n"
        ));
        // Multiple list entries / bare lines are composite.
        assert!(looks_like_multi_license_composite(
            "- type: MIT\n  url: x\n- type: Apache-2.0\n  url: y\n"
        ));
        assert!(looks_like_multi_license_composite(
            "BSD-2-Clause\nGPL-2.0-or-later"
        ));
    }
}
