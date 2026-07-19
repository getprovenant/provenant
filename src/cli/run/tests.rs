// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::app::request::ScanRequest;
use crate::app::scan_pipeline::{
    collect_top_level_license_detections_for_mode, compile_regex_patterns,
};
use crate::app::scan_plan::{configured_scan_names, effective_timeout_seconds};
use crate::app::scan_runtime::{
    NativeScanSelection, build_paths_file_warning_messages, prepare_cache_config,
    resolve_native_scan_selection,
};
use crate::assembly;
use crate::cli::ProcessMode;
use crate::license_detection::MatcherKind;
use crate::models::{LineNumber, MatchScore};
use crate::scan_result_shaping::{apply_only_findings_filter, normalize_paths};
use serde_json::json;
use std::fs;
use std::path::Path;

use crate::cache::{CacheConfig, DEFAULT_CACHE_DIR_NAME, build_collection_exclude_patterns};
use crate::license_detection::LicenseDetectionEngine;
use crate::post_processing::{
    DEFAULT_LICENSEDB_URL_TEMPLATE, apply_package_reference_following,
    collect_top_level_license_detections, collect_top_level_license_references,
};
use crate::scan_result_shaping::json_input::{
    JsonScanInput, load_scan_from_json, normalize_loaded_json_scan,
};
use crate::scanner::collect_paths;
use crate::test_support::CurrentDirGuard;

#[test]
fn process_mode_to_i32_supports_reference_compat_values() {
    assert_eq!(ProcessMode::SequentialWithoutTimeouts.to_i32(), -1);
    assert_eq!(ProcessMode::SequentialWithTimeouts.to_i32(), 0);
    assert_eq!(ProcessMode::Parallel(4).to_i32(), 4);
    assert_eq!(
        effective_timeout_seconds(ProcessMode::SequentialWithoutTimeouts, 30.0),
        0.0
    );
    assert_eq!(
        effective_timeout_seconds(ProcessMode::SequentialWithTimeouts, 30.0),
        30.0
    );
}

#[test]
fn configured_scan_names_only_lists_enabled_non_license_scans() {
    let package_cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--package",
        "README.md",
    ])
    .unwrap();
    let package_request = ScanRequest::from(
        package_cli
            .scan_args()
            .expect("scan args should be present"),
    );
    assert_eq!(configured_scan_names(&package_request), "packages");

    let package_only_cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--package-only",
        "README.md",
    ])
    .unwrap();
    let package_only_request = ScanRequest::from(
        package_only_cli
            .scan_args()
            .expect("scan args should be present"),
    );
    assert_eq!(configured_scan_names(&package_only_request), "packages");

    let mixed_cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--info",
        "--email",
        "README.md",
    ])
    .unwrap();
    let mixed_request =
        ScanRequest::from(mixed_cli.scan_args().expect("scan args should be present"));
    assert_eq!(configured_scan_names(&mixed_request), "info, emails");
}

#[test]
fn configured_scan_names_keeps_license_first_when_enabled() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--package",
        "README.md",
    ])
    .unwrap();

    let request = ScanRequest::from(cli.scan_args().expect("scan args should be present"));
    assert_eq!(configured_scan_names(&request), "licenses, packages");
}

#[test]
fn validate_scan_option_compatibility_rejects_scan_flags_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--copyright",
        "sample-scan.json",
    ])
    .unwrap();
    assert!(validate_scan_option_compatibility(&cli).is_err());
}

#[test]
fn validate_scan_option_compatibility_allows_cache_root_flags_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--cache-dir",
        "/tmp/cache",
        "sample-scan.json",
    ])
    .unwrap();

    assert!(validate_scan_option_compatibility(&cli).is_ok());
}

#[test]
fn validate_scan_option_compatibility_allows_license_cache_opt_out_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--no-license-index-cache",
        "sample-scan.json",
    ])
    .unwrap();

    assert!(validate_scan_option_compatibility(&cli).is_ok());
}

#[test]
fn validate_scan_option_compatibility_rejects_incremental_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--incremental",
        "sample-scan.json",
    ])
    .unwrap();

    let error = validate_scan_option_compatibility(&cli).unwrap_err();
    assert!(error.to_string().contains("--incremental"));
}

#[test]
fn validate_scan_option_compatibility_rejects_package_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--package",
        "sample-scan.json",
    ])
    .unwrap();
    assert!(validate_scan_option_compatibility(&cli).is_err());
}

#[test]
fn validate_scan_option_compatibility_rejects_generated_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--generated",
        "sample-scan.json",
    ])
    .unwrap();
    assert!(validate_scan_option_compatibility(&cli).is_err());
}

#[test]
fn validate_scan_option_compatibility_allows_strip_root_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--strip-root",
        "sample-scan.json",
    ])
    .unwrap();
    assert!(validate_scan_option_compatibility(&cli).is_ok());
}

#[test]
fn validate_scan_option_compatibility_allows_full_root_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--full-root",
        "sample-scan.json",
    ])
    .unwrap();
    assert!(validate_scan_option_compatibility(&cli).is_ok());
}

#[test]
fn validate_scan_option_compatibility_allows_scan_flags_without_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--copyright",
        "sample-dir",
    ])
    .unwrap();
    assert!(validate_scan_option_compatibility(&cli).is_ok());
}

#[test]
fn validate_scan_option_compatibility_allows_multiple_inputs_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "scan-a.json",
        "scan-b.json",
    ])
    .unwrap();
    assert!(validate_scan_option_compatibility(&cli).is_ok());
}

fn from_json_sbom_request(sbom_flag: &str, output_file: &str) -> ScanRequest {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        sbom_flag,
        output_file,
        "--from-json",
        "input.json",
    ])
    .expect("cli parse should succeed");
    ScanRequest::from(cli.scan_args().expect("scan args should be present"))
}

fn native_sbom_request(sbom_flag: &str, output_file: &str, extra_args: &[&str]) -> ScanRequest {
    let mut args = vec!["provenant", sbom_flag, output_file];
    args.extend_from_slice(extra_args);
    args.push("sample-dir");
    let cli = crate::cli::Cli::try_parse_from(args).expect("cli parse should succeed");
    ScanRequest::from(cli.scan_args().expect("scan args should be present"))
}

fn output_with_files(files: Vec<crate::models::FileInfo>) -> crate::models::Output {
    crate::models::Output {
        summary: None,
        tallies: None,
        tallies_of_key_files: None,
        tallies_by_facet: None,
        headers: vec![],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        files,
        license_references: vec![],
        license_rule_references: vec![],
    }
}

#[test]
fn hollow_from_json_sbom_refusal_fires_when_source_never_ran_package_detection() {
    let request = from_json_sbom_request("--cyclonedx", "bom.json");
    let output = output_with_files(vec![json_file(
        "src/main.rs",
        crate::models::FileType::File,
    )]);

    let refusal = hollow_from_json_sbom_refusal(&request, &output, true)
        .expect("hollow cyclonedx reshape must be refused");
    assert!(refusal.contains("hollow"));
    assert!(refusal.contains("--cyclonedx"));
}

