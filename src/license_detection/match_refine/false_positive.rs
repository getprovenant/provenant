// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! False positive detection for license matches.
//!
//! This module contains functions for detecting and filtering false positive
//! license matches, particularly those that appear in license lists.

use crate::license_detection::models::{LicenseMatch, MatcherKind};

const MIN_SHORT_FP_LIST_LENGTH: usize = 15;
const MIN_LONG_FP_LIST_LENGTH: usize = 150;
const MIN_UNIQUE_LICENSES: usize = MIN_SHORT_FP_LIST_LENGTH / 3;
const MIN_UNIQUE_LICENSES_PROPORTION: f64 = 1.0 / 3.0;
const MAX_CANDIDATE_LENGTH: usize = 20;
const MAX_DISTANCE_BETWEEN_CANDIDATES: usize = 10;

pub(super) fn is_candidate_false_positive(m: &LicenseMatch) -> bool {
    let is_tag_or_ref = m.is_license_reference()
        || m.is_license_tag()
        || m.is_license_intro()
        || m.is_license_clue();

    let is_not_spdx_id = m.matcher != MatcherKind::SpdxId;
    let is_exact_match = m.coverage() == 100.0;
    let is_short = m.len() <= MAX_CANDIDATE_LENGTH;

    is_tag_or_ref && is_not_spdx_id && is_exact_match && is_short
}

fn count_unique_licenses(matches: &[LicenseMatch]) -> usize {
    std::collections::HashSet::<&str>::from_iter(
        matches.iter().map(|m| m.license_expression.as_str()),
    )
    .len()
}

pub(super) fn is_list_of_false_positives(
    matches: &[LicenseMatch],
    min_matches: usize,
    min_unique_licenses: usize,
    min_unique_licenses_proportion: f64,
    min_candidate_proportion: f64,
) -> bool {
    if matches.is_empty() {
        return false;
    }

    let len_matches = matches.len();

    let len_unique_licenses = count_unique_licenses(matches);
    let unique_proportion = len_unique_licenses as f64 / len_matches as f64;
    let has_enough_licenses = unique_proportion > min_unique_licenses_proportion
        || len_unique_licenses >= min_unique_licenses;

    let has_enough_candidates = min_candidate_proportion <= 0.0 || {
        let candidates_count = matches
            .iter()
            .filter(|m| is_candidate_false_positive(m))
            .count();
        (candidates_count as f64 / len_matches as f64) > min_candidate_proportion
    };

    len_matches >= min_matches && has_enough_licenses && has_enough_candidates
}

/// Filter matches that are likely false positive license lists.
///
/// A false positive license list is a sequence of many short, exact matches
/// to different license references/tags that are likely part of a "choose your
/// license" list or similar UI element, rather than actual license declarations.
///
/// # Arguments
/// * `matches` - Vector of LicenseMatch to filter
///
/// # Returns
/// Matches that are not part of a false positive license list
pub fn filter_false_positive_license_lists_matches(
    matches: Vec<LicenseMatch>,
) -> Vec<LicenseMatch> {
    if matches.len() < MIN_SHORT_FP_LIST_LENGTH {
        return matches;
    }

    if matches.len() > MIN_LONG_FP_LIST_LENGTH
        && is_list_of_false_positives(
            &matches,
            MIN_LONG_FP_LIST_LENGTH,
            MIN_LONG_FP_LIST_LENGTH,
            MIN_UNIQUE_LICENSES_PROPORTION,
            0.95,
        )
    {
        return vec![];
    }

    let mut kept = Vec::new();
    let mut candidate_start: Option<usize> = None;
    let mut candidate_last: usize = 0;

    for (i, match_item) in matches.iter().enumerate() {
        let is_candidate = is_candidate_false_positive(match_item);

        if is_candidate {
            let is_close_enough = candidate_start.is_none_or(|_| {
                matches[candidate_last].qdistance_to(match_item) <= MAX_DISTANCE_BETWEEN_CANDIDATES
            });

            if candidate_start.is_none() || is_close_enough {
                candidate_start.get_or_insert(i);
                candidate_last = i;
            } else {
                flush_candidates(&matches, candidate_start.unwrap(), i, &mut kept);
                candidate_start = Some(i);
                candidate_last = i;
            }
        } else {
            if let Some(start) = candidate_start.take() {
                flush_candidates(&matches, start, i, &mut kept);
            }
            kept.push(match_item.clone());
        }
    }

    if let Some(start) = candidate_start {
        flush_candidates(&matches, start, matches.len(), &mut kept);
    }

    kept
}

