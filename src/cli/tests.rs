// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use clap::CommandFactory;

fn scan_command() -> clap::Command {
    Cli::command()
        .find_subcommand("scan")
        .expect("scan subcommand should exist")
        .clone()
}

#[test]
fn test_requires_at_least_one_output_option() {
    let parsed = Cli::try_parse_from(["provenant", "samples"]);
    assert!(parsed.is_err());
}

#[test]
fn test_parses_json_pretty_output_option() {
    let parsed = Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "samples"])
        .expect("cli parse should succeed");

    assert_eq!(parsed.output_json_pp.as_deref(), Some("scan.json"));
    assert_eq!(parsed.output_targets().len(), 1);
    assert_eq!(parsed.output_targets()[0].format, OutputFormat::JsonPretty);
}

#[test]
fn test_explicit_scan_subcommand_parses_scan_flags() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "scan",
        "--json-pp",
        "scan.json",
        "--license",
        "samples",
    ])
    .expect("explicit scan subcommand should parse");

    assert!(matches!(parsed.command, Command::Scan(_)));
    let scan = parsed.scan_args().expect("scan args should be present");
    assert_eq!(scan.output_json_pp.as_deref(), Some("scan.json"));
    assert!(scan.license);
    assert_eq!(scan.dir_path, vec!["samples"]);
}

#[test]
fn test_parses_compare_subcommand() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "compare",
        "--scancode-json",
        "scan-a.json",
        "--provenant-json",
        "scan-b.json",
        "--artifact-dir",
        "compare-out",
    ])
    .expect("compare subcommand should parse");

    match parsed.command {
        Command::Compare(args) => {
            assert_eq!(args.scancode_json, PathBuf::from("scan-a.json"));
            assert_eq!(args.provenant_json, PathBuf::from("scan-b.json"));
            assert_eq!(args.artifact_dir, Some(PathBuf::from("compare-out")));
        }
        other => panic!("expected compare subcommand, got {other:?}"),
    }
}

#[test]
fn test_parses_serve_subcommand() {
    let parsed = Cli::try_parse_from(["provenant", "serve", "--bind", "127.0.0.1:9090"])
        .expect("serve subcommand should parse");

    match parsed.command {
        Command::Serve(args) => assert_eq!(args.bind, "127.0.0.1:9090"),
        other => panic!("expected serve subcommand, got {other:?}"),
    }
}

#[test]
fn test_compare_subcommand_allows_default_artifact_dir() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "compare",
        "--scancode-json",
        "scan-a.json",
        "--provenant-json",
        "scan-b.json",
    ])
    .expect("compare subcommand should allow default artifact dir");

    match parsed.command {
        Command::Compare(args) => {
            assert_eq!(args.scancode_json, PathBuf::from("scan-a.json"));
            assert_eq!(args.provenant_json, PathBuf::from("scan-b.json"));
            assert!(args.artifact_dir.is_none());
        }
        other => panic!("expected compare subcommand, got {other:?}"),
    }
}

#[test]
fn test_unknown_command_like_token_is_not_rewritten_to_scan() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "future-command",
        "--json-pp",
        "scan.json",
        "samples",
    ]);

    let error = parsed.expect_err("unknown command-like token should fail");
    assert!(
        error
            .to_string()
            .contains("unrecognized subcommand 'future-command'")
    );
}

#[test]
fn test_allows_multiple_output_options_in_one_run() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json",
        "scan.json",
        "--html",
        "report.html",
        "samples",
    ])
    .expect("cli parse should allow multiple outputs");

    assert_eq!(parsed.output_targets().len(), 2);
    assert_eq!(parsed.output_targets()[0].format, OutputFormat::Json);
    assert_eq!(parsed.output_targets()[1].format, OutputFormat::Html);
}

#[test]
fn test_parses_show_attribution_subcommand() {
    let parsed = Cli::try_parse_from(["provenant", "show-attribution"])
        .expect("show-attribution subcommand should parse");

    assert!(matches!(parsed.command, Command::ShowAttribution));
}

#[test]
fn test_legacy_show_attribution_flag_is_rejected() {
    let parsed = Cli::try_parse_from(["provenant", "--show-attribution"]);
    assert!(parsed.is_err());
}