#[test]
fn hollow_from_json_sbom_refusal_fires_even_when_output_packages_are_non_empty() {
    // Regression for a merge that includes a hollow source (files present,
    // package detection never requested) alongside another merged input that
    // did request detection and found real packages: the merged
    // `output.packages` ends up non-empty, but the hollow input's files were
    // still never examined, so the refusal must still fire.
    let request = from_json_sbom_request("--cyclonedx", "bom.json");
    let mut output = output_with_files(vec![json_file(
        "src/main.rs",
        crate::models::FileType::File,
    )]);
    output.packages = vec![crate::models::Package::from_package_data(
        &crate::models::PackageData {
            package_type: Some(crate::models::PackageType::Npm),
            name: Some("demo".to_string()),
            version: Some("1.0.0".to_string()),
            ..Default::default()
        },
        "package.json".to_string(),
    )];

    let refusal = hollow_from_json_sbom_refusal(&request, &output, true)
        .expect("hollow merged input must not be silenced by another input's real packages");
    assert!(refusal.contains("hollow"));
}

#[test]
fn hollow_from_json_sbom_refusal_allows_native_scans() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--cyclonedx",
        "bom.json",
        "--package",
        "sample-dir",
    ])
    .expect("cli parse should succeed");
    let request = ScanRequest::from(cli.scan_args().expect("scan args should be present"));
    let output = output_with_files(vec![json_file(
        "src/main.rs",
        crate::models::FileType::File,
    )]);

    assert!(hollow_from_json_sbom_refusal(&request, &output, false).is_none());
}

#[test]
fn hollow_from_json_sbom_refusal_allows_non_sbom_output_formats() {
    let request = from_json_sbom_request("--json-pp", "-");
    let output = output_with_files(vec![json_file(
        "src/main.rs",
        crate::models::FileType::File,
    )]);

    assert!(hollow_from_json_sbom_refusal(&request, &output, true).is_none());
}

#[test]
fn hollow_from_json_sbom_refusal_allows_truly_empty_scan_documents() {
    let request = from_json_sbom_request("--spdx-tv", "sbom.spdx");
    let output = output_with_files(vec![]);

    assert!(hollow_from_json_sbom_refusal(&request, &output, true).is_none());
}

#[test]
fn hollow_from_json_sbom_refusal_allows_honest_zero_packages_when_detection_ran() {
    let request = from_json_sbom_request("--spdx-rdf", "sbom.rdf");
    let output = output_with_files(vec![json_file("README.md", crate::models::FileType::File)]);

    assert!(hollow_from_json_sbom_refusal(&request, &output, false).is_none());
}

#[test]
fn hollow_from_json_sbom_refusal_allows_requests_with_real_packages() {
    let request = from_json_sbom_request("--cyclonedx-xml", "bom.xml");
    let mut output = output_with_files(vec![json_file(
        "package.json",
        crate::models::FileType::File,
    )]);
    output.packages = vec![crate::models::Package::from_package_data(
        &crate::models::PackageData {
            package_type: Some(crate::models::PackageType::Npm),
            name: Some("demo".to_string()),
            version: Some("1.0.0".to_string()),
            ..Default::default()
        },
        "package.json".to_string(),
    )];

    assert!(hollow_from_json_sbom_refusal(&request, &output, false).is_none());
}

#[test]
fn assembly_skipped_sbom_refusal_fires_for_package_only_with_scanned_files() {
    let request = native_sbom_request("--cyclonedx", "bom.json", &["--package-only"]);
    let output = output_with_files(vec![json_file(
        "package.json",
        crate::models::FileType::File,
    )]);

    let refusal = assembly_skipped_sbom_refusal(&request, &output)
        .expect("--package-only must refuse a hollow cyclonedx export");
    assert!(refusal.contains("--package-only"));
    assert!(refusal.contains("--cyclonedx"));
}

#[test]
fn assembly_skipped_sbom_refusal_fires_for_no_assemble_with_scanned_files() {
    let request = native_sbom_request("--spdx-tv", "sbom.spdx", &["--no-assemble", "--package"]);
    let output = output_with_files(vec![json_file(
        "package.json",
        crate::models::FileType::File,
    )]);

    let refusal = assembly_skipped_sbom_refusal(&request, &output)
        .expect("--no-assemble must refuse a hollow spdx-tv export");
    assert!(refusal.contains("--no-assemble"));
    assert!(refusal.contains("--spdx-tv"));
}

#[test]
fn assembly_skipped_sbom_refusal_allows_non_sbom_output_formats() {
    let request = native_sbom_request("--json-pp", "-", &["--package-only"]);
    let output = output_with_files(vec![json_file(
        "package.json",
        crate::models::FileType::File,
    )]);

    assert!(assembly_skipped_sbom_refusal(&request, &output).is_none());
}

#[test]
fn assembly_skipped_sbom_refusal_allows_truly_empty_scan_documents() {
    let request = native_sbom_request("--cyclonedx-xml", "bom.xml", &["--package-only"]);
    let output = output_with_files(vec![]);

    assert!(assembly_skipped_sbom_refusal(&request, &output).is_none());
}

#[test]
fn assembly_skipped_sbom_refusal_allows_package_without_assembly_skip() {
    let request = native_sbom_request("--cyclonedx", "bom.json", &["--package"]);
    let output = output_with_files(vec![json_file(
        "package.json",
        crate::models::FileType::File,
    )]);

    assert!(assembly_skipped_sbom_refusal(&request, &output).is_none());
}

#[test]
fn assembly_skipped_sbom_refusal_does_not_fire_for_from_json() {
    let request = from_json_sbom_request("--cyclonedx", "bom.json");
    let output = output_with_files(vec![json_file(
        "package.json",
        crate::models::FileType::File,
    )]);

    assert!(assembly_skipped_sbom_refusal(&request, &output).is_none());
}

#[test]
fn paths_file_sbom_completeness_warning_fires_for_native_paths_file_sbom_export() {
    let request = native_sbom_request(
        "--cyclonedx",
        "bom.json",
        &["--package", "--paths-file", "changed.txt"],
    );
    let output = output_with_files(vec![]);

    let warning = paths_file_sbom_completeness_warning(&request, &output)
        .expect("--paths-file combined with an SBOM format must warn");
    assert!(warning.contains("--paths-file"));
    assert!(warning.contains("--cyclonedx"));
}

#[test]
fn paths_file_sbom_completeness_warning_allows_non_sbom_output_formats() {
    let request = native_sbom_request(
        "--json-pp",
        "-",
        &["--package", "--paths-file", "changed.txt"],
    );
    let output = output_with_files(vec![]);

    assert!(paths_file_sbom_completeness_warning(&request, &output).is_none());
}

#[test]
fn paths_file_sbom_completeness_warning_allows_requests_without_paths_file() {
    let request = native_sbom_request("--spdx-rdf", "sbom.rdf", &["--package"]);
    let output = output_with_files(vec![]);

    assert!(paths_file_sbom_completeness_warning(&request, &output).is_none());
}

