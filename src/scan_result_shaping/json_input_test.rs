// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::output_schema::{OutputMatch, OutputTopLevelLicenseDetection};
use crate::scan_result_shaping::test_fixtures::json_file;
use serde_json::json;
use std::fs;

fn output_json_file(path: &str, file_type: crate::models::FileType) -> OutputFileInfo {
    let internal = json_file(path, file_type);
    OutputFileInfo::from(&internal)
}

fn header_with_options(options: serde_json::Map<String, serde_json::Value>) -> JsonHeaderInput {
    JsonHeaderInput {
        options,
        ..Default::default()
    }
}

#[test]
fn requested_package_detection_is_false_without_package_flags() {
    let header = header_with_options(serde_json::Map::new());
    assert!(!header.requested_package_detection());
}

#[test]
fn requested_package_detection_is_false_when_flag_present_but_disabled() {
    // `push_bool_option` only ever records a `true` flag, but the raw options
    // map is untrusted JSON, so an explicit `false` must not be mistaken for
    // "package detection ran".
    let header = header_with_options(serde_json::Map::from_iter([(
        "--package".to_string(),
        serde_json::Value::Bool(false),
    )]));
    assert!(!header.requested_package_detection());
}

#[test]
fn requested_package_detection_is_true_for_each_recognized_package_flag() {
    for flag in [
        "--package",
        "--package-only",
        "--system-package",
        "--package-in-compiled",
    ] {
        let header = header_with_options(serde_json::Map::from_iter([(
            flag.to_string(),
            serde_json::Value::Bool(true),
        )]));
        assert!(
            header.requested_package_detection(),
            "{flag} should be recognized as package detection"
        );
    }
}

#[test]
fn is_hollow_package_detection_input_is_false_when_any_header_requested_it() {
    let loaded = JsonScanInput {
        headers: vec![
            header_with_options(serde_json::Map::new()),
            header_with_options(serde_json::Map::from_iter([(
                "--package".to_string(),
                serde_json::Value::Bool(true),
            )])),
        ],
        files: vec![output_json_file(
            "src/main.rs",
            crate::models::FileType::File,
        )],
        ..Default::default()
    };

    assert!(!loaded.is_hollow_package_detection_input());
}

#[test]
fn is_hollow_package_detection_input_is_true_when_no_header_requested_it() {
    let loaded = JsonScanInput {
        headers: vec![header_with_options(serde_json::Map::new())],
        files: vec![output_json_file(
            "src/main.rs",
            crate::models::FileType::File,
        )],
        ..Default::default()
    };

    assert!(loaded.is_hollow_package_detection_input());
}

#[test]
fn is_hollow_package_detection_input_is_false_without_scanned_files() {
    // A truly empty scan document (no files at all) is not "hollow": that
    // case keeps the existing documented empty-SBOM sentinel behavior and is
    // guarded separately by the scanned-file-count check in
    // `hollow_from_json_sbom_refusal` (`src/cli/run/mod.rs`).
    let loaded = JsonScanInput {
        headers: vec![header_with_options(serde_json::Map::new())],
        files: vec![],
        ..Default::default()
    };

    assert!(!loaded.is_hollow_package_detection_input());
}

#[test]
fn is_hollow_package_detection_input_is_false_when_packages_already_present() {
    // Real package data outweighs a missing/absent header flag: this input
    // was clearly examined for packages regardless of what its recorded
    // options say.
    let package = crate::models::Package::from_package_data(
        &crate::models::PackageData {
            package_type: Some(crate::models::PackageType::Npm),
            name: Some("demo".to_string()),
            version: Some("1.0.0".to_string()),
            ..Default::default()
        },
        "package.json".to_string(),
    );
    let loaded = JsonScanInput {
        headers: vec![header_with_options(serde_json::Map::new())],
        files: vec![output_json_file(
            "package.json",
            crate::models::FileType::File,
        )],
        packages: vec![crate::output_schema::OutputPackage::from(&package)],
        ..Default::default()
    };

    assert!(!loaded.is_hollow_package_detection_input());
}