fn flush_candidates(
    matches: &[LicenseMatch],
    start: usize,
    end: usize,
    kept: &mut Vec<LicenseMatch>,
) {
    let candidates = &matches[start..end];
    if is_list_of_false_positives(
        candidates,
        MIN_SHORT_FP_LIST_LENGTH,
        MIN_UNIQUE_LICENSES,
        MIN_UNIQUE_LICENSES_PROPORTION,
        0.0,
    ) {
        // discard all candidates
    } else {
        kept.extend(candidates.iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_detection::models::{MatchCoordinates, PositionSpan, RuleId};
    use crate::models::LineNumber;
    use crate::models::MatchScore;

    // Test helper that sets every match field explicitly; the long argument list is intentional.
    #[allow(clippy::too_many_arguments)]
    fn create_test_match_with_flags(
        rule_identifier: &str,
        start_line: usize,
        end_line: usize,
        is_license_reference: bool,
        is_license_tag: bool,
        is_license_intro: bool,
        is_license_clue: bool,
        matcher: &str,
        match_coverage: f32,
        matched_length: usize,
        rule_length: usize,
        license_expression: &str,
    ) -> LicenseMatch {
        let rid = rule_identifier.trim_start_matches('#').parse().unwrap_or(0);
        LicenseMatch {
            rid: RuleId::new(rid),
            license_expression: license_expression.to_string(),
            license_expression_spdx: Some(license_expression.to_string()),
            from_file: None,
            start_line: LineNumber::new(start_line).unwrap(),
            end_line: LineNumber::new(end_line).unwrap(),
            start_token: 0,
            end_token: 0,
            matcher: matcher.parse().expect("invalid test matcher"),
            score: MatchScore::MAX,
            matched_length,
            rule_length,
            match_coverage,
            rule_relevance: 100,
            rule_identifier: rule_identifier.to_string(),
            rule_url: String::new(),
            matched_text: None,
            referenced_filenames: None,
            rule_kind: crate::license_detection::models::RuleKind::from_rule_flags(
                false,
                false,
                is_license_reference,
                is_license_tag,
                is_license_intro,
                is_license_clue,
            )
            .unwrap(),
            is_from_license: false,
            rule_start_token: 0,
            coordinates: MatchCoordinates::query_region(PositionSpan::range(0, matched_length)),
        }
    }

    #[test]
    fn test_is_candidate_false_positive_tag_match() {
        let m = create_test_match_with_flags(
            "#1", 1, 1, false, true, false, false, "2-aho", 100.0, 5, 5, "mit",
        );
        assert!(is_candidate_false_positive(&m));
    }

    #[test]
    fn test_is_candidate_false_positive_reference_match() {
        let m = create_test_match_with_flags(
            "#2",
            1,
            1,
            true,
            false,
            false,
            false,
            "2-aho",
            100.0,
            3,
            3,
            "apache-2.0",
        );
        assert!(is_candidate_false_positive(&m));
    }

    #[test]
    fn test_is_candidate_false_positive_spdx_id_excluded() {
        let m = create_test_match_with_flags(
            "#3",
            1,
            1,
            true,
            false,
            false,
            false,
            "1-spdx-id",
            100.0,
            3,
            3,
            "mit",
        );
        assert!(!is_candidate_false_positive(&m));
    }

    #[test]
    fn test_is_candidate_false_partial_coverage_excluded() {
        let m = create_test_match_with_flags(
            "#4", 1, 1, true, false, false, false, "2-aho", 80.0, 5, 5, "mit",
        );
        assert!(!is_candidate_false_positive(&m));
    }

    #[test]
    fn test_is_candidate_false_long_match_excluded() {
        let m = create_test_match_with_flags(
            "#5", 1, 1, true, false, false, false, "2-aho", 100.0, 25, 25, "mit",
        );
        assert!(!is_candidate_false_positive(&m));
    }

    #[test]
    fn test_filter_short_list_not_filtered() {
        let matches: Vec<LicenseMatch> = (0..10)
            .map(|i| {
                create_test_match_with_flags(
                    &format!("#{}", i),
                    i + 1,
                    i + 1,
                    true,
                    false,
                    false,
                    false,
                    "2-aho",
                    100.0,
                    3,
                    3,
                    &format!("license-{}", i),
                )
            })
            .collect();

        let kept = filter_false_positive_license_lists_matches(matches);
        assert_eq!(kept.len(), 10);
    }

    #[test]
    fn test_filter_long_list_all_candidates() {
        let matches: Vec<LicenseMatch> = (0..160)
            .map(|i| {
                create_test_match_with_flags(
                    &format!("#{}", i),
                    i + 1,
                    i + 1,
                    true,
                    false,
                    false,
                    false,
                    "2-aho",
                    100.0,
                    3,
                    3,
                    &format!("license-{}", i),
                )
            })
            .collect();

        let kept = filter_false_positive_license_lists_matches(matches);
        assert_eq!(kept.len(), 0);
    }

    #[test]
    fn test_filter_mixed_list_keeps_non_candidates() {
        let mut matches = Vec::new();

        for i in 0..15 {
            matches.push(create_test_match_with_flags(
                &format!("#{}", i),
                i + 1,
                i + 1,
                true,
                false,
                false,
                false,
                "2-aho",
                100.0,
                3,
                3,
                &format!("license-{}", i),
            ));
        }

        for i in 0..5 {
            matches.push(create_test_match_with_flags(
                &format!("#{}", 100 + i),
                100 + i,
                100 + i + 20,
                false,
                false,
                false,
                false,
                "2-aho",
                100.0,
                100,
                100,
                "gpl-3.0",
            ));
        }

        let kept = filter_false_positive_license_lists_matches(matches);

        assert_eq!(kept.len(), 5);
    }

    #[test]
    fn test_filter_candidates_with_real_license() {
        let mut matches = Vec::new();

        for i in 0..15 {
            matches.push(create_test_match_with_flags(
                &format!("#{}", i),
                i + 1,
                i + 1,
                true,
                false,
                false,
                false,
                "2-aho",
                100.0,
                3,
                3,
                &format!("license-{}", i),
            ));
        }

        matches.push(create_test_match_with_flags(
            "#real", 100, 150, false, false, false, false, "2-aho", 100.0, 200, 200, "mit",
        ));

        for i in 0..15 {
            matches.push(create_test_match_with_flags(
                &format!("#{}", 200 + i),
                200 + i,
                200 + i,
                true,
                false,
                false,
                false,
                "2-aho",
                100.0,
                3,
                3,
                &format!("license-{}", 200 + i),
            ));
        }

        let kept = filter_false_positive_license_lists_matches(matches);

        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn test_count_unique_licenses() {
        let matches = vec![
            create_test_match_with_flags(
                "#1", 1, 1, false, false, false, false, "2-aho", 100.0, 5, 5, "mit",
            ),
            create_test_match_with_flags(
                "#2", 2, 2, false, false, false, false, "2-aho", 100.0, 5, 5, "mit",
            ),
            create_test_match_with_flags(
                "#3",
                3,
                3,
                false,
                false,
                false,
                false,
                "2-aho",
                100.0,
                5,
                5,
                "apache-2.0",
            ),
        ];
        assert_eq!(count_unique_licenses(&matches), 2);
    }

    #[test]
    fn test_min_unique_licenses_fallback() {
        let matches: Vec<LicenseMatch> = (0..20)
            .map(|i| {
                let mut m = LicenseMatch {
                    license_expression: format!("license-{}", i % 4),
                    matcher: crate::license_detection::models::MatcherKind::Aho,
                    matched_length: 10,
                    match_coverage: 100.0,
                    rule_relevance: 100,
                    rule_identifier: "#1".to_string(),
                    ..LicenseMatch::default()
                };
                m.rule_kind = crate::license_detection::models::RuleKind::Reference;
                m
            })
            .collect();

        assert!(is_list_of_false_positives(&matches, 15, 3, 1.0 / 3.0, 0.0));
        assert!(!is_list_of_false_positives(&matches, 15, 5, 1.0 / 3.0, 0.0));
    }
}