fn cargo_toml_package_data_with_marker(field: &str) -> crate::models::PackageData {
    let mut extra_data = std::collections::HashMap::new();
    extra_data.insert(field.to_string(), json!("workspace"));
    crate::models::PackageData {
        package_type: Some(crate::models::PackageType::Cargo),
        datasource_id: Some(crate::models::DatasourceId::CargoToml),
        name: Some("member".to_string()),
        extra_data: Some(extra_data),
        ..Default::default()
    }
}

fn cargo_workspace_root_package_data(members: &[&str]) -> crate::models::PackageData {
    let mut extra_data = std::collections::HashMap::new();
    extra_data.insert("workspace".to_string(), json!({ "members": members }));
    crate::models::PackageData {
        package_type: Some(crate::models::PackageType::Cargo),
        datasource_id: Some(crate::models::DatasourceId::CargoToml),
        extra_data: Some(extra_data),
        ..Default::default()
    }
}

#[test]
fn paths_file_sbom_completeness_warning_names_cargo_workspace_root_gap() {
    let request = native_sbom_request(
        "--cyclonedx",
        "bom.json",
        &["--package", "--paths-file", "changed.txt"],
    );
    let mut member = json_file("crates/member/Cargo.toml", crate::models::FileType::File);
    member.package_data = vec![cargo_toml_package_data_with_marker("version")];
    let output = output_with_files(vec![member]);

    let warning = paths_file_sbom_completeness_warning(&request, &output)
        .expect("--paths-file combined with an SBOM format must warn");
    assert!(warning.contains("Cargo workspace"));
    assert!(warning.contains("crates/member/Cargo.toml"));
    assert!(warning.contains("[workspace] table"));
}

#[test]
fn paths_file_sbom_completeness_warning_names_cargo_workspace_gap_from_dependency_marker() {
    let request = native_sbom_request(
        "--spdx-tv",
        "sbom.spdx",
        &["--package", "--paths-file", "changed.txt"],
    );
    let member = json_file("crates/member/Cargo.toml", crate::models::FileType::File);
    let mut output = output_with_files(vec![member]);
    let mut extra_data = std::collections::HashMap::new();
    extra_data.insert("workspace".to_string(), json!(true));
    output.dependencies = vec![crate::models::TopLevelDependency::from_dependency(
        &crate::models::Dependency {
            purl: Some("pkg:cargo/serde".to_string()),
            extracted_requirement: None,
            scope: Some("dependencies".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(false),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: Some(extra_data),
        },
        "crates/member/Cargo.toml".to_string(),
        crate::models::DatasourceId::CargoToml,
        None,
    )];

    let warning = paths_file_sbom_completeness_warning(&request, &output)
        .expect("--paths-file combined with an SBOM format must warn");
    assert!(warning.contains("Cargo workspace"));
    assert!(warning.contains("crates/member/Cargo.toml"));
}

#[test]
fn paths_file_sbom_completeness_warning_omits_cargo_gap_when_root_selected() {
    let request = native_sbom_request(
        "--cyclonedx",
        "bom.json",
        &["--package", "--paths-file", "changed.txt"],
    );
    let mut root = json_file("Cargo.toml", crate::models::FileType::File);
    root.package_data = vec![cargo_workspace_root_package_data(&["crates/member"])];
    let mut member = json_file("crates/member/Cargo.toml", crate::models::FileType::File);
    member.package_data = vec![cargo_toml_package_data_with_marker("version")];
    let output = output_with_files(vec![root, member]);

    let warning = paths_file_sbom_completeness_warning(&request, &output)
        .expect("--paths-file combined with an SBOM format must warn generically");
    assert!(warning.contains("--paths-file"));
    assert!(!warning.contains("Cargo workspace member manifest"));
}

#[test]
fn paths_file_sbom_completeness_warning_omits_cargo_gap_for_empty_members_root() {
    // Regression: `[workspace]` with an empty or omitted `members` list is
    // valid Cargo syntax for a single-package workspace; it must still count
    // as a selected root instead of being misdetected as absent.
    let request = native_sbom_request(
        "--cyclonedx",
        "bom.json",
        &["--package", "--paths-file", "changed.txt"],
    );
    let mut root = json_file("Cargo.toml", crate::models::FileType::File);
    root.package_data = vec![cargo_workspace_root_package_data(&[])];
    let mut member = json_file("crates/member/Cargo.toml", crate::models::FileType::File);
    member.package_data = vec![cargo_toml_package_data_with_marker("version")];
    let output = output_with_files(vec![root, member]);

    let warning = paths_file_sbom_completeness_warning(&request, &output)
        .expect("--paths-file combined with an SBOM format must warn generically");
    assert!(!warning.contains("Cargo workspace member manifest"));
}

#[test]
fn paths_file_sbom_completeness_warning_recognizes_pnpm_workspace_yaml_root() {
    // Regression: pnpm keeps its workspace declaration in a separate
    // pnpm-workspace.yaml rather than package.json's "workspaces" field.
    let request = native_sbom_request(
        "--cyclonedx",
        "bom.json",
        &["--package", "--paths-file", "changed.txt"],
    );
    let mut root = json_file("pnpm-workspace.yaml", crate::models::FileType::File);
    root.package_data = vec![crate::models::PackageData {
        package_type: Some(crate::models::PackageType::Npm),
        datasource_id: Some(crate::models::DatasourceId::PnpmWorkspaceYaml),
        ..Default::default()
    }];
    let member = json_file("packages/app/package.json", crate::models::FileType::File);
    let mut output = output_with_files(vec![root, member]);
    output.dependencies = vec![crate::models::TopLevelDependency::from_dependency(
        &crate::models::Dependency {
            purl: Some("pkg:npm/sibling".to_string()),
            extracted_requirement: Some("workspace:*".to_string()),
            scope: Some("dependencies".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(false),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        },
        "packages/app/package.json".to_string(),
        crate::models::DatasourceId::NpmPackageJson,
        None,
    )];

    let warning = paths_file_sbom_completeness_warning(&request, &output)
        .expect("--paths-file combined with an SBOM format must warn generically");
    assert!(!warning.contains("npm/pnpm/yarn workspace member manifest"));
}

#[test]
fn paths_file_sbom_completeness_warning_names_npm_workspace_root_gap() {
    let request = native_sbom_request(
        "--cyclonedx",
        "bom.json",
        &["--package", "--paths-file", "changed.txt"],
    );
    let member = json_file("packages/app/package.json", crate::models::FileType::File);
    let mut output = output_with_files(vec![member]);
    output.dependencies = vec![crate::models::TopLevelDependency::from_dependency(
        &crate::models::Dependency {
            purl: Some("pkg:npm/sibling".to_string()),
            extracted_requirement: Some("workspace:*".to_string()),
            scope: Some("dependencies".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(false),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        },
        "packages/app/package.json".to_string(),
        crate::models::DatasourceId::NpmPackageJson,
        None,
    )];

    let warning = paths_file_sbom_completeness_warning(&request, &output)
        .expect("--paths-file combined with an SBOM format must warn");
    assert!(warning.contains("npm/pnpm/yarn workspace"));
    assert!(warning.contains("packages/app/package.json"));
}

#[test]
fn paths_file_sbom_completeness_warning_names_mix_umbrella_root_gap() {
    let request = native_sbom_request(
        "--cyclonedx",
        "bom.json",
        &["--package", "--paths-file", "changed.txt"],
    );
    let member = json_file("apps/app_one/mix.exs", crate::models::FileType::File);
    let mut output = output_with_files(vec![member]);
    let mut extra_data = std::collections::HashMap::new();
    extra_data.insert("in_umbrella".to_string(), json!(true));
    output.dependencies = vec![crate::models::TopLevelDependency::from_dependency(
        &crate::models::Dependency {
            purl: None,
            extracted_requirement: None,
            scope: Some("deps".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: None,
            is_direct: Some(true),
            resolved_package: None,
            extra_data: Some(extra_data),
        },
        "apps/app_one/mix.exs".to_string(),
        crate::models::DatasourceId::HexMixExs,
        None,
    )];

    let warning = paths_file_sbom_completeness_warning(&request, &output)
        .expect("--paths-file combined with an SBOM format must warn");
    assert!(warning.contains("Mix umbrella"));
    assert!(warning.contains("apps/app_one/mix.exs"));
}

#[test]
fn compile_regex_patterns_rejects_invalid_regex() {
    let result = compile_regex_patterns("--ignore-author", &["[".to_string()]);

    assert!(result.is_err());
    let error = result.err().unwrap().to_string();
    assert!(error.contains("--ignore-author"));
    assert!(error.contains("Invalid regex"));
}

#[test]
fn from_json_with_no_assemble_preserves_preloaded_package_sections() {
    let temp_path = std::env::temp_dir().join("provenant-from-json-with-packages-test.json");
    let content = json!({
        "files": [],
        "packages": [
            {
                "package_uid": "pkg:npm/demo@1.0.0",
                "type": "npm",
                "name": "demo",
                "version": "1.0.0",
                "parties": [],
                "datafile_paths": ["package.json"],
                "datasource_ids": ["npm_package_json"]
            }
        ],
        "dependencies": [
            {
                "purl": "pkg:npm/dep@2.0.0",
                "scope": "dependencies",
                "is_runtime": true,
                "is_optional": false,
                "is_pinned": true,
                "dependency_uid": "pkg:npm/dep@2.0.0?uuid=test",
                "for_package_uid": "pkg:npm/demo@1.0.0",
                "datafile_path": "package.json",
                "datasource_id": "npm_package_json"
            }
        ],
        "license_detections": [],
        "license_references": [],
        "license_rule_references": []
    });
    fs::write(&temp_path, content.to_string()).expect("write json fixture");

    let parsed = load_scan_from_json(temp_path.to_str().expect("utf-8 path"))
        .expect("from-json loading should succeed");

    let packages: Vec<crate::models::Package> = parsed
        .packages
        .iter()
        .map(crate::models::Package::try_from)
        .collect::<Result<Vec<_>, _>>()
        .expect("package conversion should succeed");
    let dependencies: Vec<crate::models::TopLevelDependency> = parsed
        .dependencies
        .iter()
        .map(crate::models::TopLevelDependency::try_from)
        .collect::<Result<Vec<_>, _>>()
        .expect("dependency conversion should succeed");

    let preloaded = assembly::AssemblyResult {
        packages,
        dependencies,
    };

    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--no-assemble",
        temp_path.to_str().expect("utf-8 path"),
    ])
    .expect("cli parse should succeed");

    let assembly_result = if cli.from_json
        && (!preloaded.packages.is_empty() || !preloaded.dependencies.is_empty())
    {
        preloaded
    } else if cli.no_assemble {
        assembly::AssemblyResult {
            packages: Vec::new(),
            dependencies: Vec::new(),
        }
    } else {
        unreachable!("test only covers from-json preload precedence")
    };

    assert_eq!(assembly_result.packages.len(), 1);
    assert_eq!(assembly_result.dependencies.len(), 1);

    let _ = fs::remove_file(temp_path);
}

#[test]
fn validate_scan_option_compatibility_allows_multiple_paths_without_from_json() {
    let cli =
        crate::cli::Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "dir-a", "dir-b"])
            .unwrap();
    assert!(validate_scan_option_compatibility(&cli).is_ok());
}