#[test]
fn into_parts_passes_through_has_hollow_package_detection_input_flag() {
    for flag in [true, false] {
        let loaded = JsonScanInput {
            has_hollow_package_detection_input: flag,
            ..Default::default()
        };

        let (.., has_hollow_package_detection_input) =
            loaded.into_parts().expect("into_parts should succeed");

        assert_eq!(has_hollow_package_detection_input, flag);
    }
}

#[test]
fn load_scan_from_json_reads_files_and_metadata_sections() {
    let temp_path = std::env::temp_dir().join("provenant-from-json-test.json");
    let content = json!({
        "headers": [
            {
                "errors": ["Path: src/main.rs"],
                "warnings": ["Imported warning"]
            }
        ],
        "files": [
            {
                "name": "main.rs",
                "base_name": "main",
                "extension": ".rs",
                "path": "src/main.rs",
                "type": "file",
                "size": 10,
                "programming_language": "Rust"
            }
        ],
        "packages": [],
        "dependencies": [],
        "license_detections": [
            {
                "identifier": "mit-id",
                "license_expression": "mit",
                "license_expression_spdx": "MIT",
                "detection_count": 1,
                "reference_matches": [
                    {
                        "license_expression": "mit",
                        "license_expression_spdx": "MIT",
                        "from_file": "src/main.rs",
                        "start_line": 1,
                        "end_line": 1,
                        "score": 100.0,
                        "rule_url": null
                    }
                ]
            }
        ],
        "license_references": [
            {"name":"MIT","short_name":"MIT","spdx_license_key":"MIT","text":"..."}
        ],
        "license_rule_references": []
    });
    fs::write(&temp_path, content.to_string()).expect("write json fixture");

    let parsed = load_scan_from_json(temp_path.to_str().expect("utf-8 path"))
        .expect("from-json loading should succeed");

    let paths: Vec<_> = parsed.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, vec!["src/main.rs", "src"]);
    assert_eq!(parsed.headers.len(), 1);
    assert_eq!(parsed.headers[0].errors, vec!["Path: src/main.rs"]);
    assert_eq!(parsed.headers[0].warnings, vec!["Imported warning"]);
    assert_eq!(parsed.license_detections.len(), 1);
    assert_eq!(parsed.license_references.len(), 1);

    let _ = fs::remove_file(temp_path);
}

#[test]
fn load_scan_from_json_accepts_minimal_real_scancode_file_entries() {
    let temp_path = std::env::temp_dir().join("provenant-from-json-real-scancode-test.json");
    let content = json!({
        "headers": [
            {
                "errors": [],
                "warnings": [],
                "extra_data": {
                    "spdx_license_list_version": "3.27"
                }
            }
        ],
        "files": [
            {
                "path": "README",
                "type": "file",
                "detected_license_expression": null,
                "detected_license_expression_spdx": null,
                "license_detections": [],
                "license_clues": [],
                "percentage_of_license_text": 0,
                "scan_errors": []
            }
        ],
        "license_detections": [],
        "packages": [],
        "dependencies": []
    });
    fs::write(&temp_path, content.to_string()).expect("write json fixture");

    let parsed = load_scan_from_json(temp_path.to_str().expect("utf-8 path"))
        .expect("minimal real ScanCode JSON should load");

    assert_eq!(parsed.files.len(), 1);
    assert_eq!(parsed.files[0].path, "README");
    assert_eq!(parsed.files[0].name, "README");
    assert_eq!(parsed.files[0].base_name, "README");
    assert_eq!(parsed.files[0].extension, "");
    assert_eq!(parsed.files[0].size, 0);

    let _ = fs::remove_file(temp_path);
}

