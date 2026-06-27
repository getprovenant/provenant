// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn apply_local_file_reference_following_resolves_root_license_file() {
    let mut license = file("project/LICENSE");
    license.detected_license_expression = Some("mit".to_string());
    license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![Match {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("project/LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(20).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(100),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "mit.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "mit-license".to_string(),
    }];

    let mut notice = file("project/src/notice.js");
    notice.detected_license_expression = Some("unknown-license-reference".to_string());
    notice.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![Match {
            license_expression: "unknown-license-reference".to_string(),
            license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
            from_file: Some("project/src/notice.js".to_string()),
            start_line: LineNumber::new(2).unwrap(),
            end_line: LineNumber::new(2).unwrap(),
            matcher: MatcherKind::Aho,
            score: MatchScore::MAX,
            matched_length: Some(2),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
            rule_url: None,
            matched_text: Some("See LICENSE".to_string()),
            referenced_filenames: Some(vec!["LICENSE".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "unknown-ref".to_string(),
    }];

    let mut files = vec![dir("project"), license, notice];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let notice = files
        .iter()
        .find(|file| file.path == "project/src/notice.js")
        .expect("notice file should exist");
    let license = files
        .iter()
        .find(|file| file.path == "project/LICENSE")
        .expect("license file should exist");
    assert_eq!(notice.detected_license_expression.as_deref(), Some("mit"));
    assert!(
        license.is_referenced,
        "followed LICENSE should be marked referenced"
    );
    assert!(
        !notice.is_referenced,
        "source file should not be marked referenced"
    );
    assert_eq!(
        notice.license_detections[0].detection_log,
        vec!["unknown-reference-to-local-file"]
    );
    assert_eq!(notice.license_detections[0].matches.len(), 2);
    assert_eq!(
        notice.license_detections[0].matches[1].from_file.as_deref(),
        Some("project/LICENSE")
    );
}

#[test]
fn apply_local_file_reference_following_resolves_multi_match_root_license_reference() {
    let mut license = file("LICENSE");
    license.detected_license_expression = Some("mit".to_string());
    license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![Match {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(20).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(161),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "mit.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "mit-license".to_string(),
    }];

    let mut faqs = file("docs/faqs.md");
    faqs.detected_license_expression = Some("unknown-license-reference".to_string());
    faqs.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![
            Match {
                license_expression: "unknown-license-reference".to_string(),
                license_expression_spdx: "LicenseRef-scancode-unknown-license-reference"
                    .to_string(),
                from_file: Some("docs/faqs.md".to_string()),
                start_line: LineNumber::new(208).unwrap(),
                end_line: LineNumber::new(208).unwrap(),
                matcher: MatcherKind::Aho,
                score: MatchScore::MAX,
                matched_length: Some(2),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
                rule_url: None,
                matched_text: None,
                referenced_filenames: Some(vec!["LICENSE".to_string()]),
                matched_text_diagnostics: None,
            },
            Match {
                license_expression: "unknown-license-reference".to_string(),
                license_expression_spdx: "LicenseRef-scancode-unknown-license-reference"
                    .to_string(),
                from_file: Some("docs/faqs.md".to_string()),
                start_line: LineNumber::new(212).unwrap(),
                end_line: LineNumber::new(212).unwrap(),
                matcher: MatcherKind::Aho,
                score: MatchScore::MAX,
                matched_length: Some(2),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
                rule_url: None,
                matched_text: None,
                referenced_filenames: Some(vec!["LICENSE".to_string()]),
                matched_text_diagnostics: None,
            },
        ],
        detection_log: vec![],
        identifier: "unknown-ref-faqs".to_string(),
    }];

    let mut files = vec![dir("docs"), license, faqs];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let faqs = files
        .iter()
        .find(|file| file.path == "docs/faqs.md")
        .expect("faqs file should exist");
    assert_eq!(faqs.detected_license_expression.as_deref(), Some("mit"));
    assert_eq!(
        faqs.license_detections[0].detection_log,
        vec!["unknown-reference-to-local-file"]
    );
    assert_eq!(faqs.license_detections[0].license_expression_spdx, "MIT");
    assert!(
        faqs.license_detections[0]
            .matches
            .iter()
            .any(|detection_match| {
                detection_match.from_file.as_deref() == Some("LICENSE")
                    && detection_match.license_expression_spdx == "MIT"
            })
    );
}