#[test]
fn test_export_license_dataset_allows_mode_without_output_file() {
    let parsed = Cli::try_parse_from(["provenant", "export-license-dataset", "dataset-out"])
        .expect("cli parse should allow export mode without output flags");

    match parsed.command {
        Command::ExportLicenseDataset(args) => assert_eq!(args.dir, "dataset-out"),
        other => panic!("expected export subcommand, got {other:?}"),
    }
}

#[test]
fn test_legacy_export_license_dataset_flag_is_rejected() {
    let parsed = Cli::try_parse_from(["provenant", "--export-license-dataset", "dataset-out"]);
    assert!(parsed.is_err());
}

#[test]
fn test_license_dataset_path_parses_for_license_scans() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--license-dataset-path",
        "dataset-root",
        "samples",
    ])
    .expect("cli parse should accept custom license dataset flag");

    assert_eq!(parsed.license_dataset_path.as_deref(), Some("dataset-root"));
}

#[test]
fn test_output_header_options_use_scancode_style_keys() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--package",
        "--strip-root",
        "--paths-file",
        "changed-files.txt",
        "--ignore",
        "*.git*",
        "--ignore",
        "target/*",
        "samples",
    ])
    .expect("cli parse should succeed");

    let options = parsed.output_header_options();

    assert_eq!(
        options.get("input"),
        Some(&JsonValue::Array(vec![JsonValue::String(
            "samples".to_string()
        )]))
    );
    assert_eq!(
        options.get("--json-pp"),
        Some(&JsonValue::String("scan.json".to_string()))
    );
    assert_eq!(options.get("--license"), Some(&JsonValue::Bool(true)));
    assert_eq!(options.get("--package"), Some(&JsonValue::Bool(true)));
    assert_eq!(
        options.get("--paths-file"),
        Some(&JsonValue::Array(vec![JsonValue::String(
            "changed-files.txt".to_string()
        )]))
    );
    assert_eq!(options.get("--strip-root"), Some(&JsonValue::Bool(true)));
    assert_eq!(
        options.get("--ignore"),
        Some(&JsonValue::Array(vec![
            JsonValue::String("*.git*".to_string()),
            JsonValue::String("target/*".to_string()),
        ]))
    );
    assert!(!options.contains_key("--compat-mode"));
}

#[test]
fn test_compat_mode_parses_and_is_recorded_when_non_default() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--copyright",
        "--compat-mode",
        "scancode",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert_eq!(parsed.compat_mode, CompatibilityMode::Scancode);
    let options = parsed.output_header_options();
    assert_eq!(
        options.get("--compat-mode"),
        Some(&JsonValue::String("scancode".to_string()))
    );
}

#[test]
fn test_output_header_options_include_license_dataset_path_when_set() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--license-dataset-path",
        "dataset-root",
        "samples",
    ])
    .expect("cli parse should accept custom license dataset flag");

    let options = parsed.output_header_options();
    assert_eq!(
        options.get("--license-dataset-path"),
        Some(&JsonValue::String("dataset-root".to_string()))
    );
}

#[test]
fn test_output_header_options_skip_defaults_and_include_non_defaults() {
    let default_options = Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "samples"])
        .expect("default cli parse should succeed")
        .output_header_options();
    assert!(!default_options.contains_key("--timeout"));
    assert!(!default_options.contains_key("--processes"));

    let custom_options = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--timeout",
        "30",
        "--processes",
        "4",
        "samples",
    ])
    .expect("custom cli parse should succeed")
    .output_header_options();

    assert_eq!(
        custom_options.get("--timeout"),
        Some(&JsonValue::Number(
            JsonNumber::from_f64(30.0).expect("valid number")
        ))
    );
    assert_eq!(
        custom_options.get("--processes"),
        Some(&JsonValue::Number(4.into()))
    );
}

#[test]
fn test_allows_stdout_dash_as_output_target() {
    let parsed = Cli::try_parse_from(["provenant", "--json-pp", "-", "samples"])
        .expect("cli parse should allow stdout dash output target");

    assert_eq!(parsed.output_json_pp.as_deref(), Some("-"));
}

#[test]
fn test_debian_requires_license_copyright_and_license_text() {
    let missing_license_text = Cli::try_parse_from([
        "provenant",
        "--debian",
        "scan.copyright",
        "--license",
        "--copyright",
        "samples",
    ]);
    assert!(missing_license_text.is_err());

    let parsed = Cli::try_parse_from([
        "provenant",
        "--debian",
        "scan.copyright",
        "--license",
        "--copyright",
        "--license-text",
        "samples",
    ])
    .expect("cli parse should accept debian output");

    assert_eq!(parsed.output_targets().len(), 1);
    assert_eq!(parsed.output_targets()[0].format, OutputFormat::Debian);
    assert_eq!(parsed.output_debian.as_deref(), Some("scan.copyright"));
}

