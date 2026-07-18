// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Guards native scans that request an SBOM format (`--spdx-tv`,
//! `--spdx-rdf`, `--cyclonedx`, `--cyclonedx-xml`) against two CLI footguns
//! that can produce a schema-valid but hollow or understated inventory:
//!
//! - `--package-only`/`--no-assemble` unconditionally skip top-level package
//!   assembly for the whole run, so the SBOM would read an empty
//!   `packages`/`dependencies` view even though the scanned files may carry
//!   their own per-file package manifests (refused; see
//!   `assembly_skipped_sbom_refusal` in `src/cli/run/mod.rs`).
//! - `--paths-file` narrows collection to a caller-selected subset of files,
//!   so assembly can silently understate a monorepo if the selection omits
//!   sibling manifests or a workspace root (warned, not refused; see
//!   `paths_file_sbom_completeness_warning` in `src/cli/run/mod.rs`).

use std::fs;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

fn provenant_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_provenant"));
    command.current_dir(env!("CARGO_MANIFEST_DIR"));
    command
}

fn npm_project(temp: &TempDir) -> std::path::PathBuf {
    let project_dir = temp.path().join("project");
    fs::create_dir_all(&project_dir).expect("create project dir");
    fs::write(
        project_dir.join("package.json"),
        r#"{"name": "demo", "version": "1.0.0"}"#,
    )
    .expect("write package.json");
    project_dir
}

#[test]
fn package_only_cyclonedx_is_refused_when_files_were_scanned() {
    let temp = TempDir::new().expect("temp dir");
    let project_dir = npm_project(&temp);
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--package-only",
            project_dir.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        !output.status.success(),
        "--package-only must refuse a hollow CycloneDX export, not silently succeed"
    );
    assert_eq!(
        output.status.code(),
        Some(4),
        "the assembly-skipped SBOM guard uses the same dedicated exit code as the \
         --from-json hollow-SBOM guard"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hollow"),
        "error should explain the assembly-skipped guard, got: {stderr}"
    );
    assert!(
        stderr.contains("--package-only"),
        "error should name the offending flag, got: {stderr}"
    );
    assert!(
        stderr.contains("--cyclonedx"),
        "error should name the refused output flag, got: {stderr}"
    );
    assert!(
        !output_file.exists(),
        "the refused hollow CycloneDX target must not be written"
    );
}

#[test]
fn no_assemble_spdx_tv_is_refused_when_files_were_scanned() {
    let temp = TempDir::new().expect("temp dir");
    let project_dir = npm_project(&temp);
    let output_file = temp.path().join("sbom.spdx");

    let output = provenant_command()
        .args([
            "--spdx-tv",
            output_file.to_str().expect("utf-8 path"),
            "--package",
            "--no-assemble",
            project_dir.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        !output.status.success(),
        "--no-assemble must refuse a hollow SPDX export, not silently succeed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hollow"),
        "error should explain the assembly-skipped guard, got: {stderr}"
    );
    assert!(
        stderr.contains("--no-assemble"),
        "error should name the offending flag, got: {stderr}"
    );
    assert!(
        !output_file.exists(),
        "the refused hollow SPDX target must not be written"
    );
}

#[test]
fn package_only_cyclonedx_refusal_is_explained_even_in_quiet_mode() {
    // Regression: `ScanProgress::init_logging_bridge` never installs a
    // logger at all in `--quiet` mode, so a plain `log::error!` refusal
    // message would otherwise vanish, leaving only the exit code.
    let temp = TempDir::new().expect("temp dir");
    let project_dir = npm_project(&temp);
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--package-only",
            "--quiet",
            project_dir.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("failed to run provenant");

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hollow") && stderr.contains("--package-only"),
        "the refusal must still explain itself under --quiet, got: {stderr}"
    );
}