#[test]
fn apply_local_file_reference_following_resolves_multi_match_root_license_reference_with_dot_paths()
{
    let mut license = file("./LICENSE");
    license.detected_license_expression = Some("mit".to_string());
    license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![Match {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("./LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(20).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(161),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "mit.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "mit-license".to_string(),
    }];

    let mut faqs = file("./docs/faqs.md");
    faqs.detected_license_expression = Some("unknown-license-reference".to_string());
    faqs.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![
            Match {
                license_expression: "unknown-license-reference".to_string(),
                license_expression_spdx: "LicenseRef-scancode-unknown-license-reference"
                    .to_string(),
                from_file: Some("./docs/faqs.md".to_string()),
                start_line: LineNumber::new(208).unwrap(),
                end_line: LineNumber::new(208).unwrap(),
                matcher: MatcherKind::Aho,
                score: MatchScore::MAX,
                matched_length: Some(2),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
                rule_url: None,
                matched_text: None,
                referenced_filenames: Some(vec!["LICENSE".to_string()]),
                matched_text_diagnostics: None,
            },
            Match {
                license_expression: "unknown-license-reference".to_string(),
                license_expression_spdx: "LicenseRef-scancode-unknown-license-reference"
                    .to_string(),
                from_file: Some("./docs/faqs.md".to_string()),
                start_line: LineNumber::new(212).unwrap(),
                end_line: LineNumber::new(212).unwrap(),
                matcher: MatcherKind::Aho,
                score: MatchScore::MAX,
                matched_length: Some(2),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
                rule_url: None,
                matched_text: None,
                referenced_filenames: Some(vec!["LICENSE".to_string()]),
                matched_text_diagnostics: None,
            },
        ],
        detection_log: vec![],
        identifier: "unknown-ref-faqs-dot".to_string(),
    }];

    let mut files = vec![dir("."), dir("./docs"), license, faqs];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let faqs = files
        .iter()
        .find(|file| file.path == "./docs/faqs.md")
        .expect("faqs file should exist");
    assert_eq!(faqs.detected_license_expression.as_deref(), Some("mit"));
    assert_eq!(faqs.license_detections[0].license_expression_spdx, "MIT");
}

#[test]
fn apply_local_file_reference_following_accepts_absolute_match_sources_for_current_file() {
    let scan_root = "/tmp/conan-ref-min";

    let mut license = file("./LICENSE");
    license.detected_license_expression = Some("mit".to_string());
    license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![Match {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some(format!("{scan_root}/./LICENSE")),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(20).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(161),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "mit.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "mit-license".to_string(),
    }];

    let mut faqs = file("./docs/faqs.md");
    faqs.detected_license_expression = Some("unknown-license-reference".to_string());
    faqs.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![
            Match {
                license_expression: "unknown-license-reference".to_string(),
                license_expression_spdx: "LicenseRef-scancode-unknown-license-reference"
                    .to_string(),
                from_file: Some(format!("{scan_root}/./docs/faqs.md")),
                start_line: LineNumber::new(208).unwrap(),
                end_line: LineNumber::new(208).unwrap(),
                matcher: MatcherKind::Aho,
                score: MatchScore::MAX,
                matched_length: Some(2),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
                rule_url: None,
                matched_text: None,
                referenced_filenames: Some(vec!["LICENSE".to_string()]),
                matched_text_diagnostics: None,
            },
            Match {
                license_expression: "unknown-license-reference".to_string(),
                license_expression_spdx: "LicenseRef-scancode-unknown-license-reference"
                    .to_string(),
                from_file: Some(format!("{scan_root}/./docs/faqs.md")),
                start_line: LineNumber::new(212).unwrap(),
                end_line: LineNumber::new(212).unwrap(),
                matcher: MatcherKind::Aho,
                score: MatchScore::MAX,
                matched_length: Some(2),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
                rule_url: None,
                matched_text: None,
                referenced_filenames: Some(vec!["LICENSE".to_string()]),
                matched_text_diagnostics: None,
            },
        ],
        detection_log: vec![],
        identifier: "unknown-ref-faqs-abs".to_string(),
    }];

    let mut files = vec![dir("."), dir("./docs"), license, faqs];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let faqs = files
        .iter()
        .find(|file| file.path == "./docs/faqs.md")
        .expect("faqs file should exist");
    assert_eq!(faqs.detected_license_expression.as_deref(), Some("mit"));
    assert_eq!(faqs.license_detections[0].license_expression_spdx, "MIT");
    assert_eq!(
        faqs.license_detections[0].detection_log,
        vec!["unknown-reference-to-local-file"]
    );
}

