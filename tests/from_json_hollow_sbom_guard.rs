// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Guards `--from-json` reshapes that request an SBOM format (`--spdx-tv`,
//! `--spdx-rdf`, `--cyclonedx`, `--cyclonedx-xml`) against silently emitting a
//! hollow document when the reshaped input never ran package detection.
//!
//! See `hollow_from_json_sbom_refusal` in `src/cli/run/mod.rs`.

use std::fs;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

fn provenant_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_provenant"));
    command.current_dir(env!("CARGO_MANIFEST_DIR"));
    command
}

/// Builds a minimal `--from-json`-loadable scan document.
///
/// `header_options` mirrors the `header.options` map a real scan would record
/// (e.g. `{"--package": true}`), and `packages` is the top-level packages
/// array. `files` lets tests choose between a scan with real content and a
/// truly empty scan document.
fn write_from_json_fixture(
    dir: &TempDir,
    header_options: Value,
    files: Vec<Value>,
    packages: Vec<Value>,
) -> String {
    write_named_from_json_fixture(dir, "input.json", header_options, files, packages)
}

/// Same as [`write_from_json_fixture`], but with a caller-chosen file name so
/// a single test can write more than one fixture (e.g. to merge two
/// `--from-json` inputs).
fn write_named_from_json_fixture(
    dir: &TempDir,
    file_name: &str,
    header_options: Value,
    files: Vec<Value>,
    packages: Vec<Value>,
) -> String {
    let input_file = dir.path().join(file_name);
    fs::write(
        &input_file,
        json!({
            "headers": [{
                "tool_name": "provenant",
                "tool_version": "0.0.0-test",
                "options": header_options,
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
                    "files_count": files.len(),
                    "directories_count": 0,
                    "excluded_count": 0
                }
            }],
            "files": files,
            "packages": packages,
            "dependencies": [],
            "license_detections": [],
            "license_references": [],
            "license_rule_references": []
        })
        .to_string(),
    )
    .expect("failed to write from-json fixture");

    input_file.to_string_lossy().to_string()
}

fn sample_file(path: &str) -> Value {
    json!({
        "path": path,
        "type": "file",
        "name": path,
        "base_name": path,
        "extension": "",
        "size": 10,
        "programming_language": "Rust"
    })
}

fn sample_package() -> Value {
    json!({
        "package_uid": "pkg:npm/demo@1.0.0",
        "type": "npm",
        "name": "demo",
        "version": "1.0.0",
        "parties": [],
        "datafile_paths": ["package.json"],
        "datasource_ids": ["npm_package_json"]
    })
}

#[test]
fn from_json_cyclonedx_fails_when_source_never_ran_package_detection() {
    let temp = TempDir::new().expect("temp dir");
    let input_file =
        write_from_json_fixture(&temp, json!({}), vec![sample_file("src/main.rs")], vec![]);
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--from-json",
            &input_file,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        !output.status.success(),
        "a hollow CycloneDX reshape must fail, not silently succeed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hollow"),
        "error should explain the hollow-SBOM guard, got: {stderr}"
    );
    assert!(
        stderr.contains("--cyclonedx"),
        "error should name the offending flag, got: {stderr}"
    );
}

#[test]
fn from_json_spdx_tv_fails_when_source_never_ran_package_detection() {
    let temp = TempDir::new().expect("temp dir");
    let input_file =
        write_from_json_fixture(&temp, json!({}), vec![sample_file("src/main.rs")], vec![]);
    let output_file = temp.path().join("sbom.spdx");

    let output = provenant_command()
        .args([
            "--spdx-tv",
            output_file.to_str().expect("utf-8 path"),
            "--from-json",
            &input_file,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        !output.status.success(),
        "a hollow SPDX reshape must fail, not silently succeed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hollow"),
        "error should explain the hollow-SBOM guard, got: {stderr}"
    );
    assert!(
        stderr.contains("--spdx-tv"),
        "error should name the offending flag, got: {stderr}"
    );
}

#[test]
fn from_json_cyclonedx_succeeds_when_source_ran_package_detection_and_found_none() {
    let temp = TempDir::new().expect("temp dir");
    let input_file = write_from_json_fixture(
        &temp,
        json!({"--package": true}),
        vec![sample_file("README.md")],
        vec![],
    );
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--from-json",
            &input_file,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        output.status.success(),
        "package detection honestly finding zero packages must not be treated as hollow: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bom: Value =
        serde_json::from_str(&fs::read_to_string(&output_file).expect("read bom file"))
            .expect("bom should be valid json");
    assert_eq!(bom["components"], json!([]));
}

