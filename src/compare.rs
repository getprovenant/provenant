// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::compare_driver_shared::*;
use crate::compare_normalization::{
    canonical_section_value, classify_scalar_value, metric_values, normalize_compare_path,
    normalize_license_expression, scalar_field_value, structured_field_value,
};
use crate::version::BUILD_VERSION;

const COMPARISON_MODE: &str = "direct_json";

#[derive(Debug, Clone)]
pub(crate) struct CompareArtifactLayout {
    pub artifact_dir: PathBuf,
    pub raw_dir: PathBuf,
    pub scancode_json: PathBuf,
    pub provenant_json: PathBuf,
    pub comparison_dir: PathBuf,
    pub samples_dir: PathBuf,
    pub summary_json: PathBuf,
    pub summary_tsv: PathBuf,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct CompareCommandResult {
    pub comparison_status: String,
    pub artifact_dir: PathBuf,
    pub scancode_json: PathBuf,
    pub provenant_json: PathBuf,
    pub summary_json: PathBuf,
    pub summary_tsv: PathBuf,
    pub samples_dir: PathBuf,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct CompareManifest {
    mode: &'static str,
    tool_version: &'static str,
    created_at: String,
    inputs: CompareInputManifest,
    artifacts: CompareArtifactManifest,
}

#[derive(Debug, Serialize)]
struct CompareInputManifest {
    scancode_json_source: PathBuf,
    provenant_json_source: PathBuf,
}

#[derive(Debug, Serialize)]
struct CompareArtifactManifest {
    artifact_dir: PathBuf,
    raw_dir: PathBuf,
    scancode_json: PathBuf,
    provenant_json: PathBuf,
    comparison_dir: PathBuf,
    summary_json: PathBuf,
    summary_tsv: PathBuf,
    samples_dir: PathBuf,
}

pub(crate) fn compare_json_files(
    scancode_source: &Path,
    provenant_source: &Path,
    artifact_dir: Option<&Path>,
) -> Result<CompareCommandResult> {
    validate_json_input(scancode_source, "--scancode-json")?;
    validate_json_input(provenant_source, "--provenant-json")?;

    let artifact_dir = resolve_artifact_dir(artifact_dir)?;
    let layout = prepare_layout(&artifact_dir)?;
    materialize_file(scancode_source, &layout.scancode_json)?;
    materialize_file(provenant_source, &layout.provenant_json)?;

    let summary =
        write_comparison_artifacts(&layout.scancode_json, &layout.provenant_json, &layout, &[])?;
    write_manifest(scancode_source, provenant_source, &layout)?;

    let comparison_status = summary
        .get("comparison_status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    Ok(CompareCommandResult {
        comparison_status,
        artifact_dir: layout.artifact_dir.clone(),
        scancode_json: layout.scancode_json.clone(),
        provenant_json: layout.provenant_json.clone(),
        summary_json: layout.summary_json.clone(),
        summary_tsv: layout.summary_tsv.clone(),
        samples_dir: layout.samples_dir.clone(),
        manifest_path: layout.manifest_path.clone(),
    })
}

fn resolve_artifact_dir(artifact_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(artifact_dir) = artifact_dir {
        return Ok(artifact_dir.to_path_buf());
    }

    let cwd = std::env::current_dir().context("failed to determine current working directory")?;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    Ok(cwd.join(format!("provenant-compare-{timestamp}")))
}

pub(crate) fn write_comparison_artifacts(
    scancode_json_path: &Path,
    provenant_json_path: &Path,
    layout: &CompareArtifactLayout,
    scan_args: &[String],
) -> Result<Value> {
    let scancode: Value =
        serde_json::from_str(&fs::read_to_string(scancode_json_path).with_context(|| {
            format!(
                "failed to read ScanCode JSON {}",
                scancode_json_path.display()
            )
        })?)?;
    let provenant: Value =
        serde_json::from_str(&fs::read_to_string(provenant_json_path).with_context(|| {
            format!(
                "failed to read Provenant JSON {}",
                provenant_json_path.display()
            )
        })?)?;

    let scancode_files = files_by_path(&scancode);
    let provenant_files = files_by_path(&provenant);
    let scancode_resources = resources_by_path(&scancode);
    let provenant_resources = resources_by_path(&provenant);
    let scancode_paths: BTreeSet<String> = scancode_files.keys().cloned().collect();
    let provenant_paths: BTreeSet<String> = provenant_files.keys().cloned().collect();
    let scancode_resource_paths: BTreeSet<String> = scancode_resources.keys().cloned().collect();
    let provenant_resource_paths: BTreeSet<String> = provenant_resources.keys().cloned().collect();
    let common_paths: Vec<String> = scancode_paths
        .intersection(&provenant_paths)
        .cloned()
        .collect();
    let scancode_only_output_paths: Vec<String> = scancode_paths
        .difference(&provenant_paths)
        .cloned()
        .collect();
    let provenant_only_output_paths: Vec<String> = provenant_paths
        .difference(&scancode_paths)
        .cloned()
        .collect();
    let common_resource_paths: Vec<String> = scancode_resource_paths
        .intersection(&provenant_resource_paths)
        .cloned()
        .collect();
    let scancode_only_output_resource_paths: Vec<String> = scancode_resource_paths
        .difference(&provenant_resource_paths)
        .cloned()
        .collect();
    let provenant_only_output_resource_paths: Vec<String> = provenant_resource_paths
        .difference(&scancode_resource_paths)
        .cloned()
        .collect();
    let only_findings_active = compare_uses_only_findings(scan_args, &scancode, &provenant);
    let path_presence_note = only_findings_active.then_some(
        "This compare run used --only-findings. Path-presence buckets reflect final filtered outputs, not proven scan coverage gaps: a missing path may simply have had no findings after filtering.",
    );

    let metrics = [
        "license_detections",
        "license_clues",
        "license_policy",
        "package_data",
        "copyrights",
        "holders",
        "authors",
        "emails",
        "urls",
        "scan_errors",
    ];
    let info_metrics = [
        "mime_type",
        "file_type",
        "programming_language",
        "sha1",
        "md5",
        "sha256",
        "sha1_git",
        "is_binary",
        "is_text",
        "is_archive",
        "is_media",
        "is_source",
        "is_script",
        "files_count",
        "dirs_count",
        "size_count",
        "source_count",
    ];
    let classify_metrics = [
        "is_legal",
        "is_manifest",
        "is_readme",
        "is_top_level",
        "is_key_file",
        "is_community",
    ];
    let row2_value_metrics = ["facets", "tallies"];
    let row2_top_level_sections = [
        "summary",
        "tallies",
        "tallies_of_key_files",
        "tallies_by_facet",
    ];

    let info_mode = scan_args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--info" | "--mark-source"))
        || resources_contain_any_field(&scancode_resources, &info_metrics)
        || resources_contain_any_field(&provenant_resources, &info_metrics);
    let row2_mode = scan_args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--classify"
                | "--summary"
                | "--license-clarity-score"
                | "--tallies"
                | "--tallies-key-files"
                | "--tallies-with-details"
                | "--tallies-by-facet"
                | "--facet"
        )
    }) || resources_contain_any_field(&scancode_resources, &classify_metrics)
        || resources_contain_any_field(&provenant_resources, &classify_metrics)
        || resources_contain_any_field(&scancode_resources, &row2_value_metrics)
        || resources_contain_any_field(&provenant_resources, &row2_value_metrics)
        || value_contains_any_section(&scancode, &row2_top_level_sections)
        || value_contains_any_section(&provenant, &row2_top_level_sections);

    let mut lower_counts: BTreeMap<String, Vec<CountDeltaEntry>> = metrics
        .iter()
        .map(|metric| ((*metric).to_string(), Vec::new()))
        .collect();
    let mut higher_counts: BTreeMap<String, Vec<CountDeltaEntry>> = metrics
        .iter()
        .map(|metric| ((*metric).to_string(), Vec::new()))
        .collect();
    let mut value_differences: BTreeMap<String, Vec<ValueDifferenceEntry>> = metrics
        .iter()
        .map(|metric| ((*metric).to_string(), Vec::new()))
        .collect();
    let mut info_value_differences: BTreeMap<String, Vec<ScalarDifferenceEntry>> = info_metrics
        .iter()
        .map(|metric| ((*metric).to_string(), Vec::new()))
        .collect();
    let mut classify_value_differences: BTreeMap<String, Vec<ScalarDifferenceEntry>> =
        classify_metrics
            .iter()
            .map(|metric| ((*metric).to_string(), Vec::new()))
            .collect();
    let mut row2_value_differences: BTreeMap<String, Vec<ScalarDifferenceEntry>> =
        row2_value_metrics
            .iter()
            .map(|metric| ((*metric).to_string(), Vec::new()))
            .collect();
    let mut row2_top_level_differences = Vec::new();

    for path in &common_paths {
        let scancode_file = scancode_files.get(path).expect("common path exists");
        let provenant_file = provenant_files.get(path).expect("common path exists");
        for metric in metrics {
            let sc_values = metric_values(scancode_file, metric);
            let pr_values = metric_values(provenant_file, metric);
            let sc_counter = value_counter(&sc_values);
            let pr_counter = value_counter(&pr_values);
            let sc_signal_counter = metric_signal_counter(metric, &sc_values);
            let pr_signal_counter = metric_signal_counter(metric, &pr_values);
            let sc_count = counter_total(&sc_signal_counter);
            let pr_count = counter_total(&pr_signal_counter);
            if pr_count < sc_count {
                lower_counts
                    .get_mut(metric)
                    .expect("metric bucket exists")
                    .push(CountDeltaEntry {
                        path: path.clone(),
                        scancode: sc_count,
                        provenant: pr_count,
                        delta: pr_count as isize - sc_count as isize,
                        scancode_sample_values: sample_values(&sc_values),
                        provenant_sample_values: sample_values(&pr_values),
                    });
            } else if pr_count > sc_count {
                higher_counts
                    .get_mut(metric)
                    .expect("metric bucket exists")
                    .push(CountDeltaEntry {
                        path: path.clone(),
                        scancode: sc_count,
                        provenant: pr_count,
                        delta: pr_count as isize - sc_count as isize,
                        scancode_sample_values: sample_values(&sc_values),
                        provenant_sample_values: sample_values(&pr_values),
                    });
            }
            let signal_missing = subtract_counters(&sc_signal_counter, &pr_signal_counter);
            let signal_extra = subtract_counters(&pr_signal_counter, &sc_signal_counter);
            if !signal_missing.is_empty() || !signal_extra.is_empty() {
                let missing = filter_counter_to_signal_keys(
                    &subtract_counters(&sc_counter, &pr_counter),
                    &signal_missing,
                );
                let extra = filter_counter_to_signal_keys(
                    &subtract_counters(&pr_counter, &sc_counter),
                    &signal_extra,
                );
                value_differences
                    .get_mut(metric)
                    .expect("metric bucket exists")
                    .push(ValueDifferenceEntry {
                        path: path.clone(),
                        scancode: sc_count,
                        provenant: pr_count,
                        missing_in_provenant: counter_entries(&missing),
                        extra_in_provenant: counter_entries(&extra),
                    });
            }
        }
    }

    for path in &common_resource_paths {
        let scancode_resource = scancode_resources
            .get(path)
            .expect("common resource exists");
        let provenant_resource = provenant_resources
            .get(path)
            .expect("common resource exists");
        for metric in info_metrics {
            let scancode_value = scalar_field_value(scancode_resource, metric);
            let provenant_value = scalar_field_value(provenant_resource, metric);
            if scancode_value != provenant_value {
                info_value_differences
                    .get_mut(metric)
                    .expect("metric bucket exists")
                    .push(ScalarDifferenceEntry {
                        path: path.clone(),
                        scancode: scancode_value,
                        provenant: provenant_value,
                    });
            }
        }
        for metric in classify_metrics {
            let scancode_value = classify_scalar_value(scancode_resource, metric);
            let provenant_value = classify_scalar_value(provenant_resource, metric);
            if scancode_value != provenant_value {
                classify_value_differences
                    .get_mut(metric)
                    .expect("metric bucket exists")
                    .push(ScalarDifferenceEntry {
                        path: path.clone(),
                        scancode: scancode_value,
                        provenant: provenant_value,
                    });
            }
        }
        for metric in row2_value_metrics {
            let scancode_value = structured_field_value(scancode_resource, metric);
            let provenant_value = structured_field_value(provenant_resource, metric);
            if scancode_value != provenant_value {
                row2_value_differences
                    .get_mut(metric)
                    .expect("metric bucket exists")
                    .push(ScalarDifferenceEntry {
                        path: path.clone(),
                        scancode: scancode_value,
                        provenant: provenant_value,
                    });
            }
        }
    }

    for section in row2_top_level_sections {
        let scancode_value = canonical_section_value(&scancode, section);
        let provenant_value = canonical_section_value(&provenant, section);
        if scancode_value != provenant_value {
            row2_top_level_differences.push(TopLevelSectionDifferenceEntry {
                section: section.to_string(),
                scancode: scancode_value,
                provenant: provenant_value,
            });
        }
    }

    let sc_top = top_level_counts(&scancode);
    let pr_top = top_level_counts(&provenant);
    let license_deltas = top_level_license_deltas(&scancode, &provenant);
    let top_level_scancode_favored_differences =
        top_level_directional_differences(&sc_top, &pr_top);
    let top_level_provenant_favored_differences =
        top_level_directional_differences(&pr_top, &sc_top);
    let skipped_comparisons = skipped_comparisons(&sc_top, &pr_top);

    let mut file_metric_summary = Map::new();
    let mut rows = vec![];
    for key in [
        "files",
        "packages",
        "dependencies",
        "license_detections",
        "license_references",
        "license_rule_references",
    ] {
        rows.push(tsv_row(
            key,
            sc_top.count(key),
            pr_top.count(key),
            pr_top.count(key) - sc_top.count(key),
            &top_level_count_note(key, &sc_top, &pr_top),
        ));
    }
    rows.push(tsv_row(
        "common_file_paths",
        common_paths.len() as i64,
        common_paths.len() as i64,
        0,
        "paths present in both final outputs",
    ));
    rows.push(tsv_row(
        "scancode_only_output_file_paths",
        scancode_only_output_paths.len() as i64,
        0,
        -(scancode_only_output_paths.len() as i64),
        &output_only_path_note("ScanCode", "file", only_findings_active),
    ));
    rows.push(tsv_row(
        "provenant_only_output_file_paths",
        0,
        provenant_only_output_paths.len() as i64,
        provenant_only_output_paths.len() as i64,
        &output_only_path_note("Provenant", "file", only_findings_active),
    ));
    rows.push(tsv_row(
        "common_resource_paths",
        common_resource_paths.len() as i64,
        common_resource_paths.len() as i64,
        0,
        "resource paths present in both final outputs",
    ));
    rows.push(tsv_row(
        "scancode_only_output_resource_paths",
        scancode_only_output_resource_paths.len() as i64,
        0,
        -(scancode_only_output_resource_paths.len() as i64),
        &output_only_path_note("ScanCode", "resource", only_findings_active),
    ));
    rows.push(tsv_row(
        "provenant_only_output_resource_paths",
        0,
        provenant_only_output_resource_paths.len() as i64,
        provenant_only_output_resource_paths.len() as i64,
        &output_only_path_note("Provenant", "resource", only_findings_active),
    ));

    let mut scancode_favored_signal_count =
        scancode_only_output_paths.len() + top_level_scancode_favored_differences.len();
    let mut provenant_favored_signal_count =
        provenant_only_output_paths.len() + top_level_provenant_favored_differences.len();
    let mut non_directional_signal_count = 0;
    if info_mode {
        scancode_favored_signal_count += scancode_only_output_resource_paths.len();
        provenant_favored_signal_count += provenant_only_output_resource_paths.len();
    }
    if row2_mode {
        non_directional_signal_count += row2_top_level_differences.len();
    }

    for metric in metrics {
        let missing = value_differences[metric]
            .iter()
            .filter(|entry| !entry.missing_in_provenant.is_empty())
            .count();
        let extra = value_differences[metric]
            .iter()
            .filter(|entry| !entry.extra_in_provenant.is_empty())
            .count();
        file_metric_summary.insert(
            metric.to_string(),
            json!({
                "lower_counts": lower_counts[metric].len(),
                "higher_counts": higher_counts[metric].len(),
                "missing_in_provenant": missing,
                "extra_in_provenant": extra,
            }),
        );
        if metric == "scan_errors" {
            scancode_favored_signal_count += higher_counts[metric].len();
            scancode_favored_signal_count += extra;
            provenant_favored_signal_count += missing;
        } else {
            scancode_favored_signal_count += lower_counts[metric].len();
            provenant_favored_signal_count += higher_counts[metric].len();
            scancode_favored_signal_count += missing;
            provenant_favored_signal_count += extra;
        }
        rows.push(tsv_row(
            &format!("{metric}_lower_counts"),
            lower_counts[metric].len() as i64,
            0,
            -(lower_counts[metric].len() as i64),
            "common-path files where Provenant count is lower",
        ));
        rows.push(tsv_row(
            &format!("{metric}_higher_counts"),
            0,
            higher_counts[metric].len() as i64,
            higher_counts[metric].len() as i64,
            "common-path files where Provenant count is higher",
        ));
        rows.push(tsv_row(
            &format!("{metric}_missing_in_provenant"),
            missing as i64,
            0,
            -(missing as i64),
            "paths where normalized values exist only in ScanCode output",
        ));
        rows.push(tsv_row(
            &format!("{metric}_extra_in_provenant"),
            0,
            extra as i64,
            extra as i64,
            "paths where normalized values exist only in Provenant output",
        ));
    }

    let mut info_metric_summary = Map::new();
    for metric in info_metrics {
        let differences = info_value_differences[metric].len();
        info_metric_summary.insert(
            metric.to_string(),
            json!({
                "value_differences": differences,
            }),
        );
        if info_mode {
            non_directional_signal_count += differences;
        }
        rows.push(tsv_row(
            &format!("info_{metric}_value_differences"),
            differences as i64,
            differences as i64,
            0,
            "common-path resources where info values differ",
        ));
    }

    let mut classify_metric_summary = Map::new();
    for metric in classify_metrics {
        let differences = classify_value_differences[metric].len();
        classify_metric_summary.insert(
            metric.to_string(),
            json!({
                "value_differences": differences,
            }),
        );
        if row2_mode {
            non_directional_signal_count += differences;
        }
        rows.push(tsv_row(
            &format!("classify_{metric}_value_differences"),
            differences as i64,
            differences as i64,
            0,
            "common-path resources where classify values differ",
        ));
    }

    let mut row2_metric_summary = Map::new();
    for metric in row2_value_metrics {
        let differences = row2_value_differences[metric].len();
        row2_metric_summary.insert(
            metric.to_string(),
            json!({
                "value_differences": differences,
            }),
        );
        if row2_mode {
            non_directional_signal_count += differences;
        }
        rows.push(tsv_row(
            &format!("row2_{metric}_value_differences"),
            differences as i64,
            differences as i64,
            0,
            "common-path resources where row-2 workflow values differ",
        ));
    }

    rows.push(tsv_row(
        "row2_top_level_section_differences",
        row2_top_level_differences.len() as i64,
        row2_top_level_differences.len() as i64,
        0,
        "top-level row-2 workflow sections with normalized JSON differences",
    ));

    let top_level_package_skip_reason = skipped_comparisons.get("packages").cloned();
    let top_level_package_value_differences = top_level_package_differences(&scancode, &provenant);
    let top_level_package_missing = top_level_package_value_differences
        .iter()
        .filter(|entry| !entry.missing_in_provenant.is_empty())
        .map(|entry| entry.missing_in_provenant.len())
        .sum::<usize>();
    let top_level_package_extra = top_level_package_value_differences
        .iter()
        .filter(|entry| !entry.extra_in_provenant.is_empty())
        .map(|entry| entry.extra_in_provenant.len())
        .sum::<usize>();
    let top_level_dependency_skip_reason = skipped_comparisons.get("dependencies").cloned();
    let top_level_dependency_value_differences =
        top_level_dependency_differences(&scancode, &provenant);
    let top_level_dependency_missing = top_level_dependency_value_differences
        .iter()
        .filter(|entry| !entry.missing_in_provenant.is_empty())
        .map(|entry| entry.missing_in_provenant.len())
        .sum::<usize>();
    let top_level_dependency_extra = top_level_dependency_value_differences
        .iter()
        .filter(|entry| !entry.extra_in_provenant.is_empty())
        .map(|entry| entry.extra_in_provenant.len())
        .sum::<usize>();
    let package_field_content_value_differences =
        package_field_content_differences(&scancode, &provenant);
    // Value-vs-value mismatches (both sides non-null but different) are tracked
    // in their own bucket rather than being double-counted into both directional
    // totals. This keeps the summary self-consistent: `missing + extra +
    // value_vs_value_mismatch == sum(by_field.values()) == one entry per delta`,
    // so a consumer can cross-check the totals against `by_field`.
    let package_field_content_tally =
        tally_package_field_content_differences(&package_field_content_value_differences);
    let package_field_content_missing = package_field_content_tally.missing_in_provenant;
    let package_field_content_extra = package_field_content_tally.extra_in_provenant;
    let package_field_content_value_vs_value_mismatch =
        package_field_content_tally.value_vs_value_mismatch;
    let package_field_content_by_field = package_field_content_tally.by_field;
    let raw_dependency_value_differences = raw_dependency_differences(&scancode, &provenant);
    let raw_dependency_missing = raw_dependency_value_differences
        .iter()
        .filter(|entry| !entry.missing_in_provenant.is_empty())
        .map(|entry| entry.missing_in_provenant.len())
        .sum::<usize>();
    let raw_dependency_extra = raw_dependency_value_differences
        .iter()
        .filter(|entry| !entry.extra_in_provenant.is_empty())
        .map(|entry| entry.extra_in_provenant.len())
        .sum::<usize>();
    let top_level_package_summary = json!({
        "missing_in_provenant": top_level_package_missing,
        "extra_in_provenant": top_level_package_extra,
        "comparison_skipped": top_level_package_skip_reason.is_some(),
        "skip_reason": top_level_package_skip_reason,
    });
    let top_level_dependency_summary = json!({
        "missing_in_provenant": top_level_dependency_missing,
        "extra_in_provenant": top_level_dependency_extra,
        "comparison_skipped": top_level_dependency_skip_reason.is_some(),
        "skip_reason": top_level_dependency_skip_reason,
    });
    let raw_dependency_summary = json!({
        "missing_in_provenant": raw_dependency_missing,
        "extra_in_provenant": raw_dependency_extra,
    });
    file_metric_summary.insert(
        "raw_package_dependencies".to_string(),
        json!({
            "missing_in_provenant": raw_dependency_missing,
            "extra_in_provenant": raw_dependency_extra,
        }),
    );
    file_metric_summary.insert(
        "package_field_content".to_string(),
        json!({
            "missing_in_provenant": package_field_content_missing,
            "extra_in_provenant": package_field_content_extra,
            "value_vs_value_mismatch": package_field_content_value_vs_value_mismatch,
            "by_field": package_field_content_by_field,
        }),
    );
    let package_field_content_summary = json!({
        "missing_in_provenant": package_field_content_missing,
        "extra_in_provenant": package_field_content_extra,
        "value_vs_value_mismatch": package_field_content_value_vs_value_mismatch,
        "by_field": package_field_content_by_field,
        "fields_compared": PACKAGE_CONTENT_FIELDS,
    });
    scancode_favored_signal_count += package_field_content_missing;
    provenant_favored_signal_count += package_field_content_extra;
    // A value-vs-value mismatch favors neither side, so it is a non-directional
    // review signal rather than a directional one.
    non_directional_signal_count += package_field_content_value_vs_value_mismatch;
    if top_level_package_skip_reason.is_none() {
        scancode_favored_signal_count += top_level_package_missing;
        provenant_favored_signal_count += top_level_package_extra;
    }
    if top_level_dependency_skip_reason.is_none() {
        scancode_favored_signal_count += top_level_dependency_missing;
        provenant_favored_signal_count += top_level_dependency_extra;
    }
    scancode_favored_signal_count += raw_dependency_missing;
    provenant_favored_signal_count += raw_dependency_extra;
    non_directional_signal_count += license_deltas.len();
    rows.push(tsv_row(
        "top_level_packages_missing_in_provenant",
        top_level_package_missing as i64,
        0,
        -(top_level_package_missing as i64),
        top_level_package_skip_reason
            .as_deref()
            .unwrap_or("top-level package identities present only in ScanCode output"),
    ));
    rows.push(tsv_row(
        "top_level_packages_extra_in_provenant",
        0,
        top_level_package_extra as i64,
        top_level_package_extra as i64,
        top_level_package_skip_reason
            .as_deref()
            .unwrap_or("top-level package identities present only in Provenant output"),
    ));
    rows.push(tsv_row(
        "top_level_dependencies_missing_in_provenant",
        top_level_dependency_missing as i64,
        0,
        -(top_level_dependency_missing as i64),
        top_level_dependency_skip_reason
            .as_deref()
            .unwrap_or("top-level dependency identities present only in ScanCode output"),
    ));
    rows.push(tsv_row(
        "top_level_dependencies_extra_in_provenant",
        0,
        top_level_dependency_extra as i64,
        top_level_dependency_extra as i64,
        top_level_dependency_skip_reason
            .as_deref()
            .unwrap_or("top-level dependency identities present only in Provenant output"),
    ));
    rows.push(tsv_row(
        "raw_package_dependencies_missing_in_provenant",
        raw_dependency_missing as i64,
        0,
        -(raw_dependency_missing as i64),
        "raw dependency identities present only in ScanCode file-level package_data output",
    ));
    rows.push(tsv_row(
        "raw_package_dependencies_extra_in_provenant",
        0,
        raw_dependency_extra as i64,
        raw_dependency_extra as i64,
        "raw dependency identities present only in Provenant file-level package_data output",
    ));
    rows.push(tsv_row(
        "package_field_content_missing_in_provenant",
        package_field_content_missing as i64,
        0,
        -(package_field_content_missing as i64),
        "identity-matched packages where declared-license/holder content is present only in ScanCode output",
    ));
    rows.push(tsv_row(
        "package_field_content_extra_in_provenant",
        0,
        package_field_content_extra as i64,
        package_field_content_extra as i64,
        "identity-matched packages where declared-license/holder content is present only in Provenant output",
    ));
    rows.push(tsv_row(
        "package_field_content_value_vs_value_mismatch",
        package_field_content_value_vs_value_mismatch as i64,
        package_field_content_value_vs_value_mismatch as i64,
        0,
        "identity-matched packages where both outputs carry declared-license/holder content but the values differ",
    ));
    rows.push(tsv_row(
        "top_level_license_expression_deltas",
        license_deltas.len() as i64,
        license_deltas.len() as i64,
        0,
        "expressions with different top-level detection counts",
    ));

    let comparison_status = if scancode_favored_signal_count > 0
        || provenant_favored_signal_count > 0
        || non_directional_signal_count > 0
    {
        "review_required"
    } else {
        "no_detected_differences"
    };

    let sample_paths = [
        (
            "scancode_only_output_paths",
            layout.samples_dir.join("scancode_only_output_paths.json"),
        ),
        (
            "provenant_only_output_paths",
            layout.samples_dir.join("provenant_only_output_paths.json"),
        ),
        (
            "file_metric_lower_counts",
            layout.samples_dir.join("file_metric_lower_counts.json"),
        ),
        (
            "file_metric_higher_counts",
            layout.samples_dir.join("file_metric_higher_counts.json"),
        ),
        (
            "file_metric_value_differences",
            layout
                .samples_dir
                .join("file_metric_value_differences.json"),
        ),
        (
            "top_level_license_expression_deltas",
            layout
                .samples_dir
                .join("top_level_license_expression_deltas.json"),
        ),
        (
            "top_level_package_value_differences",
            layout
                .samples_dir
                .join("top_level_package_value_differences.json"),
        ),
        (
            "top_level_dependency_value_differences",
            layout
                .samples_dir
                .join("top_level_dependency_value_differences.json"),
        ),
        (
            "raw_dependency_value_differences",
            layout
                .samples_dir
                .join("raw_dependency_value_differences.json"),
        ),
        (
            "info_value_differences",
            layout.samples_dir.join("info_value_differences.json"),
        ),
        (
            "classify_value_differences",
            layout.samples_dir.join("classify_value_differences.json"),
        ),
        (
            "row2_value_differences",
            layout.samples_dir.join("row2_value_differences.json"),
        ),
        (
            "row2_top_level_differences",
            layout.samples_dir.join("row2_top_level_differences.json"),
        ),
        (
            "package_field_content_value_differences",
            layout
                .samples_dir
                .join("package_field_content_value_differences.json"),
        ),
    ];

    write_pretty_json(&sample_paths[0].1, &scancode_only_output_paths)?;
    write_pretty_json(&sample_paths[1].1, &provenant_only_output_paths)?;
    write_pretty_json(&sample_paths[2].1, &lower_counts)?;
    write_pretty_json(&sample_paths[3].1, &higher_counts)?;
    write_pretty_json(&sample_paths[4].1, &value_differences)?;
    write_pretty_json(&sample_paths[5].1, &license_deltas)?;
    write_pretty_json(&sample_paths[6].1, &top_level_package_value_differences)?;
    write_pretty_json(&sample_paths[7].1, &top_level_dependency_value_differences)?;
    write_pretty_json(&sample_paths[8].1, &raw_dependency_value_differences)?;
    write_pretty_json(&sample_paths[9].1, &info_value_differences)?;
    write_pretty_json(&sample_paths[10].1, &classify_value_differences)?;
    write_pretty_json(&sample_paths[11].1, &row2_value_differences)?;
    write_pretty_json(&sample_paths[12].1, &row2_top_level_differences)?;
    write_pretty_json(
        &sample_paths[13].1,
        &package_field_content_value_differences,
    )?;

    let summary = json!({
        "comparison_status": comparison_status,
        "comparison_signal_summary": {
            "scancode_favored": scancode_favored_signal_count,
            "provenant_favored": provenant_favored_signal_count,
            "non_directional": non_directional_signal_count,
        },
        "top_level_counts": {
            "scancode": sc_top.counts_json(),
            "provenant": pr_top.counts_json(),
            "delta": {
                "files": pr_top.count("files") - sc_top.count("files"),
                "packages": pr_top.count("packages") - sc_top.count("packages"),
                "dependencies": pr_top.count("dependencies") - sc_top.count("dependencies"),
                "license_detections": pr_top.count("license_detections") - sc_top.count("license_detections"),
                "license_references": pr_top.count("license_references") - sc_top.count("license_references"),
                "license_rule_references": pr_top.count("license_rule_references") - sc_top.count("license_rule_references"),
            },
            "sources": {
                "scancode": sc_top.sources_json(),
                "provenant": pr_top.sources_json(),
            },
        },
        "skipped_comparisons": skipped_comparisons,
        "top_level_package_summary": top_level_package_summary,
        "top_level_dependency_summary": top_level_dependency_summary,
        "raw_dependency_summary": raw_dependency_summary,
        "package_field_content_summary": package_field_content_summary,
        "comparison_context": {
            "only_findings_active": only_findings_active,
            "path_presence_semantics": "final_output_membership",
            "path_presence_note": path_presence_note,
        },
        "file_path_comparison": {
            "common_paths": common_paths.len(),
            "scancode_only_output_paths": scancode_only_output_paths.len(),
            "provenant_only_output_paths": provenant_only_output_paths.len(),
        },
        "resource_path_comparison": {
            "common_paths": common_resource_paths.len(),
            "scancode_only_output_paths": scancode_only_output_resource_paths.len(),
            "provenant_only_output_paths": provenant_only_output_resource_paths.len(),
        },
        "file_metric_summary": file_metric_summary,
        "info_metric_summary": info_metric_summary,
        "classify_metric_summary": classify_metric_summary,
        "row2_metric_summary": row2_metric_summary,
        "row2_top_level_section_difference_count": row2_top_level_differences.len(),
        "top_level_scancode_favored_differences": top_level_scancode_favored_differences,
        "top_level_provenant_favored_differences": top_level_provenant_favored_differences,
        "top_level_license_expression_delta_count": license_deltas.len(),
        "sample_artifacts": BTreeMap::from(sample_paths.map(|(name, path)| (name.to_string(), path.display().to_string()))),
    });

    write_pretty_json(&layout.summary_json, &summary)?;
    write_tsv(
        &layout.summary_tsv,
        &["metric", "scancode", "provenant", "delta", "notes"],
        &rows,
    )?;

    Ok(summary)
}