#[test]
fn apply_local_file_reference_following_preserves_notice_expression_alongside_resolved_license() {
    let mut license = file("LICENSE");
    license.detected_license_expression = Some("mit".to_string());
    license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![Match {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(20).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(161),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "mit.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "mit-license".to_string(),
    }];

    let mut patch = file("patches/example.patch");
    patch.detected_license_expression = Some("bsd-new".to_string());
    patch.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "bsd-new".to_string(),
        license_expression_spdx: "BSD-3-Clause".to_string(),
        matches: vec![Match {
            license_expression: "bsd-new".to_string(),
            license_expression_spdx: "BSD-3-Clause".to_string(),
            from_file: Some("patches/example.patch".to_string()),
            start_line: LineNumber::new(4).unwrap(),
            end_line: LineNumber::new(5).unwrap(),
            matcher: MatcherKind::Aho,
            score: MatchScore::from_percentage(95.0),
            matched_length: Some(19),
            match_coverage: Some(100.0),
            rule_relevance: Some(95),
            rule_identifier: "bsd-new_1169.RULE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: Some(vec!["LICENSE".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "bsd-ref".to_string(),
    }];

    let mut files = vec![dir("patches"), license, patch];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let patch = files
        .iter()
        .find(|file| file.path == "patches/example.patch")
        .expect("patch file should exist");
    assert_eq!(
        patch.detected_license_expression.as_deref(),
        Some("bsd-new AND mit")
    );
    assert_eq!(
        patch.license_detections[0].license_expression_spdx,
        "BSD-3-Clause AND MIT"
    );
    assert_eq!(
        patch.license_detections[0].detection_log,
        vec!["unknown-reference-to-local-file"]
    );
    assert!(
        patch.license_detections[0]
            .matches
            .iter()
            .any(|detection_match| {
                detection_match.from_file.as_deref() == Some("patches/example.patch")
                    && detection_match.license_expression_spdx == "BSD-3-Clause"
            })
    );
    assert!(
        patch.license_detections[0]
            .matches
            .iter()
            .any(|detection_match| {
                detection_match.from_file.as_deref() == Some("LICENSE")
                    && detection_match.license_expression_spdx == "MIT"
            })
    );
}

