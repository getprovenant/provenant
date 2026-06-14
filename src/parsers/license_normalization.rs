// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, LazyLock};

#[cfg(test)]
use std::cell::Cell;

use crate::parser_warn as warn;
use crate::parsers::active_parser_license_engine;
use crate::parsers::utils::{RecursionGuard, capped_iteration_limit};

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

    let (declared, declared_spdx) = match (
        declared_license_expression,
        declared_license_expression_spdx,
    ) {
        (Some(declared), Some(declared_spdx)) => (declared, declared_spdx),
        // Free-text detection produced no public expression. A bare ambiguous
        // license name (e.g. "BSD", "GPL") only matches a clue-only rule, which
        // detection deliberately keeps expression-less for arbitrary file text.
        // For a declared manifest statement that *is* the whole bounded license
        // field, promote that clue to a confident declared expression.
        _ => match promote_whole_statement_clue(&detections) {
            Some(normalized) => (
                normalized.declared_license_expression,
                normalized.declared_license_expression_spdx,
            ),
            None => return empty_declared_license_data(),
        },
    };

    let references: Vec<&str> = referenced_filename.into_iter().collect();
    build_declared_license_data_from_pair(
        declared,
        declared_spdx,
        DeclaredLicenseMatchMetadata::single_line(text).with_referenced_filenames(&references),
    )
}

/// Promotes a clue-only whole-statement detection into a confident declared
/// expression.
///
/// Free-text detection keeps bare ambiguous license names (e.g. "BSD", "GPL",
/// "BSD-style") as expression-less *clues* so they never become hard detections
/// in arbitrary file text. A declared manifest statement is a bounded,
/// trustworthy license field, so when the statement is exactly such a name we
/// honor the clue's license expression — the declared-context analog of
/// ScanCode's `package_license` handling.
///
/// Stays conservative: requires a single detection whose single match is a
/// whole-statement *hash* match (the statement's tokens are exactly a rule's
/// tokens). This is what distinguishes a true bare name such as "BSD" or
/// "BSD-style" from a fragment match where the bare name is only part of a
/// longer statement (e.g. "BSD with advertising", which means BSD-4-Clause, must
/// NOT collapse to bsd-new). A statement with no clue match, or whose clue match
/// only covers a fragment, yields `None` so the declared expression stays an
/// honest `null`.
fn promote_whole_statement_clue(
    detections: &[crate::license_detection::detection::LicenseDetection],
) -> Option<NormalizedDeclaredLicense> {
    let [detection] = detections else {
        return None;
    };
    // Only a fully expression-less clue detection is the bare-name case this
    // promotes. A detection that already carries a public expression in either the
    // scancode or SPDX form is a real detection and must not be re-derived here
    // (the `(None, Some)` combination is not produced by the current rule set, but
    // the symmetric guard keeps the invariant explicit).
    if detection.license_expression.is_some() || detection.license_expression_spdx.is_some() {
        return None;
    }
    let [match_item] = detection.matches.as_slice() else {
        return None;
    };
    if match_item.matcher != crate::license_detection::models::MatcherKind::Hash {
        return None;
    }
    let expression = match_item.license_expression.trim();
    if expression.is_empty() {
        return None;
    }

    // Do not promote a generic/unknown catch-all license (e.g. `commercial-license`,
    // `proprietary-license`, `free-unknown`). A non-specific match means the declared
    // statement is a custom/unstated license — for example `"Acme Commercial License"`,
    // where the custom token is dropped during tokenization and only `"Commercial
    // License"` matches the generic rule. Such statements must stay an extracted-only
    // raw value; only a specific license (e.g. `bsd-new`) is promoted to declared.
    if let Some(engine) = parser_license_engine()
        && let Some(license) = engine.index().licenses_by_key.get(expression)
        && (license.is_generic || license.is_unknown)
    {
        return None;
    }

    // Re-normalize through the index so both the ScanCode and SPDX forms are
    // canonical, rather than trusting whatever the clue match happened to carry.
    normalize_spdx_expression(expression)
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

    // The npm `SEE LICENSE IN <file>`/`<url>` convention points at a file-stored
    // custom license rather than naming an expression. Free-text detection
    // collapses it to a low-value `unknown-license-reference`, so leave the field
    // unset and let file-reference resolution recover the real license.
    if statement_is_see_license_in_pointer(statement) {
        return;
    }

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
        // Structured "or later" version-range idioms (CRAN/Debian style, e.g.
        // "GPL (>= 2)") encode an `-or-later` expression that free-text detection
        // otherwise mis-reads as the `-only` form. Rewrite the idiom into a valid
        // SPDX expression and normalize that instead of the raw statement.
        let rewritten = rewrite_version_range_or_later_idiom(statement);
        let effective_statement = rewritten.as_deref().unwrap_or(statement);

        let normalized = normalize_spdx_declared_license(Some(effective_statement));
        if normalized.0.is_some() {
            normalized
        } else if rewritten.is_some() || looks_like_multi_license_composite(statement) {
            // The idiom matched a known family but did not normalize confidently:
            // prefer an honest unset over the wrong `-only` expression free-text
            // detection would otherwise derive from the original statement.
            empty_declared_license_data()
        } else {
            // The statement is the manifest's own declared value, not a
            // referenced file, so it must not be recorded as a referenced
            // filename in the resulting detection.
            detect_declared_license_from_text(statement, None)
        }
    };

    // A versionless GNU-style license page (e.g. `.../licenses/lgpl.html`) must
    // not be over-pinned to a versioned "or later" expression: when the only
    // signal is a URL and detection invented an `-or-later`/`-plus` version the
    // URL does not contain, prefer an honest unset. Correct unversioned or
    // identity-versioned detections from a URL (e.g. `mit`, `ms-net-library`,
    // `ms-net-library-2018-11`, `apache-2.0`) are preserved.
    if let Some(declared_key) = declared.as_deref()
        && statement_is_url_only(statement)
        && expression_is_or_later_with_version_absent_from_statement(declared_key, statement)
    {
        return;
    }

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

