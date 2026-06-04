// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::license_detection::models::{LicenseMatch, MatchCoordinates, MatcherKind, PositionSpan};
use crate::models::LineNumber;
use crate::models::MatchScore;

fn create_test_match(coverage: f32, rule_identifier: &str) -> LicenseMatch {
    LicenseMatch {
        rid: crate::license_detection::models::RuleId::NONE,
        license_expression: "mit".to_string(),
        license_expression_spdx: Some("MIT".to_string()),
        from_file: Some("test.txt".to_string()),
        start_line: LineNumber::ONE,
        end_line: LineNumber::new(10).expect("valid line number"),
        start_token: 1,
        end_token: 11,
        matcher: crate::license_detection::models::MatcherKind::Hash,
        score: MatchScore::from_percentage(95.0),
        matched_length: 100,
        match_coverage: coverage,
        rule_relevance: 100,
        rule_identifier: rule_identifier.to_string(),
        rule_url: "https://example.com".to_string(),
        matched_text: Some("MIT License".to_string()),
        referenced_filenames: None,
        rule_kind: crate::license_detection::models::RuleKind::None,
        is_from_license: false,
        rule_length: 100,
        rule_start_token: 0,
        coordinates: MatchCoordinates::query_region(PositionSpan::range(1, 11)),
        candidate_resemblance: 0.0,
        candidate_containment: 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn create_test_match_full(
    license_expression: &str,
    matcher: &str,
    start_line: usize,
    end_line: usize,
    score: MatchScore,
    matched_length: usize,
    rule_length: usize,
    match_coverage: f32,
    rule_relevance: u8,
    rule_identifier: &str,
) -> LicenseMatch {
    let start_line = LineNumber::new(start_line).expect("valid start_line");
    let end_line = LineNumber::new(end_line).expect("valid end_line");
    LicenseMatch {
        rid: crate::license_detection::models::RuleId::NONE,
        license_expression: license_expression.to_string(),
        license_expression_spdx: Some(license_expression.to_string()),
        from_file: Some("test.txt".to_string()),
        start_line,
        end_line,
        start_token: start_line.get(),
        end_token: end_line.get() + 1,
        matcher: matcher.parse().expect("invalid test matcher"),
        score,
        matched_length,
        rule_length,
        match_coverage,
        rule_relevance,
        rule_identifier: rule_identifier.to_string(),
        rule_url: "https://example.com".to_string(),
        matched_text: Some("License text".to_string()),
        referenced_filenames: None,
        rule_kind: crate::license_detection::models::RuleKind::None,
        is_from_license: false,
        rule_start_token: 0,
        coordinates: MatchCoordinates::query_region(PositionSpan::range(
            start_line.get(),
            end_line.get() + 1,
        )),
        candidate_resemblance: 0.0,
        candidate_containment: 0.0,
    }
}

#[test]
fn test_is_match_coverage_below_threshold_above() {
    let matches = vec![create_test_match(95.0, "mit.LICENSE")];
    assert!(!is_match_coverage_below_threshold(&matches, 70.0, true));
}

#[test]
fn test_is_match_coverage_below_threshold_below() {
    let matches = vec![create_test_match(65.0, "mit.LICENSE")];
    assert!(is_match_coverage_below_threshold(&matches, 70.0, true));
}

#[test]
fn test_is_match_coverage_below_threshold_exact() {
    let matches = vec![create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::from_percentage(60.0),
        100,
        100,
        60.0,
        100,
        "mit.LICENSE",
    )];
    assert!(!is_match_coverage_below_threshold(&matches, 60.0, true));
}

#[test]
fn test_is_match_coverage_below_threshold_uses_rounded_coverage() {
    let matches = vec![create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::from_percentage(80.0),
        100,
        100,
        79.996,
        100,
        "mit.LICENSE",
    )];
    assert!(!is_match_coverage_below_threshold(&matches, 80.0, true));
}

#[test]
fn test_has_correct_license_clue_matches_requires_full_coverage() {
    let mut m = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::MAX,
        100,
        100,
        99.99,
        100,
        "mit.LICENSE",
    );
    m.rule_kind = crate::license_detection::models::RuleKind::Clue;

    assert!(!has_correct_license_clue_matches(&[m]));
}

#[test]
fn test_is_match_coverage_below_threshold_empty() {
    let matches: Vec<LicenseMatch> = vec![];
    assert!(!is_match_coverage_below_threshold(&matches, 70.0, true));
}

#[test]
fn test_has_unknown_matches_false() {
    let matches = vec![create_test_match(95.0, "mit.LICENSE")];
    assert!(!has_unknown_matches(&matches));
}