#[test]
fn test_debian_help_mentions_required_companion_flags() {
    let command = scan_command();
    let debian_arg = command
        .get_arguments()
        .find(|arg| arg.get_long() == Some("debian"))
        .expect("debian arg should exist");

    let help = debian_arg
        .get_help()
        .expect("debian arg should have help text")
        .to_string();

    assert!(help.contains("requires --license, --copyright, and --license-text"));
}

#[test]
fn test_scan_help_mentions_pdf_oxide_rust_log_escape_hatch() {
    let help = scan_command().render_help().to_string();

    assert!(help.contains("RUST_LOG=pdf_oxide=warn"));
    assert!(help.contains("suppresses noisy pdf_oxide logs by default"));
}

#[test]
fn test_root_help_mentions_subcommands() {
    let help = Cli::command().render_help().to_string();

    assert!(help.contains("scan"));
    assert!(help.contains("serve"));
    assert!(help.contains("compare"));
    assert!(help.contains("show-attribution"));
    assert!(help.contains("export-license-dataset"));
}

#[test]
fn test_root_help_mentions_non_affiliation() {
    let help = Cli::command().render_help().to_string();

    assert!(help.contains("Not affiliated with, endorsed by, or sponsored by"));
    assert!(help.contains("ScanCode Toolkit"));
}

#[test]
fn test_parses_license_policy_flag() {
    let temp = tempfile::tempdir().expect("temp dir");
    let policy_path = temp.path().join("policy.yml");
    std::fs::write(&policy_path, "license_policies: []\n").expect("policy written");

    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license-policy",
        policy_path.to_str().expect("utf8 path"),
        "samples",
    ])
    .expect("cli parse should accept license-policy");

    assert_eq!(
        parsed.license_policy.as_deref(),
        Some(policy_path.to_str().expect("utf8 path"))
    );
}

#[test]
fn test_rejects_invalid_license_policy_flag_value() {
    let temp = tempfile::tempdir().expect("temp dir");
    let policy_path = temp.path().join("policy.yml");
    std::fs::write(&policy_path, "not_license_policies: []\n").expect("policy written");

    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license-policy",
        policy_path.to_str().expect("utf8 path"),
        "samples",
    ]);

    assert!(parsed.is_err());
}

#[test]
fn test_custom_template_and_output_must_be_paired() {
    let missing_template =
        Cli::try_parse_from(["provenant", "--custom-output", "result.txt", "samples"]);
    assert!(missing_template.is_err());

    let missing_output =
        Cli::try_parse_from(["provenant", "--custom-template", "tpl.tera", "samples"]);
    assert!(missing_output.is_err());
}

#[test]
fn test_parses_processes_and_timeout_options() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "-n",
        "4",
        "--timeout",
        "30",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert_eq!(parsed.processes, ProcessMode::Parallel(4));
    assert_eq!(parsed.timeout, 30.0);
}

#[test]
fn test_strip_root_conflicts_with_full_root() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--strip-root",
        "--full-root",
        "samples",
    ]);
    assert!(parsed.is_err());
}

#[test]
fn test_parses_include_and_only_findings_and_filter_clues() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--include",
        "src/**,Cargo.toml",
        "--only-findings",
        "--filter-clues",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert_eq!(parsed.include, vec!["src/**", "Cargo.toml"]);
    assert!(parsed.only_findings);
    assert!(parsed.filter_clues);
}

#[test]
fn test_parses_repeated_paths_file_flags_including_stdin_dash() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--paths-file",
        "changed-files.txt",
        "--paths-file",
        "-",
        "samples",
    ])
    .expect("cli parse should accept repeated --paths-file flags");

    assert_eq!(parsed.paths_file, vec!["changed-files.txt", "-"]);
}

#[test]
fn test_parses_ignore_author_and_holder_filters() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--ignore-author",
        "Jane.*",
        "--ignore-author",
        ".*Bot$",
        "--ignore-copyright-holder",
        "Example Corp",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert_eq!(parsed.ignore_author, vec!["Jane.*", ".*Bot$"]);
    assert_eq!(parsed.ignore_copyright_holder, vec!["Example Corp"]);
}