#[test]
fn validate_scan_option_compatibility_rejects_paths_file_with_from_json() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--paths-file",
        "changed-files.txt",
        "sample-scan.json",
    ])
    .unwrap();

    let error = validate_scan_option_compatibility(&cli).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("--paths-file is only supported for native scan mode")
    );
}

#[test]
fn validate_scan_option_compatibility_rejects_paths_file_without_single_root() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--paths-file",
        "changed-files.txt",
    ])
    .unwrap();

    let error = validate_scan_option_compatibility(&cli).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("--paths-file requires exactly one positional scan root")
    );
}

#[test]
fn validate_scan_option_compatibility_rejects_mark_source_without_info() {
    let cli =
        crate::cli::Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "sample-dir"])
            .unwrap();
    let mut cli = cli
        .scan_args()
        .expect("scan args should be present")
        .clone();
    cli.mark_source = true;

    let error = validate_scan_option_compatibility(&cli).unwrap_err();
    assert!(error.to_string().contains("--mark-source requires --info"));
}

#[test]
fn from_json_skips_final_native_projection_block() {
    let mut loaded = JsonScanInput {
        headers: vec![],
        files: vec![crate::output_schema::OutputFileInfo::from(&json_file(
            "/tmp/archive/root/src/main.rs",
            crate::models::FileType::File,
        ))],
        packages: vec![],
        dependencies: vec![],
        license_detections: vec![],
        license_references: vec![],
        license_rule_references: vec![],
        excluded_count: 0,
        has_hollow_package_detection_input: false,
    };

    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--full-root",
        "sample-scan.json",
    ])
    .expect("cli parse should succeed");

    normalize_loaded_json_scan(&mut loaded, false, true);

    if !cli.from_json && (cli.strip_root || cli.full_root) {
        let mut files: Vec<crate::models::FileInfo> = loaded
            .files
            .iter()
            .map(crate::models::FileInfo::try_from)
            .collect::<Result<Vec<_>, _>>()
            .expect("file conversion should succeed");
        normalize_paths(
            &mut files,
            cli.dir_path.first().expect("input path exists"),
            cli.strip_root,
            cli.full_root,
        );
    }

    assert_eq!(loaded.files[0].path, "tmp/archive/root/src/main.rs");
}

