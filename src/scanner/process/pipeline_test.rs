// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::{
    LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES, MAX_CONTENT_DETECTION_BYTES,
    cap_content_detection_text, cap_non_source_json_license_text,
    cap_non_source_text_dump_license_text, has_line_rich_json_prefix,
    maybe_record_processing_timeout, merge_parse_results, process_file,
};
use crate::models::DatasourceId;
use crate::models::{DiagnosticSeverity, PackageData, PackageType, ScanDiagnostic};
use crate::parsers::ParsePackagesResult;
use crate::progress::{ProgressMode, ScanProgress};
use crate::scanner::{LicenseScanOptions, TextDetectionOptions};
use crate::utils::file::FileInfoClassification;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::tempdir;

#[test]
fn test_process_file_suppresses_non_actionable_pdf_extraction_failure() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("broken.pdf");
    fs::write(&path, b"%PDF-1.7\nthis is not a valid pdf object graph\n")
        .expect("write malformed pdf");
    let metadata = fs::metadata(&path).expect("metadata");
    let progress = ScanProgress::new(ProgressMode::Quiet);

    let file_info = process_file(
        &path,
        &metadata,
        &progress,
        None,
        LicenseScanOptions::default(),
        &TextDetectionOptions::default(),
    );

    assert!(file_info.scan_diagnostics.is_empty());
}

#[test]
fn test_processing_timeout_is_not_duplicated_after_stage_specific_timeout() {
    let started = Instant::now() - Duration::from_secs(2);
    let mut scan_diagnostics = vec![ScanDiagnostic::timeout(
        "Timeout before license scan (> 1.00s)",
    )];

    maybe_record_processing_timeout(&mut scan_diagnostics, started, 1.0);

    assert_eq!(scan_diagnostics.len(), 1);
    assert_eq!(
        scan_diagnostics[0].message,
        "Timeout before license scan (> 1.00s)"
    );
}

#[test]
fn test_processing_timeout_is_recorded_when_no_timeout_error_exists() {
    let started = Instant::now() - Duration::from_secs(2);
    let mut scan_diagnostics = Vec::new();

    maybe_record_processing_timeout(&mut scan_diagnostics, started, 1.0);

    assert_eq!(scan_diagnostics.len(), 1);
    assert_eq!(scan_diagnostics[0].severity, DiagnosticSeverity::Timeout);
    assert_eq!(
        scan_diagnostics[0].message,
        "Processing interrupted due to timeout after 1.00 seconds"
    );
}