#[test]
fn test_parses_ignore_alias_for_exclude_patterns() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--ignore",
        "*.git*,target/*",
        "samples",
    ])
    .expect("cli parse should accept --ignore alias");

    assert_eq!(parsed.exclude, vec!["*.git*", "target/*"]);
}

#[test]
fn test_quiet_conflicts_with_verbose() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--quiet",
        "--verbose",
        "samples",
    ]);
    assert!(parsed.is_err());
}

#[test]
fn test_parses_from_json_and_mark_source() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--from-json",
        "--info",
        "--mark-source",
        "sample-scan.json",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.from_json);
    assert!(parsed.info);
    assert_eq!(parsed.dir_path, vec!["sample-scan.json"]);
    assert!(parsed.mark_source);
}

#[test]
fn test_mark_source_requires_info() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--mark-source",
        "samples",
    ]);

    assert!(parsed.is_err());
}

#[test]
fn test_parses_classify_facet_and_tallies_by_facet() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--classify",
        "--tallies",
        "--facet",
        "dev=*.c",
        "--facet",
        "tests=*/tests/*",
        "--tallies-by-facet",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.classify);
    assert!(parsed.tallies);
    assert_eq!(parsed.facet, vec!["dev=*.c", "tests=*/tests/*"]);
    assert!(parsed.tallies_by_facet);
}

#[test]
fn test_tallies_by_facet_requires_facet_definitions() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--tallies-by-facet",
        "samples",
    ]);

    assert!(parsed.is_err());
}

#[test]
fn test_summary_requires_classify() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--summary",
        "samples",
    ]);

    assert!(parsed.is_err());
}

#[test]
fn test_tallies_key_files_requires_tallies_and_classify() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--tallies-key-files",
        "samples",
    ]);

    assert!(parsed.is_err());
}

#[test]
fn test_parses_summary_tallies_and_generated_flags() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--classify",
        "--summary",
        "--license-clarity-score",
        "--tallies",
        "--tallies-key-files",
        "--tallies-with-details",
        "--generated",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.classify);
    assert!(parsed.summary);
    assert!(parsed.license_clarity_score);
    assert!(parsed.tallies);
    assert!(parsed.tallies_key_files);
    assert!(parsed.tallies_with_details);
    assert!(parsed.generated);
}

#[test]
fn test_parses_copyright_flag() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--copyright",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.copyright);
}

#[test]
fn test_package_flag_defaults_to_disabled() {
    let parsed = Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "samples"])
        .expect("cli parse should succeed");

    assert!(!parsed.package);
}

#[test]
fn test_parses_system_package_flag() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--system-package",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.system_package);
}

#[test]
fn test_parses_package_in_compiled_flag() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--package-in-compiled",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.package_in_compiled);
}

#[test]
fn test_parses_package_only_flag() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--package-only",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.package_only);
}

#[test]
fn test_package_only_conflicts_with_upstream_incompatible_flags() {
    let with_license = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--package-only",
        "--license",
        "samples",
    ]);
    assert!(with_license.is_err());

    let with_package = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--package-only",
        "--package",
        "samples",
    ]);
    assert!(with_package.is_err());
}

#[test]
fn test_parses_package_flag() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--package",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.package);
}

#[test]
fn test_package_short_flag() {
    let parsed = Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "-p", "samples"])
        .expect("cli parse should succeed");

    assert!(parsed.package);
}

#[test]
fn test_parses_license_flag() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.license);
}

#[test]
fn test_license_short_flag() {
    let parsed = Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "-l", "samples"])
        .expect("cli parse should succeed");

    assert!(parsed.license);
}

#[test]
fn test_license_text_requires_license() {
    let result = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license-text",
        "samples",
    ]);
    assert!(result.is_err());
}

#[test]
fn test_include_text_is_rejected() {
    let result = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--include-text",
        "samples",
    ]);

    assert!(result.is_err());
}

#[test]
fn test_license_text_diagnostics_requires_license_text() {
    let result = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--license-text-diagnostics",
        "samples",
    ]);

    assert!(result.is_err());
}

#[test]
fn test_parses_license_text_and_diagnostics_flags() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--license-text",
        "--license-text-diagnostics",
        "--license-diagnostics",
        "--unknown-licenses",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.license_text);
    assert!(parsed.license_text_diagnostics);
    assert!(parsed.license_diagnostics);
    assert!(parsed.unknown_licenses);
    assert_eq!(parsed.license_score, 0);
    assert_eq!(parsed.license_url_template, DEFAULT_LICENSEDB_URL_TEMPLATE);
}