fn prepare_layout(artifact_dir: &Path) -> Result<CompareArtifactLayout> {
    if artifact_dir.exists() && !artifact_dir.is_dir() {
        bail!(
            "compare artifact path is not a directory: {}",
            artifact_dir.display()
        );
    }

    fs::create_dir_all(artifact_dir).with_context(|| {
        format!(
            "failed to create compare artifact directory {}",
            artifact_dir.display()
        )
    })?;

    let raw_dir = artifact_dir.join("raw");
    let comparison_dir = artifact_dir.join("comparison");
    let samples_dir = comparison_dir.join("samples");
    fs::create_dir_all(&raw_dir)?;
    fs::create_dir_all(&samples_dir)?;

    Ok(CompareArtifactLayout {
        artifact_dir: artifact_dir.to_path_buf(),
        raw_dir: raw_dir.clone(),
        scancode_json: raw_dir.join("scancode.json"),
        provenant_json: raw_dir.join("provenant.json"),
        comparison_dir: comparison_dir.clone(),
        samples_dir: samples_dir.clone(),
        summary_json: comparison_dir.join("summary.json"),
        summary_tsv: comparison_dir.join("summary.tsv"),
        manifest_path: artifact_dir.join("run-manifest.json"),
    })
}

fn validate_json_input(path: &Path, flag_name: &str) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to read {flag_name} {}", path.display()))?;
    if !metadata.is_file() {
        bail!(
            "{flag_name} must point to a regular file: {}",
            path.display()
        );
    }
    Ok(())
}