#[test]
fn test_cap_non_source_json_license_text_truncates_large_json() {
    let classification = FileInfoClassification {
        mime_type: "application/json".to_string(),
        file_type: "JSON text data".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let large_json = format!("{{\"items\":\"{}\"}}", "x".repeat(200_000));

    let capped = cap_non_source_json_license_text(
        Path::new("resolution.json"),
        &classification,
        &large_json,
    );

    assert!(capped.len() <= LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert!(capped.len() < large_json.len());
}

#[test]
fn test_cap_non_source_json_license_text_keeps_sourcemaps_intact() {
    let classification = FileInfoClassification {
        mime_type: "application/json".to_string(),
        file_type: "JSON text data".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let large_json = format!("{{\"mappings\":\"{}\"}}", "x".repeat(200_000));

    let capped =
        cap_non_source_json_license_text(Path::new("bundle.js.map"), &classification, &large_json);

    assert_eq!(capped.as_ref(), large_json);
}

#[test]
fn test_cap_non_source_json_license_text_keeps_package_locks_intact() {
    let classification = FileInfoClassification {
        mime_type: "application/json".to_string(),
        file_type: "JSON text data".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let large_json = format!("{{\"packages\":\"{}\"}}", "x".repeat(200_000));

    let capped = cap_non_source_json_license_text(
        Path::new("package-lock.json"),
        &classification,
        &large_json,
    );

    assert_eq!(capped.as_ref(), large_json);
}

#[test]
fn test_cap_non_source_json_license_text_keeps_npm_shrinkwrap_intact() {
    let classification = FileInfoClassification {
        mime_type: "application/json".to_string(),
        file_type: "JSON text data".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let large_json = format!("{{\"packages\":\"{}\"}}", "x".repeat(200_000));

    let capped = cap_non_source_json_license_text(
        Path::new("npm-shrinkwrap.json"),
        &classification,
        &large_json,
    );

    assert_eq!(capped.as_ref(), large_json);
}

#[test]
fn test_cap_non_source_json_license_text_keeps_line_rich_large_json_intact() {
    let classification = FileInfoClassification {
        mime_type: "application/json".to_string(),
        file_type: "JSON text data".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let entries = (0..2_000)
        .map(|index| {
            format!(
                "  {{\"id\":{index},\"description\":\"This project is free software under GPL2 and Apache-2.0 terms\"}}"
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let large_json = format!("[\n{entries}\n]");

    let capped = cap_non_source_json_license_text(
        Path::new("benchmark-data.json"),
        &classification,
        &large_json,
    );

    assert!(large_json.len() > LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert_eq!(capped.as_ref(), large_json);
}

#[test]
fn test_cap_non_source_json_license_text_truncates_generated_scan_result_json() {
    let classification = FileInfoClassification {
        mime_type: "application/json".to_string(),
        file_type: "JSON text data".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let entries = (0..4_000)
        .map(|index| {
            format!("      {{\"path\":\"file-{index}\",\"license\":\"GPL-2.0 AND Apache-2.0\"}}")
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let large_json = format!(
        "{{\n  \"headers\": [\n    {{\n      \"tool_name\": \"scanpipe\",\n      \"notice\": \"Generated with ScanCode.io and provided on an AS IS BASIS\"\n    }}\n  ],\n  \"files\": [\n{entries}\n  ]\n}}"
    );

    let capped = cap_non_source_json_license_text(
        Path::new("scan-result.json"),
        &classification,
        &large_json,
    );

    assert!(large_json.len() > LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert!(capped.len() <= LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert!(capped.len() < large_json.len());
}

#[test]
fn test_cap_non_source_text_dump_license_text_truncates_large_ildump() {
    let classification = FileInfoClassification {
        mime_type: "text/plain".to_string(),
        file_type: "UTF-8 Unicode text".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let line = ".assembly extern mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089\n.class public auto ansi import windowsruntime Foo\n";
    let large_dump = line.repeat(2_000);

    let capped = cap_non_source_text_dump_license_text(
        Path::new("Windows.ildump"),
        &classification,
        &large_dump,
    );

    assert!(large_dump.len() > LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert!(capped.len() <= LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert!(capped.len() < large_dump.len());
}

#[test]
fn test_cap_non_source_text_dump_license_text_truncates_dump_like_text_without_ildump_extension() {
    let classification = FileInfoClassification {
        mime_type: "text/plain".to_string(),
        file_type: "UTF-8 Unicode text".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let line = ".assembly extern mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089\n.class public auto ansi import windowsruntime Foo\n";
    let large_dump = line.repeat(2_000);

    let capped = cap_non_source_text_dump_license_text(
        Path::new("Windows.txt"),
        &classification,
        &large_dump,
    );

    assert!(large_dump.len() > LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert!(capped.len() <= LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert!(capped.len() < large_dump.len());
}

#[test]
fn test_cap_non_source_text_dump_license_text_keeps_license_rich_text() {
    let classification = FileInfoClassification {
        mime_type: "text/plain".to_string(),
        file_type: "UTF-8 Unicode text".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let line = "Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the \"Software\"), to deal in the Software without restriction.\n";
    let large_notice = line.repeat(2_000);

    let capped = cap_non_source_text_dump_license_text(
        Path::new("Windows.ildump"),
        &classification,
        &large_notice,
    );

    assert!(large_notice.len() > LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert_eq!(capped.as_ref(), large_notice);
}

#[test]
fn test_cap_non_source_text_dump_license_text_keeps_generic_large_plain_text() {
    let classification = FileInfoClassification {
        mime_type: "text/plain".to_string(),
        file_type: "UTF-8 Unicode text".to_string(),
        programming_language: None,
        is_binary: false,
        is_text: true,
        is_archive: false,
        is_media: false,
        is_source: false,
        is_script: false,
    };
    let line =
        "This is ordinary large plain text without metadata dump markers or license notices.\n";
    let large_text = line.repeat(4_000);

    let capped =
        cap_non_source_text_dump_license_text(Path::new("large.txt"), &classification, &large_text);

    assert!(large_text.len() > LARGE_NON_SOURCE_JSON_LICENSE_TEXT_BYTES);
    assert_eq!(capped.as_ref(), large_text);
}

#[test]
fn test_cap_content_detection_text_keeps_small_text_untouched() {
    let text = "Copyright (c) 2024 Acme Corp\nLicensed under the MIT License.\n".to_string();
    let mut diagnostics = Vec::new();
    let capped = cap_content_detection_text(&mut diagnostics, text.clone());

    assert_eq!(capped, text);
    assert!(diagnostics.is_empty());
}

#[test]
fn test_cap_content_detection_text_truncates_oversized_text_and_records_info() {
    let header = "Copyright (c) 2024 Acme Corp\n";
    let mut text = String::with_capacity(MAX_CONTENT_DETECTION_BYTES + header.len() + 4096);
    text.push_str(header);
    while text.len() <= MAX_CONTENT_DETECTION_BYTES + 2048 {
        text.push_str("row_a,row_b,row_c,1234,5678\n");
    }
    let original_len = text.len();

    let mut diagnostics = Vec::new();
    let capped = cap_content_detection_text(&mut diagnostics, text);

    assert!(capped.len() <= MAX_CONTENT_DETECTION_BYTES);
    assert!(original_len > MAX_CONTENT_DETECTION_BYTES);
    // The header (where notices live) is preserved for detection.
    assert!(capped.starts_with(header));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Info);
}

#[test]
fn test_has_line_rich_json_prefix_detects_multiline_json() {
    let entries = (0..512)
        .map(|index| format!("  {{\"id\":{index}}}"))
        .collect::<Vec<_>>()
        .join(",\n");
    let large_json = format!("[\n{entries}\n]");

    assert!(has_line_rich_json_prefix(&large_json));
}

#[test]
fn test_has_line_rich_json_prefix_rejects_compact_json() {
    let compact_json = format!("{{\"items\":[\"{}\"]}}", "x".repeat(200_000));

    assert!(!has_line_rich_json_prefix(&compact_json));
}

#[test]
fn test_merge_parse_results_keeps_multiple_package_surfaces() {
    let misc = ParsePackagesResult {
        packages: vec![PackageData {
            package_type: Some(PackageType::Nsis),
            datasource_id: Some(DatasourceId::NsisInstaller),
            ..Default::default()
        }],
        ..Default::default()
    };
    let winexe = ParsePackagesResult {
        packages: vec![PackageData {
            package_type: Some(PackageType::Winexe),
            datasource_id: Some(DatasourceId::WindowsExecutable),
            ..Default::default()
        }],
        scan_diagnostics: vec![ScanDiagnostic::error("windows metadata warning")],
    };

    let merged = merge_parse_results(vec![misc, winexe]).expect("merged parse result");

    assert_eq!(merged.packages.len(), 2);
    assert!(
        merged
            .packages
            .iter()
            .any(|pkg| pkg.datasource_id == Some(DatasourceId::NsisInstaller))
    );
    assert!(
        merged
            .packages
            .iter()
            .any(|pkg| pkg.datasource_id == Some(DatasourceId::WindowsExecutable))
    );
    assert_eq!(merged.scan_diagnostics.len(), 1);
    assert_eq!(
        merged.scan_diagnostics[0].message,
        "windows metadata warning"
    );
}

#[test]
fn test_process_file_detects_versioned_project_banner_on_minified_js() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("jquery-3.7.1.min.js");
    let mut content = String::from(
        "/*! jQuery v3.7.1 | (c) OpenJS Foundation and other contributors | jquery.org/license */\n",
    );
    content.push_str(
        &r#"!function(){var meta={"description":"demo","url":"https://example.com"};return meta;}"#
            .repeat(40),
    );
    fs::write(&path, content).expect("write minified jquery fixture");
    let metadata = fs::metadata(&path).expect("metadata");
    let progress = ScanProgress::new(ProgressMode::Quiet);

    let file_info = process_file(
        &path,
        &metadata,
        &progress,
        None,
        LicenseScanOptions::default(),
        &TextDetectionOptions::default(),
    );

    assert!(
        file_info
            .copyrights
            .iter()
            .any(|c| c.copyright == "(c) OpenJS Foundation and other contributors"),
        "copyrights: {:?}",
        file_info.copyrights
    );
    assert!(
        !file_info
            .copyrights
            .iter()
            .any(|c| c.copyright.contains("jquery.org/license")),
        "copyrights: {:?}",
        file_info.copyrights
    );
    assert!(
        file_info
            .holders
            .iter()
            .any(|h| h.holder == "OpenJS Foundation and other contributors"),
        "holders: {:?}",
        file_info.holders
    );
    assert!(
        !file_info
            .holders
            .iter()
            .any(|h| h.holder.contains("jquery.org/license")),
        "holders: {:?}",
        file_info.holders
    );
}

#[test]
fn test_process_file_drops_license_prose_parties_but_keeps_real_notice() {
    use crate::license_detection::LicenseDetectionEngine;
    use std::sync::Arc;

    // A real BSD-original license body: the year-bearing copyright/holder notice
    // sits above the matched license-text region, while the advertising-clause
    // "...and its contributors" author is license prose inside the region.
    let path = Path::new("testdata/license-golden/datadriven/lic2/bsd-original_1.txt");
    assert!(path.exists(), "fixture missing: {}", path.display());

    let engine =
        Arc::new(LicenseDetectionEngine::from_embedded().expect("embedded engine should load"));
    let metadata = fs::metadata(path).expect("metadata");
    let progress = ScanProgress::new(ProgressMode::Quiet);

    let file_info = process_file(
        path,
        &metadata,
        &progress,
        Some(engine),
        LicenseScanOptions::default(),
        &TextDetectionOptions::default(),
    );

    // License detection itself is unaffected.
    assert!(
        file_info
            .license_detections
            .iter()
            .any(|d| d.license_expression.contains("bsd")),
        "license detection should still fire: {:?}",
        file_info.license_detections,
    );
    // The genuine, year-bearing copyright notice and holder survive, even though
    // a license-text region was detected in the same file.
    assert!(
        file_info
            .copyrights
            .iter()
            .any(|c| c.copyright.contains("Kungliga")),
        "real notice dropped: {:?}",
        file_info.copyrights,
    );
    assert!(
        file_info
            .holders
            .iter()
            .any(|h| h.holder.contains("Kungliga")),
        "real holder dropped: {:?}",
        file_info.holders,
    );
    // The advertising-clause author fragment is license prose inside the region
    // and carries no year, so it is suppressed.
    assert!(
        !file_info
            .authors
            .iter()
            .any(|a| a.author.contains("and its contributors")),
        "license-prose author not suppressed: {:?}",
        file_info.authors,
    );
}