#[test]
fn from_json_loaded_manifest_detections_can_be_recomputed_into_top_level_uniques() {
    let mut file0 = json_file("project/package.json", crate::models::FileType::File);
    file0.package_data = vec![crate::models::PackageData {
        package_type: Some(crate::models::PackageType::Npm),
        license_detections: vec![crate::models::LicenseDetection {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            matches: vec![crate::models::Match {
                license_expression: "mit".to_string(),
                license_expression_spdx: "MIT".to_string(),
                from_file: None,
                start_line: LineNumber::ONE,
                end_line: LineNumber::ONE,
                matcher: MatcherKind::Declared,
                score: MatchScore::MAX,
                matched_length: Some(1),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: String::new(),
                rule_url: None,
                matched_text: Some("MIT".to_string()),
                referenced_filenames: None,
                matched_text_diagnostics: None,
            }],
            detection_log: vec![],
            identifier: String::new(),
        }],
        ..Default::default()
    }];
    let mut files = vec![file0];

    for file in &mut files {
        file.backfill_license_provenance();
    }

    let top_level = collect_top_level_license_detections(&files);

    assert_eq!(top_level.len(), 1);
    assert_eq!(top_level[0].license_expression, "mit");
    assert_eq!(
        top_level[0].reference_matches[0].from_file.as_deref(),
        Some("project/package.json")
    );
    assert_eq!(
        top_level[0].reference_matches[0].rule_identifier.as_str(),
        "parser-declared-license"
    );
}

#[test]
fn from_json_recomputes_top_level_uniques_even_without_shaping_flags() {
    let mut file0 = json_file("project/package.json", crate::models::FileType::File);
    file0.package_data = vec![crate::models::PackageData {
        package_type: Some(crate::models::PackageType::Npm),
        other_license_detections: vec![crate::models::LicenseDetection {
            license_expression: "gpl-2.0-only".to_string(),
            license_expression_spdx: "GPL-2.0-only".to_string(),
            matches: vec![crate::models::Match {
                license_expression: "gpl-2.0-only".to_string(),
                license_expression_spdx: "GPL-2.0-only".to_string(),
                from_file: None,
                start_line: LineNumber::ONE,
                end_line: LineNumber::ONE,
                matcher: MatcherKind::Declared,
                score: MatchScore::MAX,
                matched_length: Some(1),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: String::new(),
                rule_url: None,
                matched_text: Some("GPL-2.0-only".to_string()),
                referenced_filenames: None,
                matched_text_diagnostics: None,
            }],
            detection_log: vec![],
            identifier: String::new(),
        }],
        ..Default::default()
    }];
    let mut files = vec![file0];

    for file in &mut files {
        file.backfill_license_provenance();
    }

    let top_level = collect_top_level_license_detections(&files);

    assert_eq!(top_level.len(), 1);
    assert_eq!(top_level[0].license_expression, "gpl-2.0-only");
    assert_ne!(top_level[0].identifier, "stale-id");
    assert_eq!(
        top_level[0].reference_matches[0].rule_identifier.as_str(),
        "parser-declared-license"
    );
}

#[test]
fn from_json_only_findings_keeps_files_with_findings() {
    let mut file = json_file("project/package.json", crate::models::FileType::File);
    file.detected_license_expression = Some("mit".to_string());
    let mut files = vec![file];

    apply_only_findings_filter(&mut files);

    assert_eq!(files.len(), 1);
}

#[test]
fn native_only_findings_still_keeps_files_with_findings() {
    let mut file = json_file("project/package.json", crate::models::FileType::File);
    file.detected_license_expression = Some("mit".to_string());
    let mut files = vec![file];

    apply_only_findings_filter(&mut files);

    assert_eq!(files.len(), 1);
}

#[test]
fn from_json_only_findings_preserves_preloaded_top_level_detections() {
    let files = vec![json_file(
        "project/package.json",
        crate::models::FileType::File,
    )];
    let preloaded = vec![crate::models::TopLevelLicenseDetection {
        identifier: "mit-id".to_string(),
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        detection_count: 1,
        detection_log: vec![],
        reference_matches: vec![],
    }];

    let detections = collect_top_level_license_detections_for_mode(&files, preloaded, true, false);

    assert_eq!(detections.len(), 1);
    assert_eq!(detections[0].license_expression, "mit");
}

#[test]
fn from_json_filtered_replay_preserves_preloaded_top_level_detections() {
    let files = vec![json_file(
        "project/package.json",
        crate::models::FileType::File,
    )];
    let preloaded = vec![crate::models::TopLevelLicenseDetection {
        identifier: "mit-id".to_string(),
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        detection_count: 1,
        detection_log: vec![],
        reference_matches: vec![],
    }];

    let detections = collect_top_level_license_detections_for_mode(&files, preloaded, true, false);

    assert_eq!(detections.len(), 1);
    assert_eq!(detections[0].license_expression, "mit");
}

#[test]
fn from_json_multi_input_replay_clears_top_level_detections() {
    let files = vec![json_file(
        "project/package.json",
        crate::models::FileType::File,
    )];
    let preloaded = vec![crate::models::TopLevelLicenseDetection {
        identifier: "mit-id".to_string(),
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        detection_count: 1,
        detection_log: vec![],
        reference_matches: vec![],
    }];

    let detections = collect_top_level_license_detections_for_mode(&files, preloaded, false, true);

    assert!(detections.is_empty());
}