/// Rewrites a CRAN/Debian-style "or later" version-range idiom such as
/// `GPL (>= 2)` into an equivalent `<family>-<version>+` expression
/// (`GPL-2.0+`) that normalizes to the correct `-or-later` SPDX form.
///
/// The `(>= N)` convention means "version N or any later version", so the
/// honest mapping is the `+`/`-or-later` form, not the `-only` form free-text
/// detection would otherwise produce. Only the GPL/LGPL/AGPL families use this
/// idiom in declared metadata; anything else returns `None` so normal handling
/// applies. A bare major version is expanded to its canonical point release
/// (e.g. `2` -> `2.0`), while an explicit minor version is preserved
/// (e.g. `2.1`). Both `>= N.M` and `>= N-M` separators are accepted.
///
/// The idiom must span the WHOLE statement. A compound CRAN declaration such as
/// `LGPL (>= 2.1) | GPL-2` carries additional license operands after the range;
/// rewriting only the leading operand would silently drop the rest and bypass
/// the multi-license composite guard. When anything follows the closing paren,
/// this returns `None` so the statement falls through to that guard (which
/// leaves the declared expression an honest `null`).
fn rewrite_version_range_or_later_idiom(statement: &str) -> Option<String> {
    let trimmed = statement.trim();
    let open = trimmed.find('(')?;
    let close = trimmed.rfind(')')?;
    if close <= open {
        return None;
    }
    if !trimmed[close + 1..].trim().is_empty() {
        return None;
    }

    let family_raw = trimmed[..open].trim();
    let family = match family_raw.to_ascii_uppercase().as_str() {
        "GPL" => "GPL",
        "LGPL" => "LGPL",
        "AGPL" => "AGPL",
        _ => return None,
    };

    let inner = trimmed[open + 1..close].trim();
    let version_raw = inner.strip_prefix(">=")?.trim();
    if version_raw.is_empty() {
        return None;
    }

    // Accept "N", "N.M", or "N-M"; reject anything with stray characters so an
    // unexpected idiom falls back to honest unset rather than a guess.
    let mut parts = version_raw.split(['.', '-']);
    let major = parts.next()?;
    let minor = parts.next().unwrap_or("0");
    if parts.next().is_some() {
        return None;
    }
    if major.is_empty()
        || !major.bytes().all(|b| b.is_ascii_digit())
        || !minor.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }

    Some(format!("{family}-{major}.{minor}+"))
}