#[test]
fn apply_local_file_reference_following_prefers_root_license_for_imperfect_subdir_reference() {
    let mut root_license = file("LICENSE");
    root_license.detected_license_expression = Some("npsl-exception-0.95".to_string());
    root_license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "npsl-exception-0.95".to_string(),
        license_expression_spdx: "LicenseRef-scancode-npsl-exception-0.95".to_string(),
        matches: vec![Match {
            license_expression: "npsl-exception-0.95".to_string(),
            license_expression_spdx: "LicenseRef-scancode-npsl-exception-0.95".to_string(),
            from_file: Some("LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(582).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(4720),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "npsl-exception-0.95.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "npsl-license".to_string(),
    }];

    let mut sibling_license = file("third_party/LICENSE");
    sibling_license.detected_license_expression = Some("bsd-new".to_string());
    sibling_license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "bsd-new".to_string(),
        license_expression_spdx: "BSD-3-Clause".to_string(),
        matches: vec![Match {
            license_expression: "bsd-new".to_string(),
            license_expression_spdx: "BSD-3-Clause".to_string(),
            from_file: Some("third_party/LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(30).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(150),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "bsd-new.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "bsd-license".to_string(),
    }];

    let mut header = file("src/FPEngine.h");
    header.detected_license_expression = Some("gpl-1.0-plus OR mit".to_string());
    header.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "gpl-1.0-plus OR mit".to_string(),
        license_expression_spdx: "GPL-1.0-or-later OR MIT".to_string(),
        matches: vec![Match {
            license_expression: "gpl-1.0-plus OR mit".to_string(),
            license_expression_spdx: "GPL-1.0-or-later OR MIT".to_string(),
            from_file: Some("src/FPEngine.h".to_string()),
            start_line: LineNumber::new(49).unwrap(),
            end_line: LineNumber::new(57).unwrap(),
            matcher: MatcherKind::Seq,
            score: MatchScore::from_percentage(41.79),
            matched_length: Some(28),
            match_coverage: Some(41.79),
            rule_relevance: Some(100),
            rule_identifier: "gpl-1.0-plus_or_mit_2.RULE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: Some(vec!["LICENSE".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "nmap-header-ref".to_string(),
    }];

    let mut files = vec![
        dir("src"),
        dir("third_party"),
        root_license,
        sibling_license,
        header,
    ];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let header = files
        .iter()
        .find(|file| file.path == "src/FPEngine.h")
        .expect("header file should exist");
    assert_eq!(
        header.detected_license_expression.as_deref(),
        Some("npsl-exception-0.95")
    );
    assert_eq!(
        header.license_detections[0].license_expression_spdx,
        "LicenseRef-scancode-npsl-exception-0.95"
    );
    assert_eq!(
        header.license_detections[0].detection_log,
        vec!["unknown-reference-to-local-file"]
    );
    assert_eq!(header.license_detections[0].matches.len(), 2);
    assert_eq!(
        header.license_detections[0].matches[1].from_file.as_deref(),
        Some("LICENSE")
    );
}

#[test]
fn apply_local_file_reference_following_does_not_reuse_followed_license_as_second_hop_source() {
    let mut root_license = file("project/LICENSE");
    root_license.detected_license_expression = Some("mit".to_string());
    root_license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![Match {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("project/LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(20).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(100),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "mit.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "root-license".to_string(),
    }];

    let mut followed_license = file("project/ncat/LICENSE");
    followed_license.detected_license_expression = Some("mit".to_string());
    followed_license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![
            Match {
                license_expression: "unknown-license-reference".to_string(),
                license_expression_spdx: "LicenseRef-scancode-unknown-license-reference"
                    .to_string(),
                from_file: Some("project/ncat/LICENSE".to_string()),
                start_line: LineNumber::ONE,
                end_line: LineNumber::ONE,
                matcher: MatcherKind::Aho,
                score: MatchScore::MAX,
                matched_length: Some(2),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
                rule_url: None,
                matched_text: Some("See LICENSE".to_string()),
                referenced_filenames: Some(vec!["LICENSE".to_string()]),
                matched_text_diagnostics: None,
            },
            Match {
                license_expression: "mit".to_string(),
                license_expression_spdx: "MIT".to_string(),
                from_file: Some("project/LICENSE".to_string()),
                start_line: LineNumber::ONE,
                end_line: LineNumber::new(20).unwrap(),
                matcher: MatcherKind::Hash,
                score: MatchScore::MAX,
                matched_length: Some(100),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "mit.LICENSE".to_string(),
                rule_url: None,
                matched_text: None,
                referenced_filenames: None,
                matched_text_diagnostics: None,
            },
        ],
        detection_log: vec!["unknown-reference-to-local-file".to_string()],
        identifier: "followed-license".to_string(),
    }];

    let mut source = file("project/ncat/ncat_core.h");
    source.detected_license_expression = Some("unknown-license-reference".to_string());
    source.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![Match {
            license_expression: "unknown-license-reference".to_string(),
            license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
            from_file: Some("project/ncat/ncat_core.h".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
            matcher: MatcherKind::Aho,
            score: MatchScore::MAX,
            matched_length: Some(2),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
            rule_url: None,
            matched_text: Some("See LICENSE".to_string()),
            referenced_filenames: Some(vec!["LICENSE".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "second-hop-source".to_string(),
    }];

    let mut files = vec![
        dir("project"),
        dir("project/ncat"),
        root_license,
        followed_license,
        source,
    ];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let source = files
        .iter()
        .find(|file| file.path == "project/ncat/ncat_core.h")
        .expect("source file should exist");
    // The reference must not resolve to the already-followed license; with no
    // resolvable target the bare `See LICENSE` reference is a clue, not a
    // detected license.
    assert_eq!(source.detected_license_expression, None);
    assert!(source.license_detections.is_empty());
    assert_eq!(source.license_clues.len(), 1);
    assert_eq!(
        source.license_clues[0].license_expression,
        "unknown-license-reference"
    );
}

#[test]
fn apply_local_file_reference_following_requires_exact_filename_match() {
    let mut license = file("project/LICENSE");
    license.detected_license_expression = Some("mit".to_string());
    license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![Match {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("project/LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(20).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(100),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "mit.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "mit-license".to_string(),
    }];

    let mut notice = file("project/src/notice.js");
    notice.detected_license_expression = Some("unknown-license-reference".to_string());
    notice.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![Match {
            license_expression: "unknown-license-reference".to_string(),
            license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
            from_file: Some("project/src/notice.js".to_string()),
            start_line: LineNumber::new(2).unwrap(),
            end_line: LineNumber::new(2).unwrap(),
            matcher: MatcherKind::Aho,
            score: MatchScore::MAX,
            matched_length: Some(2),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
            rule_url: None,
            matched_text: Some("See LICENSE.txt".to_string()),
            referenced_filenames: Some(vec!["LICENSE.txt".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "unknown-ref".to_string(),
    }];

    let mut files = vec![dir("project"), license, notice];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let notice = files
        .iter()
        .find(|file| file.path == "project/src/notice.js")
        .expect("notice file should exist");
    // `LICENSE.txt` does not match the sibling `LICENSE`, so the reference does
    // not resolve and the bare reference stays a clue rather than asserting a
    // license.
    assert_eq!(notice.detected_license_expression, None);
    assert!(notice.license_detections.is_empty());
    assert_eq!(notice.license_clues.len(), 1);
}

#[test]
fn apply_local_file_reference_following_does_not_search_unrelated_top_level_directories() {
    let mut nested_copying = file("libssh2/COPYING");
    nested_copying.detected_license_expression = Some("bsd-new".to_string());
    nested_copying.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "bsd-new".to_string(),
        license_expression_spdx: "BSD-3-Clause".to_string(),
        matches: vec![Match {
            license_expression: "bsd-new".to_string(),
            license_expression_spdx: "BSD-3-Clause".to_string(),
            from_file: Some("libssh2/COPYING".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(20).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(100),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "bsd-new.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "nested-copying".to_string(),
    }];

    let mut notice = file("docs/3rd-party-licenses.txt");
    notice.detected_license_expression = Some("unknown-license-reference".to_string());
    notice.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![Match {
            license_expression: "unknown-license-reference".to_string(),
            license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
            from_file: Some("docs/3rd-party-licenses.txt".to_string()),
            start_line: LineNumber::new(10).unwrap(),
            end_line: LineNumber::new(10).unwrap(),
            matcher: MatcherKind::Aho,
            score: MatchScore::MAX,
            matched_length: Some(2),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
            rule_url: None,
            matched_text: Some("See COPYING".to_string()),
            referenced_filenames: Some(vec!["COPYING".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "docs-copying-ref".to_string(),
    }];

    let mut files = vec![dir("docs"), dir("libssh2"), nested_copying, notice];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let notice = files
        .iter()
        .find(|file| file.path == "docs/3rd-party-licenses.txt")
        .expect("notice file should exist");
    // The unrelated top-level `libssh2/COPYING` must not be searched, so the
    // reference does not resolve and the bare reference stays a clue.
    assert_eq!(notice.detected_license_expression, None);
    assert!(notice.license_detections.is_empty());
    assert_eq!(notice.license_clues.len(), 1);
}

#[test]
fn apply_local_file_reference_following_drops_unknown_intro_from_resolved_target() {
    let mut license = file("project/LICENSE");
    license.detected_license_expression = Some("apache-2.0".to_string());
    license.license_detections = vec![
        crate::models::LicenseDetection {
            license_expression: "unknown-license-reference".to_string(),
            license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
            matches: vec![Match {
                license_expression: "unknown-license-reference".to_string(),
                license_expression_spdx: "LicenseRef-scancode-unknown-license-reference"
                    .to_string(),
                from_file: Some("project/LICENSE".to_string()),
                start_line: LineNumber::new(2).unwrap(),
                end_line: LineNumber::new(2).unwrap(),
                matcher: MatcherKind::Aho,
                score: MatchScore::from_percentage(50.0),
                matched_length: Some(2),
                match_coverage: Some(100.0),
                rule_relevance: Some(50),
                rule_identifier: "license-intro_2.RULE".to_string(),
                rule_url: None,
                matched_text: Some("Apache License".to_string()),
                referenced_filenames: None,
                matched_text_diagnostics: None,
            }],
            detection_log: vec![],
            identifier: "license-intro".to_string(),
        },
        crate::models::LicenseDetection {
            license_expression: "apache-2.0".to_string(),
            license_expression_spdx: "Apache-2.0".to_string(),
            matches: vec![Match {
                license_expression: "apache-2.0".to_string(),
                license_expression_spdx: "Apache-2.0".to_string(),
                from_file: Some("project/LICENSE".to_string()),
                start_line: LineNumber::new(5).unwrap(),
                end_line: LineNumber::new(205).unwrap(),
                matcher: MatcherKind::Hash,
                score: MatchScore::MAX,
                matched_length: Some(1584),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "apache-2.0.LICENSE".to_string(),
                rule_url: None,
                matched_text: None,
                referenced_filenames: None,
                matched_text_diagnostics: None,
            }],
            detection_log: vec![],
            identifier: "apache-license".to_string(),
        },
    ];

    let mut notice = file("project/src/notice.js");
    notice.detected_license_expression = Some("unknown-license-reference".to_string());
    notice.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![Match {
            license_expression: "unknown-license-reference".to_string(),
            license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
            from_file: Some("project/src/notice.js".to_string()),
            start_line: LineNumber::new(2).unwrap(),
            end_line: LineNumber::new(2).unwrap(),
            matcher: MatcherKind::Aho,
            score: MatchScore::MAX,
            matched_length: Some(2),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
            rule_url: None,
            matched_text: Some("See LICENSE".to_string()),
            referenced_filenames: Some(vec!["LICENSE".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "unknown-ref".to_string(),
    }];

    let mut files = vec![dir("project"), license, notice];
    let mut packages = Vec::new();
    apply_package_reference_following(&mut files, &mut packages);

    let notice = files
        .iter()
        .find(|file| file.path == "project/src/notice.js")
        .expect("notice file should exist");
    assert_eq!(
        notice.detected_license_expression.as_deref(),
        Some("apache-2.0")
    );
    assert_eq!(
        notice.license_detections[0].detection_log,
        vec!["unknown-reference-to-local-file"]
    );
    assert_eq!(notice.license_detections[0].matches.len(), 2);
    assert!(notice.license_detections[0].matches.iter().all(|m| {
        m.license_expression != "unknown-license-reference"
            || m.from_file.as_deref() != Some("project/LICENSE")
    }));
}

#[test]
fn apply_local_file_reference_following_resolves_files_beside_manifest() {
    let package_uid = "pkg:pypi/demo?uuid=test".to_string();
    let mut package = super::test_utils::package(&package_uid, "project/demo.dist-info/METADATA");
    package.datafile_paths = vec!["project/demo.dist-info/METADATA".to_string()];

    let mut license = file("project/demo.dist-info/LICENSE");
    license.detected_license_expression = Some("mit".to_string());
    license.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![Match {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("project/demo.dist-info/LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(20).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(100),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "mit.LICENSE".to_string(),
            rule_url: None,
            matched_text: None,
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "mit-license".to_string(),
    }];

    let mut source = file("project/demo/__init__.py");
    source.for_packages = vec![PackageUid::from_raw(package_uid.clone())];
    source.detected_license_expression = Some("unknown-license-reference".to_string());
    source.license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![Match {
            license_expression: "unknown-license-reference".to_string(),
            license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
            from_file: Some("project/demo/__init__.py".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
            matcher: MatcherKind::Aho,
            score: MatchScore::MAX,
            matched_length: Some(2),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "unknown-license-reference_see-license_1.RULE".to_string(),
            rule_url: None,
            matched_text: Some("See LICENSE".to_string()),
            referenced_filenames: Some(vec!["LICENSE".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: "unknown-ref".to_string(),
    }];

    let mut files = vec![dir("project"), license, source];
    let mut packages = vec![package];
    apply_package_reference_following(&mut files, &mut packages);

    let source = files
        .iter()
        .find(|file| file.path == "project/demo/__init__.py")
        .expect("source file should exist");
    let license = files
        .iter()
        .find(|file| file.path == "project/demo.dist-info/LICENSE")
        .expect("license file should exist");
    assert_eq!(source.detected_license_expression.as_deref(), Some("mit"));
    assert!(
        license.is_referenced,
        "followed sidecar license should be marked referenced"
    );
    assert!(
        !source.is_referenced,
        "source file should not be marked referenced"
    );
    assert_eq!(
        source.license_detections[0].matches[1].from_file.as_deref(),
        Some("project/demo.dist-info/LICENSE")
    );
}
