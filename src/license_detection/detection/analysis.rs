// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Detection analysis and heuristics.

use super::types::LicenseDetection;
use super::*;
use crate::license_detection::expression::{
    combine_expressions_and_preserving_structure, combine_expressions_or_preserving_structure,
};
use crate::license_detection::models::{LicenseMatch, MatcherKind};
use crate::utils::spdx::{
    ExpressionRelation, combine_license_expressions_preserving_structure_strict,
    combine_license_expressions_with_relation_preserving_structure_strict,
};

/// Coverage value below which detections are not perfect.
/// Any value < 100 means detection is imperfect.
pub const IMPERFECT_MATCH_COVERAGE_THR: f32 = 100.0;

/// Coverage values below this are reported as license clues.
pub const CLUES_MATCH_COVERAGE_THR: f32 = 60.0;

/// False positive threshold for rule length (in tokens).
/// Rules with length <= this are potential false positives.
pub const FALSE_POSITIVE_RULE_LENGTH_THRESHOLD: usize = 3;

/// False positive threshold for start line.
/// Matches after this line with short rules are potential false positives.
pub const FALSE_POSITIVE_START_LINE_THRESHOLD: usize = 1000;

/// Check if match coverage is below threshold.
///
/// Based on Python: is_match_coverage_less_than_threshold() at detection.py:1095
///
/// - If any_matches is True (default), returns True if ANY match has coverage < threshold
/// - If any_matches is False, returns True if NONE of the matches have coverage > threshold
pub(super) fn is_match_coverage_below_threshold(
    matches: &[LicenseMatch],
    threshold: f32,
    any_matches: bool,
) -> bool {
    if any_matches {
        return matches.iter().any(|m| m.coverage() < threshold);
    }
    !matches.iter().any(|m| m.coverage() > threshold)
}

/// Check if all matches have unknown license identifiers.
pub(super) fn has_unknown_matches(matches: &[LicenseMatch]) -> bool {
    matches
        .iter()
        .any(|m| m.rule_identifier.contains("unknown") || m.license_expression.contains("unknown"))
}

/// Check if matches have extra words.
///
/// Extra words are present when score < (coverage * relevance) / 100.
/// Based on Python: calculate_query_coverage_coefficient() at detection.py:1124
/// and has_extra_words() at detection.py:1139
pub(super) fn has_extra_words(matches: &[LicenseMatch]) -> bool {
    matches.iter().any(|m| {
        let score_coverage_relevance =
            f64::from(m.coverage()) * f64::from(m.rule_relevance) / 100.0;
        score_coverage_relevance - m.score.value() > 0.01
    })
}

/// Check if detection is a false positive.
///
/// False positives are identified based on:
/// - Single matches with bare identifiers and low relevance
/// - All-GPL match groups where every matched rule is one token long
/// - Late matches with short rules and low relevance
/// - Tag matches with short length
///
/// Based on Python: is_false_positive() at detection.py:1162
pub(super) fn is_false_positive(matches: &[LicenseMatch]) -> bool {
    if matches.is_empty() {
        return false;
    }

    let has_full_relevance = matches.iter().all(|m| m.rule_relevance == 100);

    let copyright_words = ["copyright", "(c)"];
    let has_copyrights = matches.iter().all(|m| {
        m.matched_text
            .as_ref()
            .map(|text| {
                let text_lower = text.to_lowercase();
                copyright_words.iter().any(|word| text_lower.contains(word))
            })
            .unwrap_or(false)
    });

    if has_copyrights || has_full_relevance {
        return false;
    }

    let start_line = matches
        .iter()
        .map(|m| m.start_line)
        .min()
        .map(|ln| ln.get())
        .unwrap_or(0);

    let bare_rules = ["gpl_bare", "freeware_bare", "public-domain_bare"];
    let is_bare_rule = matches.iter().all(|m| {
        bare_rules
            .iter()
            .any(|bare| m.rule_identifier.to_lowercase().contains(bare))
    });

    let is_gpl = matches.iter().all(|m| {
        let id = m.rule_identifier.to_lowercase();
        id.contains("gpl") && !id.contains("lgpl")
    });

    // Use rule_length (token count) instead of matched_length (character count)
    let rule_length_values: Vec<usize> = matches.iter().map(|m| m.rule_length).collect();

    let all_rule_length_one = rule_length_values.iter().all(|&l| l == 1);

    let all_low_relevance = matches.iter().all(|m| m.rule_relevance < 60);
    let all_exact_spdx_license_id_rules = matches
        .iter()
        .all(|m| m.rule_identifier.starts_with("spdx_license_id_"));

    let is_single = matches.len() == 1;

    // Check if all matches are license tags with length == 1
    let all_is_license_tag = matches.iter().all(LicenseMatch::is_license_tag);

    // Check 1: Single bare rule with low relevance
    if is_single && is_bare_rule && all_low_relevance {
        return true;
    }

    // Check 2: GPL with rule_length == 1 (matching Python's all_match_rule_length_one)
    if is_gpl && all_rule_length_one {
        return true;
    }

    // Check 3: Late matches (after line 1000) with short rules (<=3 tokens) and low relevance
    // Python: any(rule_length <= 3) not all(rule_length == 1)
    if all_low_relevance
        && start_line > FALSE_POSITIVE_START_LINE_THRESHOLD
        && !all_exact_spdx_license_id_rules
        && rule_length_values
            .iter()
            .any(|&l| l <= FALSE_POSITIVE_RULE_LENGTH_THRESHOLD)
    {
        return true;
    }

    // Check 4: License tags with short rule length
    if all_is_license_tag && all_rule_length_one {
        return true;
    }

    false
}

