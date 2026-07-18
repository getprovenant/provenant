// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::app::request::{InputMode, OutputTarget, ScanRequest};
use crate::app::scan_pipeline::execute_request;
use crate::cli::{Cli, Command, ScanArgs};
use crate::compare::compare_json_files;
use crate::license_detection::dataset::export_embedded_license_dataset;
use crate::models::{FileType, Output};
use crate::output::{OutputFormat, OutputWriteConfig, write_output_file};
use crate::progress::{ProgressMode, ScanProgress, init_cli_logger};
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

/// Exit code returned when one or more `--from-json` SBOM output targets were
/// refused by [`hollow_from_json_sbom_refusal`] because the reshaped input
/// never ran package detection. Distinct from the scan/runtime error code (1)
/// and the license-policy gate code (3), so CI can tell "an SBOM target was
/// skipped for honesty reasons" apart from "the tool broke" or "a policy
/// violation." Non-SBOM output targets in the same request are still written
/// (see the output loop in `run`), matching the ADR 0011 pattern of never
/// losing an artifact to a gate.
const HOLLOW_SBOM_GUARD_EXIT_CODE: u8 = 4;

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

    // A hollow-SBOM refusal only blocks the specific SBOM output target(s)
    // below; it must not short-circuit non-SBOM outputs in the same request
    // (e.g. `--spdx-tv out.spdx --json out.json`), so it is computed as a
    // value up front instead of using `?` to bail out of `run` immediately.
    // Covers two independent causes: a `--from-json` reshape whose source
    // scan never ran package detection, and a native `--package-only`/
    // `--no-assemble` scan that unconditionally skipped assembly this run.
    let hollow_sbom_refusal = hollow_from_json_sbom_refusal(
        &request,
        &output,
        executed.has_hollow_package_detection_input,
    )
    .or_else(|| assembly_skipped_sbom_refusal(&request, &output));

    // Unlike the refusal above, this only warns: `--paths-file` assembly can
    // produce a genuinely well-formed SBOM, just one that may understate the
    // repository if the selection omitted sibling manifests or a workspace
    // root. Skipped once a hollow-SBOM refusal already blocks every SBOM
    // target this run, since there is then no written SBOM left to warn
    // about.
    if hollow_sbom_refusal.is_none()
        && let Some(warning) = paths_file_sbom_completeness_warning(&request)
    {
        emit_sbom_guard_diagnostic(request.progress_mode, &warning, false);
    }

    let output_schema_output =
        crate::output_schema::Output::from_with_compat_mode(&output, cli.compat_mode);
    progress.start_output();
    for target in &request.output_targets {
        if hollow_sbom_refusal.is_some() && SBOM_OUTPUT_FORMATS.contains(&target.format) {
            emit_sbom_guard_diagnostic(
                request.progress_mode,
                &format!(
                    "Skipping {:?} output to {}: {}",
                    target.format,
                    target.file,
                    hollow_sbom_refusal.as_deref().unwrap_or_default()
                ),
                true,
            );
            continue;
        }

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

    // Fails the run after all allowed outputs are already written (same
    // never-lose-the-artifact shape as the license-policy gate below).
    if let Some(refusal) = hollow_sbom_refusal {
        emit_sbom_guard_diagnostic(
            request.progress_mode,
            &format!(
                "Hollow-SBOM guard: {refusal} Failing with exit code {HOLLOW_SBOM_GUARD_EXIT_CODE}."
            ),
            true,
        );
        return Ok(ExitCode::from(HOLLOW_SBOM_GUARD_EXIT_CODE));
    }

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

/// Emits an SBOM-guard diagnostic (a refusal explanation or a completeness
/// warning) so it is never silently dropped.
///
/// `ScanProgress::init_logging_bridge` deliberately never installs a logger
/// at all in `--quiet` mode (see `src/progress.rs`), so a plain
/// `log::error!`/`log::warn!` call would vanish with no trace — even though a
/// refusal still changes the process's exit code, and a `--quiet` caller who
/// only checks the exit code would otherwise have no way to learn why an SBOM
/// target was skipped. Falling back to a direct `eprintln!` in that case
/// keeps the message honest without depending on whether a logger happens to
/// be installed. In Default/Verbose mode the bridge is installed and active,
/// so this routes through `log` as usual to stay interleaved with progress
/// bars instead of writing over them.
fn emit_sbom_guard_diagnostic(progress_mode: ProgressMode, message: &str, is_error: bool) {
    if progress_mode == ProgressMode::Quiet {
        eprintln!("{message}");
    } else if is_error {
        log::error!("{message}");
    } else {
        log::warn!("{message}");
    }
}

/// SBOM-oriented output formats whose entire value proposition is the package
/// inventory: an SPDX or CycloneDX document with zero packages looks like a
/// normal, successful export while actually reporting no components at all.
const SBOM_OUTPUT_FORMATS: &[OutputFormat] = &[
    OutputFormat::SpdxTv,
    OutputFormat::SpdxRdf,
    OutputFormat::CycloneDxJson,
    OutputFormat::CycloneDxXml,
];

fn sbom_output_flag(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::SpdxTv => "--spdx-tv",
        OutputFormat::SpdxRdf => "--spdx-rdf",
        OutputFormat::CycloneDxJson => "--cyclonedx",
        OutputFormat::CycloneDxXml => "--cyclonedx-xml",
        _ => "<sbom-format>",
    }
}