#[test]
fn from_json_recomputes_top_level_outputs_after_manifest_reference_following() {
    let file0 = json_file("project/Cargo.toml", crate::models::FileType::File);
    let file1 = json_file("project/LICENSE", crate::models::FileType::File);
    let mut files = vec![file0, file1];

    files[0].package_data = vec![crate::models::PackageData {
        package_type: Some(crate::models::PackageType::Cargo),
        datasource_id: Some(crate::models::DatasourceId::CargoToml),
        name: Some("demo".to_string()),
        version: Some("1.0.0".to_string()),
        ..Default::default()
    }];
    let mut package = crate::models::Package::from_package_data(
        &files[0].package_data[0],
        "project/Cargo.toml".to_string(),
    );
    let package_uid = package.package_uid.clone();
    files[0].for_packages = vec![package_uid.clone()];
    files[0].license_detections = vec![crate::models::LicenseDetection {
        license_expression: "unknown-license-reference".to_string(),
        license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
        matches: vec![crate::models::Match {
            license_expression: "unknown-license-reference".to_string(),
            license_expression_spdx: "LicenseRef-scancode-unknown-license-reference".to_string(),
            from_file: Some("project/Cargo.toml".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
            matcher: MatcherKind::Aho,
            score: MatchScore::MAX,
            matched_length: Some(2),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "unknown-license-reference_see_license_at_manifest_1.RULE".to_string(),
            rule_url: None,
            matched_text: Some("See LICENSE".to_string()),
            referenced_filenames: Some(vec!["LICENSE".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: String::new(),
    }];
    files[1].license_detections = vec![crate::models::LicenseDetection {
        license_expression: "mit".to_string(),
        license_expression_spdx: "MIT".to_string(),
        matches: vec![crate::models::Match {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("project/LICENSE".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(10).unwrap(),
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(50),
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

    for file in &mut files {
        file.backfill_license_provenance();
    }
    package.backfill_license_provenance();

    let mut packages = vec![package];
    apply_package_reference_following(&mut files, &mut packages);

    assert_eq!(
        packages[0].declared_license_expression.as_deref(),
        Some("mit")
    );

    let top_level = collect_top_level_license_detections(&files);
    assert!(
        top_level
            .iter()
            .any(|detection| detection.license_expression == "mit")
    );

    let engine = LicenseDetectionEngine::from_embedded().expect("embedded engine should load");
    let (license_references, license_rule_references) = collect_top_level_license_references(
        &files,
        &packages,
        engine.index(),
        DEFAULT_LICENSEDB_URL_TEMPLATE,
    );
    assert!(
        license_references
            .iter()
            .any(|reference| reference.key.as_deref() == Some("mit"))
    );
    assert!(license_rule_references.iter().any(|rule| {
        rule.identifier == "unknown-license-reference_see_license_at_manifest_1.RULE"
    }));
}

#[test]
fn from_json_recomputes_top_level_outputs_after_package_inheritance_following() {
    let file0 = json_file(
        "venv/lib/python3.11/site-packages/demo-1.0.dist-info/METADATA",
        crate::models::FileType::File,
    );
    let file1 = json_file(
        "venv/lib/python3.11/site-packages/locale/django.po",
        crate::models::FileType::File,
    );
    let mut files = vec![file0, file1];

    files[0].package_data = vec![crate::models::PackageData {
        package_type: Some(crate::models::PackageType::Pypi),
        datasource_id: Some(crate::models::DatasourceId::PypiWheelMetadata),
        name: Some("demo".to_string()),
        version: Some("1.0.0".to_string()),
        ..Default::default()
    }];
    files[0].license_detections = vec![crate::models::LicenseDetection {
        license_expression: "bsd-new".to_string(),
        license_expression_spdx: "BSD-3-Clause".to_string(),
        matches: vec![crate::models::Match {
            license_expression: "bsd-new".to_string(),
            license_expression_spdx: "BSD-3-Clause".to_string(),
            from_file: Some(
                "venv/lib/python3.11/site-packages/demo-1.0.dist-info/METADATA".to_string(),
            ),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
            matcher: MatcherKind::Hash,
            score: MatchScore::MAX,
            matched_length: Some(1),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "bsd-new_195.RULE".to_string(),
            rule_url: None,
            matched_text: Some("BSD-3-Clause".to_string()),
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: String::new(),
    }];
    let mut package = crate::models::Package::from_package_data(
        &files[0].package_data[0],
        "venv/lib/python3.11/site-packages/demo-1.0.dist-info/METADATA".to_string(),
    );
    let package_uid = package.package_uid.clone();
    files[0].for_packages = vec![package_uid.clone()];
    files[1].for_packages = vec![package_uid.clone()];
    files[1].license_detections = vec![crate::models::LicenseDetection {
        license_expression: "free-unknown".to_string(),
        license_expression_spdx: "LicenseRef-scancode-free-unknown".to_string(),
        matches: vec![crate::models::Match {
            license_expression: "free-unknown".to_string(),
            license_expression_spdx: "LicenseRef-scancode-free-unknown".to_string(),
            from_file: Some("venv/lib/python3.11/site-packages/locale/django.po".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
            matcher: MatcherKind::Aho,
            score: MatchScore::MAX,
            matched_length: Some(11),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "free-unknown-package_1.RULE".to_string(),
            rule_url: None,
            matched_text: Some("same license as package".to_string()),
            referenced_filenames: Some(vec!["INHERIT_LICENSE_FROM_PACKAGE".to_string()]),
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: String::new(),
    }];

    for file in &mut files {
        file.backfill_license_provenance();
    }
    package.backfill_license_provenance();

    let mut packages = vec![package];
    apply_package_reference_following(&mut files, &mut packages);

    assert_eq!(
        packages[0].declared_license_expression.as_deref(),
        Some("bsd-new")
    );
    assert_eq!(
        files[1].license_detections[0].detection_log,
        vec!["unknown-reference-in-file-to-package"]
    );

    let top_level = collect_top_level_license_detections(&files);
    let bsd_new_detections = top_level
        .iter()
        .filter(|detection| detection.license_expression == "bsd-new")
        .collect::<Vec<_>>();
    assert_eq!(bsd_new_detections.len(), 2);
    assert!(
        bsd_new_detections
            .iter()
            .all(|detection| detection.detection_count == 1)
    );
    assert!(
        bsd_new_detections
            .iter()
            .any(|detection| detection.detection_log.is_empty())
    );
    assert!(
        bsd_new_detections.iter().any(|detection| {
            detection.detection_log == ["unknown-reference-in-file-to-package"]
        })
    );

    let engine = LicenseDetectionEngine::from_embedded().expect("embedded engine should load");
    let (license_references, license_rule_references) = collect_top_level_license_references(
        &files,
        &packages,
        engine.index(),
        DEFAULT_LICENSEDB_URL_TEMPLATE,
    );
    assert!(
        license_references
            .iter()
            .any(|reference| { reference.key.as_deref() == Some("bsd-new") })
    );
    assert!(
        license_rule_references
            .iter()
            .any(|rule| rule.identifier == "free-unknown-package_1.RULE")
    );
}

#[test]
fn from_json_keeps_multi_datafile_package_license_provenance_on_manifest_package() {
    let file0 = json_file("project/package-lock.json", crate::models::FileType::File);
    let file1 = json_file("project/package.json", crate::models::FileType::File);
    let mut files = vec![file0, file1];

    files[0].package_data = vec![crate::models::PackageData {
        package_type: Some(crate::models::PackageType::Npm),
        datasource_id: Some(crate::models::DatasourceId::NpmPackageLockJson),
        name: Some("phoenix".to_string()),
        version: Some("1.8.5".to_string()),
        ..Default::default()
    }];
    files[0].license_detections = vec![crate::models::LicenseDetection {
        license_expression: "apache-2.0".to_string(),
        license_expression_spdx: "Apache-2.0".to_string(),
        matches: vec![crate::models::Match {
            license_expression: "apache-2.0".to_string(),
            license_expression_spdx: "Apache-2.0".to_string(),
            from_file: Some("project/package-lock.json".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
            matcher: MatcherKind::Aho,
            score: MatchScore::MAX,
            matched_length: Some(10),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: "apache-2.0_65.RULE".to_string(),
            rule_url: None,
            matched_text: Some("Apache-2.0".to_string()),
            referenced_filenames: None,
            matched_text_diagnostics: None,
        }],
        detection_log: vec![],
        identifier: String::new(),
    }];

    files[1].package_data = vec![crate::models::PackageData {
        package_type: Some(crate::models::PackageType::Npm),
        datasource_id: Some(crate::models::DatasourceId::NpmPackageJson),
        name: Some("phoenix".to_string()),
        version: Some("1.8.5".to_string()),
        declared_license_expression: Some("mit".to_string()),
        declared_license_expression_spdx: Some("MIT".to_string()),
        license_detections: vec![crate::models::LicenseDetection {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            matches: vec![crate::models::Match {
                license_expression: "mit".to_string(),
                license_expression_spdx: "MIT".to_string(),
                from_file: Some("project/package.json".to_string()),
                start_line: LineNumber::ONE,
                end_line: LineNumber::ONE,
                matcher: MatcherKind::Aho,
                score: MatchScore::MAX,
                matched_length: Some(3),
                match_coverage: Some(100.0),
                rule_relevance: Some(100),
                rule_identifier: "mit_30.RULE".to_string(),
                rule_url: None,
                matched_text: Some("MIT".to_string()),
                referenced_filenames: None,
                matched_text_diagnostics: None,
            }],
            detection_log: vec![],
            identifier: String::new(),
        }],
        ..Default::default()
    }];

    let mut package = crate::models::Package::from_package_data(
        &files[1].package_data[0],
        "project/package.json".to_string(),
    );
    package.datafile_paths = vec![
        "project/package-lock.json".to_string(),
        "project/package.json".to_string(),
    ];
    let package_uid = package.package_uid.clone();
    files[0].for_packages = vec![package_uid.clone()];
    files[1].for_packages = vec![package_uid];

    for file in &mut files {
        file.backfill_license_provenance();
    }
    package.backfill_license_provenance();

    let mut packages = vec![package];
    apply_package_reference_following(&mut files, &mut packages);

    assert_eq!(
        packages[0].declared_license_expression.as_deref(),
        Some("mit")
    );
    assert_eq!(
        packages[0].declared_license_expression_spdx.as_deref(),
        Some("MIT")
    );
    assert_eq!(packages[0].license_detections.len(), 1);
    assert_eq!(
        packages[0].license_detections[0].license_expression_spdx,
        "MIT"
    );
}

fn json_file(path: &str, file_type: crate::models::FileType) -> crate::models::FileInfo {
    crate::models::FileInfo::new(
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string(),
        Path::new(path)
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string(),
        Path::new(path)
            .extension()
            .and_then(|name| name.to_str())
            .map(|ext| format!(".{ext}"))
            .unwrap_or_default(),
        path.to_string(),
        file_type,
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
    )
}

#[test]
fn progress_mode_from_cli_maps_quiet_verbose_default() {
    let default_cli =
        crate::cli::Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "sample-dir"])
            .unwrap();
    let default_request = ScanRequest::from(
        default_cli
            .scan_args()
            .expect("scan args should be present"),
    );
    assert_eq!(
        default_request.progress_mode,
        crate::progress::ProgressMode::Default
    );

    let quiet_cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--quiet",
        "sample-dir",
    ])
    .unwrap();
    let quiet_request =
        ScanRequest::from(quiet_cli.scan_args().expect("scan args should be present"));
    assert_eq!(
        quiet_request.progress_mode,
        crate::progress::ProgressMode::Quiet
    );

    let verbose_cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--verbose",
        "sample-dir",
    ])
    .unwrap();
    let verbose_request = ScanRequest::from(
        verbose_cli
            .scan_args()
            .expect("scan args should be present"),
    );
    assert_eq!(
        verbose_request.progress_mode,
        crate::progress::ProgressMode::Verbose
    );
}

#[test]
fn prepare_cache_for_scan_defaults_to_scan_root_cache_directory_without_creating_dirs() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("scan");
    fs::create_dir_all(&scan_root).expect("create scan root");

    let cli =
        crate::cli::Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "sample-dir"])
            .unwrap();
    let request = ScanRequest::from(cli.scan_args().expect("scan args should be present"));
    let config = prepare_cache_config(Some(&scan_root), &request).unwrap();

    assert_eq!(config.root_dir(), CacheConfig::default_root_dir(&scan_root));
    assert!(!config.incremental_enabled());
}

#[test]
fn prepare_cache_for_scan_respects_cache_dir_and_cache_clear() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("scan");
    fs::create_dir_all(&scan_root).expect("create scan root");

    let explicit_cache_dir = temp_dir.path().join("explicit-cache");
    fs::create_dir_all(explicit_cache_dir.join("incremental")).unwrap();
    let stale_file = explicit_cache_dir.join("incremental").join("stale.txt");
    fs::write(&stale_file, "old").unwrap();

    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--cache-dir",
        explicit_cache_dir.to_str().unwrap(),
        "--cache-clear",
        "sample-dir",
    ])
    .unwrap();
    let request = ScanRequest::from(cli.scan_args().expect("scan args should be present"));
    let config = prepare_cache_config(Some(&scan_root), &request).unwrap();

    assert_eq!(config.root_dir(), explicit_cache_dir);
    assert!(!stale_file.exists());
}

#[test]
fn prepare_cache_for_scan_creates_incremental_dir_when_enabled() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("scan");
    fs::create_dir_all(&scan_root).expect("create scan root");

    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--incremental",
        "sample-dir",
    ])
    .unwrap();
    let request = ScanRequest::from(cli.scan_args().expect("scan args should be present"));
    let config = prepare_cache_config(Some(&scan_root), &request).unwrap();

    assert!(config.incremental_enabled());
    assert!(config.incremental_dir().exists());
}

#[test]
fn prepare_cache_config_without_scan_root_uses_non_scan_default() {
    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "sample-scan.json",
    ])
    .unwrap();

    let request = ScanRequest::from(cli.scan_args().expect("scan args should be present"));
    let config = prepare_cache_config(None, &request).unwrap();

    assert_eq!(
        config.root_dir(),
        CacheConfig::default_root_dir_without_scan_root()
    );
    assert!(!config.incremental_enabled());
}