fn write_manifest(
    scancode_source: &Path,
    provenant_source: &Path,
    layout: &CompareArtifactLayout,
) -> Result<()> {
    let manifest = CompareManifest {
        mode: COMPARISON_MODE,
        tool_version: BUILD_VERSION,
        created_at: chrono::Utc::now().to_rfc3339(),
        inputs: CompareInputManifest {
            scancode_json_source: scancode_source.to_path_buf(),
            provenant_json_source: provenant_source.to_path_buf(),
        },
        artifacts: CompareArtifactManifest {
            artifact_dir: layout.artifact_dir.clone(),
            raw_dir: layout.raw_dir.clone(),
            scancode_json: layout.scancode_json.clone(),
            provenant_json: layout.provenant_json.clone(),
            comparison_dir: layout.comparison_dir.clone(),
            summary_json: layout.summary_json.clone(),
            summary_tsv: layout.summary_tsv.clone(),
            samples_dir: layout.samples_dir.clone(),
        },
    };
    write_pretty_json(&layout.manifest_path, &manifest)
}

fn materialize_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent directory {}", parent.display()))?;
    }
    if dst.exists() {
        fs::remove_file(dst)
            .with_context(|| format!("failed to remove existing file {}", dst.display()))?;
    }
    match fs::hard_link(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(src, dst).with_context(|| {
                format!(
                    "failed to copy compare artifact {} -> {}",
                    src.display(),
                    dst.display()
                )
            })?;
            Ok(())
        }
    }
}