/// CLI flags for whichever SBOM formats (see [`SBOM_OUTPUT_FORMATS`]) appear
/// among `output_targets`, in request order. Empty when none were requested.
fn requested_sbom_flags(output_targets: &[OutputTarget]) -> Vec<&'static str> {
    output_targets
        .iter()
        .map(|target| target.format)
        .filter(|format| SBOM_OUTPUT_FORMATS.contains(format))
        .map(sbom_output_flag)
        .collect()
}

/// Returns a refusal message when a `--from-json` reshape requests an SBOM
/// format (`--spdx-tv`/`--spdx-rdf`/`--cyclonedx`/`--cyclonedx-xml`) but at
/// least one merged input would contribute a hollow document: real scanned
/// files that no package detection ever examined.
///
/// `--from-json` reshapes an existing scan without rescanning (see
/// `docs/CLI_GUIDE.md`), so it cannot recover package data the original scan
/// never collected. Emitting the SBOM anyway would silently succeed with an
/// empty (or partially examined) inventory: CycloneDX would write an empty
/// `components` array and SPDX would fall back to its single-synthetic-package
/// "no packages" projection, both indistinguishable from a normal successful
/// export.
///
/// This is a query, not a hard gate: it returns `Some(message)` instead of an
/// `Err` so the caller can skip only the requested SBOM output target(s)
/// while still writing any other requested output formats in the same
/// request, then fail the run's exit code once all allowed outputs are
/// written (see the output loop and [`HOLLOW_SBOM_GUARD_EXIT_CODE`] in `run`).
///
/// `has_hollow_package_detection_input` is `true` when *any* merged
/// `--from-json` input had scanned files, no packages of its own, and never
/// requested package detection in its own recorded header options
/// (`--package`, `--package-only`, `--system-package`, or
/// `--package-in-compiled`) — see `is_hollow_package_detection_input` in
/// `src/scan_result_shaping/json_input.rs`. This is tracked per input at
/// merge time, so **one merged input honestly requesting package detection
/// (or already carrying real packages) can never silence another merged
/// input's hollow contribution**: the refusal fires even if the overall
/// `output.packages` ends up non-empty because of a different input.
///
/// This intentionally does **not** fire for:
/// - native scans (only `--from-json` reshapes lack a fresh detection pass);
/// - requests that do not ask for an SBOM format;
/// - a truly empty scan document (no files at all) — that keeps the existing
///   documented empty-SBOM sentinel behavior;
/// - a `--from-json` input (or merge of inputs) where every input either had
///   no files or actually requested package detection upstream, since zero
///   packages then means the codebase honestly has none, not that nothing
///   looked.
fn hollow_from_json_sbom_refusal(
    request: &ScanRequest,
    output: &Output,
    has_hollow_package_detection_input: bool,
) -> Option<String> {
    if !matches!(request.input_mode, InputMode::FromJson) || !has_hollow_package_detection_input {
        return None;
    }

    let scanned_file_count = output
        .files
        .iter()
        .filter(|file| file.file_type == FileType::File)
        .count();
    if scanned_file_count == 0 {
        return None;
    }

    let requested_sbom_flags = requested_sbom_flags(&request.output_targets);
    if requested_sbom_flags.is_empty() {
        return None;
    }

    Some(format!(
        "Refusing to write a hollow SBOM for {}: the merged --from-json input has {} scanned file(s) \
         overall, but at least one merged source's files were never examined for packages (its recorded \
         scan options never requested --package, --package-only, --system-package, or \
         --package-in-compiled), even though another merged source may have. Rerun that source's original \
         scan with one of those flags before reshaping to an SBOM format, or drop {} if a package-less \
         scan was intended.",
        requested_sbom_flags.join(", "),
        scanned_file_count,
        requested_sbom_flags.join(", "),
    ))
}