/// Check if matches are low quality based on coverage.
///
/// Low quality matches have:
/// - Coverage < CLUES_MATCH_COVERAGE_THR
/// - OR (coverage < IMPERFECT_MATCH_COVERAGE_THR AND has extra words)
///
/// Based on Python: is_low_quality_matches() at detection.py:1223
pub(super) fn is_low_quality_matches(matches: &[LicenseMatch]) -> bool {
    matches.iter().all(|m| {
        m.coverage() < CLUES_MATCH_COVERAGE_THR
            || (m.coverage() < IMPERFECT_MATCH_COVERAGE_THR
                && has_extra_words(std::slice::from_ref(m)))
    })
}

/// Check if any match has correct license clue.
pub(super) fn has_correct_license_clue_matches(matches: &[LicenseMatch]) -> bool {
    !matches.is_empty()
        && matches.iter().all(|m| {
            matches!(
                m.matcher,
                MatcherKind::Hash | MatcherKind::SpdxId | MatcherKind::Aho
            )
        })
        && matches.iter().all(|m| m.coverage() == 100.0)
        && matches.iter().all(LicenseMatch::is_license_clue)
}

/// Check if matches represent undetected licenses.
///
/// Returns true if matches were detected by the "undetected" matcher.
/// Based on Python: is_undetected_license_matches() at detection.py:1376
pub(super) fn is_undetected_license_matches(matches: &[LicenseMatch]) -> bool {
    !matches.is_empty() && matches.iter().all(|m| m.matcher == MatcherKind::Undetected)
}

/// Check if there are unknown license intros before detection.
///
/// Based on Python: has_unknown_intro_before_detection() at detection.py:1196
pub(super) fn has_unknown_intro_before_detection(matches: &[LicenseMatch]) -> bool {
    // Python: if len(license_matches) == 1: return False
    if matches.len() == 1 {
        return false;
    }

    // Python: if all([is_unknown_intro(match) for match in license_matches]): return False
    if matches.iter().all(is_unknown_intro) {
        return false;
    }

    for m in matches {
        if m.matcher == MatcherKind::Undetected {
            continue;
        }
        let has_unknown = m.license_expression.contains("unknown");
        let is_intro =
            m.is_license_intro() || m.is_license_clue() || m.license_expression == "free-unknown";
        if has_unknown && is_intro {
            // Check if there's a non-intro, non-unknown match after this
            let has_unknown_intro = matches.iter().any(|other| {
                other.matcher != MatcherKind::Undetected
                    && other.start_line > m.start_line
                    && !other.rule_identifier.contains("unknown")
                    && !other.license_expression.contains("unknown")
                    && !other.is_license_intro()
                    && !other.is_license_clue()
            });

            if has_unknown_intro {
                let coverage_ok = m.coverage() >= IMPERFECT_MATCH_COVERAGE_THR;
                let not_unknown = !m.rule_identifier.contains("unknown")
                    && !m.license_expression.contains("unknown");
                if coverage_ok && not_unknown {
                    return true;
                }
            }
        }
    }

    if matches.iter().any(is_unknown_intro) {
        let filtered_matches = filter_license_intros(matches);
        if filtered_matches.len() != matches.len()
            && is_match_coverage_below_threshold(
                &filtered_matches,
                IMPERFECT_MATCH_COVERAGE_THR,
                false,
            )
        {
            return true;
        }
    }

    false
}