#[test]
fn test_has_unknown_matches_true_in_identifier() {
    let matches = vec![create_test_match(95.0, "unknown.LICENSE")];
    assert!(has_unknown_matches(&matches));
}

#[test]
fn test_has_unknown_matches_true_in_expression() {
    let mut m = create_test_match(95.0, "mit.LICENSE");
    m.license_expression = "unknown".to_string();
    let matches = vec![m];
    assert!(has_unknown_matches(&matches));
}

#[test]
fn test_has_extra_words_false() {
    let matches = vec![create_test_match(95.0, "mit.LICENSE")];
    assert!(!has_extra_words(&matches));
}

#[test]
fn test_has_extra_words_true() {
    let mut m = create_test_match(95.0, "mit.LICENSE");
    m.score = MatchScore::from_percentage(50.0);
    let matches = vec![m];
    assert!(has_extra_words(&matches));
}

#[test]
fn test_is_false_positive_empty() {
    let matches: Vec<LicenseMatch> = vec![];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_perfect_match() {
    let matches = vec![create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::from_percentage(95.0),
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    )];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_bare_single() {
    let matches = vec![create_test_match_full(
        "gpl",
        "2-aho",
        2000,
        2005,
        MatchScore::from_percentage(30.0),
        3,
        3,
        30.0,
        50,
        "gpl_bare.LICENSE",
    )];
    assert!(is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_gpl_short() {
    let matches = vec![create_test_match_full(
        "gpl-2.0",
        "2-aho",
        1,
        10,
        MatchScore::from_percentage(50.0),
        2,
        1,
        50.0,
        50,
        "gpl-2.0.LICENSE",
    )];
    assert!(is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_lgpl_short_not_filtered() {
    let matches = vec![create_test_match_full(
        "lgpl-2.0-plus",
        "2-aho",
        6,
        8,
        MatchScore::from_percentage(50.0),
        1,
        1,
        100.0,
        60,
        "lgpl_bare_single_word.RULE",
    )];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_late_short_low_relevance() {
    let matches = vec![create_test_match_full(
        "mit",
        "2-aho",
        1500,
        1505,
        MatchScore::from_percentage(30.0),
        3,
        1,
        30.0,
        50,
        "mit.LICENSE",
    )];
    assert!(is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_late_exact_spdx_id_rule_not_filtered() {
    let matches = vec![create_test_match_full(
        "mpl-2.0",
        "2-aho",
        1500,
        1503,
        MatchScore::from_percentage(50.0),
        3,
        3,
        100.0,
        50,
        "spdx_license_id_mpl-2.0_for_mpl-2.0.RULE",
    )];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_single_license_reference_short() {
    let mut m = create_test_match_full(
        "borceux",
        "2-aho",
        1,
        10,
        MatchScore::MAX,
        1,
        1,
        100.0,
        80,
        "borceux.LICENSE",
    );
    m.rule_kind = crate::license_detection::models::RuleKind::Reference;
    let matches = vec![m];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_single_license_reference_long_rule() {
    let mut m = create_test_match_full(
        "some-license",
        "2-aho",
        1,
        10,
        MatchScore::MAX,
        10,
        10,
        100.0,
        80,
        "some-license.LICENSE",
    );
    m.rule_kind = crate::license_detection::models::RuleKind::Reference;
    let matches = vec![m];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_single_license_reference_full_relevance() {
    let mut m = create_test_match_full(
        "some-license",
        "2-aho",
        1,
        10,
        MatchScore::MAX,
        1,
        1,
        100.0,
        100,
        "some-license.LICENSE",
    );
    m.rule_kind = crate::license_detection::models::RuleKind::Reference;
    let matches = vec![m];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_with_copyright_word() {
    let mut m = create_test_match_full(
        "gpl-2.0",
        "2-aho",
        1,
        10,
        MatchScore::from_percentage(50.0),
        100,
        1,
        50.0,
        50,
        "gpl-2.0.LICENSE",
    );
    m.matched_text = Some("This is copyrighted material under GPL".to_string());
    let matches = vec![m];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_with_c_symbol() {
    let mut m = create_test_match_full(
        "mit",
        "2-aho",
        1500,
        1510,
        MatchScore::from_percentage(30.0),
        10,
        2,
        30.0,
        50,
        "mit.RULE",
    );
    m.matched_text = Some("Licensed under MIT (c) 2024".to_string());
    let matches = vec![m];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_without_copyright_word() {
    let mut m = create_test_match_full(
        "gpl-2.0",
        "2-aho",
        1,
        10,
        MatchScore::from_percentage(50.0),
        5,
        1,
        50.0,
        50,
        "gpl-2.0.LICENSE",
    );
    m.matched_text = Some("GPL licensed software".to_string());
    let matches = vec![m];
    assert!(is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_partial_copyright() {
    let mut m1 = create_test_match_full(
        "gpl-2.0",
        "2-aho",
        1,
        5,
        MatchScore::from_percentage(50.0),
        10,
        1,
        50.0,
        50,
        "gpl-2.0.LICENSE",
    );
    m1.matched_text = Some("Copyright GPL".to_string());
    let mut m2 = create_test_match_full(
        "gpl-2.0",
        "2-aho",
        6,
        10,
        MatchScore::from_percentage(50.0),
        10,
        1,
        50.0,
        50,
        "gpl-2.0.LICENSE",
    );
    m2.matched_text = Some("GPL licensed".to_string());
    let matches = vec![m1, m2];
    assert!(is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_all_matches_with_copyright() {
    let mut m1 = create_test_match_full(
        "gpl-2.0",
        "2-aho",
        1,
        5,
        MatchScore::from_percentage(50.0),
        10,
        1,
        50.0,
        50,
        "gpl-2.0.LICENSE",
    );
    m1.matched_text = Some("Copyright GPL".to_string());
    let mut m2 = create_test_match_full(
        "gpl-2.0",
        "2-aho",
        6,
        10,
        MatchScore::from_percentage(50.0),
        10,
        1,
        50.0,
        50,
        "gpl-2.0.LICENSE",
    );
    m2.matched_text = Some("(c) GPL".to_string());
    let matches = vec![m1, m2];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_matched_text_none() {
    let mut m = create_test_match_full(
        "gpl-2.0",
        "2-aho",
        1,
        10,
        MatchScore::from_percentage(50.0),
        5,
        1,
        50.0,
        50,
        "gpl-2.0.LICENSE",
    );
    m.matched_text = None;
    let matches = vec![m];
    assert!(is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_copyright_case_insensitive() {
    let mut m = create_test_match_full(
        "mit",
        "2-aho",
        1,
        10,
        MatchScore::from_percentage(50.0),
        10,
        1,
        50.0,
        50,
        "mit.RULE",
    );
    m.matched_text = Some("COPYRIGHT HOLDER NAME".to_string());
    let matches = vec![m];
    assert!(!is_false_positive(&matches));
}

#[test]
fn test_is_false_positive_copyright_empty_string() {
    let mut m = create_test_match_full(
        "gpl-2.0",
        "2-aho",
        1,
        10,
        MatchScore::from_percentage(50.0),
        5,
        1,
        50.0,
        50,
        "gpl-2.0.LICENSE",
    );
    m.matched_text = Some("".to_string());
    let matches = vec![m];
    assert!(is_false_positive(&matches));
}

#[test]
fn test_is_low_quality_matches_low_coverage() {
    let matches = vec![create_test_match_full(
        "mit",
        "2-aho",
        1,
        10,
        MatchScore::from_percentage(40.0),
        20,
        20,
        40.0,
        80,
        "mit.LICENSE",
    )];
    assert!(is_low_quality_matches(&matches));
}

#[test]
fn test_is_low_quality_matches_false_perfect() {
    let matches = vec![create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::from_percentage(95.0),
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    )];
    assert!(!is_low_quality_matches(&matches));
}

#[test]
fn test_is_low_quality_matches_empty() {
    let matches: Vec<LicenseMatch> = vec![];
    assert!(is_low_quality_matches(&matches));
}

#[test]
fn test_compute_detection_score_single() {
    let matches = vec![create_test_match(95.0, "mit.LICENSE")];
    let score = compute_detection_score(&matches);
    assert!(score > 90.0);
}

#[test]
fn test_compute_detection_score_multiple_equal() {
    let m1 = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::from_percentage(90.0),
        100,
        100,
        90.0,
        100,
        "mit.LICENSE",
    );
    let m2 = create_test_match_full(
        "mit",
        "1-hash",
        11,
        20,
        MatchScore::from_percentage(90.0),
        100,
        100,
        90.0,
        100,
        "mit.LICENSE",
    );
    let matches = vec![m1, m2];
    let score = compute_detection_score(&matches);
    assert!((score - 90.0).abs() < 0.1);
}

#[test]
fn test_compute_detection_score_weighted() {
    let m1 = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::from_percentage(80.0),
        100,
        100,
        80.0,
        100,
        "mit.LICENSE",
    );
    let m2 = create_test_match_full(
        "mit",
        "1-hash",
        11,
        20,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );
    let matches = vec![m1, m2];
    let score = compute_detection_score(&matches);
    assert!(score > 80.0 && score < 100.0);
}

#[test]
fn test_compute_detection_score_empty() {
    let matches: Vec<LicenseMatch> = vec![];
    let score = compute_detection_score(&matches);
    assert_eq!(score, 0.0);
}

#[test]
fn test_compute_detection_score_capped_at_100() {
    let m = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );
    let matches = vec![m];
    let score = compute_detection_score(&matches);
    assert_eq!(score, 100.0);
}

#[test]
fn test_compute_detection_score_weights_by_match_length_only() {
    let m1 = create_test_match_full(
        "mit",
        "1-hash",
        1,
        100,
        MatchScore::from_percentage(50.0),
        100,
        100,
        100.0,
        50,
        "mit.LICENSE",
    );
    let m2 = create_test_match_full(
        "apache-2.0",
        "1-hash",
        101,
        200,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "apache.LICENSE",
    );

    let score = compute_detection_score(&[m1, m2]);
    assert_eq!(score, 75.0);
}

#[test]
fn test_compute_detection_score_rounds_to_two_decimals() {
    let m1 = create_test_match_full(
        "mit",
        "1-hash",
        1,
        100,
        MatchScore::from_percentage(80.0),
        100,
        100,
        20.0,
        100,
        "mit.LICENSE",
    );
    let m2 = create_test_match_full(
        "apache-2.0",
        "1-hash",
        101,
        110,
        MatchScore::MAX,
        10,
        100,
        100.0,
        100,
        "apache.LICENSE",
    );

    let score = compute_detection_score(&[m1, m2]);
    assert_eq!(score, 81.82);
}

#[test]
fn test_determine_license_expression_single() {
    let matches = vec![create_test_match(95.0, "mit.LICENSE")];
    let result = determine_license_expression(&matches, None);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "mit");
}

#[test]
fn test_determine_license_expression_multiple() {
    let m1 = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );
    let mut m2 = create_test_match_full(
        "apache-2.0",
        "1-hash",
        11,
        20,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "apache.LICENSE",
    );
    m2.license_expression = "apache-2.0".to_string();
    let matches = vec![m1, m2];
    let result = determine_license_expression(&matches, None);
    assert!(result.is_ok());
    let expr = result.unwrap();
    assert!(expr.contains("mit"));
    assert!(expr.contains("apache-2.0"));
}

#[test]
fn test_determine_license_expression_flattens_same_operator_chain() {
    let expressions = [
        "apache-2.0",
        "bsd-3-clause",
        "gpl-2.0-only",
        "licenseref-scancode-oracle-openjdk-exception-2.0",
        "apsl-1.0",
        "apsl-2.0",
    ];

    let matches = expressions
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            let mut license_match = create_test_match_full(
                expression,
                "1-hash",
                index + 1,
                index + 1,
                MatchScore::MAX,
                100,
                100,
                100.0,
                100,
                expression,
            );
            license_match.license_expression = (*expression).to_string();
            license_match
        })
        .collect::<Vec<_>>();

    let result = determine_license_expression(&matches, None);

    assert_eq!(
        result.as_deref(),
        Ok(
            "apache-2.0 AND bsd-3-clause AND gpl-2.0-only AND licenseref-scancode-oracle-openjdk-exception-2.0 AND apsl-1.0 AND apsl-2.0"
        )
    );
}

#[test]
fn test_determine_license_expression_preserves_distinct_nested_operands() {
    let m1 = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );
    let mut m2 = create_test_match_full(
        "mit",
        "1-hash",
        11,
        20,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "apache.LICENSE",
    );
    m2.license_expression = "apache-2.0 OR mit".to_string();

    let result = determine_license_expression(&[m1, m2], None);

    assert_eq!(result.as_deref(), Ok("mit AND (apache-2.0 OR mit)"));
}

#[test]
fn test_determine_license_expression_empty() {
    let matches: Vec<LicenseMatch> = vec![];
    let result = determine_license_expression(&matches, None);
    assert!(result.is_err());
}

#[test]
fn test_determine_license_expression_uses_or_for_alternative_notice() {
    let mut apache = create_test_match_full(
        "apache-2.0",
        "3-seq",
        3,
        7,
        MatchScore::from_percentage(93.75),
        15,
        16,
        93.75,
        100,
        "apache-2.0_910.RULE",
    );
    apache.license_expression = "apache-2.0".to_string();

    let mut boost = create_test_match_full(
        "boost-1.0",
        "3-seq",
        10,
        12,
        MatchScore::from_percentage(51.43),
        18,
        35,
        51.43,
        100,
        "boost-1.0_60.RULE",
    );
    boost.license_expression = "boost-1.0".to_string();

    let fixture =
        include_str!("../../../testdata/license-golden/datadriven/external/boost-json-d2s.ipp");
    let result = determine_license_expression(&[apache, boost], Some(fixture));

    assert_eq!(result.as_deref(), Ok("apache-2.0 OR boost-1.0"));
}

#[test]
fn test_determine_spdx_expression_uses_or_for_alternative_notice_with_disclaimer() {
    let mut apache = create_test_match_full(
        "apache-2.0",
        "3-seq",
        3,
        7,
        MatchScore::from_percentage(93.75),
        15,
        16,
        93.75,
        100,
        "apache-2.0_910.RULE",
    );
    apache.license_expression = "apache-2.0".to_string();
    apache.license_expression_spdx = Some("Apache-2.0".to_string());

    let mut boost = create_test_match_full(
        "boost-1.0",
        "3-seq",
        10,
        12,
        MatchScore::from_percentage(51.43),
        18,
        35,
        51.43,
        100,
        "boost-1.0_60.RULE",
    );
    boost.license_expression = "boost-1.0".to_string();
    boost.license_expression_spdx = Some("BSL-1.0".to_string());

    let mut disclaimer = create_test_match_full(
        "warranty-disclaimer",
        "3-seq",
        14,
        16,
        MatchScore::from_percentage(57.45),
        27,
        47,
        57.45,
        100,
        "warranty-disclaimer_18.RULE",
    );
    disclaimer.license_expression = "warranty-disclaimer".to_string();
    disclaimer.license_expression_spdx =
        Some("LicenseRef-scancode-warranty-disclaimer".to_string());

    let fixture =
        include_str!("../../../testdata/license-golden/datadriven/external/boost-json-d2s.ipp");
    let result = determine_spdx_expression(&[apache, boost, disclaimer], Some(fixture));

    assert_eq!(
        result.as_deref(),
        Ok("(Apache-2.0 OR BSL-1.0) AND LicenseRef-scancode-warranty-disclaimer")
    );
}

#[test]
fn test_determine_spdx_expression_uses_or_for_rust_dual_license_notice() {
    let mut apache_choice = create_test_match_full(
        "apache-2.0",
        "3-seq",
        3,
        3,
        MatchScore::from_percentage(100.0),
        16,
        16,
        100.0,
        100,
        "apache-2.0_910.RULE",
    );
    apache_choice.license_expression = "apache-2.0".to_string();
    apache_choice.license_expression_spdx = Some("Apache-2.0".to_string());

    let mut mit_choice = create_test_match_full(
        "mit",
        "3-seq",
        4,
        4,
        MatchScore::from_percentage(100.0),
        3,
        3,
        100.0,
        100,
        "mit_14.RULE",
    );
    mit_choice.license_expression = "mit".to_string();
    mit_choice.license_expression_spdx = Some("MIT".to_string());

    let mut apache_reference = create_test_match_full(
        "apache-2.0",
        "2-aho",
        10,
        10,
        MatchScore::from_percentage(100.0),
        2,
        2,
        100.0,
        100,
        "apache-2.0_57.RULE",
    );
    apache_reference.license_expression = "apache-2.0".to_string();
    apache_reference.license_expression_spdx = Some("Apache-2.0".to_string());

    let notice = concat!(
        "## License\n\n",
        "Licensed under either of:\n\n",
        " * Apache License, Version 2.0\n",
        " * MIT license\n\n",
        "at your option.\n\n",
        "### Contribution\n\n",
        "Unless you explicitly state otherwise, any contribution intentionally submitted\n",
        "for inclusion in the work by you, as defined in the Apache-2.0 license, shall be\n",
        "dual licensed as above, without any additional terms or conditions.\n",
    );

    let result =
        determine_spdx_expression(&[apache_choice, mit_choice, apache_reference], Some(notice));

    assert!(
        matches!(
            result.as_deref(),
            Ok("Apache-2.0 OR MIT") | Ok("MIT OR Apache-2.0")
        ),
        "result: {result:?}"
    );
}

#[test]
fn test_determine_spdx_expression_uses_or_for_dual_licensed_under_notice() {
    let mut mit_choice = create_test_match_full(
        "mit",
        "3-seq",
        3,
        3,
        MatchScore::from_percentage(100.0),
        3,
        3,
        100.0,
        100,
        "mit_14.RULE",
    );
    mit_choice.license_expression = "mit".to_string();
    mit_choice.license_expression_spdx = Some("MIT".to_string());

    let mut mit_or_apache = create_test_match_full(
        "mit OR apache-2.0",
        "3-seq",
        3,
        3,
        MatchScore::from_percentage(85.71),
        6,
        6,
        100.0,
        100,
        "mit_or_apache-2.0_22.RULE",
    );
    mit_or_apache.license_expression = "mit OR apache-2.0".to_string();
    mit_or_apache.license_expression_spdx = Some("MIT OR Apache-2.0".to_string());

    let notice = concat!(
        "## License\n\n",
        "This project is dual-licensed under MIT and Apache 2.0.\n",
    );

    let result = determine_spdx_expression(&[mit_choice, mit_or_apache], Some(notice));

    assert!(
        matches!(
            result.as_deref(),
            Ok("Apache-2.0 OR MIT") | Ok("MIT OR Apache-2.0")
        ),
        "result: {result:?}"
    );
}

#[test]
fn test_classify_detection_valid_perfect() {
    let m = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::from_percentage(95.0),
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );
    let detection = LicenseDetection {
        license_expression: Some("mit".to_string()),
        license_expression_spdx: Some("MIT".to_string()),
        matches: vec![m],
        detection_log: vec!["perfect-detection".to_string()],
        identifier: None,
    };
    assert!(classify_detection(&detection, 0.0));
}

#[test]
fn test_classify_detection_invalid_low_score() {
    let m = create_test_match_full(
        "mit",
        "2-aho",
        1,
        10,
        MatchScore::from_percentage(30.0),
        100,
        100,
        30.0,
        50,
        "mit.LICENSE",
    );
    let detection = LicenseDetection {
        license_expression: Some("mit".to_string()),
        license_expression_spdx: Some("MIT".to_string()),
        matches: vec![m],
        detection_log: vec![],
        identifier: None,
    };
    assert!(!classify_detection(&detection, 50.0));
}

#[test]
fn test_classify_detection_invalid_false_positive() {
    let m = create_test_match_full(
        "gpl",
        "2-aho",
        2000,
        2005,
        MatchScore::from_percentage(30.0),
        3,
        3,
        30.0,
        50,
        "gpl_bare.LICENSE",
    );
    let detection = LicenseDetection {
        license_expression: Some("gpl".to_string()),
        license_expression_spdx: Some("GPL".to_string()),
        matches: vec![m],
        detection_log: vec![],
        identifier: None,
    };
    assert!(!classify_detection(&detection, 0.0));
}

#[test]
fn test_classify_detection_keeps_true_license_clue_even_if_false_positive_heuristic_matches() {
    let mut m = create_test_match_full(
        "gpl-1.0-plus",
        "2-aho",
        1,
        1,
        MatchScore::MAX,
        1,
        1,
        100.0,
        50,
        "gpl_bare_word_only.RULE",
    );
    m.rule_kind = crate::license_detection::models::RuleKind::Clue;

    let detection = LicenseDetection {
        license_expression: None,
        license_expression_spdx: None,
        matches: vec![m],
        detection_log: vec![DETECTION_LOG_LICENSE_CLUES.to_string()],
        identifier: None,
    };

    assert!(classify_detection(&detection, 0.0));
}

#[test]
fn test_classify_detection_invalid_empty() {
    let detection = LicenseDetection {
        license_expression: None,
        license_expression_spdx: None,
        matches: vec![],
        detection_log: vec![],
        identifier: None,
    };
    assert!(!classify_detection(&detection, 0.0));
}

#[test]
fn test_classify_detection_score_threshold() {
    let m = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::from_percentage(45.0),
        100,
        100,
        45.0,
        100,
        "mit.LICENSE",
    );
    let detection = LicenseDetection {
        license_expression: Some("mit".to_string()),
        license_expression_spdx: Some("MIT".to_string()),
        matches: vec![m],
        detection_log: vec![],
        identifier: None,
    };
    assert!(classify_detection(&detection, 45.0));
    assert!(!classify_detection(&detection, 50.0));
}

#[test]
fn test_classify_detection_perfect_matches() {
    let m = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );
    let detection = LicenseDetection {
        license_expression: Some("mit".to_string()),
        license_expression_spdx: Some("MIT".to_string()),
        matches: vec![m],
        detection_log: vec!["perfect-detection".to_string()],
        identifier: None,
    };
    assert!(classify_detection(&detection, 0.0));
}

#[test]
fn test_determine_spdx_expression_single() {
    let matches = vec![create_test_match(95.0, "mit.LICENSE")];
    let result = determine_spdx_expression(&matches, None);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "MIT");
}

#[test]
fn test_determine_spdx_expression_multiple() {
    let m1 = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );
    let mut m2 = create_test_match_full(
        "apache-2.0",
        "1-hash",
        11,
        20,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "apache.LICENSE",
    );
    m2.license_expression_spdx = Some("Apache-2.0".to_string());
    let matches = vec![m1, m2];
    let result = determine_spdx_expression(&matches, None);
    assert!(result.is_ok());
}

#[test]
fn test_determine_spdx_expression_preserves_distinct_nested_operands() {
    let mut m1 = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );
    m1.license_expression_spdx = Some("MIT".to_string());

    let mut m2 = create_test_match_full(
        "mit",
        "1-hash",
        11,
        20,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "apache.LICENSE",
    );
    m2.license_expression_spdx = Some("Apache-2.0 OR MIT".to_string());

    let result = determine_spdx_expression(&[m1, m2], None);

    assert_eq!(result.as_deref(), Ok("MIT AND (Apache-2.0 OR MIT)"));
}

#[test]
fn test_determine_spdx_expression_empty() {
    let matches: Vec<LicenseMatch> = vec![];
    let result = determine_spdx_expression(&matches, None);
    assert!(result.is_err());
}

#[test]
fn test_is_undetected_license_matches_single_undetected() {
    let mut m = create_test_match(100.0, "mit.LICENSE");
    m.matcher = crate::license_detection::models::MatcherKind::Undetected;
    let matches = vec![m];
    assert!(is_undetected_license_matches(&matches));
}

#[test]
fn test_is_undetected_license_matches_wrong_matcher() {
    let matches = vec![create_test_match(100.0, "mit.LICENSE")];
    assert!(!is_undetected_license_matches(&matches));
}

#[test]
fn test_is_undetected_license_matches_multiple() {
    let mut m1 = create_test_match(100.0, "mit.LICENSE");
    m1.matcher = crate::license_detection::models::MatcherKind::Undetected;
    let mut m2 = create_test_match(100.0, "apache.LICENSE");
    m2.matcher = crate::license_detection::models::MatcherKind::Undetected;
    let matches = vec![m1, m2];
    assert!(is_undetected_license_matches(&matches));
}

#[test]
fn test_is_undetected_license_matches_empty() {
    let matches: Vec<LicenseMatch> = vec![];
    assert!(!is_undetected_license_matches(&matches));
}

#[test]
fn test_analyze_detection_undetected() {
    let mut m = create_test_match(100.0, "mit.LICENSE");
    m.matcher = crate::license_detection::models::MatcherKind::Undetected;
    let matches = vec![m];
    assert_eq!(
        analyze_detection(&matches, false),
        DETECTION_LOG_UNDETECTED_LICENSE
    );
}

#[test]
fn test_analyze_detection_perfect() {
    let m = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );
    let matches = vec![m];
    assert_eq!(analyze_detection(&matches, false), "");
}

#[test]
fn test_analyze_detection_false_positive() {
    let matches = vec![create_test_match_full(
        "gpl",
        "2-aho",
        2000,
        2005,
        MatchScore::from_percentage(30.0),
        3,
        3,
        30.0,
        50,
        "gpl_bare.LICENSE",
    )];
    assert_eq!(analyze_detection(&matches, false), "false-positive");
}

#[test]
fn test_analyze_detection_clue_takes_precedence_over_false_positive_for_bare_gpl() {
    let mut clue = create_test_match_full(
        "gpl-1.0-plus",
        "2-aho",
        1,
        1,
        MatchScore::MAX,
        1,
        1,
        100.0,
        50,
        "gpl_bare_word_only.RULE",
    );
    clue.rule_kind = crate::license_detection::models::RuleKind::Clue;

    assert_eq!(
        analyze_detection(&[clue], false),
        DETECTION_LOG_LICENSE_CLUES
    );
}

#[test]
fn test_analyze_detection_unknown_match() {
    let matches = vec![create_test_match(95.0, "unknown.LICENSE")];
    assert_eq!(
        analyze_detection(&matches, false),
        DETECTION_LOG_UNKNOWN_MATCH
    );
}

#[test]
fn test_analyze_detection_imperfect_coverage() {
    let m = create_test_match_full(
        "mit",
        "1-hash",
        1,
        10,
        MatchScore::from_percentage(80.0),
        100,
        100,
        80.0,
        100,
        "mit.LICENSE",
    );
    let matches = vec![m];
    assert_eq!(
        analyze_detection(&matches, false),
        DETECTION_LOG_IMPERFECT_COVERAGE
    );
}

#[test]
fn test_analyze_detection_mixed_clue_and_detection_is_not_license_clues() {
    let mut clue = create_test_match_full(
        "mit",
        "2-aho",
        1,
        3,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit-clue.RULE",
    );
    clue.rule_kind = crate::license_detection::models::RuleKind::Clue;

    let detection = create_test_match_full(
        "mit",
        "1-hash",
        4,
        20,
        MatchScore::MAX,
        100,
        100,
        100.0,
        100,
        "mit.LICENSE",
    );

    let matches = vec![clue, detection];
    assert_eq!(analyze_detection(&matches, false), "");
}

#[test]
fn test_is_unknown_intro_true_with_is_license_intro_flag() {
    let mut m = create_test_match(100.0, "mit.LICENSE");
    m.license_expression = "unknown".to_string();
    m.rule_kind = crate::license_detection::models::RuleKind::Intro;
    assert!(is_unknown_intro(&m));
}

#[test]
fn test_is_unknown_intro_true_with_is_license_clue_flag() {
    let mut m = create_test_match(100.0, "mit.LICENSE");
    m.license_expression = "unknown".to_string();
    m.rule_kind = crate::license_detection::models::RuleKind::Clue;
    assert!(is_unknown_intro(&m));
}

#[test]
fn test_is_unknown_intro_true_with_free_unknown_expression() {
    let mut m = create_test_match(100.0, "mit.LICENSE");
    m.license_expression = "free-unknown".to_string();
    assert!(is_unknown_intro(&m));
}

#[test]
fn test_is_unknown_intro_false_no_unknown_in_expression() {
    let m = create_test_match(100.0, "mit.LICENSE");
    assert!(!is_unknown_intro(&m));
}

#[test]
fn test_is_unknown_intro_false_no_flags_or_free_unknown() {
    let mut m = create_test_match(100.0, "mit.LICENSE");
    m.license_expression = "unknown".to_string();
    m.rule_kind = crate::license_detection::models::RuleKind::None;

    assert!(!is_unknown_intro(&m));
}

#[test]
fn test_is_license_reference_local_file_true() {
    let mut m = create_test_match(100.0, "mit.LICENSE");
    m.referenced_filenames = Some(vec!["LICENSE".to_string()]);
    assert!(is_license_reference_local_file(&m));
}

#[test]
fn test_is_license_reference_local_file_true_multiple() {
    let mut m = create_test_match(100.0, "apache-2.0.COPYING");
    m.referenced_filenames = Some(vec!["COPYING".to_string()]);
    assert!(is_license_reference_local_file(&m));
}

#[test]
fn test_is_license_reference_local_file_false_empty() {
    let m = create_test_match(100.0, "mit.RULE");
    assert!(!is_license_reference_local_file(&m));
}

#[test]
fn test_filter_license_references_filters_matches() {
    let mut m1 = create_test_match(100.0, "mit.LICENSE");
    m1.referenced_filenames = Some(vec!["LICENSE".to_string()]);
    let m2 = create_test_match(100.0, "mit.RULE");
    let filtered = filter_license_references(&[m1, m2]);
    assert_eq!(filtered.len(), 1);
}

#[test]
fn test_filter_license_references_returns_original_when_empty() {
    let filtered = filter_license_references(&[]);
    assert!(filtered.is_empty());
}

#[test]
fn test_filter_license_references_no_filtering_needed() {
    let m1 = create_test_match(100.0, "mit.RULE");
    let m2 = create_test_match(100.0, "apache.RULE");
    let filtered = filter_license_references(&[m1, m2]);
    assert_eq!(filtered.len(), 2);
}

#[test]
fn test_filter_license_references_filters_local_file_references() {
    let mut m1 = create_test_match(100.0, "mit.LICENSE");
    m1.rule_kind = crate::license_detection::models::RuleKind::Intro;
    m1.matcher = MatcherKind::Aho;
    m1.match_coverage = 100.0;
    m1.referenced_filenames = Some(vec!["LICENSE".to_string()]);
    let m2 = create_test_match(100.0, "mit.RULE");
    let filtered = filter_license_references(&[m1, m2]);
    assert_eq!(filtered.len(), 1);
}

#[test]
fn test_has_unknown_intro_before_detection_single_match_returns_false() {
    let m = create_test_match(100.0, "mit.LICENSE");
    let matches = vec![m];
    assert!(!has_unknown_intro_before_detection(&matches));
}

#[test]
fn test_is_license_reference_local_file_false_none() {
    let m = create_test_match(100.0, "mit.RULE");
    assert!(!is_license_reference_local_file(&m));
}