fn write_pretty_json<T: ?Sized + Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn write_tsv(path: &Path, headers: &[&str], rows: &[Vec<String>]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut content = String::new();
    content.push_str(&headers.join("\t"));
    content.push('\n');
    for row in rows {
        content.push_str(&row.join("\t"));
        content.push('\n');
    }
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn top_level_counts(value: &Value) -> TopLevelCounts {
    let package_count = array_len(value, "packages");
    let fallback_package_count = file_package_data_count(value);
    let dependency_count = array_len(value, "dependencies");
    let fallback_dependency_count = file_package_data_dependency_count(value);

    let packages_source = if package_count == 0 && fallback_package_count > 0 {
        PACKAGE_DATA_COUNT_SOURCE
    } else {
        PACKAGES_COUNT_SOURCE
    };
    let dependencies_source = if dependency_count == 0 && fallback_dependency_count > 0 {
        PACKAGE_DATA_DEPENDENCIES_COUNT_SOURCE
    } else {
        DEPENDENCIES_COUNT_SOURCE
    };

    TopLevelCounts {
        counts: HashMap::from([
            ("files", file_entry_count(value) as i64),
            ("packages", package_count as i64),
            ("dependencies", dependency_count as i64),
            (
                "license_detections",
                array_len(value, "license_detections") as i64,
            ),
            (
                "license_references",
                array_len(value, "license_references") as i64,
            ),
            (
                "license_rule_references",
                array_len(value, "license_rule_references") as i64,
            ),
        ]),
        sources: HashMap::from([
            ("files", FILES_COUNT_SOURCE),
            ("packages", packages_source),
            ("dependencies", dependencies_source),
            ("license_detections", LICENSE_DETECTIONS_COUNT_SOURCE),
            ("license_references", LICENSE_REFERENCES_COUNT_SOURCE),
            (
                "license_rule_references",
                LICENSE_RULE_REFERENCES_COUNT_SOURCE,
            ),
        ]),
    }
}

fn top_level_license_deltas(scancode: &Value, provenant: &Value) -> Vec<Value> {
    let mut counter = BTreeMap::new();
    for (label, value) in [("scancode", scancode), ("provenant", provenant)] {
        for item in value
            .get("license_detections")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let key = item
                .get("license_expression_spdx")
                .or_else(|| item.get("license_expression"))
                .or_else(|| item.get("identifier"))
                .and_then(Value::as_str)
                .map(normalize_license_expression)
                .unwrap_or_else(|| "<unknown>".to_string());
            let count = top_level_license_detection_count(item) as i64;
            let entry = counter.entry(key).or_insert((0_i64, 0_i64));
            if label == "scancode" {
                entry.0 += count;
            } else {
                entry.1 += count;
            }
        }
    }
    counter
        .into_iter()
        .filter_map(|(key, (sc, pr))| {
            (sc != pr).then_some(json!({
                "license_expression": key,
                "scancode": sc,
                "provenant": pr,
                "delta": pr - sc
            }))
        })
        .collect()
}