#[test]
fn build_collection_exclude_patterns_skips_default_cache_dir() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("scan");
    fs::create_dir_all(scan_root.join("src")).unwrap();
    fs::create_dir_all(scan_root.join(DEFAULT_CACHE_DIR_NAME).join("incremental")).unwrap();
    fs::write(scan_root.join("src").join("main.rs"), "fn main() {}").unwrap();
    fs::write(
        scan_root
            .join(DEFAULT_CACHE_DIR_NAME)
            .join("incremental")
            .join("stale.txt"),
        "cached",
    )
    .unwrap();

    let config = CacheConfig::from_scan_root(&scan_root);
    let exclude_patterns = build_collection_exclude_patterns(&scan_root, config.root_dir());
    let collected = collect_paths(&scan_root, 0, &exclude_patterns);

    assert!(
        collected
            .files
            .iter()
            .all(|(path, _)| !path.starts_with(config.root_dir()))
    );
    assert!(collected.excluded_count >= 1);
}

#[test]
fn build_collection_exclude_patterns_skips_explicit_in_tree_cache_dir() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("scan");
    let explicit_cache_dir = scan_root.join("custom-cache");
    fs::create_dir_all(scan_root.join("docs")).unwrap();
    fs::create_dir_all(explicit_cache_dir.join("incremental")).unwrap();
    fs::write(scan_root.join("docs").join("README.md"), "hello").unwrap();
    fs::write(
        explicit_cache_dir
            .join("incremental")
            .join("manifest.postcard"),
        "cached",
    )
    .unwrap();

    let config = CacheConfig::new(explicit_cache_dir.clone());
    let exclude_patterns = build_collection_exclude_patterns(&scan_root, config.root_dir());
    let collected = collect_paths(&scan_root, 0, &exclude_patterns);

    assert!(
        collected
            .files
            .iter()
            .all(|(path, _)| !path.starts_with(&explicit_cache_dir))
    );
    assert!(collected.excluded_count >= 1);
}