/// Returns a refusal message when a **native** scan requests an SBOM format
/// while `--package-only` or `--no-assemble` forced top-level package
/// assembly to be skipped for the whole run (see `skip_assembly` in
/// `src/app/scan_pipeline.rs`).
///
/// Both flags unconditionally zero out `output.packages`/`output.dependencies`
/// regardless of what was scanned: `--package-only` intentionally trades the
/// top-level assembled view for a faster, narrower per-file pass, and
/// `--no-assemble` disables assembly outright. Either way, CycloneDX would
/// write an empty `components` array and SPDX would fall back to its
/// no-package projection — both indistinguishable from a normal, honest
/// export with zero packages, even though the scanned files may carry
/// per-file package manifests that were simply never assembled into the
/// top-level view the SBOM formats read from.
///
/// Like [`hollow_from_json_sbom_refusal`], this is a query, not a hard gate:
/// it returns `Some(message)` so the caller can skip only the requested SBOM
/// output target(s) while still writing any other requested output formats
/// in the same request (see the output loop and
/// [`HOLLOW_SBOM_GUARD_EXIT_CODE`] in `run`).
///
/// This intentionally does **not** fire for:
/// - `--from-json` reshapes (covered separately by
///   [`hollow_from_json_sbom_refusal`], which reasons about the *source*
///   scan's recorded options rather than this run's flags);
/// - requests that do not ask for an SBOM format;
/// - a truly empty scan document (no files at all);
/// - the (currently impossible, but defensively checked) case where
///   `output.packages` is non-empty despite the skip, so this guard degrades
///   gracefully rather than fires a false positive if that invariant ever
///   changes.
fn assembly_skipped_sbom_refusal(request: &ScanRequest, output: &Output) -> Option<String> {
    if !matches!(request.input_mode, InputMode::Native) {
        return None;
    }
    if !(request.package_only || request.no_assemble) {
        return None;
    }
    if !output.packages.is_empty() {
        return None;
    }

    let scanned_file_count = output
        .files
        .iter()
        .filter(|file| file.file_type == FileType::File)
        .count();
    if scanned_file_count == 0 {
        return None;
    }

    let requested_sbom_flags = requested_sbom_flags(&request.output_targets);
    if requested_sbom_flags.is_empty() {
        return None;
    }

    let cause_flag = if request.package_only {
        "--package-only"
    } else {
        "--no-assemble"
    };

    Some(format!(
        "Refusing to write a hollow SBOM for {}: {cause_flag} skips top-level package assembly for \
         this entire scan, so the {} scanned file(s) were never assembled into the top-level \
         packages/dependencies view that SBOM export reads from, even if some of those files carry \
         their own per-file package manifests. Rerun without {cause_flag} (e.g. with --package \
         instead) before requesting an SBOM format, or drop {} if a package-less export was intended.",
        requested_sbom_flags.join(", "),
        scanned_file_count,
        requested_sbom_flags.join(", "),
    ))
}

/// Returns a loud, non-blocking warning when a **native** `--paths-file` scan
/// requests an SBOM format.
///
/// `--paths-file` deliberately narrows collection to a caller-selected subset
/// of files under the scan root (see `docs/CLI_GUIDE.md`), so assembly only
/// ever sees the manifests, lockfiles, and workspace context that fell inside
/// that selection. Unlike [`assembly_skipped_sbom_refusal`], assembly still
/// runs and can produce a genuinely non-empty, well-formed SBOM — but that
/// SBOM can silently understate a monorepo's real inventory whenever the
/// selection omits sibling member manifests or a workspace root that
/// topology-aware assembly would otherwise use to complete the picture.
/// Provenant has no bounded, static way to tell "this selection happens to be
/// complete" apart from "this selection quietly dropped sibling manifests"
/// without rescanning the whole tree, which would defeat the point of
/// `--paths-file`. So this warns instead of refusing: the export is still
/// written, and callers who need a guaranteed-complete inventory should
/// rerun without `--paths-file`.
///
/// Returns `None` when `--paths-file` was not used, the request is not a
/// native scan, or no SBOM format was requested.
fn paths_file_sbom_completeness_warning(request: &ScanRequest) -> Option<String> {
    if !matches!(request.input_mode, InputMode::Native) || request.paths_files.is_empty() {
        return None;
    }

    let requested_sbom_flags = requested_sbom_flags(&request.output_targets);
    if requested_sbom_flags.is_empty() {
        return None;
    }

    Some(format!(
        "--paths-file restricted this scan to a caller-selected subset of files, so the {} export \
         only reflects packages whose manifests fell inside that selection. If this repository has \
         sibling member manifests, lockfiles, or a workspace root outside the selection, the \
         resulting inventory may understate the full repository. Rerun without --paths-file (or \
         widen the selection to include the full workspace) if you need a guaranteed-complete SBOM.",
        requested_sbom_flags.join(", "),
    ))
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