fn top_level_license_detection_count(item: &Value) -> usize {
    let Some(reference_matches) = item.get("reference_matches").and_then(Value::as_array) else {
        return item
            .get("detection_count")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(1);
    };

    let identities: BTreeSet<String> = reference_matches
        .iter()
        .map(|match_item| {
            let expr = match_item
                .get("license_expression_spdx")
                .or_else(|| match_item.get("license_expression"))
                .and_then(Value::as_str)
                .map(normalize_license_expression)
                .unwrap_or_else(|| "<unknown>".to_string());
            let path = match_item
                .get("from_file")
                .and_then(Value::as_str)
                .map(normalize_compare_path)
                .unwrap_or_else(|| "<unknown>".to_string());
            format!("{expr}@{path}")
        })
        .collect();

    if identities.is_empty() {
        item.get("detection_count")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(1)
    } else {
        identities.len()
    }
}

fn top_level_package_differences(scancode: &Value, provenant: &Value) -> Vec<ValueDifferenceEntry> {
    let sc_top = top_level_counts(scancode);
    let pr_top = top_level_counts(provenant);
    if !count_delta_is_directly_comparable("packages", &sc_top, &pr_top) {
        return Vec::new();
    }

    let sc_identities = top_level_package_identities(scancode);
    let pr_identities = top_level_package_identities(provenant);
    let missing = difference_entries(&sc_identities, &pr_identities);
    let extra = difference_entries(&pr_identities, &sc_identities);
    if missing.is_empty() && extra.is_empty() {
        return Vec::new();
    }

    vec![ValueDifferenceEntry {
        path: "<top-level>".to_string(),
        scancode: sc_identities.len(),
        provenant: pr_identities.len(),
        missing_in_provenant: missing,
        extra_in_provenant: extra,
    }]
}

