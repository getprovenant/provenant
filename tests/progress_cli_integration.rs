// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::process::Command;

use regex::Regex;
use serde_json::Value;
use tempfile::TempDir;

fn provenant_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_provenant"));
    command.current_dir(env!("CARGO_MANIFEST_DIR"));
    command
}

fn create_scan_fixture() -> (TempDir, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let scan_dir = temp.path().join("scan");
    fs::create_dir_all(&scan_dir).expect("failed to create scan dir");
    fs::write(scan_dir.join("a.txt"), "hello cache@example.com\n")
        .expect("failed to write fixture file");
    (temp, scan_dir.to_string_lossy().to_string())
}

fn create_mit_license_fixture() -> (TempDir, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let scan_dir = temp.path().join("scan");
    fs::create_dir_all(&scan_dir).expect("failed to create scan dir");
    fs::write(
        scan_dir.join("LICENSE"),
        "Permission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction.",
    )
    .expect("failed to write MIT fixture");
    (temp, scan_dir.to_string_lossy().to_string())
}

fn create_malformed_package_fixture() -> (TempDir, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let scan_dir = temp.path().join("scan");
    fs::create_dir_all(&scan_dir).expect("failed to create scan dir");
    fs::write(scan_dir.join("package.json"), "{ this is not valid json }")
        .expect("failed to write malformed fixture");
    (temp, scan_dir.to_string_lossy().to_string())
}

fn create_ignore_fixture() -> (TempDir, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let scan_dir = temp.path().join("scan");
    let build_dir = scan_dir.join("build");

    fs::create_dir_all(&build_dir).expect("failed to create build dir");
    fs::write(scan_dir.join("keep.txt"), "keep me\n").expect("failed to write keep.txt");
    fs::write(scan_dir.join("report.csv"), "col\n1\n").expect("failed to write report.csv");
    fs::write(build_dir.join("generated.txt"), "generated\n")
        .expect("failed to write generated.txt");

    (temp, scan_dir.to_string_lossy().to_string())
}