/// Check if a match is an unknown license intro.
///
/// Based on Python: is_unknown_intro() at detection.py:1250-1262
pub(super) fn is_unknown_intro(m: &LicenseMatch) -> bool {
    let has_unknown = m.license_expression.contains("unknown");
    has_unknown
        && (m.is_license_intro() || m.is_license_clue() || m.license_expression == "free-unknown")
}

/// Check if a match should be considered a license intro for filtering.
///
/// A match is considered a license intro if it has is_license_intro or
/// is_license_clue flag set OR its license_expression is "free-unknown",
/// AND it was matched by the "2-aho" matcher OR has 100% match coverage.
///
/// Based on Python: is_license_intro() at detection.py:1349-1365
pub(super) fn is_license_intro(match_item: &LicenseMatch) -> bool {
    (match_item.is_license_intro()
        || match_item.is_license_clue()
        || match_item.license_expression == "free-unknown")
        && (match_item.matcher == MatcherKind::Aho || match_item.coverage() == 100.0)
}

/// Filter out license intro matches from a list of matches.
///
/// Based on Python: filter_license_intros() at detection.py:1368-1383
pub(super) fn filter_license_intros(matches: &[LicenseMatch]) -> Vec<LicenseMatch> {
    matches
        .iter()
        .filter(|m| !is_license_intro(m))
        .cloned()
        .collect()
}

/// Check if a match references a local file.
///
/// Based on Python: is_license_reference_local_file() at detection.py:1368-1377
pub(super) fn is_license_reference_local_file(m: &LicenseMatch) -> bool {
    m.referenced_filenames
        .as_ref()
        .is_some_and(|v| !v.is_empty())
}

/// Filter out license reference matches that point to local files.
///
/// Based on Python: filter_license_references() at detection.py:1404-1419
#[cfg(test)]
pub(super) fn filter_license_references(matches: &[LicenseMatch]) -> Vec<LicenseMatch> {
    matches
        .iter()
        .filter(|m| !is_license_reference_local_file(m))
        .cloned()
        .collect()
}

/// Check if any matches reference local files.
fn has_references_to_local_files(matches: &[LicenseMatch]) -> bool {
    matches.iter().any(is_license_reference_local_file)
}

/// Analyze detection and return detection log message.
///
/// Based on Python: analyze_detection() at detection.py:1445-1561
pub(super) fn analyze_detection(matches: &[LicenseMatch], package_license: bool) -> &'static str {
    if matches.is_empty() {
        return "";
    }

    // Check 1: Undetected matches
    if is_undetected_license_matches(matches) {
        return DETECTION_LOG_UNDETECTED_LICENSE;
    }

    // Check 2: Unknown intro before detection
    if has_unknown_intro_before_detection(matches) {
        return "unknown-intro-followed-by-match";
    }

    // Check 3: References to local files
    if has_references_to_local_files(matches) {
        return "unknown-reference-to-local-file";
    }

    // Check 4: License clues
    //
    // Clue-only matches should remain visible as clues rather than being
    // swallowed by the false-positive branch. This matters for weak GPL
    // shorthand such as bare "GPL", where maintainer sentiment favors
    // preserving the weak evidence without asserting a detected GPL license.
    if !package_license && has_correct_license_clue_matches(matches) {
        return DETECTION_LOG_LICENSE_CLUES;
    }

    // Check 5: False positive (unless package_license is set)
    if !package_license && is_false_positive(matches) {
        return "false-positive";
    }

    // Check 6: Perfect detection (correct AND no unknowns AND no extra words)
    if is_correct_detection_non_unknown(matches) {
        return "";
    }

    // Check 7: Unknown matches
    if has_unknown_matches(matches) {
        return DETECTION_LOG_UNKNOWN_MATCH;
    }

    // Check 8: Low quality matches
    if !package_license && is_low_quality_matches(matches) {
        return "low-quality-match-fragments";
    }

    // Check 9: Imperfect coverage
    if matches
        .iter()
        .any(|m| m.coverage() < IMPERFECT_MATCH_COVERAGE_THR)
    {
        return DETECTION_LOG_IMPERFECT_COVERAGE;
    }

    // Check 10: Extra words
    if has_extra_words(matches) {
        return DETECTION_LOG_EXTRA_WORDS;
    }

    ""
}