#[test]
fn build_collection_exclude_patterns_skips_license_index_files_under_cache_root() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("scan");
    let explicit_cache_dir = scan_root.join("custom-cache");
    fs::create_dir_all(scan_root.join("docs")).unwrap();
    fs::create_dir_all(explicit_cache_dir.join("license-index").join("embedded")).unwrap();
    fs::write(scan_root.join("docs").join("README.md"), "hello").unwrap();
    fs::write(
        explicit_cache_dir
            .join("license-index")
            .join("embedded")
            .join("cache.rkyv"),
        "cached",
    )
    .unwrap();

    let config = CacheConfig::new(explicit_cache_dir.clone());
    let exclude_patterns = build_collection_exclude_patterns(&scan_root, config.root_dir());
    let collected = collect_paths(&scan_root, 0, &exclude_patterns);

    assert!(
        collected
            .files
            .iter()
            .all(|(path, _)| !path.starts_with(&explicit_cache_dir))
    );
    assert!(collected.excluded_count >= 1);
}

#[test]
fn build_collection_exclude_patterns_does_not_exclude_scan_root_when_cache_root_matches_it() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("scan");
    fs::create_dir_all(scan_root.join("src")).unwrap();
    fs::write(scan_root.join("src").join("main.rs"), "fn main() {}").unwrap();

    let config = CacheConfig::new(scan_root.clone());
    let exclude_patterns = build_collection_exclude_patterns(&scan_root, config.root_dir());
    let collected = collect_paths(&scan_root, 0, &exclude_patterns);

    assert_eq!(collected.file_count(), 1);
    assert_eq!(collected.excluded_count, 0);
}

#[test]
fn build_collection_exclude_patterns_skips_vcs_metadata_directories() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("scan");
    fs::create_dir_all(scan_root.join("src")).unwrap();
    fs::create_dir_all(scan_root.join(".git")).unwrap();
    fs::write(scan_root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(scan_root.join(".git").join("index"), b"git index contents").unwrap();
    fs::write(scan_root.join(".gitignore"), "target/\n").unwrap();
    fs::create_dir_all(scan_root.join("nested")).unwrap();
    fs::write(scan_root.join("nested").join(".gitignore"), "*.log\n").unwrap();

    let config = CacheConfig::from_scan_root(&scan_root);
    let exclude_patterns = build_collection_exclude_patterns(&scan_root, config.root_dir());
    let collected = collect_paths(&scan_root, 0, &exclude_patterns);

    assert!(
        collected
            .files
            .iter()
            .all(|(path, _)| !path.starts_with(scan_root.join(".git")))
    );
    assert!(
        collected
            .files
            .iter()
            .all(|(path, _)| path.file_name().and_then(|name| name.to_str()) != Some(".gitignore"))
    );
    assert_eq!(collected.file_count(), 1);
    assert!(collected.excluded_count >= 3);
}

#[test]
fn resolve_native_scan_selection_uses_paths_file_under_explicit_root() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("repo");
    fs::create_dir_all(scan_root.join("src")).expect("create src dir");
    fs::create_dir_all(scan_root.join("docs")).expect("create docs dir");
    fs::write(scan_root.join("src/lib.rs"), "pub fn demo() {}\n").expect("write lib");
    fs::write(scan_root.join("docs/guide.md"), "# guide\n").expect("write guide");

    let paths_file_a = temp_dir.path().join("changed-a.txt");
    let paths_file_b = temp_dir.path().join("changed-b.txt");
    fs::write(&paths_file_a, "src/lib.rs\r\nmissing.rs\n").expect("write first paths file");
    fs::write(&paths_file_b, "docs\nsrc/lib.rs\n").expect("write second paths file");

    let other_cwd = tempfile::TempDir::new().expect("create alternate cwd");
    let _cwd_guard = CurrentDirGuard::change_to(other_cwd.path());

    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--paths-file",
        paths_file_a.to_str().expect("utf-8 path"),
        "--paths-file",
        paths_file_b.to_str().expect("utf-8 path"),
        scan_root.to_str().expect("utf-8 path"),
    ])
    .expect("cli parse should succeed");

    let request = ScanRequest::from(cli.scan_args().expect("scan args should be present"));
    let result = resolve_native_scan_selection(&request);

    let NativeScanSelection {
        scan_path: resolved_root,
        selected_paths: includes,
        collection_frontier: frontier,
        missing_entries,
    } = result.expect("paths file selection should resolve");
    assert_eq!(resolved_root, scan_root.to_str().expect("utf-8 path"));
    assert_eq!(
        includes,
        vec![
            crate::scan_result_shaping::SelectedPath::Exact("src/lib.rs".to_string()),
            crate::scan_result_shaping::SelectedPath::Subtree("docs".to_string())
        ]
    );
    assert_eq!(
        frontier,
        vec![
            crate::scanner::CollectionFrontier {
                path: std::path::PathBuf::from("src/lib.rs"),
                recurse: false,
            },
            crate::scanner::CollectionFrontier {
                path: std::path::PathBuf::from("docs"),
                recurse: true,
            }
        ]
    );
    assert_eq!(missing_entries, vec!["missing.rs"]);
}

#[test]
fn resolve_native_scan_selection_errors_when_paths_file_keeps_no_existing_entries() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let scan_root = temp_dir.path().join("repo");
    fs::create_dir_all(&scan_root).expect("create scan root");
    let paths_file = temp_dir.path().join("changed.txt");
    fs::write(&paths_file, "missing.rs\n").expect("write paths file");

    let cli = crate::cli::Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--paths-file",
        paths_file.to_str().expect("utf-8 path"),
        scan_root.to_str().expect("utf-8 path"),
    ])
    .expect("cli parse should succeed");

    let request = ScanRequest::from(cli.scan_args().expect("scan args should be present"));
    let error = resolve_native_scan_selection(&request).expect_err("selection should fail");
    assert!(
        error
            .to_string()
            .contains("did not resolve to any existing files or directories")
    );
}

#[test]
fn build_paths_file_warning_messages_formats_missing_entries_for_headers() {
    let warnings =
        build_paths_file_warning_messages(&["missing.rs".to_string(), "docs/guide.md".to_string()]);

    assert_eq!(
        warnings,
        vec![
            "Skipping missing --paths-file entry: missing.rs".to_string(),
            "Skipping missing --paths-file entry: docs/guide.md".to_string(),
        ]
    );
}