fn create_from_json_fixture_with_provenance() -> (TempDir, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let input_file = temp.path().join("input.json");
    fs::write(
        &input_file,
        serde_json::json!({
            "headers": [{
                "tool_name": "provenant",
                "tool_version": "0.0.0-test",
                "options": {},
                "notice": "test",
                "start_timestamp": "2026-01-01T000000.000000",
                "end_timestamp": "2026-01-01T000001.000000",
                "output_format_version": "4.1.0",
                "duration": 1.0,
                "errors": [],
                "warnings": [],
                "extra_data": {
                    "system_environment": {
                        "operating_system": "linux",
                        "cpu_architecture": "x86_64",
                        "platform": "linux",
                        "platform_version": "test",
                        "rust_version": "1.0.0"
                    },
                    "spdx_license_list_version": "9.99",
                    "files_count": 1,
                    "directories_count": 0,
                    "excluded_count": 0,
                    "license_index_provenance": {
                        "source": "custom-license-dataset",
                        "dataset_fingerprint": "imported-fingerprint",
                        "ignored_rules": ["imported-rule.RULE"]
                    }
                }
            }],
            "files": [{
                "path": "src/main.rs",
                "type": "file",
                "name": "main.rs",
                "base_name": "main",
                "extension": ".rs",
                "size": 10,
                "programming_language": "Rust"
            }],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("failed to write from-json fixture");

    (temp, input_file.to_string_lossy().to_string())
}

fn create_from_json_fixture_with_warning() -> (TempDir, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let input_file = temp.path().join("input-warning.json");
    fs::write(
        &input_file,
        serde_json::json!({
            "headers": [{
                "tool_name": "provenant",
                "tool_version": "0.0.0-test",
                "options": {},
                "notice": "test",
                "start_timestamp": "2026-01-01T000000.000000",
                "end_timestamp": "2026-01-01T000001.000000",
                "output_format_version": "4.1.0",
                "duration": 1.0,
                "errors": [],
                "warnings": ["custom recoverable warning: src/main.rs"],
                "extra_data": {
                    "system_environment": {
                        "operating_system": "linux",
                        "cpu_architecture": "x86_64",
                        "platform": "linux",
                        "platform_version": "test",
                        "rust_version": "1.0.0"
                    },
                    "spdx_license_list_version": "9.99",
                    "files_count": 1,
                    "directories_count": 0,
                    "excluded_count": 0
                }
            }],
            "files": [{
                "path": "src/main.rs",
                "type": "file",
                "name": "main.rs",
                "base_name": "main",
                "extension": ".rs",
                "size": 10,
                "programming_language": "Rust",
                "scan_errors": ["custom recoverable warning"]
            }],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("failed to write from-json warning fixture");

    (temp, input_file.to_string_lossy().to_string())
}

fn create_compare_json_fixtures() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let scancode_file = temp.path().join("scancode.json");
    let provenant_file = temp.path().join("provenant.json");

    fs::write(
        &scancode_file,
        serde_json::json!({
            "files": [{
                "path": "src/lib.rs",
                "type": "file",
                "license_detections": [{
                    "license_expression": "mit",
                    "license_expression_spdx": "MIT",
                    "detection_count": 1
                }]
            }],
            "license_detections": [{
                "license_expression": "mit",
                "license_expression_spdx": "MIT",
                "detection_count": 1
            }],
            "packages": [],
            "dependencies": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("failed to write scancode fixture");

    fs::write(
        &provenant_file,
        serde_json::json!({
            "files": [{
                "path": "src/lib.rs",
                "type": "file",
                "license_detections": [{
                    "license_expression": "apache-2.0",
                    "license_expression_spdx": "Apache-2.0",
                    "detection_count": 1
                }]
            }],
            "license_detections": [{
                "license_expression": "apache-2.0",
                "license_expression_spdx": "Apache-2.0",
                "detection_count": 1
            }],
            "packages": [],
            "dependencies": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("failed to write provenant fixture");

    (
        temp,
        scancode_file.to_string_lossy().to_string(),
        provenant_file.to_string_lossy().to_string(),
    )
}

fn create_compare_json_fixtures_with_file_level_package_fallback() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let scancode_file = temp.path().join("scancode-fallback.json");
    let provenant_file = temp.path().join("provenant-fallback.json");

    let shared_package_data = serde_json::json!([{
        "type": "npm",
        "name": "left-pad",
        "version": "1.3.0",
        "purl": "pkg:npm/left-pad@1.3.0",
        "dependencies": [{
            "purl": "pkg:npm/ansi-regex@5.0.1",
            "scope": "dependencies",
            "is_runtime": true
        }]
    }]);

    fs::write(
        &scancode_file,
        serde_json::json!({
            "files": [{
                "path": "package-lock.json",
                "type": "file",
                "package_data": shared_package_data.clone()
            }],
            "packages": [],
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("failed to write scancode fallback fixture");

    fs::write(
        &provenant_file,
        serde_json::json!({
            "files": [{
                "path": "package-lock.json",
                "type": "file",
                "package_data": shared_package_data
            }],
            "packages": [{
                "type": "npm",
                "name": "left-pad",
                "version": "1.3.0",
                "purl": "pkg:npm/left-pad@1.3.0"
            }],
            "dependencies": [{
                "purl": "pkg:npm/ansi-regex@5.0.1",
                "datafile_path": "package-lock.json",
                "scope": "dependencies",
                "is_runtime": true
            }],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("failed to write provenant fallback fixture");

    (
        temp,
        scancode_file.to_string_lossy().to_string(),
        provenant_file.to_string_lossy().to_string(),
    )
}

fn create_compare_json_fixtures_with_equivalent_license_expressions() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let scancode_file = temp.path().join("scancode-license-parens.json");
    let provenant_file = temp.path().join("provenant-license-parens.json");

    fs::write(
        &scancode_file,
        serde_json::json!({
            "files": [{
                "path": "src/lib.rs",
                "type": "file",
                "license_detections": [{
                    "license_expression": "(MIT OR Apache-2.0)",
                    "license_expression_spdx": "(MIT OR Apache-2.0)",
                    "detection_count": 1
                }]
            }],
            "license_detections": [{
                "license_expression": "(MIT OR Apache-2.0)",
                "license_expression_spdx": "(MIT OR Apache-2.0)",
                "detection_count": 1
            }],
            "packages": [],
            "dependencies": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("failed to write scancode equivalent-license fixture");

    fs::write(
        &provenant_file,
        serde_json::json!({
            "files": [{
                "path": "src/lib.rs",
                "type": "file",
                "license_detections": [{
                    "license_expression": "Apache-2.0 OR MIT",
                    "license_expression_spdx": "Apache-2.0 OR MIT",
                    "detection_count": 1
                }]
            }],
            "license_detections": [{
                "license_expression": "Apache-2.0 OR MIT",
                "license_expression_spdx": "Apache-2.0 OR MIT",
                "detection_count": 1
            }],
            "packages": [],
            "dependencies": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("failed to write provenant equivalent-license fixture");

    (
        temp,
        scancode_file.to_string_lossy().to_string(),
        provenant_file.to_string_lossy().to_string(),
    )
}

fn normalize_multi_parser_header(output: &mut Value) {
    let header = output["headers"]
        .as_array_mut()
        .and_then(|headers| headers.first_mut())
        .expect("headers[0] should exist");

    header["tool_version"] = Value::String("<tool_version>".to_string());
    header["start_timestamp"] = Value::String("<start_timestamp>".to_string());
    header["end_timestamp"] = Value::String("<end_timestamp>".to_string());
    header["duration"] = Value::String("<duration>".to_string());
    header["options"]["--json-pp"] = Value::String("<output_file>".to_string());
    header["extra_data"]["spdx_license_list_version"] =
        Value::String("<spdx_license_list_version>".to_string());
    header["extra_data"]["system_environment"]["operating_system"] =
        Value::String("<operating_system>".to_string());
    header["extra_data"]["system_environment"]["cpu_architecture"] =
        Value::String("<cpu_architecture>".to_string());
    header["extra_data"]["system_environment"]["platform"] =
        Value::String("<platform>".to_string());
    header["extra_data"]["system_environment"]["platform_version"] =
        Value::String("<platform_version>".to_string());
    header["extra_data"]["system_environment"]["rust_version"] =
        Value::String("<rust_version>".to_string());
}

#[test]
fn version_flag_reports_git_aware_build_version() {
    let output = provenant_command()
        .arg("--version")
        .output()
        .expect("failed to run provenant --version");

    assert!(output.status.success(), "--version should succeed");

    let stdout = String::from_utf8(output.stdout).expect("version output should be utf-8");
    let first_line = stdout
        .lines()
        .next()
        .expect("version output should contain at least one line");

    let reported_version = first_line
        .split_whitespace()
        .last()
        .expect("version line should include a version token");

    assert_eq!(reported_version, provenant::version::BUILD_VERSION);
}

#[test]
fn json_header_uses_git_aware_build_version() {
    let (_temp, scan_dir) = create_scan_fixture();

    let output = provenant_command()
        .args(["--json-pp", "-", &scan_dir])
        .output()
        .expect("failed to run provenant for json header version test");

    assert!(output.status.success(), "json scan should succeed");

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    let tool_version = json["headers"]
        .as_array()
        .and_then(|headers| headers.first())
        .and_then(|header| header["tool_version"].as_str())
        .expect("headers[0].tool_version should exist");

    assert_eq!(tool_version, provenant::version::BUILD_VERSION);
}

#[test]
fn short_version_flag_stays_single_line_and_parse_safe() {
    let output = provenant_command()
        .arg("-V")
        .output()
        .expect("failed to run provenant -V");

    assert!(output.status.success(), "-V should succeed");

    let stdout = String::from_utf8(output.stdout).expect("short version output should be utf-8");
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "-V should remain single-line for xtask parsing"
    );

    let reported_version = lines[0]
        .split_whitespace()
        .last()
        .expect("short version line should include a version token");
    assert_eq!(reported_version, provenant::version::BUILD_VERSION);
}

#[test]
fn quiet_mode_suppresses_stderr_output() {
    let (temp, scan_dir) = create_scan_fixture();
    let output_file = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--json-pp",
            output_file.to_str().expect("utf8 output path"),
            "--quiet",
            &scan_dir,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "quiet mode should not emit stderr"
    );
}

#[test]
fn default_mode_emits_summary_to_stderr() {
    let (temp, scan_dir) = create_scan_fixture();
    let output_file = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--json-pp",
            output_file.to_str().expect("utf8 output path"),
            "--package",
            &scan_dir,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Scanning 1 file..."));
    assert!(stderr.contains("Scan complete."));
    assert!(stderr.contains("Summary:"));
    assert!(!stderr.contains("Scanning done."));

    let scan_timestamp_re = Regex::new(r"scan_(start|end):\s+\d{4}-\d{2}-\d{2}T\d{6}\.\d{6}")
        .expect("timestamp regex should compile");
    let matches = scan_timestamp_re.find_iter(&stderr).count();
    assert_eq!(matches, 2, "summary should emit ScanCode-style timestamps");
}

#[test]
fn default_mode_emits_hierarchical_timing_summary() {
    let (temp, scan_dir) = create_scan_fixture();
    let output_file = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--json-pp",
            output_file.to_str().expect("utf8 output path"),
            "--only-findings",
            "--package",
            &scan_dir,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Timings:"));
    assert!(stderr.contains("  setup:"));
    assert!(stderr.contains("  inventory:"));
    assert!(stderr.contains("  scan:"));
    assert!(stderr.contains("  post-scan:"));
    assert!(stderr.contains("  finalize:"));
    assert!(stderr.contains("  output:"));
    assert!(stderr.contains("  total:"));
    assert!(stderr.contains("    scan:packages:"));
    assert!(stderr.contains("    output-filter:only-findings:"));
}

#[test]
fn verbose_mode_suppresses_success_paths_on_non_tty_stderr() {
    let (temp, scan_dir) = create_scan_fixture();
    let output_file = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--json-pp",
            output_file.to_str().expect("utf8 output path"),
            "--verbose",
            "--package",
            &scan_dir,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Scanning 1 file..."),
        "stderr was: {stderr}"
    );
    assert!(stderr.contains("Scan complete."), "stderr was: {stderr}");
    assert!(
        !stderr.contains("a.txt"),
        "non-TTY verbose output should suppress successful per-file paths: {stderr}"
    );
}

#[test]
fn default_mode_keeps_parser_failures_concise_on_stderr() {
    let (temp, scan_dir) = create_malformed_package_fixture();
    let output_file = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--json-pp",
            output_file.to_str().expect("utf8 output path"),
            "--package",
            &scan_dir,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to read or parse package.json:"),
        "default mode should report a concise failure reason"
    );
    assert!(
        stderr.contains("package.json"),
        "default mode should report the failing path"
    );
    assert!(
        !stderr.contains("key must be a string at line 1 column 3"),
        "default mode should avoid duplicating parser failure details"
    );
}

#[test]
fn verbose_mode_includes_structured_parser_failure_details() {
    let (temp, scan_dir) = create_malformed_package_fixture();
    let output_file = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--json-pp",
            output_file.to_str().expect("utf8 output path"),
            "--verbose",
            "--package",
            &scan_dir,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("package.json"));
    assert!(
        stderr.contains("Failed to read or parse package.json"),
        "verbose mode should include structured parser failure details"
    );
}

#[test]
fn incremental_mode_reuses_unchanged_files_and_keeps_them_in_output() {
    let (temp, scan_dir) = create_scan_fixture();
    let cache_dir = temp.path().join("shared-cache");
    let first_output = temp.path().join("first.json");
    let second_output = temp.path().join("second.json");

    let first = provenant_command()
        .args([
            "--json-pp",
            first_output.to_str().expect("utf8 output path"),
            "--cache-dir",
            cache_dir.to_str().expect("utf8 cache path"),
            "--incremental",
            "--email",
            &scan_dir,
        ])
        .output()
        .expect("failed to run first incremental scan");
    assert!(first.status.success());

    let second = provenant_command()
        .args([
            "--json-pp",
            second_output.to_str().expect("utf8 output path"),
            "--cache-dir",
            cache_dir.to_str().expect("utf8 cache path"),
            "--incremental",
            "--email",
            &scan_dir,
        ])
        .output()
        .expect("failed to run second incremental scan");
    assert!(second.status.success());

    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("Incremental:"), "stderr was: {stderr}");
    assert!(
        stderr.contains("1 unchanged file(s) reused"),
        "stderr was: {stderr}"
    );

    let output_json: Value = serde_json::from_slice(
        &fs::read(&second_output).expect("failed to read second incremental output"),
    )
    .expect("failed to parse second incremental output");
    let files = output_json["files"]
        .as_array()
        .expect("files should be an array");
    assert!(files.iter().any(|file| {
        file["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("a.txt"))
    }));
}

#[test]
fn ignore_build_glob_excludes_build_subtree_from_cli_output() {
    let (temp, scan_dir) = create_ignore_fixture();
    let output_file = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--json-pp",
            output_file.to_str().expect("utf8 output path"),
            "--ignore",
            "build/*",
            &scan_dir,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());
    let output_json: Value =
        serde_json::from_slice(&fs::read(&output_file).expect("failed to read output json"))
            .expect("output json should parse");
    let files = output_json["files"]
        .as_array()
        .expect("files should be an array");
    let paths: Vec<&str> = files
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();

    assert!(
        paths
            .iter()
            .any(|path| path.ends_with("/keep.txt") || *path == "keep.txt"),
        "paths: {paths:#?}"
    );
    assert!(
        paths
            .iter()
            .any(|path| path.ends_with("/build") || *path == "build"),
        "paths: {paths:#?}"
    );
    assert!(
        !paths
            .iter()
            .any(|path| path.ends_with("/build/generated.txt") || *path == "build/generated.txt"),
        "build descendants should be excluded: {paths:#?}"
    );
}

#[test]
fn ignore_root_csv_glob_excludes_root_csv_from_cli_output() {
    let (temp, scan_dir) = create_ignore_fixture();
    let output_file = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--json-pp",
            output_file.to_str().expect("utf8 output path"),
            "--ignore",
            "*.csv",
            &scan_dir,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());
    let output_json: Value =
        serde_json::from_slice(&fs::read(&output_file).expect("failed to read output json"))
            .expect("output json should parse");
    let files = output_json["files"]
        .as_array()
        .expect("files should be an array");
    let paths: Vec<&str> = files
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();

    assert!(
        paths
            .iter()
            .any(|path| path.ends_with("/keep.txt") || *path == "keep.txt"),
        "paths: {paths:#?}"
    );
    assert!(
        !paths
            .iter()
            .any(|path| path.ends_with("/report.csv") || *path == "report.csv"),
        "root csv should be excluded: {paths:#?}"
    );
}

#[test]
fn multi_parser_expected_header_fixture_matches_cli_output() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let output_file = temp.path().join("multi-parser.json");

    let output = provenant_command()
        .args([
            "--json-pp",
            output_file.to_str().expect("utf8 output path"),
            "--package",
            "--info",
            "testdata/integration/multi-parser",
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());

    let mut actual: Value =
        serde_json::from_slice(&fs::read(&output_file).expect("failed to read output json"))
            .expect("output json should parse");
    let mut expected: Value = serde_json::from_str(
        &fs::read_to_string("testdata/integration/multi-parser.expected.json")
            .expect("failed to read expected fixture"),
    )
    .expect("expected fixture should parse");

    normalize_multi_parser_header(&mut actual);
    normalize_multi_parser_header(&mut expected);

    assert_eq!(actual["headers"], expected["headers"]);
}

#[test]
fn from_json_preserves_imported_license_index_provenance() {
    let (_temp, input_file) = create_from_json_fixture_with_provenance();

    let output = provenant_command()
        .args(["--json-pp", "-", "--from-json", &input_file])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    let extra_data = &json["headers"][0]["extra_data"];

    assert_eq!(
        extra_data["spdx_license_list_version"].as_str(),
        Some("9.99")
    );
    assert_eq!(
        extra_data["license_index_provenance"]["source"].as_str(),
        Some("custom-license-dataset")
    );
    assert_eq!(
        extra_data["license_index_provenance"]["dataset_fingerprint"].as_str(),
        Some("imported-fingerprint")
    );
    assert_eq!(
        extra_data["license_index_provenance"]["ignored_rules"][0].as_str(),
        Some("imported-rule.RULE")
    );
}

#[test]
fn from_json_warning_summary_matches_output_header_warnings() {
    let (_temp, input_file) = create_from_json_fixture_with_warning();

    let output = provenant_command()
        .args(["--json-pp", "-", "--from-json", &input_file])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Some files reported recoverable scan warnings:"),
        "stderr was: {stderr}"
    );
    assert!(stderr.contains("Warnings count: 1"), "stderr was: {stderr}");

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(
        json["headers"][0]["warnings"][0].as_str(),
        Some("custom recoverable warning: virtual_root/src/main.rs")
    );
}

#[test]
fn explicit_scan_subcommand_matches_legacy_bare_scan_behavior() {
    let (_temp, scan_dir) = create_scan_fixture();

    let bare_output = provenant_command()
        .args(["--json-pp", "-", "--info", &scan_dir])
        .output()
        .expect("failed to run bare scan command");
    assert!(bare_output.status.success());

    let explicit_output = provenant_command()
        .args(["scan", "--json-pp", "-", "--info", &scan_dir])
        .output()
        .expect("failed to run explicit scan command");
    assert!(explicit_output.status.success());

    let bare_json: Value = serde_json::from_slice(&bare_output.stdout).expect("bare stdout json");
    let explicit_json: Value =
        serde_json::from_slice(&explicit_output.stdout).expect("explicit stdout json");

    assert_eq!(
        bare_json["headers"][0]["options"],
        explicit_json["headers"][0]["options"]
    );
    assert_eq!(bare_json["files"], explicit_json["files"]);
}

#[test]
fn compare_subcommand_writes_artifacts_and_summary() {
    let (_temp, scancode_json, provenant_json) = create_compare_json_fixtures();
    let artifact_temp = TempDir::new().expect("artifact temp dir");
    let artifact_dir = artifact_temp.path().join("compare-artifacts");

    let output = provenant_command()
        .args([
            "compare",
            "--scancode-json",
            &scancode_json,
            "--provenant-json",
            &provenant_json,
            "--artifact-dir",
            artifact_dir.to_str().expect("utf8 artifact path"),
        ])
        .output()
        .expect("failed to run compare subcommand");

    assert!(output.status.success(), "compare should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Comparison status:"),
        "stdout was: {stdout}"
    );

    let summary_path = artifact_dir.join("comparison").join("summary.json");
    let manifest_path = artifact_dir.join("run-manifest.json");
    assert!(summary_path.is_file());
    assert!(manifest_path.is_file());
    assert!(artifact_dir.join("raw").join("scancode.json").is_file());
    assert!(artifact_dir.join("raw").join("provenant.json").is_file());

    let summary: Value =
        serde_json::from_str(&fs::read_to_string(&summary_path).expect("summary json")).unwrap();
    assert_eq!(
        summary["comparison_status"].as_str(),
        Some("potential_regressions_detected")
    );
}

#[test]
fn compare_subcommand_defaults_to_timestamped_artifact_dir_in_cwd() {
    let (_temp, scancode_json, provenant_json) = create_compare_json_fixtures();
    let working_dir = TempDir::new().expect("working dir temp");

    let output = Command::new(env!("CARGO_BIN_EXE_provenant"))
        .current_dir(working_dir.path())
        .args([
            "compare",
            "--scancode-json",
            &scancode_json,
            "--provenant-json",
            &provenant_json,
        ])
        .output()
        .expect("failed to run compare subcommand with default artifact dir");

    assert!(output.status.success(), "compare should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Artifact directory:"),
        "stdout was: {stdout}"
    );

    let entries: Vec<_> = fs::read_dir(working_dir.path())
        .expect("working dir should be readable")
        .map(|entry| entry.expect("entry should be readable").path())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "expected exactly one artifact dir: {entries:?}"
    );

    let artifact_dir = &entries[0];
    assert!(artifact_dir.is_dir());
    assert!(
        artifact_dir
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with("provenant-compare-"))
    );
    assert!(
        artifact_dir
            .join("comparison")
            .join("summary.json")
            .is_file()
    );
}

#[test]
fn compare_subcommand_uses_file_level_package_fallback_without_false_regressions() {
    let (_temp, scancode_json, provenant_json) =
        create_compare_json_fixtures_with_file_level_package_fallback();
    let artifact_temp = TempDir::new().expect("artifact temp dir");
    let artifact_dir = artifact_temp.path().join("compare-artifacts");

    let output = provenant_command()
        .args([
            "compare",
            "--scancode-json",
            &scancode_json,
            "--provenant-json",
            &provenant_json,
            "--artifact-dir",
            artifact_dir.to_str().expect("utf8 artifact path"),
        ])
        .output()
        .expect("failed to run compare subcommand");

    assert!(output.status.success(), "compare should succeed");

    let summary_path = artifact_dir.join("comparison").join("summary.json");
    let summary: Value =
        serde_json::from_str(&fs::read_to_string(&summary_path).expect("summary json")).unwrap();

    assert_eq!(
        summary["comparison_status"].as_str(),
        Some("no_detected_differences")
    );
    assert_eq!(
        summary["top_level_counts"]["scancode"]["packages"].as_i64(),
        Some(0)
    );
    assert_eq!(
        summary["top_level_counts"]["scancode"]["dependencies"].as_i64(),
        Some(0)
    );
    assert_eq!(
        summary["top_level_counts"]["sources"]["scancode"]["packages"].as_str(),
        Some("packages[] empty; files[].package_data present")
    );
    assert_eq!(
        summary["top_level_counts"]["sources"]["scancode"]["dependencies"].as_str(),
        Some("dependencies[] empty; files[].package_data[].dependencies present")
    );
    assert_eq!(
        summary["skipped_comparisons"]["packages"].as_str(),
        Some(
            "top-level packages comparison skipped: ScanCode packages[] empty; files[].package_data present; Provenant packages[]"
        )
    );
    assert_eq!(
        summary["skipped_comparisons"]["dependencies"].as_str(),
        Some(
            "top-level dependencies comparison skipped: ScanCode dependencies[] empty; files[].package_data[].dependencies present; Provenant dependencies[]"
        )
    );
    assert_eq!(
        summary["raw_dependency_summary"]["missing_in_provenant"].as_u64(),
        Some(0)
    );
    assert_eq!(
        summary["raw_dependency_summary"]["extra_in_provenant"].as_u64(),
        Some(0)
    );

    let summary_tsv = fs::read_to_string(artifact_dir.join("comparison").join("summary.tsv"))
        .expect("summary tsv");
    assert!(summary_tsv.contains(
        "top-level packages comparison skipped: ScanCode packages[] empty; files[].package_data present; Provenant packages[]"
    ));
    assert!(summary_tsv.contains(
        "top-level dependencies comparison skipped: ScanCode dependencies[] empty; files[].package_data[].dependencies present; Provenant dependencies[]"
    ));
}

#[test]
fn compare_subcommand_treats_trivial_license_expression_parentheses_as_equal() {
    let (_temp, scancode_json, provenant_json) =
        create_compare_json_fixtures_with_equivalent_license_expressions();
    let artifact_temp = TempDir::new().expect("artifact temp dir");
    let artifact_dir = artifact_temp.path().join("compare-artifacts");

    let output = provenant_command()
        .args([
            "compare",
            "--scancode-json",
            &scancode_json,
            "--provenant-json",
            &provenant_json,
            "--artifact-dir",
            artifact_dir.to_str().expect("utf8 artifact path"),
        ])
        .output()
        .expect("failed to run compare subcommand");

    assert!(output.status.success(), "compare should succeed");

    let summary_path = artifact_dir.join("comparison").join("summary.json");
    let samples_path = artifact_dir
        .join("comparison")
        .join("samples")
        .join("file_metric_value_differences.json");
    let deltas_path = artifact_dir
        .join("comparison")
        .join("samples")
        .join("top_level_license_expression_deltas.json");

    let summary: Value =
        serde_json::from_str(&fs::read_to_string(&summary_path).expect("summary json")).unwrap();
    let samples: Value =
        serde_json::from_str(&fs::read_to_string(&samples_path).expect("samples json")).unwrap();
    let deltas: Value =
        serde_json::from_str(&fs::read_to_string(&deltas_path).expect("deltas json")).unwrap();

    assert_eq!(
        summary["comparison_status"].as_str(),
        Some("no_detected_differences")
    );
    assert_eq!(
        summary["file_metric_summary"]["license_detections"]["missing_in_provenant"].as_u64(),
        Some(0)
    );
    assert_eq!(
        summary["file_metric_summary"]["license_detections"]["extra_in_provenant"].as_u64(),
        Some(0)
    );
    assert_eq!(
        summary["top_level_license_expression_delta_count"].as_u64(),
        Some(0)
    );
    assert_eq!(samples["license_detections"], serde_json::json!([]));
    assert_eq!(deltas, serde_json::json!([]));
}

#[test]
fn export_license_dataset_writes_expected_dataset_structure() {
    let temp = TempDir::new().expect("temp dir");
    let export_dir = temp.path().join("dataset");

    let output = provenant_command()
        .args([
            "export-license-dataset",
            export_dir.to_str().expect("utf8 export path"),
        ])
        .output()
        .expect("failed to run dataset export");

    assert!(output.status.success(), "dataset export should succeed");
    assert!(export_dir.join("manifest.json").is_file());
    assert!(export_dir.join("README.md").is_file());
    assert!(export_dir.join("rules").is_dir());
    assert!(export_dir.join("licenses").is_dir());
    assert!(
        fs::read_dir(export_dir.join("rules"))
            .expect("rules dir should be readable")
            .next()
            .is_some()
    );
    assert!(
        fs::read_dir(export_dir.join("licenses"))
            .expect("licenses dir should be readable")
            .next()
            .is_some()
    );
}

#[test]
fn exported_dataset_can_be_reused_via_license_dataset_path() {
    let export_temp = TempDir::new().expect("export temp dir");
    let export_dir = export_temp.path().join("dataset");
    let export_output = provenant_command()
        .args([
            "export-license-dataset",
            export_dir.to_str().expect("utf8 export path"),
        ])
        .output()
        .expect("failed to export dataset");
    assert!(
        export_output.status.success(),
        "dataset export should succeed"
    );

    let (_scan_temp, scan_dir) = create_mit_license_fixture();

    let embedded_output = provenant_command()
        .args(["--json-pp", "-", "--license", &scan_dir])
        .output()
        .expect("embedded scan should run");
    assert!(embedded_output.status.success());
    let embedded_json: Value =
        serde_json::from_slice(&embedded_output.stdout).expect("embedded stdout json");

    let custom_output = provenant_command()
        .args([
            "--json-pp",
            "-",
            "--license",
            "--license-dataset-path",
            export_dir.to_str().expect("utf8 export path"),
            &scan_dir,
        ])
        .output()
        .expect("custom dataset scan should run");
    assert!(custom_output.status.success());
    let custom_json: Value =
        serde_json::from_slice(&custom_output.stdout).expect("custom stdout json");

    assert_eq!(
        embedded_json["files"][0]["license_detections"],
        custom_json["files"][0]["license_detections"]
    );
    assert_eq!(
        custom_json["headers"][0]["extra_data"]["license_index_provenance"]["source"].as_str(),
        Some("custom-license-dataset")
    );
}