fn top_level_dependency_differences(
    scancode: &Value,
    provenant: &Value,
) -> Vec<ValueDifferenceEntry> {
    let sc_top = top_level_counts(scancode);
    let pr_top = top_level_counts(provenant);
    if !count_delta_is_directly_comparable("dependencies", &sc_top, &pr_top) {
        return Vec::new();
    }

    let sc_by_path = top_level_dependency_identities_by_path(scancode);
    let pr_by_path = top_level_dependency_identities_by_path(provenant);
    let mut paths = BTreeSet::new();
    paths.extend(sc_by_path.keys().cloned());
    paths.extend(pr_by_path.keys().cloned());
    let mut differences = Vec::new();
    for path in paths {
        let sc_identities = sc_by_path.get(&path).cloned().unwrap_or_default();
        let pr_identities = pr_by_path.get(&path).cloned().unwrap_or_default();
        let missing = difference_entries(&sc_identities, &pr_identities);
        let extra = difference_entries(&pr_identities, &sc_identities);
        if !missing.is_empty() || !extra.is_empty() {
            differences.push(ValueDifferenceEntry {
                path,
                scancode: sc_identities.len(),
                provenant: pr_identities.len(),
                missing_in_provenant: missing,
                extra_in_provenant: extra,
            });
        }
    }
    differences
}

fn raw_dependency_differences(scancode: &Value, provenant: &Value) -> Vec<ValueDifferenceEntry> {
    let sc_by_path = raw_dependency_identities_by_path(scancode);
    let pr_by_path = raw_dependency_identities_by_path(provenant);
    let mut paths = BTreeSet::new();
    paths.extend(sc_by_path.keys().cloned());
    paths.extend(pr_by_path.keys().cloned());
    let mut differences = Vec::new();
    for path in paths {
        let sc_identities = sc_by_path.get(&path).cloned().unwrap_or_default();
        let pr_identities = pr_by_path.get(&path).cloned().unwrap_or_default();
        let missing = difference_entries(&sc_identities, &pr_identities);
        let extra = difference_entries(&pr_identities, &sc_identities);
        if !missing.is_empty() || !extra.is_empty() {
            differences.push(ValueDifferenceEntry {
                path,
                scancode: sc_identities.len(),
                provenant: pr_identities.len(),
                missing_in_provenant: missing,
                extra_in_provenant: extra,
            });
        }
    }
    differences
}