/// Returns true when the statement is the npm `SEE LICENSE IN <file>`/`<url>`
/// pointer convention.
fn statement_is_see_license_in_pointer(statement: &str) -> bool {
    statement
        .trim()
        .to_ascii_uppercase()
        .starts_with("SEE LICENSE IN ")
}

/// Returns true when the statement's only license signal is a single URL
/// (optionally preceded by a trivial connector such as "see"), with no other
/// license-name token that could have driven detection.
fn statement_is_url_only(statement: &str) -> bool {
    let mut url_tokens = 0usize;
    for token in statement.split_whitespace() {
        if token.contains("://") {
            url_tokens += 1;
        } else if !token.eq_ignore_ascii_case("see") {
            return false;
        }
    }
    url_tokens == 1
}

/// Returns true when a derived license expression is an "or later" form
/// (`-or-later`, `-plus`, or a trailing `+`) whose pinned version does not
/// appear in the source statement.
///
/// The over-pinning risk is specific to versionless GNU-style license pages:
/// `.../licenses/lgpl.html` detects `lgpl-2.0-plus`, inventing both the `2.0`
/// version and the "or later" qualifier the page never states. A bare numeric
/// key that is simply the license's own identity (e.g. `apache-2.0`,
/// `ms-net-library-2018-11`) is NOT second-guessed, nor is an unversioned
/// detection (`mit`, `ms-net-library`).
fn expression_is_or_later_with_version_absent_from_statement(
    expression: &str,
    statement: &str,
) -> bool {
    if !is_or_later_expression(expression) {
        return false;
    }
    let Some(version) = version_token_of_expression(expression) else {
        return false;
    };
    !statement_contains_version(statement, &version)
}

/// Returns true when a license key is an "or later" form: a `-or-later`/`-plus`
/// suffix or a trailing `+`.
fn is_or_later_expression(expression: &str) -> bool {
    let key = expression.trim();
    key.ends_with('+')
        || key.to_ascii_lowercase().ends_with("-or-later")
        || key.to_ascii_lowercase().ends_with("-plus")
}