#[test]
fn load_scan_from_json_reconstructs_normalized_copyrights_from_raw_output() {
    let temp_path = std::env::temp_dir().join("provenant-from-json-raw-copyright-test.json");
    let content = json!({
        "headers": [{"errors": [], "warnings": []}],
        "files": [
            {
                "path": "README",
                "type": "file",
                "size": 10,
                "copyrights": [
                    {
                        "copyright": "Copyright 2024 Example Corp. All rights reserved.",
                        "start_line": 1,
                        "end_line": 1
                    }
                ],
                "license_detections": [],
                "license_clues": [],
                "scan_errors": []
            }
        ],
        "license_detections": [],
        "packages": [],
        "dependencies": []
    });
    fs::write(&temp_path, content.to_string()).expect("write json fixture");

    let parsed = load_scan_from_json(temp_path.to_str().expect("utf-8 path"))
        .expect("raw copyright JSON should load");
    let file = crate::models::FileInfo::try_from(&parsed.files[0]).expect("file conversion");

    assert_eq!(
        file.copyrights[0].copyright,
        "Copyright 2024 Example Corp. All rights reserved."
    );
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("Copyright 2024 Example Corp.")
    );

    let _ = fs::remove_file(temp_path);
}

#[test]
fn normalize_loaded_json_scan_applies_strip_root_per_loaded_input() {
    let mut loaded = JsonScanInput {
        headers: vec![JsonHeaderInput {
            errors: vec![
                "Failed to read or parse package.json: archive/root/src/main.rs".to_string(),
            ],
            warnings: vec!["custom recoverable warning: archive/root/src/main.rs".to_string()],
            extra_data: None,
            ..Default::default()
        }],
        files: vec![
            output_json_file("archive/root", crate::models::FileType::Directory),
            output_json_file("archive/root/src/main.rs", crate::models::FileType::File),
        ],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![OutputTopLevelLicenseDetection {
            identifier: "mit-id".to_string(),
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            detection_count: 1,
            detection_log: vec![],
            reference_matches: vec![OutputMatch {
                license_expression: "mit".to_string(),
                license_expression_spdx: "MIT".to_string(),
                from_file: Some("archive/root/src/main.rs".to_string()),
                start_line: 1,
                end_line: 1,
                matcher: None,
                score: 100.0,
                matched_length: None,
                match_coverage: None,
                rule_relevance: None,
                rule_identifier: None,
                rule_url: None,
                matched_text: None,
                matched_text_diagnostics: None,
                referenced_filenames: None,
            }],
        }],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    normalize_loaded_json_scan(&mut loaded, true, false);

    let paths: Vec<_> = loaded.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, vec!["root", "src/main.rs", "src"]);
    assert_eq!(
        loaded.headers[0].errors,
        vec!["Failed to read or parse package.json: src/main.rs"]
    );
    assert_eq!(
        loaded.headers[0].warnings,
        vec!["custom recoverable warning: src/main.rs"]
    );
    assert_eq!(
        loaded.license_detections[0].reference_matches[0]
            .from_file
            .as_deref(),
        Some("src/main.rs")
    );
}

#[test]
fn normalize_loaded_json_scan_trims_full_root_display_without_absolutizing() {
    let mut loaded = JsonScanInput {
        headers: vec![JsonHeaderInput {
            errors: vec!["Path: /tmp/archive/root/src/main.rs".to_string()],
            warnings: vec!["custom recoverable warning: /tmp/archive/root/src/main.rs".to_string()],
            extra_data: None,
            ..Default::default()
        }],
        files: vec![output_json_file(
            "/tmp/archive/root/src/main.rs",
            crate::models::FileType::File,
        )],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![OutputTopLevelLicenseDetection {
            identifier: "mit-id".to_string(),
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            detection_count: 1,
            detection_log: vec![],
            reference_matches: vec![OutputMatch {
                license_expression: "mit".to_string(),
                license_expression_spdx: "MIT".to_string(),
                from_file: Some("/tmp/archive/root/src/main.rs".to_string()),
                start_line: 1,
                end_line: 1,
                matcher: None,
                score: 100.0,
                matched_length: None,
                match_coverage: None,
                rule_relevance: None,
                rule_identifier: None,
                rule_url: None,
                matched_text: None,
                matched_text_diagnostics: None,
                referenced_filenames: None,
            }],
        }],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    normalize_loaded_json_scan(&mut loaded, false, true);

    assert_eq!(loaded.files[0].path, "tmp/archive/root/src/main.rs");
    assert_eq!(
        loaded.headers[0].errors,
        vec!["Path: tmp/archive/root/src/main.rs"]
    );
    assert_eq!(
        loaded.headers[0].warnings,
        vec!["custom recoverable warning: tmp/archive/root/src/main.rs"]
    );
    assert_eq!(
        loaded.license_detections[0].reference_matches[0]
            .from_file
            .as_deref(),
        Some("tmp/archive/root/src/main.rs")
    );
}

#[test]
fn normalize_loaded_json_scan_prefixes_multi_resource_relative_replay_with_virtual_root() {
    let mut loaded = JsonScanInput {
        headers: vec![],
        files: vec![
            output_json_file("README.md", crate::models::FileType::File),
            output_json_file("src/lib.rs", crate::models::FileType::File),
        ],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    normalize_loaded_json_scan(&mut loaded, false, false);

    let paths: Vec<_> = loaded.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "virtual_root/README.md",
            "virtual_root/src/lib.rs",
            "virtual_root",
            "virtual_root/src",
        ]
    );
}

