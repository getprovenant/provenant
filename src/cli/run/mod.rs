// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::app::request::ScanRequest;
use crate::app::scan_pipeline::execute_request;
use crate::cli::{Cli, Command, ScanArgs};
use crate::compare::compare_json_files;
use crate::license_detection::dataset::export_embedded_license_dataset;
use crate::output::{OutputWriteConfig, write_output_file};
use crate::progress::{ScanProgress, init_cli_logger};
use crate::serve::run as run_serve_shell;
use crate::time::format_scancode_timestamp;
use anyhow::{Result, anyhow};
use chrono::Utc;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

/// Exit code returned when the `--fail-on` license-policy gate trips, distinct from
/// the code used for scan/runtime errors (see ADR 0011).
const POLICY_GATE_EXIT_CODE: u8 = 3;

pub fn run() -> Result<ExitCode> {
    #[cfg(feature = "golden-tests")]
    touch_license_golden_symbols();

    let cli = Cli::parse();

    // Every subcommand except `scan` installs a plain global logger here so its
    // `log::*` diagnostics are actually emitted (respecting `-q`/`-v`). `scan`
    // installs an indicatif-aware bridge later, once its progress system exists.
    match &cli.command {
        Command::Scan(_) => {}
        Command::Serve(args) => init_cli_logger(args.verbosity.verbosity().log_level()),
        Command::Compare(args) => init_cli_logger(args.verbosity.verbosity().log_level()),
        Command::ExportLicenseDataset(args) => {
            init_cli_logger(args.verbosity.verbosity().log_level())
        }
        Command::ShowAttribution => init_cli_logger(crate::cli::Verbosity::Normal.log_level()),
    }

    match &cli.command {
        Command::ShowAttribution => {
            print!("{}", include_str!("../../../NOTICE"));
            return Ok(ExitCode::SUCCESS);
        }
        Command::Serve(args) => {
            return run_serve_shell(args).map(|()| ExitCode::SUCCESS);
        }
        Command::Compare(args) => {
            log::info!(
                "Comparing ScanCode JSON {} against Provenant JSON {}...",
                args.scancode_json.display(),
                args.provenant_json.display()
            );
            let result = compare_json_files(
                &args.scancode_json,
                &args.provenant_json,
                args.artifact_dir.as_deref(),
            )?;
            println!("Comparison status: {}", result.comparison_status);
            println!("Artifacts:");
            println!("  Artifact directory: {}", result.artifact_dir.display());
            println!("  Run manifest:       {}", result.manifest_path.display());
            println!("  Raw ScanCode JSON:  {}", result.scancode_json.display());
            println!("  Raw Provenant JSON: {}", result.provenant_json.display());
            println!("  Summary JSON:       {}", result.summary_json.display());
            println!("  Summary TSV:        {}", result.summary_tsv.display());
            println!("  Sample artifacts:   {}", result.samples_dir.display());
            return Ok(ExitCode::SUCCESS);
        }
        Command::ExportLicenseDataset(args) => {
            let dir = Path::new(&args.dir);
            log::info!("Exporting built-in license dataset to {}...", dir.display());

            let started = Instant::now();
            let outcome = export_embedded_license_dataset(dir)?;

            log::info!(
                "Exported {} license dataset files to {} in {:.1}s (SPDX list {})",
                outcome.files_written,
                dir.display(),
                started.elapsed().as_secs_f64(),
                outcome.manifest.spdx_license_list_version
            );
            return Ok(ExitCode::SUCCESS);
        }
        Command::Scan(_) => {}
    }

    let cli = cli
        .scan_args()
        .expect("scan arguments should exist after command dispatch");

    validate_scan_option_compatibility(cli)?;

    let request = ScanRequest::from(cli);
    let executed = execute_request(&request)?;
    let output = executed.output;
    let progress = executed.progress;
    let start_time = executed.start_time;

    let output_schema_output =
        crate::output_schema::Output::from_with_compat_mode(&output, cli.compat_mode);
    progress.start_output();
    for target in &request.output_targets {
        let output_config = OutputWriteConfig {
            format: target.format,
            custom_template: target.custom_template.clone(),
            scanned_path: if request.input_paths.len() == 1 {
                request.input_paths.first().cloned()
            } else {
                None
            },
        };

        let timing_name = format!("output:{:?}", target.format).to_lowercase();
        record_detail_timing(&progress, timing_name, || {
            write_output_file(&target.file, &output_schema_output, &output_config)
        })?;
        progress.output_written(&format!(
            "{:?} output written to {}",
            target.format, target.file
        ));
    }
    progress.record_final_counts(&output.files);
    progress.record_final_header_counts(&output.headers);
    progress.finish_output();

    let summary_end = Utc::now();
    progress.display_summary(
        &format_scancode_timestamp(&start_time),
        &format_scancode_timestamp(&summary_end),
    );

    // License-policy gate: evaluated after the report is written so the artifact is
    // never lost to a failing gate (ADR 0011). Covers file-level detections plus
    // package/dependency declared licenses.
    if let Some(threshold) = request.fail_on {
        let file_violations = count_policy_violations(&output.files, threshold);
        // Fail closed: propagate a policy re-evaluation failure instead of treating
        // it as zero package/dependency violations.
        let declared_violations = match request.license_policy.as_deref() {
            Some(policy_path) => crate::post_processing::count_declared_license_policy_violations(
                Path::new(policy_path),
                &output.packages,
                &output.dependencies,
                threshold,
            )?,
            None => 0,
        };
        let violations = file_violations + declared_violations;
        if violations > 0 {
            log::error!(
                "License policy gate: {file_violations} file(s) and {declared_violations} package/dependency license(s) match a policy at or above `{threshold:?}` severity; failing with exit code {POLICY_GATE_EXIT_CODE}."
            );
            return Ok(ExitCode::from(POLICY_GATE_EXIT_CODE));
        }
    }

    Ok(ExitCode::SUCCESS)
}

