// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::suppress_license_text_region_parties;
use crate::license_detection::MatcherKind;
use crate::models::{
    Author, Copyright, FileInfo, FileType, Holder, LicenseDetection, LineNumber, Match, MatchScore,
};

fn line(n: usize) -> LineNumber {
    LineNumber::new(n).expect("nonzero line")
}

fn license_match(start: usize, end: usize, matched_length: usize) -> Match {
    Match {
        license_expression: "bsd-new".to_string(),
        license_expression_spdx: "BSD-3-Clause".to_string(),
        from_file: None,
        start_line: line(start),
        end_line: line(end),
        matcher: MatcherKind::Aho,
        score: MatchScore::MAX,
        matched_length: Some(matched_length),
        match_coverage: Some(100.0),
        rule_relevance: Some(100),
        rule_identifier: String::new(),
        rule_url: None,
        matched_text: None,
        referenced_filenames: None,
        matched_text_diagnostics: None,
    }
}

fn file_info_with_region(matched_length: usize) -> FileInfo {
    let mut file_info = FileInfo::new(
        "LICENSE".to_string(),
        "LICENSE".to_string(),
        String::new(),
        "LICENSE".to_string(),
        FileType::File,
        None,
        None,
        0,
        None,
        None,
        None,
        None,
        None,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    file_info.license_detections = vec![LicenseDetection {
        license_expression: "bsd-new".to_string(),
        license_expression_spdx: "BSD-3-Clause".to_string(),
        matches: vec![license_match(3, 9, matched_length)],
        detection_log: Vec::new(),
        identifier: String::new(),
    }];
    file_info
}

fn copyright(text: &str, start: usize, end: usize) -> Copyright {
    Copyright {
        copyright: text.to_string(),
        normalized_copyright: Some(text.to_string()),
        start_line: line(start),
        end_line: line(end),
    }
}

#[test]
fn drops_year_free_party_inside_license_body_region() {
    let mut file_info = file_info_with_region(107);
    // Outside the license body region (lines 3..9): a real notice with a year.
    file_info.copyrights = vec![copyright("Copyright (c) 2020 Acme, Inc.", 1, 1)];
    file_info.holders = vec![Holder {
        holder: "Acme, Inc.".to_string(),
        start_line: line(1),
        end_line: line(1),
    }];
    // Inside the region: license prose extracted as an author/holder, no year.
    file_info.authors = vec![Author {
        author: "MAKE NO REPRESENTATIONS about the suitability of".to_string(),
        start_line: line(8),
        end_line: line(8),
    }];
    file_info.holders.push(Holder {
        holder: "Software Foundation".to_string(),
        start_line: line(5),
        end_line: line(5),
    });

    suppress_license_text_region_parties(&mut file_info);

    assert_eq!(file_info.copyrights.len(), 1, "real notice kept");
    assert_eq!(file_info.holders.len(), 1, "{:?}", file_info.holders);
    assert_eq!(file_info.holders[0].holder, "Acme, Inc.");
    assert!(file_info.authors.is_empty(), "{:?}", file_info.authors);
}

#[test]
fn keeps_year_bearing_notice_inside_license_body_region() {
    let mut file_info = file_info_with_region(107);
    // A genuine embedded copyright notice with a year, inside the region.
    file_info.copyrights = vec![copyright("Copyright (c) 1995 Kungliga Tekniska", 4, 4)];
    file_info.holders = vec![Holder {
        holder: "Copyright 2016 Jane Doe".to_string(),
        start_line: line(6),
        end_line: line(6),
    }];

    suppress_license_text_region_parties(&mut file_info);

    assert_eq!(
        file_info.copyrights.len(),
        1,
        "year notice kept inside region"
    );
    assert_eq!(file_info.holders.len(), 1, "year holder kept inside region");
}

#[test]
fn ignores_short_reference_matches_as_regions() {
    // A short license reference (few matched tokens) must not gate suppression.
    let mut file_info = file_info_with_region(3);
    file_info.authors = vec![Author {
        author: "MAKE NO REPRESENTATIONS about the suitability of".to_string(),
        start_line: line(8),
        end_line: line(8),
    }];

    suppress_license_text_region_parties(&mut file_info);

    assert_eq!(
        file_info.authors.len(),
        1,
        "short reference region must not suppress findings"
    );
}

#[test]
fn keeps_party_outside_license_region() {
    let mut file_info = file_info_with_region(107);
    // Year-free finding, but on a line outside the license body region.
    file_info.authors = vec![Author {
        author: "Jane Roe".to_string(),
        start_line: line(1),
        end_line: line(1),
    }];

    suppress_license_text_region_parties(&mut file_info);

    assert_eq!(file_info.authors.len(), 1, "finding outside region kept");
}

#[test]
fn no_regions_leaves_parties_untouched() {
    let mut file_info = FileInfo::new(
        "x.txt".to_string(),
        "x".to_string(),
        ".txt".to_string(),
        "x.txt".to_string(),
        FileType::File,
        None,
        None,
        0,
        None,
        None,
        None,
        None,
        None,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    file_info.authors = vec![Author {
        author: "MAKE NO REPRESENTATIONS about the suitability of".to_string(),
        start_line: line(8),
        end_line: line(8),
    }];

    suppress_license_text_region_parties(&mut file_info);

    assert_eq!(file_info.authors.len(), 1, "no regions, nothing suppressed");
}