#[test]
fn normalize_loaded_json_scan_adds_virtual_root_directory_for_relative_replay() {
    let mut loaded = JsonScanInput {
        headers: vec![],
        files: vec![output_json_file("README.md", crate::models::FileType::File)],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    normalize_loaded_json_scan(&mut loaded, false, false);

    let paths: Vec<_> = loaded.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, vec!["README.md"]);
}

#[test]
fn load_and_merge_json_inputs_namespaces_multiple_replay_inputs() {
    let temp_dir = std::env::temp_dir().join("provenant-from-json-merge-test");
    let _ = fs::create_dir_all(&temp_dir);
    let first = temp_dir.join("first.json");
    let second = temp_dir.join("second.json");

    fs::write(
        &first,
        json!({
            "headers": [
                {"warnings": ["custom recoverable warning: README"]}
            ],
            "files": [
                {"path": "README", "type": "file", "scan_errors": []}
            ],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("write first json fixture");
    fs::write(
        &second,
        json!({
            "headers": [
                {"warnings": ["custom recoverable warning: README.adoc"]}
            ],
            "files": [
                {"path": "README.adoc", "type": "file", "scan_errors": []},
                {"path": "bench/data/gsoc-2018.json", "type": "file", "scan_errors": []}
            ],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("write second json fixture");

    let merged = load_and_merge_json_inputs(
        &[
            first.to_str().expect("utf-8 path").to_string(),
            second.to_str().expect("utf-8 path").to_string(),
        ],
        false,
        false,
    )
    .expect("merged replay inputs should load");

    let paths: Vec<_> = merged.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "virtual_root/codebase-1/README",
            "virtual_root/codebase-2/README.adoc",
            "virtual_root/codebase-2/bench/data/gsoc-2018.json",
            "virtual_root/codebase-2/bench",
            "virtual_root/codebase-2/bench/data",
            "virtual_root/codebase-2",
        ]
    );
    assert_eq!(
        merged.headers[0].warnings,
        vec!["custom recoverable warning: virtual_root/codebase-1/README"]
    );
    assert_eq!(
        merged.headers[1].warnings,
        vec!["custom recoverable warning: virtual_root/codebase-2/README.adoc"]
    );

    let _ = fs::remove_file(first);
    let _ = fs::remove_file(second);
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn load_and_merge_json_inputs_flags_hollow_when_any_input_never_requested_detection() {
    // Regression for the merge-masking bug: input A never ran package
    // detection (hollow) but input B did. An aggregate `any()` across all
    // merged headers would report "requested" for the whole merge and hide
    // A's hollow files. The merged flag must stay hollow regardless.
    let temp_dir = std::env::temp_dir().join("provenant-from-json-hollow-merge-masking-test");
    let _ = fs::create_dir_all(&temp_dir);
    let hollow_input = temp_dir.join("hollow.json");
    let requested_input = temp_dir.join("requested.json");

    fs::write(
        &hollow_input,
        json!({
            "headers": [{"options": {}}],
            "files": [
                {"path": "src/main.rs", "type": "file", "scan_errors": []}
            ],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("write hollow json fixture");
    fs::write(
        &requested_input,
        json!({
            "headers": [{"options": {"--package": true}}],
            "files": [
                {"path": "README.md", "type": "file", "scan_errors": []}
            ],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("write requested json fixture");

    let merged = load_and_merge_json_inputs(
        &[
            hollow_input.to_str().expect("utf-8 path").to_string(),
            requested_input.to_str().expect("utf-8 path").to_string(),
        ],
        false,
        false,
    )
    .expect("merged inputs should load");

    assert!(merged.has_hollow_package_detection_input);

    // Order must not matter: the hollow input silencing the merge would be
    // just as wrong regardless of which side of the merge it is on.
    let merged_reversed = load_and_merge_json_inputs(
        &[
            requested_input.to_str().expect("utf-8 path").to_string(),
            hollow_input.to_str().expect("utf-8 path").to_string(),
        ],
        false,
        false,
    )
    .expect("merged inputs should load in reverse order");

    assert!(merged_reversed.has_hollow_package_detection_input);

    let (.., has_hollow_package_detection_input) =
        merged.into_parts().expect("into_parts should succeed");
    assert!(has_hollow_package_detection_input);

    let _ = fs::remove_file(hollow_input);
    let _ = fs::remove_file(requested_input);
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn load_and_merge_json_inputs_not_hollow_when_every_input_requested_or_empty() {
    let temp_dir = std::env::temp_dir().join("provenant-from-json-hollow-merge-safe-test");
    let _ = fs::create_dir_all(&temp_dir);
    let requested_input = temp_dir.join("requested.json");
    let empty_input = temp_dir.join("empty.json");

    fs::write(
        &requested_input,
        json!({
            "headers": [{"options": {"--package": true}}],
            "files": [
                {"path": "README.md", "type": "file", "scan_errors": []}
            ],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("write requested json fixture");
    fs::write(
        &empty_input,
        json!({
            "headers": [{"options": {}}],
            "files": [],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("write empty json fixture");

    let merged = load_and_merge_json_inputs(
        &[
            requested_input.to_str().expect("utf-8 path").to_string(),
            empty_input.to_str().expect("utf-8 path").to_string(),
        ],
        false,
        false,
    )
    .expect("merged inputs should load");

    assert!(!merged.has_hollow_package_detection_input);

    let _ = fs::remove_file(requested_input);
    let _ = fs::remove_file(empty_input);
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn load_and_merge_json_inputs_restores_warning_severity_after_namespacing() {
    let temp_dir = std::env::temp_dir().join("provenant-from-json-warning-merge-test");
    let _ = fs::create_dir_all(&temp_dir);
    let first = temp_dir.join("first.json");
    let second = temp_dir.join("second.json");

    fs::write(
        &first,
        json!({
            "headers": [
                {"warnings": ["custom recoverable warning: README"]}
            ],
            "files": [
                {"path": "README", "type": "file", "scan_errors": ["custom recoverable warning"]}
            ],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("write first json fixture");
    fs::write(
        &second,
        json!({
            "files": [
                {"path": "README.adoc", "type": "file", "scan_errors": []}
            ],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("write second json fixture");

    let merged = load_and_merge_json_inputs(
        &[
            first.to_str().expect("utf-8 path").to_string(),
            second.to_str().expect("utf-8 path").to_string(),
        ],
        false,
        false,
    )
    .expect("merged replay inputs should load");

    let (process_result, ..) = merged.into_parts().expect("into_parts should succeed");
    let readme = process_result
        .files
        .into_iter()
        .find(|file| file.path == "virtual_root/codebase-1/README")
        .expect("namespaced replayed file");

    assert_eq!(readme.scan_diagnostics.len(), 1);
    assert_eq!(
        readme.scan_diagnostics[0].severity,
        crate::models::DiagnosticSeverity::Warning
    );

    let _ = fs::remove_file(first);
    let _ = fs::remove_file(second);
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn load_scan_from_json_synthesizes_missing_ancestor_directories() {
    let temp_path = std::env::temp_dir().join("provenant-from-json-missing-dirs-test.json");
    let content = json!({
        "files": [
            {
                "path": "virtual_root/.github/actions/build/action.yml",
                "type": "file",
                "scan_errors": []
            }
        ],
        "packages": [],
        "dependencies": [],
        "license_detections": [],
        "license_references": [],
        "license_rule_references": []
    });
    fs::write(&temp_path, content.to_string()).expect("write json fixture");

    let parsed = load_scan_from_json(temp_path.to_str().expect("utf-8 path"))
        .expect("from-json loading should synthesize ancestor directories");

    let paths: Vec<_> = parsed.files.iter().map(|file| file.path.as_str()).collect();
    assert!(paths.contains(&"virtual_root"));
    assert!(paths.contains(&"virtual_root/.github"));
    assert!(paths.contains(&"virtual_root/.github/actions"));
    assert!(paths.contains(&"virtual_root/.github/actions/build"));
    assert!(paths.contains(&"virtual_root/.github/actions/build/action.yml"));

    let _ = fs::remove_file(temp_path);
}

#[test]
fn into_parts_preserves_imported_header_errors_as_extra_errors() {
    let loaded = JsonScanInput {
        headers: vec![JsonHeaderInput {
            errors: vec!["Failed to read directory: src/main.rs".to_string()],
            warnings: vec!["Imported warning".to_string()],
            extra_data: Some(JsonHeaderExtraDataInput {
                spdx_license_list_version: Some("3.27".to_string()),
                license_index_provenance: Some(crate::models::LicenseIndexProvenance {
                    source: "embedded-artifact".to_string(),
                    dataset_fingerprint: "abc123".to_string(),
                    ignored_rules: vec!["rule.RULE".to_string()],
                    ignored_licenses: vec![],
                    ignored_rules_due_to_licenses: vec![],
                    added_rules: vec![],
                    replaced_rules: vec![],
                    added_licenses: vec![],
                    replaced_licenses: vec![],
                }),
            }),
            ..Default::default()
        }],
        files: vec![output_json_file(
            "src/main.rs",
            crate::models::FileType::File,
        )],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    let (
        _process_result,
        _assembly_result,
        _dets,
        _refs,
        _rule_refs,
        extra_errors,
        imported_spdx_license_list_version,
        imported_license_index_provenance,
        _package_detection_requested_in_source,
    ) = loaded.into_parts().expect("into_parts should succeed");

    assert_eq!(extra_errors, vec!["Failed to read directory: src/main.rs"]);
    assert_eq!(imported_spdx_license_list_version.as_deref(), Some("3.27"));
    assert_eq!(
        imported_license_index_provenance
            .as_ref()
            .map(|provenance| provenance.dataset_fingerprint.as_str()),
        Some("abc123")
    );
}

#[test]
fn into_parts_drops_imported_warnings_and_file_summary_errors() {
    let loaded = JsonScanInput {
        headers: vec![JsonHeaderInput {
            errors: vec![
                "Failed to read or parse package.json: src/main.rs".to_string(),
                "Failed to read directory: src/vendor".to_string(),
            ],
            warnings: vec!["Imported warning".to_string()],
            extra_data: None,
            ..Default::default()
        }],
        files: vec![{
            let mut file = output_json_file("src/main.rs", crate::models::FileType::File);
            file.scan_errors = vec!["Imported file failure detail".to_string()];
            file
        }],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    let (
        _process_result,
        _assembly_result,
        _dets,
        _refs,
        _rule_refs,
        extra_errors,
        imported_spdx_license_list_version,
        imported_license_index_provenance,
        _package_detection_requested_in_source,
    ) = loaded.into_parts().expect("into_parts should succeed");

    assert_eq!(extra_errors, vec!["Failed to read directory: src/vendor"]);
    assert!(imported_spdx_license_list_version.is_none());
    assert!(imported_license_index_provenance.is_none());
}

#[test]
fn into_parts_restores_file_warning_severity_from_header_warnings() {
    let loaded = JsonScanInput {
        headers: vec![JsonHeaderInput {
            errors: vec![],
            warnings: vec!["custom recoverable warning: src/main.rs".to_string()],
            extra_data: None,
            ..Default::default()
        }],
        files: vec![{
            let mut file = output_json_file("src/main.rs", crate::models::FileType::File);
            file.scan_errors = vec!["custom recoverable warning".to_string()];
            file
        }],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    let (process_result, _assembly_result, _dets, _refs, _rule_refs, extra_errors, _, _, _) =
        loaded.into_parts().expect("into_parts should succeed");

    assert!(extra_errors.is_empty());
    assert_eq!(process_result.files.len(), 1);
    assert_eq!(process_result.files[0].scan_diagnostics.len(), 1);
    assert_eq!(
        process_result.files[0].scan_diagnostics[0].message,
        "custom recoverable warning"
    );
    assert_eq!(
        process_result.files[0].scan_diagnostics[0].severity,
        crate::models::DiagnosticSeverity::Warning
    );
}

#[test]
fn normalize_loaded_json_scan_rewrites_verbose_header_error_path_prefix() {
    let mut loaded = JsonScanInput {
        headers: vec![JsonHeaderInput {
            errors: vec![
                "Failed to parse package.json: /tmp/archive/root/src/main.rs\n  Failed to parse package.json".to_string(),
            ],
            warnings: vec![],
            extra_data: None,
            ..Default::default()
        }],
        files: vec![output_json_file(
            "/tmp/archive/root/src/main.rs",
            crate::models::FileType::File,
        )],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    normalize_loaded_json_scan(&mut loaded, false, true);

    assert_eq!(
        loaded.headers[0].errors,
        vec![
            "Failed to parse package.json: tmp/archive/root/src/main.rs\n  Failed to parse package.json"
        ]
    );
}

#[test]
fn normalize_loaded_json_scan_rewrites_header_warnings_too() {
    let mut loaded = JsonScanInput {
        headers: vec![JsonHeaderInput {
            errors: vec![],
            warnings: vec!["custom recoverable warning: /tmp/archive/root/src/main.rs".to_string()],
            extra_data: None,
            ..Default::default()
        }],
        files: vec![output_json_file(
            "/tmp/archive/root/src/main.rs",
            crate::models::FileType::File,
        )],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    normalize_loaded_json_scan(&mut loaded, false, true);

    assert_eq!(
        loaded.headers[0].warnings,
        vec!["custom recoverable warning: tmp/archive/root/src/main.rs"]
    );
}

#[test]
fn into_parts_discards_conflicting_imported_header_provenance() {
    let loaded = JsonScanInput {
        headers: vec![
            JsonHeaderInput {
                errors: vec![],
                warnings: vec![],
                extra_data: Some(JsonHeaderExtraDataInput {
                    spdx_license_list_version: Some("3.27".to_string()),
                    license_index_provenance: Some(crate::models::LicenseIndexProvenance {
                        source: "embedded-artifact".to_string(),
                        dataset_fingerprint: "one".to_string(),
                        ignored_rules: vec![],
                        ignored_licenses: vec![],
                        ignored_rules_due_to_licenses: vec![],
                        added_rules: vec![],
                        replaced_rules: vec![],
                        added_licenses: vec![],
                        replaced_licenses: vec![],
                    }),
                }),
                ..Default::default()
            },
            JsonHeaderInput {
                errors: vec![],
                warnings: vec![],
                extra_data: Some(JsonHeaderExtraDataInput {
                    spdx_license_list_version: Some("3.28".to_string()),
                    license_index_provenance: Some(crate::models::LicenseIndexProvenance {
                        source: "custom-license-dataset".to_string(),
                        dataset_fingerprint: "two".to_string(),
                        ignored_rules: vec![],
                        ignored_licenses: vec![],
                        ignored_rules_due_to_licenses: vec![],
                        added_rules: vec![],
                        replaced_rules: vec![],
                        added_licenses: vec![],
                        replaced_licenses: vec![],
                    }),
                }),
                ..Default::default()
            },
        ],
        files: vec![output_json_file(
            "src/main.rs",
            crate::models::FileType::File,
        )],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    let (
        _process_result,
        _assembly_result,
        _dets,
        _refs,
        _rule_refs,
        _extra_errors,
        imported_spdx_license_list_version,
        imported_license_index_provenance,
        _package_detection_requested_in_source,
    ) = loaded.into_parts().expect("into_parts should succeed");

    assert!(imported_spdx_license_list_version.is_none());
    assert!(imported_license_index_provenance.is_none());
}