fn is_correct_detection_non_unknown(matches: &[LicenseMatch]) -> bool {
    matches.iter().all(|m| m.coverage() == 100.0)
        && !has_unknown_matches(matches)
        && !has_extra_words(matches)
}

/// Compute detection score from matches.
///
/// Based on Python: compute_detection_score() at detection.py:1585-1608
pub fn compute_detection_score(matches: &[LicenseMatch]) -> f32 {
    if matches.is_empty() {
        return 0.0;
    }

    let total_length: f64 = matches.iter().map(|m| m.matched_length as f64).sum();
    if total_length == 0.0 {
        return 0.0;
    }

    let weighted_score: f64 = matches
        .iter()
        .map(|m| m.score.value() * (m.matched_length as f64 / total_length))
        .sum();

    ((weighted_score * 100.0).round() / 100.0).min(100.0) as f32
}

/// Determine license expression from matches.
///
/// Combines license expressions from all matches using AND/OR relationships.
///
/// Based on Python: determine_license_expression() at detection.py:1611-1635
pub fn determine_license_expression(
    matches: &[LicenseMatch],
    source_text: Option<&str>,
) -> Result<String, String> {
    if matches.is_empty() {
        return Err("No matches to determine expression from".to_string());
    }

    if let Some(expr) = determine_alternative_notice_expression(matches, source_text)? {
        return Ok(expr);
    }

    let expressions: Vec<&str> = matches
        .iter()
        .map(|m| m.license_expression.as_str())
        .collect();

    combine_expressions_and_preserving_structure(&expressions, true)
        .map_err(|e| format!("Failed to combine expressions: {}", e))
}

/// Determine SPDX expression from matches.
///
/// Converts license expressions to SPDX identifiers.
///
/// Based on Python: determine_spdx_expression() at detection.py:1638-1671
pub fn determine_spdx_expression(
    matches: &[LicenseMatch],
    source_text: Option<&str>,
) -> Result<String, String> {
    if matches.is_empty() {
        return Err("No matches to determine SPDX expression from".to_string());
    }

    if let Some(expr) = determine_alternative_notice_spdx_expression(matches, source_text)? {
        return Ok(expr);
    }

    let expressions: Option<Vec<&str>> = matches
        .iter()
        .map(|m| m.license_expression_spdx.as_deref())
        .collect();

    let expressions = expressions
        .ok_or_else(|| "Missing SPDX expressions for one or more matches".to_string())?;

    combine_license_expressions_preserving_structure_strict(
        expressions.into_iter().map(str::to_string),
    )
    .ok_or_else(|| "Failed to combine SPDX expressions".to_string())
}

fn determine_alternative_notice_expression(
    matches: &[LicenseMatch],
    source_text: Option<&str>,
) -> Result<Option<String>, String> {
    if !has_alternative_license_notice(matches, source_text) {
        return Ok(None);
    }

    let (substantive, supplemental): (Vec<&LicenseMatch>, Vec<&LicenseMatch>) = matches
        .iter()
        .partition(|m| !is_supplemental_alternative_match(m.license_expression.as_str()));

    if substantive.len() < 2 {
        return Ok(None);
    }

    let alternative_expressions: Vec<&str> = substantive
        .iter()
        .map(|m| m.license_expression.as_str())
        .collect();
    let alternative_expression =
        combine_expressions_or_preserving_structure(&alternative_expressions, true)
            .map_err(|e| format!("Failed to combine alternative expressions: {}", e))?;

    let mut parts = vec![alternative_expression];
    parts.extend(
        supplemental
            .iter()
            .map(|m| m.license_expression.clone())
            .collect::<Vec<_>>(),
    );

    let part_refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    combine_expressions_and_preserving_structure(&part_refs, true)
        .map(Some)
        .map_err(|e| format!("Failed to combine alternative expression parts: {}", e))
}