#[test]
fn package_only_hollow_sbom_target_is_skipped_but_other_outputs_still_write() {
    let temp = TempDir::new().expect("temp dir");
    let project_dir = npm_project(&temp);
    let bom_file = temp.path().join("bom.json");
    let json_file_path = temp.path().join("out.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            bom_file.to_str().expect("utf-8 path"),
            "--json-pp",
            json_file_path.to_str().expect("utf-8 path"),
            "--package-only",
            project_dir.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        !output.status.success(),
        "the run must still fail overall when an SBOM target is refused"
    );
    assert_eq!(output.status.code(), Some(4));
    assert!(!bom_file.exists());
    assert!(
        json_file_path.exists(),
        "the non-SBOM --json-pp target must still be written even though the SBOM target was refused"
    );
}

#[test]
fn package_only_plain_json_output_is_unaffected_by_the_guard() {
    let temp = TempDir::new().expect("temp dir");
    let project_dir = npm_project(&temp);

    let output = provenant_command()
        .args([
            "--json-pp",
            "-",
            "--package-only",
            project_dir.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        output.status.success(),
        "the guard must only apply to SBOM formats, not plain JSON output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn package_only_cyclonedx_allows_truly_empty_scan_document() {
    let temp = TempDir::new().expect("temp dir");
    let empty_dir = temp.path().join("empty");
    fs::create_dir_all(&empty_dir).expect("create empty dir");
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--package-only",
            empty_dir.to_str().expect("utf-8 path"),
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
fn paths_file_cyclonedx_warns_but_still_writes_the_export() {
    let temp = TempDir::new().expect("temp dir");
    let project_dir = npm_project(&temp);
    let paths_file = temp.path().join("changed.txt");
    fs::write(&paths_file, "package.json\n").expect("write paths file");
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--package",
            "--paths-file",
            paths_file.to_str().expect("utf-8 path"),
            project_dir.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        output.status.success(),
        "--paths-file must warn, not fail, since assembly can still produce a real SBOM: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--paths-file"),
        "stderr should carry a loud completeness warning, got: {stderr}"
    );
    assert!(
        stderr.contains("--cyclonedx"),
        "warning should name the SBOM flag it applies to, got: {stderr}"
    );
    assert!(
        output_file.exists(),
        "the SBOM target must still be written"
    );
    let bom: Value =
        serde_json::from_str(&fs::read_to_string(&output_file).expect("read bom file"))
            .expect("bom should be valid json");
    let components = bom["components"].as_array().expect("components array");
    assert_eq!(components.len(), 1);
    assert_eq!(components[0]["name"], "demo");
}

#[test]
fn paths_file_cyclonedx_warning_is_visible_even_in_quiet_mode() {
    let temp = TempDir::new().expect("temp dir");
    let project_dir = npm_project(&temp);
    let paths_file = temp.path().join("changed.txt");
    fs::write(&paths_file, "package.json\n").expect("write paths file");
    let output_file = temp.path().join("bom.json");

    let output = provenant_command()
        .args([
            "--cyclonedx",
            output_file.to_str().expect("utf-8 path"),
            "--package",
            "--paths-file",
            paths_file.to_str().expect("utf-8 path"),
            "--quiet",
            project_dir.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("failed to run provenant");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--paths-file") && stderr.contains("--cyclonedx"),
        "the completeness warning must still be visible under --quiet, got: {stderr}"
    );
    assert!(output_file.exists());
}

#[test]
fn paths_file_plain_json_output_does_not_carry_the_completeness_warning() {
    let temp = TempDir::new().expect("temp dir");
    let project_dir = npm_project(&temp);
    let paths_file = temp.path().join("changed.txt");
    fs::write(&paths_file, "package.json\n").expect("write paths file");

    let output = provenant_command()
        .args([
            "--json-pp",
            "-",
            "--package",
            "--paths-file",
            paths_file.to_str().expect("utf-8 path"),
            project_dir.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("failed to run provenant");

    assert!(
        output.status.success(),
        "a plain JSON --paths-file scan should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("may understate the full repository"),
        "the SBOM completeness warning must not fire for non-SBOM output formats, got: {stderr}"
    );
}