/// Count files whose license policy carries a `compliance_alert` at or above `threshold`.
fn count_policy_violations(
    files: &[crate::models::FileInfo],
    threshold: crate::models::ComplianceAlert,
) -> usize {
    files
        .iter()
        .filter(|file| {
            file.license_policy.as_ref().is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry
                        .compliance_alert
                        .is_some_and(|alert| alert >= threshold)
                })
            })
        })
        .count()
}

#[cfg(feature = "golden-tests")]
fn touch_license_golden_symbols() {
    let _ = crate::license_detection::golden_utils::read_golden_input_content;
    let _ = crate::license_detection::golden_utils::detect_matches_for_golden;
    let _ = crate::license_detection::golden_utils::detect_license_expressions_for_golden;
    let _ = crate::license_detection::LicenseDetectionEngine::detect_matches_with_kind;
}

fn validate_scan_option_compatibility(cli: &ScanArgs) -> Result<()> {
    if cli.from_json
        && (cli.package
            || cli.system_package
            || cli.package_in_compiled
            || cli.package_only
            || cli.copyright
            || cli.email
            || cli.url
            || cli.generated)
    {
        return Err(anyhow!(
            "When using --from-json, file scan options like --package/--copyright/--email/--url/--generated are not allowed"
        ));
    }

    if cli.from_json && !cli.paths_file.is_empty() {
        return Err(anyhow!(
            "--paths-file is only supported for native scan mode, not --from-json"
        ));
    }

    if cli.from_json && cli.incremental {
        return Err(anyhow!(
            "--incremental is only supported for directory scan mode, not --from-json"
        ));
    }

    if !cli.paths_file.is_empty() && cli.dir_path.len() != 1 {
        return Err(anyhow!(
            "--paths-file requires exactly one positional scan root"
        ));
    }

    if !cli.from_json && cli.dir_path.is_empty() {
        return Err(anyhow!("Directory path is required for scan operations"));
    }

    if cli.tallies_by_facet && cli.facet.is_empty() {
        return Err(anyhow!(
            "--tallies-by-facet requires at least one --facet <facet>=<pattern> definition"
        ));
    }

    if cli.mark_source && !cli.info {
        return Err(anyhow!("--mark-source requires --info"));
    }

    Ok(())
}

fn record_detail_timing<T, F>(progress: &Arc<ScanProgress>, name: impl Into<String>, f: F) -> T
where
    F: FnOnce() -> T,
{
    let started = Instant::now();
    let result = f();
    progress.record_detail_timing(name.into(), started.elapsed().as_secs_f64());
    result
}

#[cfg(test)]
mod tests;