fn determine_alternative_notice_spdx_expression(
    matches: &[LicenseMatch],
    source_text: Option<&str>,
) -> Result<Option<String>, String> {
    if !has_alternative_license_notice(matches, source_text) {
        return Ok(None);
    }

    let (substantive, supplemental): (Vec<&LicenseMatch>, Vec<&LicenseMatch>) = matches
        .iter()
        .partition(|m| !is_supplemental_alternative_match(m.license_expression.as_str()));

    if substantive.len() < 2 {
        return Ok(None);
    }

    let alternative_expressions: Option<Vec<String>> = substantive
        .iter()
        .map(|m| m.license_expression_spdx.clone())
        .collect();
    let alternative_expressions = alternative_expressions.ok_or_else(|| {
        "Missing SPDX expressions for one or more alternative-license matches".to_string()
    })?;
    let alternative_expression =
        combine_license_expressions_with_relation_preserving_structure_strict(
            alternative_expressions,
            ExpressionRelation::Or,
        )
        .ok_or_else(|| "Failed to combine alternative SPDX expressions".to_string())?;

    let mut parts = vec![alternative_expression];
    let supplemental_expressions: Option<Vec<String>> = supplemental
        .iter()
        .map(|m| m.license_expression_spdx.clone())
        .collect();
    parts.extend(supplemental_expressions.ok_or_else(|| {
        "Missing SPDX expressions for one or more supplemental matches".to_string()
    })?);

    combine_license_expressions_with_relation_preserving_structure_strict(
        parts,
        ExpressionRelation::And,
    )
    .ok_or_else(|| "Failed to combine alternative SPDX expression parts".to_string())
    .map(Some)
}

fn has_alternative_license_notice(matches: &[LicenseMatch], source_text: Option<&str>) -> bool {
    if matches.len() < 2 {
        return false;
    }

    let Some(source_text) = source_text else {
        return false;
    };

    let start_line = matches
        .iter()
        .map(|m| m.start_line)
        .min()
        .map(|ln| ln.get())
        .unwrap_or(0);
    let end_line = matches
        .iter()
        .map(|m| m.end_line)
        .max()
        .map(|ln| ln.get())
        .unwrap_or(0);
    if start_line == 0 || end_line < start_line {
        return false;
    }

    let region = source_text
        .lines()
        .skip(start_line.saturating_sub(1))
        .take(end_line - start_line + 1)
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();

    let python_alternative_notice =
        region.contains("alternatively") && region.contains("may be used under the terms of");
    let rust_dual_license_notice = (region.contains("licensed under either of")
        && region.contains("at your option"))
        || region.contains("dual-licensed under")
        || region.contains("dual licensed under");

    python_alternative_notice || rust_dual_license_notice
}

fn is_supplemental_alternative_match(expression: &str) -> bool {
    expression.contains("warranty-disclaimer")
}

/// Determine SPDX expression from ScanCode license keys.
///
/// Based on Python: determine_spdx_expression_from_scancode() at detection.py:1674-1709
pub fn determine_spdx_expression_from_scancode(
    scancode_expression: &str,
    spdx_mapping: &SpdxMapping,
) -> Result<String, String> {
    if scancode_expression.is_empty() {
        return Ok(String::new());
    }

    spdx_mapping
        .expression_scancode_to_spdx(scancode_expression)
        .map_err(|e| e.to_string())
}
///
/// A detection is valid if:
/// - Score meets minimum threshold
/// - Not identified as low quality matches
/// - Not identified as false positive
///
/// Based on Python: is_correct_detection_non_unknown() at detection.py:1066
pub(super) fn classify_detection(detection: &LicenseDetection, min_score: f32) -> bool {
    if detection.matches.is_empty() {
        return false;
    }

    let score = compute_detection_score(&detection.matches);
    let meets_score_threshold = score >= min_score - 0.01;
    let is_true_license_clue = has_correct_license_clue_matches(&detection.matches);
    let not_false_positive = is_true_license_clue || !is_false_positive(&detection.matches);

    // Python does NOT filter out low-quality matches - it returns them with
    // "low-quality-matches" in detection_log but still includes them.
    // See: detection.py get_detected_license_expression() lines 1565-1571
    meets_score_threshold && not_false_positive
}

#[cfg(test)]
#[path = "analysis_test.rs"]
mod tests;