#[test]
fn test_parses_no_sequence_matching_flag() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--no-sequence-matching",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.no_sequence_matching);
}

#[test]
fn test_license_score_requires_license() {
    let result = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license-score",
        "70",
        "samples",
    ]);

    assert!(result.is_err());
}

#[test]
fn test_license_url_template_requires_license() {
    let result = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license-url-template",
        "https://example.com/licenses/{}/",
        "samples",
    ]);

    assert!(result.is_err());
}

#[test]
fn test_parses_license_score_and_url_template_flags() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--license-score",
        "70",
        "--license-url-template",
        "https://example.com/licenses/{}/",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert_eq!(parsed.license_score, 70);
    assert_eq!(
        parsed.license_url_template,
        "https://example.com/licenses/{}/"
    );
}

#[test]
fn test_rejects_license_score_above_range() {
    let result = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--license-score",
        "101",
        "samples",
    ]);

    assert!(result.is_err());
}

#[test]
fn test_license_references_requires_license() {
    let result = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license-references",
        "samples",
    ]);

    assert!(result.is_err());
}

#[test]
fn test_parses_license_references_flag() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--license-references",
        "samples",
    ])
    .expect("cli parse should succeed");

    assert!(parsed.license_references);
}

#[test]
fn test_include_text_alias_is_not_supported() {
    let result = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--include-text",
        "samples",
    ]);

    assert!(result.is_err());
}

#[test]
fn test_parses_short_scan_flags() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "-c",
        "-e",
        "-u",
        "samples",
    ])
    .expect("cli parse should support short scan flags");

    assert!(parsed.copyright);
    assert!(parsed.email);
    assert!(parsed.url);
}

#[test]
fn test_parses_processes_compat_values_zero_and_minus_one() {
    let zero = Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "-n", "0", "samples"])
        .expect("cli parse should accept processes=0");
    assert_eq!(zero.processes, ProcessMode::SequentialWithTimeouts);

    let parsed =
        Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "-n", "-1", "samples"])
            .expect("cli parse should accept processes=-1");
    assert_eq!(parsed.processes, ProcessMode::SequentialWithoutTimeouts);
}

#[test]
fn test_parses_cache_flags() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--cache-dir",
        "/tmp/sc-cache",
        "--cache-clear",
        "--max-in-memory",
        "5000",
        "samples",
    ])
    .expect("cli parse should accept cache flags");

    assert_eq!(parsed.cache_dir.as_deref(), Some("/tmp/sc-cache"));
    assert!(parsed.cache_clear);
    assert!(!parsed.incremental);
    assert_eq!(parsed.max_in_memory, MemoryMode::Limit(5000));
}

#[test]
fn test_parses_incremental_flag() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--incremental",
        "samples",
    ])
    .expect("cli parse should accept incremental flag");

    assert!(parsed.incremental);
}

#[test]
fn test_parses_license_cache_control_flags() {
    let parsed = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--license",
        "--reindex",
        "--no-license-index-cache",
        "samples",
    ])
    .expect("cli parse should accept license cache flags");

    assert!(parsed.license);
    assert!(parsed.reindex);
    assert!(parsed.no_license_index_cache);
}

#[test]
fn test_max_in_memory_defaults_and_special_values() {
    let default_parsed = Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "samples"])
        .expect("default max-in-memory should parse");
    assert_eq!(default_parsed.max_in_memory, MemoryMode::Limit(10000));

    let disk_only = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--max-in-memory",
        "-1",
        "samples",
    ])
    .expect("-1 should parse");
    assert_eq!(disk_only.max_in_memory, MemoryMode::StreamUnlimited);

    let unlimited = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--max-in-memory",
        "0",
        "samples",
    ])
    .expect("0 should parse");
    assert_eq!(unlimited.max_in_memory, MemoryMode::CollectFirst);
}

#[test]
fn test_max_in_memory_rejects_values_below_negative_one() {
    let result = Cli::try_parse_from([
        "provenant",
        "--json-pp",
        "scan.json",
        "--max-in-memory",
        "-2",
        "samples",
    ]);

    assert!(result.is_err());
}

#[test]
fn test_max_depth_default_matches_reference_behavior() {
    let parsed = Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "samples"])
        .expect("cli parse should succeed");

    assert_eq!(parsed.max_depth, 0);
}