/// Extracts the first numeric version component of a license key, if any
/// (e.g. `lgpl-2.0-plus` -> `2.0`, `gpl-3.0` -> `3.0`, `mit` -> `None`).
fn version_token_of_expression(expression: &str) -> Option<String> {
    let mut current = String::new();
    for ch in expression.chars() {
        if ch.is_ascii_digit() || (ch == '.' && !current.is_empty()) {
            current.push(ch);
        } else if !current.is_empty() {
            break;
        }
    }
    let trimmed = current.trim_end_matches('.');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Returns true when the statement contains the given version, accepting the
/// common URL spelling variants (`2.0`, `2-0`, or a bare `2` for an `N.0`).
fn statement_contains_version(statement: &str, version: &str) -> bool {
    if statement.contains(version) {
        return true;
    }
    let dashed = version.replace('.', "-");
    if statement.contains(&dashed) {
        return true;
    }
    if let Some(major) = version.strip_suffix(".0") {
        return statement.contains(major);
    }
    false
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
            let limit = capped_iteration_limit(
                values.len(),
                "license_normalization: declared license reference array",
            );
            for value in values.iter().take(limit) {
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

        // The canonical ScanCode key for this license is the rule's own
        // `license_expression` (e.g. `bsd-new` for SPDX `BSD-3-Clause`,
        // `bsd-simplified` for `BSD-2-Clause`). Lowercasing the SPDX id only
        // coincides with the ScanCode key for licenses like `mit`/`apache-2.0`;
        // for others it fabricated a key absent from the index, diverging from
        // both ScanCode output and Provenant's own file-level detections.
        let declared_license_expression_spdx = index
            .licenses_by_key
            .get(&rule_license_expression)
            .and_then(|license| license.spdx_license_key.clone())
            .unwrap_or_else(|| normalized_key.to_string());

        return Some(NormalizedDeclaredLicense::new(
            rule_license_expression,
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
    fn test_populate_declared_license_ambiguous_bare_bsd() {
        // Bare "BSD" fails strict SPDX parsing and only matches a clue-only
        // rule; on a declared manifest statement it must normalize to bsd-new
        // (BSD-3-Clause), matching ScanCode.
        let mut package = package_with(Some("BSD"), None, None);
        populate_declared_license_and_holder(&mut package);

        assert_eq!(
            package.declared_license_expression.as_deref(),
            Some("bsd-new")
        );
        assert_eq!(
            package.declared_license_expression_spdx.as_deref(),
            Some("BSD-3-Clause")
        );
        assert_eq!(package.license_detections.len(), 1);
    }

    #[test]
    fn test_populate_declared_license_ambiguous_bare_gpl_and_lgpl() {
        // Bare GPL/LGPL shorthand should also resolve via the clue-promotion
        // path (GPL is clue-only; LGPL has a non-clue bare rule).
        let mut gpl = package_with(Some("GPL"), None, None);
        populate_declared_license_and_holder(&mut gpl);
        assert_eq!(
            gpl.declared_license_expression.as_deref(),
            Some("gpl-1.0-plus")
        );

        let mut lgpl = package_with(Some("LGPL"), None, None);
        populate_declared_license_and_holder(&mut lgpl);
        assert_eq!(
            lgpl.declared_license_expression.as_deref(),
            Some("lgpl-2.0-plus")
        );
    }

    #[test]
    fn test_populate_declared_license_custom_generic_stays_extracted_only() {
        // A custom/proprietary statement whose only match resolves to a generic
        // catch-all license (here `commercial-license`, with the custom `Acme`
        // token dropped during tokenization) must NOT be promoted to declared; it
        // stays an extracted-only raw value.
        let mut package = package_with(Some("Acme Commercial License"), None, None);
        populate_declared_license_and_holder(&mut package);

        assert_eq!(
            package.extracted_license_statement.as_deref(),
            Some("Acme Commercial License")
        );
        assert_eq!(package.declared_license_expression, None);
        assert_eq!(package.declared_license_expression_spdx, None);
        assert!(package.license_detections.is_empty());
    }

    #[test]
    fn test_populate_declared_license_unambiguous_bare_names_unchanged() {
        // Names that already normalize via strict SPDX parsing must be
        // unaffected by the clue-promotion fallback.
        let mut mit = package_with(Some("MIT"), None, None);
        populate_declared_license_and_holder(&mut mit);
        assert_eq!(mit.declared_license_expression.as_deref(), Some("mit"));

        let mut apache = package_with(Some("Apache-2.0"), None, None);
        populate_declared_license_and_holder(&mut apache);
        assert_eq!(
            apache.declared_license_expression.as_deref(),
            Some("apache-2.0")
        );

        let mut dual = package_with(Some("MIT OR Apache-2.0"), None, None);
        populate_declared_license_and_holder(&mut dual);
        assert_eq!(
            dual.declared_license_expression.as_deref(),
            Some("apache-2.0 OR mit")
        );
    }

    #[test]
    fn test_populate_declared_license_bare_name_fragment_stays_null() {
        // "BSD with advertising" means BSD-4-Clause; the bare-BSD clue rule only
        // matches the "BSD" fragment (an Aho match, not a whole-statement hash
        // match), so the clue-promotion path must NOT collapse it to bsd-new.
        let mut package = package_with(Some("BSD with advertising"), None, None);
        populate_declared_license_and_holder(&mut package);

        // Bidirectional guard: the fragment is neither collapsed to the bare-name
        // expression nor mapped to anything else — it stays an honest null.
        assert_ne!(
            package.declared_license_expression.as_deref(),
            Some("bsd-new"),
            "a bare-name fragment must not be promoted to the bare-name expression"
        );
        assert_eq!(package.declared_license_expression, None);
        assert_eq!(package.declared_license_expression_spdx, None);
        assert!(package.license_detections.is_empty());
    }

    #[test]
    fn test_populate_declared_license_unmatchable_short_stays_null() {
        // A short statement that matches no license rule must stay an honest
        // null rather than guessing.
        let mut package = package_with(Some("zzgarbage"), None, None);
        populate_declared_license_and_holder(&mut package);

        assert!(package.declared_license_expression.is_none());
        assert!(package.declared_license_expression_spdx.is_none());
        assert!(package.license_detections.is_empty());
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

    fn declared_for(statement: &str) -> (Option<String>, Option<String>) {
        let mut package = package_with(Some(statement), None, None);
        populate_declared_license_and_holder(&mut package);
        (
            package.declared_license_expression,
            package.declared_license_expression_spdx,
        )
    }

    #[test]
    fn test_version_range_or_later_idiom_maps_to_or_later() {
        // Defect 1: the CRAN/Debian `(>= N)` idiom means "or later", so it must
        // derive the `-or-later` SPDX form across the GPL/LGPL/AGPL families,
        // not the `-only` form free-text detection would otherwise produce.
        for (statement, expected_key, expected_spdx) in [
            ("GPL (>= 2)", "gpl-2.0-plus", "GPL-2.0-or-later"),
            ("GPL (>= 3)", "gpl-3.0-plus", "GPL-3.0-or-later"),
            ("LGPL (>= 2.1)", "lgpl-2.1-plus", "LGPL-2.1-or-later"),
            ("LGPL (>= 3)", "lgpl-3.0-plus", "LGPL-3.0-or-later"),
            ("AGPL (>= 3)", "agpl-3.0-plus", "AGPL-3.0-or-later"),
        ] {
            let (declared, declared_spdx) = declared_for(statement);
            assert_eq!(
                declared.as_deref(),
                Some(expected_key),
                "declared key for {statement:?}"
            );
            assert_eq!(
                declared_spdx.as_deref(),
                Some(expected_spdx),
                "declared spdx for {statement:?}"
            );
        }
    }

    #[test]
    fn test_version_range_idiom_accepts_dash_separator() {
        // Debian-style "GPL (>= 2-0)" uses a dash between major and minor.
        let (declared, declared_spdx) = declared_for("GPL (>= 2-0)");
        assert_eq!(declared.as_deref(), Some("gpl-2.0-plus"));
        assert_eq!(declared_spdx.as_deref(), Some("GPL-2.0-or-later"));
    }

    #[test]
    fn test_existing_or_later_and_only_forms_are_preserved() {
        // Regression guard for the still-correct cases listed under Defect 1.
        for (statement, expected_key, expected_spdx) in [
            ("GPL-2.0+", "gpl-2.0-plus", "GPL-2.0-or-later"),
            ("GPL-2.0", "gpl-2.0", "GPL-2.0-only"),
            ("GPLv2", "gpl-2.0", "GPL-2.0-only"),
            ("MIT", "mit", "MIT"),
            ("Apache License 2.0", "apache-2.0", "Apache-2.0"),
            // SPDX ids whose canonical ScanCode key is not the lowercased SPDX
            // id must resolve to the real index key, not a fabricated one.
            ("BSD-3-Clause", "bsd-new", "BSD-3-Clause"),
            ("BSD-2-Clause", "bsd-simplified", "BSD-2-Clause"),
            ("Artistic-2.0", "artistic-2.0", "Artistic-2.0"),
            ("WTFPL", "wtfpl-2.0", "WTFPL"),
            ("MPL-2.0", "mpl-2.0", "MPL-2.0"),
            ("EPL-2.0", "epl-2.0", "EPL-2.0"),
        ] {
            let (declared, declared_spdx) = declared_for(statement);
            assert_eq!(
                declared.as_deref(),
                Some(expected_key),
                "declared key for {statement:?}"
            );
            assert_eq!(
                declared_spdx.as_deref(),
                Some(expected_spdx),
                "declared spdx for {statement:?}"
            );
        }
    }

    /// Invariant guard for the whole license set, not a hand-maintained sample:
    /// for every SPDX id the bundled index knows, declared-license normalization
    /// must resolve to a ScanCode key that actually exists in the index. This is
    /// what `normalize_license_key` should do by consulting the authoritative
    /// index; the earlier lowercase-the-SPDX-id shortcut fabricated keys absent
    /// from the index (e.g. `bsd-3-clause`, `wtfpl`) for the ~246 licenses whose
    /// canonical key is not simply the lowercased SPDX id.
    #[test]
    fn test_every_indexed_spdx_key_normalizes_to_a_real_index_key() {
        let engine = parser_license_engine().expect("embedded parser license engine");
        let index = engine.index();

        let mut checked = 0usize;
        for license in index.licenses_by_key.values() {
            let Some(spdx_key) = license.spdx_license_key.as_deref() else {
                continue;
            };
            // LicenseRef-* ids round-trip as themselves; they are not part of the
            // indexed-key invariant under test here.
            if spdx_key.starts_with("LicenseRef-") {
                continue;
            }

            let Some(normalized) = normalize_license_key(spdx_key, index) else {
                continue;
            };

            for token in normalized
                .declared_license_expression
                .split([' ', '(', ')'])
                .filter(|token| !token.is_empty())
            {
                if matches!(token, "AND" | "OR" | "WITH" | "and" | "or" | "with")
                    || token.contains("licenseref-")
                {
                    continue;
                }
                assert!(
                    index.licenses_by_key.contains_key(token),
                    "declared key {token:?} for SPDX {spdx_key:?} is absent from the index"
                );
            }
            checked += 1;
        }

        assert!(
            checked > 100,
            "expected to validate many indexed SPDX keys, only checked {checked}"
        );
    }

    #[test]
    fn test_versionless_url_with_invented_version_falls_back_to_null() {
        // Defect 2: a generic versionless license page must not be over-pinned to
        // a concrete version. `.../licenses/lgpl.html` detects `lgpl-2.0-plus`,
        // but the URL carries no `2.0`, so the invented version is suppressed to
        // an honest null.
        let (declared, declared_spdx) = declared_for("see http://www.gnu.org/licenses/lgpl.html");
        assert!(declared.is_none(), "got {declared:?}");
        assert!(declared_spdx.is_none(), "got {declared_spdx:?}");
    }

    #[test]
    fn test_unversioned_license_url_detections_are_preserved() {
        // Regression guard for Defect 2: a URL that resolves to an *unversioned*
        // license carries no invented version, so it must stay populated. These
        // are the nuget bootstrap / jquery-ui / aspnet-mvc golden cases.
        for (statement, expected_key) in [
            (
                "https://github.com/twbs/bootstrap/blob/master/LICENSE",
                "mit",
            ),
            ("http://jquery.org/license", "mit"),
            (
                "http://www.microsoft.com/web/webpi/eula/net_library_eula_enu.htm",
                "ms-net-library",
            ),
            // Identity-versioned key (date baked into the license name, not an
            // invented SPDX "or later" version) from a URL with no matching
            // digits must still be preserved.
            (
                "http://go.microsoft.com/fwlink/?LinkId=329770",
                "ms-net-library-2018-11",
            ),
        ] {
            let (declared, _spdx) = declared_for(statement);
            assert_eq!(
                declared.as_deref(),
                Some(expected_key),
                "declared for {statement:?}"
            );
        }
    }

    #[test]
    fn test_versioned_license_url_still_normalizes() {
        // Regression guard for Defect 2: a URL that names the version it derives
        // (e.g. the chef nuspec Apache case, `.../LICENSE-2.0` -> `apache-2.0`)
        // must still normalize, since the version is present in the URL.
        let (declared, declared_spdx) = declared_for("http://www.apache.org/licenses/LICENSE-2.0");
        assert_eq!(declared.as_deref(), Some("apache-2.0"));
        assert_eq!(declared_spdx.as_deref(), Some("Apache-2.0"));
    }

    #[test]
    fn test_npm_see_license_in_falls_back_to_null() {
        // Defect 3: the npm `SEE LICENSE IN <file>`/`<url>` pointer must not be
        // synthesized into a low-value `unknown-license-reference`.
        for statement in [
            "SEE LICENSE IN LICENSE.txt",
            "SEE LICENSE IN https://example.com/license",
            "see license in COPYING",
        ] {
            let (declared, declared_spdx) = declared_for(statement);
            assert!(declared.is_none(), "{statement:?} -> {declared:?}");
            assert!(
                declared_spdx.is_none(),
                "{statement:?} -> {declared_spdx:?}"
            );
        }
    }

    #[test]
    fn test_custom_licenseref_expression_still_normalizes() {
        // Regression guard for Defect 3: a legitimate `LicenseRef-...` SPDX
        // expression is still handled by the SPDX path.
        let (declared, declared_spdx) = declared_for("LicenseRef-Custom");
        assert_eq!(declared.as_deref(), Some("licenseref-custom"));
        assert_eq!(declared_spdx.as_deref(), Some("LicenseRef-custom"));
    }

    #[test]
    fn test_rewrite_version_range_or_later_idiom_rejects_non_families() {
        assert!(rewrite_version_range_or_later_idiom("MIT (>= 2)").is_none());
        assert!(rewrite_version_range_or_later_idiom("GPL (== 2)").is_none());
        assert!(rewrite_version_range_or_later_idiom("GPL (>= 2.0.1)").is_none());
        assert!(rewrite_version_range_or_later_idiom("GPL").is_none());
    }

    #[test]
    fn test_rewrite_version_range_or_later_idiom_rejects_trailing_operands() {
        // A compound CRAN declaration carries more license operands after the
        // range. Rewriting only the leading operand would silently drop the rest
        // and bypass the composite guard, so the idiom must not fire.
        assert!(rewrite_version_range_or_later_idiom("LGPL (>= 2.1) | GPL-2").is_none());
        assert!(rewrite_version_range_or_later_idiom("GPL (>= 2), MIT").is_none());
        assert!(rewrite_version_range_or_later_idiom("GPL (>= 2) | file LICENSE").is_none());
        // The whole-statement idiom still rewrites.
        assert_eq!(
            rewrite_version_range_or_later_idiom("GPL (>= 2)").as_deref(),
            Some("GPL-2.0+")
        );
        assert_eq!(
            rewrite_version_range_or_later_idiom("GPL (>= 3)").as_deref(),
            Some("GPL-3.0+")
        );
    }

    #[test]
    fn test_compound_version_range_statement_is_null() {
        // Hook-level guard: a compound statement must yield an honest null, not a
        // partial `lgpl-2.1-plus` that drops the trailing operand.
        for statement in [
            "LGPL (>= 2.1) | GPL-2",
            "GPL (>= 2), MIT",
            "GPL (>= 2) | file LICENSE",
        ] {
            let (declared, declared_spdx) = declared_for(statement);
            assert!(declared.is_none(), "{statement:?} -> {declared:?}");
            assert!(
                declared_spdx.is_none(),
                "{statement:?} -> {declared_spdx:?}"
            );
        }
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