#[test]
fn from_json_cyclonedx_succeeds_with_real_packages_in_input() {
    let temp = TempDir::new().expect("temp dir");
    let input_file = write_from_json_fixture(
        &temp,
        json!({}),
        vec![sample_file("package.json")],
        vec![sample_package()],
    );
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--from-json",
            &input_file,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        output.status.success(),
        "a --from-json input with real packages must still emit a real SBOM: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bom: Value =
        serde_json::from_str(&fs::read_to_string(&output_file).expect("read bom file"))
            .expect("bom should be valid json");
    let components = bom["components"].as_array().expect("components array");
    assert_eq!(components.len(), 1);
    assert_eq!(components[0]["name"], "demo");
}

#[test]
fn from_json_spdx_tv_succeeds_with_real_packages_in_input() {
    let temp = TempDir::new().expect("temp dir");
    let input_file = write_from_json_fixture(
        &temp,
        json!({}),
        vec![sample_file("package.json")],
        vec![sample_package()],
    );
    let output_file = temp.path().join("sbom.spdx");

    let output = provenant_command()
        .args([
            "--spdx-tv",
            output_file.to_str().expect("utf-8 path"),
            "--from-json",
            &input_file,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        output.status.success(),
        "a --from-json input with real packages must still emit a real SBOM: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sbom = fs::read_to_string(&output_file).expect("read sbom file");
    assert!(sbom.contains("PackageName: demo"));
}

#[test]
fn from_json_cyclonedx_allows_truly_empty_scan_document() {
    let temp = TempDir::new().expect("temp dir");
    let input_file = write_from_json_fixture(&temp, json!({}), vec![], vec![]);
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--from-json",
            &input_file,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        output.status.success(),
        "a truly empty scan document is a legitimate empty SBOM, not a hollow one: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn from_json_spdx_tv_allows_truly_empty_scan_document() {
    let temp = TempDir::new().expect("temp dir");
    let input_file = write_from_json_fixture(&temp, json!({}), vec![], vec![]);
    let output_file = temp.path().join("sbom.spdx");

    let output = provenant_command()
        .args([
            "--spdx-tv",
            output_file.to_str().expect("utf-8 path"),
            "--from-json",
            &input_file,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        output.status.success(),
        "a truly empty scan document is a legitimate empty SBOM, not a hollow one: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sbom = fs::read_to_string(&output_file).expect("read sbom file");
    assert!(sbom.contains("No results for package"));
}

#[test]
fn from_json_plain_json_output_is_unaffected_by_the_hollow_sbom_guard() {
    let temp = TempDir::new().expect("temp dir");
    let input_file =
        write_from_json_fixture(&temp, json!({}), vec![sample_file("src/main.rs")], vec![]);

    let output = provenant_command()
        .args(["--json-pp", "-", "--from-json", &input_file])
        .output()
        .expect("failed to run provenant");

    assert!(
        output.status.success(),
        "the guard must only apply to SBOM formats, not plain JSON reshapes: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn from_json_cyclonedx_fails_when_one_merged_input_is_hollow_even_if_another_requested_detection() {
    // Regression: a merge that includes a hollow source (files present,
    // package detection never requested) must not be silenced just because
    // another merged input honestly requested detection.
    let temp = TempDir::new().expect("temp dir");
    let hollow_input = write_named_from_json_fixture(
        &temp,
        "hollow.json",
        json!({}),
        vec![sample_file("src/main.rs")],
        vec![],
    );
    let requested_input = write_named_from_json_fixture(
        &temp,
        "requested.json",
        json!({"--package": true}),
        vec![sample_file("README.md")],
        vec![],
    );
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--from-json",
            &hollow_input,
            &requested_input,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        !output.status.success(),
        "a merge including a hollow source must fail even though another merged input requested \
         package detection"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hollow"),
        "error should explain the hollow-SBOM guard, got: {stderr}"
    );
}

#[test]
fn from_json_hollow_sbom_target_is_skipped_but_other_outputs_still_write() {
    // A refused SBOM target must not prevent other, non-SBOM output targets
    // in the same request from being written.
    let temp = TempDir::new().expect("temp dir");
    let input_file =
        write_from_json_fixture(&temp, json!({}), vec![sample_file("src/main.rs")], vec![]);
    let bom_file = temp.path().join("bom.json");
    let json_file_path = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            bom_file.to_str().expect("utf-8 path"),
            "--json-pp",
            json_file_path.to_str().expect("utf-8 path"),
            "--from-json",
            &input_file,
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        !output.status.success(),
        "the run must still fail overall when an SBOM target is refused"
    );
    assert_eq!(
        output.status.code(),
        Some(4),
        "hollow-SBOM guard failures use a dedicated exit code distinct from \
         scan/runtime errors (1) and the license-policy gate (3)"
    );
    assert!(
        !bom_file.exists(),
        "the refused hollow CycloneDX target must not be written"
    );
    assert!(
        json_file_path.exists(),
        "the non-SBOM --json-pp target must still be written even though the SBOM target was refused"
    );
    let plain_json: Value =
        serde_json::from_str(&fs::read_to_string(&json_file_path).expect("read plain json output"))
            .expect("plain json output should be valid json");
    assert!(plain_json.get("files").is_some());
}
