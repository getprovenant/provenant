// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::utils::spdx::combine_license_expressions;
use crate::version::BUILD_VERSION;

const COMPARISON_MODE: &str = "direct_json";
const FILES_COUNT_SOURCE: &str = "files[]";
const PACKAGES_COUNT_SOURCE: &str = "packages[]";
const PACKAGE_DATA_COUNT_SOURCE: &str = "packages[] empty; files[].package_data present";
const DEPENDENCIES_COUNT_SOURCE: &str = "dependencies[]";
const PACKAGE_DATA_DEPENDENCIES_COUNT_SOURCE: &str =
    "dependencies[] empty; files[].package_data[].dependencies present";
const LICENSE_DETECTIONS_COUNT_SOURCE: &str = "license_detections[]";
const LICENSE_REFERENCES_COUNT_SOURCE: &str = "license_references[]";
const LICENSE_RULE_REFERENCES_COUNT_SOURCE: &str = "license_rule_references[]";

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

#[derive(Debug, Serialize)]
struct ValueCountEntry {
    value: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct CountDeltaEntry {
    path: String,
    scancode: usize,
    provenant: usize,
    delta: isize,
    scancode_sample_values: Vec<String>,
    provenant_sample_values: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ValueDifferenceEntry {
    path: String,
    scancode: usize,
    provenant: usize,
    missing_in_provenant: Vec<ValueCountEntry>,
    extra_in_provenant: Vec<ValueCountEntry>,
}

#[derive(Debug, Serialize)]
struct ScalarDifferenceEntry {
    path: String,
    scancode: Option<String>,
    provenant: Option<String>,
}

#[derive(Debug, Serialize)]
struct TopLevelSectionDifferenceEntry {
    section: String,
    scancode: Option<Value>,
    provenant: Option<Value>,
}

#[derive(Debug, Clone)]
struct TopLevelCounts {
    counts: HashMap<&'static str, i64>,
    sources: HashMap<&'static str, &'static str>,
}

impl TopLevelCounts {
    fn count(&self, key: &str) -> i64 {
        *self.counts.get(key).expect("top-level count exists")
    }

    fn source(&self, key: &str) -> &'static str {
        self.sources
            .get(key)
            .copied()
            .expect("top-level count source exists")
    }

    fn counts_json(&self) -> BTreeMap<String, i64> {
        self.counts
            .iter()
            .map(|(key, value)| ((*key).to_string(), *value))
            .collect()
    }

    fn sources_json(&self) -> BTreeMap<String, String> {
        self.sources
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }
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
            let sc_count = metric_count(scancode_file, metric);
            let pr_count = metric_count(provenant_file, metric);
            let sc_values = metric_values(scancode_file, metric);
            let pr_values = metric_values(provenant_file, metric);
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
            let sc_counter = value_counter(&sc_values);
            let pr_counter = value_counter(&pr_values);
            let missing = subtract_counters(&sc_counter, &pr_counter);
            let extra = subtract_counters(&pr_counter, &sc_counter);
            if !missing.is_empty() || !extra.is_empty() {
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

fn resources_contain_any_field(resources: &BTreeMap<String, Value>, fields: &[&str]) -> bool {
    resources.values().any(|entry| {
        fields.iter().any(|field| {
            entry.get(field).is_some_and(|value| match value {
                Value::Null => false,
                Value::Array(values) => !values.is_empty(),
                Value::String(text) => !text.trim().is_empty(),
                _ => true,
            })
        })
    })
}

fn value_contains_any_section(value: &Value, sections: &[&str]) -> bool {
    sections
        .iter()
        .any(|section| value.get(section).is_some_and(|entry| !entry.is_null()))
}

fn files_by_path(value: &Value) -> BTreeMap<String, Value> {
    value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            if entry.get("type").and_then(Value::as_str) != Some("file") {
                return None;
            }
            entry
                .get("path")
                .and_then(Value::as_str)
                .map(|path| (normalize_compare_path(path), entry.clone()))
        })
        .collect()
}

fn resources_by_path(value: &Value) -> BTreeMap<String, Value> {
    value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            entry.get("path").and_then(Value::as_str).and_then(|path| {
                let normalized = normalize_compare_path(path);
                (normalized != "<root>").then_some((normalized, entry.clone()))
            })
        })
        .collect()
}

fn metric_count(entry: &Value, key: &str) -> usize {
    entry
        .get(key)
        .and_then(Value::as_array)
        .map(|values| values.len())
        .unwrap_or(0)
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_compare_copyright_value(value: &str) -> String {
    let normalized = normalize_text(value);
    if normalized.is_empty() {
        return normalized;
    }

    let out = strip_compare_all_rights_reserved(&normalized);
    let mut out = strip_compare_confidentiality_suffix(&out)
        .trim()
        .to_string();
    while out.ends_with(['.', ',', ';', ':']) {
        out.pop();
        out = out.trim_end().to_string();
    }
    out
}

fn strip_compare_all_rights_reserved(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let marker = "all rights reserved";
    if let Some(idx) = lower.rfind(marker) {
        let tail = lower[idx + marker.len()..].trim();
        if tail.is_empty() || tail.chars().all(|ch| matches!(ch, '.' | ',' | ';' | ':')) {
            return value[..idx]
                .trim_end_matches([' ', '.', ',', ';', ':'])
                .trim_end()
                .to_string();
        }
    }
    value.to_string()
}

fn strip_compare_confidentiality_suffix(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    for marker in [
        "confidential and proprietary",
        "confidential proprietary",
        "confidential information",
    ] {
        if let Some(idx) = lower.rfind(marker) {
            let tail = lower[idx + marker.len()..].trim();
            let boundary_ok = idx == 0
                || lower[..idx]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_whitespace() || matches!(ch, '.' | ',' | ';' | ':'));
            if boundary_ok
                && (tail.is_empty() || tail.chars().all(|ch| matches!(ch, '.' | ',' | ';' | ':')))
            {
                return value[..idx]
                    .trim_end_matches([' ', '.', ',', ';', ':'])
                    .trim_end()
                    .to_string();
            }
        }
    }
    value.to_string()
}

fn metric_values(entry: &Value, metric: &str) -> Vec<String> {
    let Some(values) = entry.get(metric).and_then(Value::as_array) else {
        return Vec::new();
    };
    values
        .iter()
        .filter_map(|item| {
            let value = match metric {
                "license_detections" => item
                    .get("license_expression_spdx")
                    .or_else(|| item.get("license_expression"))
                    .or_else(|| item.get("identifier"))
                    .and_then(Value::as_str)
                    .map(normalize_license_expression),
                "license_clues" | "license_policy" => Some(canonical_value_string(item)),
                "package_data" => package_metric_identity(item),
                "copyrights" => item
                    .get("copyright")
                    .and_then(Value::as_str)
                    .map(normalize_compare_copyright_value),
                "holders" => item
                    .get("holder")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                "authors" => item
                    .get("author")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                "emails" => item
                    .get("email")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                "urls" => item.get("url").and_then(Value::as_str).map(str::to_string),
                "scan_errors" => scan_error_identity(item).map(str::to_string),
                _ => None,
            }?;
            let normalized = normalize_text(&value);
            (!normalized.is_empty()).then_some(normalized)
        })
        .collect()
}

fn package_identity(item: &Value) -> Option<&str> {
    item.get("purl")
        .and_then(Value::as_str)
        .or_else(|| item.get("package_url").and_then(Value::as_str))
}

fn package_metric_identity(item: &Value) -> Option<String> {
    package_identity(item)
        .map(str::to_string)
        .or_else(|| package_fallback_identity(item))
}

fn package_fallback_identity(item: &Value) -> Option<String> {
    let mut parts = Vec::new();
    for key in [
        "type",
        "package_type",
        "scope",
        "namespace",
        "name",
        "version",
        "datasource_id",
    ] {
        if let Some(value) = item.get(key).and_then(Value::as_str) {
            let normalized = normalize_text(value);
            if !normalized.is_empty() {
                parts.push(format!("{key}={normalized}"));
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("|"))
    }
}

fn scan_error_identity(item: &Value) -> Option<&str> {
    item.as_str()
        .or_else(|| item.get("error").and_then(Value::as_str))
        .or_else(|| item.get("message").and_then(Value::as_str))
        .or_else(|| item.get("scan_error").and_then(Value::as_str))
        .or_else(|| item.get("details").and_then(Value::as_str))
}

fn normalize_compare_path(path: &str) -> String {
    let trimmed = path.trim();
    if matches!(trimmed, "" | "." | "input" | "/input") {
        "<root>".to_string()
    } else {
        trimmed
            .trim_start_matches("./")
            .trim_start_matches("/input/")
            .trim_start_matches("input/")
            .to_string()
    }
}

fn normalize_license_expression(value: &str) -> String {
    let normalized = normalize_text(value);
    if normalized.is_empty() {
        return normalized;
    }

    let stripped = strip_trivial_outer_parens(&normalized);
    let canonical =
        combine_license_expressions(std::iter::once(stripped.clone())).unwrap_or(stripped);
    strip_trivial_outer_parens(&canonical)
}

fn strip_trivial_outer_parens(value: &str) -> String {
    let mut current = value.trim();
    while has_trivial_outer_parens(current) {
        current = current[1..current.len() - 1].trim();
    }
    current.to_string()
}

fn has_trivial_outer_parens(value: &str) -> bool {
    if !(value.starts_with('(') && value.ends_with(')')) {
        return false;
    }

    let mut depth = 0usize;
    for (index, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
                if depth == 0 && index != value.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }

    depth == 0
}

fn scalar_field_value(entry: &Value, key: &str) -> Option<String> {
    let value = entry.get(key)?;
    let normalized = match value {
        Value::Null => return None,
        Value::String(text) => normalize_text(text),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        _ => normalize_text(&value.to_string()),
    };
    (!normalized.is_empty()).then_some(normalized)
}

fn structured_field_value(entry: &Value, key: &str) -> Option<String> {
    let value = entry.get(key)?;
    if value.is_null() {
        return None;
    }
    match key {
        "facets" if value.as_array().is_some_and(|items| items.is_empty()) => None,
        "tallies" => canonical_tallies_field_string(value),
        _ => Some(canonical_value_string(value)),
    }
}

fn classify_scalar_value(entry: &Value, key: &str) -> Option<String> {
    match entry.get(key) {
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        Some(Value::Null) | None => Some("false".to_string()),
        Some(other) => scalar_field_value(&json!({ key: other }), key),
    }
}

fn canonical_section_value(value: &Value, key: &str) -> Option<Value> {
    let section = value.get(key)?;
    match key {
        "summary" => Some(canonicalize_summary_section(section)),
        "tallies" | "tallies_of_key_files" => canonical_tallies_section(section),
        "tallies_by_facet" => canonical_tallies_by_facet_section(section),
        _ => Some(canonicalize_json_value(section)),
    }
}

fn canonical_value_string(value: &Value) -> String {
    serde_json::to_string(&canonicalize_json_value(value)).unwrap_or_else(|_| value.to_string())
}

fn canonicalize_json_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => {
            let mut normalized: Vec<Value> = values.iter().map(canonicalize_json_value).collect();
            normalized.sort_by_cached_key(canonical_value_string);
            Value::Array(normalized)
        }
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(left, _)| *left);
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), canonicalize_json_value(value)))
                    .collect(),
            )
        }
        _ => value.clone(),
    }
}

fn is_empty_tallies_value(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object
        .values()
        .all(|entry| entry.as_array().is_some_and(|items| items.is_empty()))
}

fn canonical_tallies_field_string(value: &Value) -> Option<String> {
    canonical_tallies_section(value).map(|value| canonical_value_string(&value))
}

fn canonicalize_summary_section(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return canonicalize_json_value(value);
    };

    let mut normalized = serde_json::Map::new();
    for (key, section_value) in object {
        let normalized_value = match key.as_str() {
            "other_license_expressions" => {
                canonicalize_tally_entry_array(section_value, "detected_license_expression")
            }
            "other_holders" => canonicalize_tally_entry_array(section_value, "holders"),
            "other_languages" => {
                canonicalize_tally_entry_array(section_value, "programming_language")
            }
            _ => canonicalize_json_value(section_value),
        };
        normalized.insert(key.clone(), normalized_value);
    }

    for key in [
        "other_license_expressions",
        "other_holders",
        "other_languages",
    ] {
        normalized
            .entry(key.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
    }

    Value::Object(normalized)
}

fn canonical_tallies_section(value: &Value) -> Option<Value> {
    let Some(object) = value.as_object() else {
        return Some(canonicalize_json_value(value));
    };

    let mut normalized = serde_json::Map::new();
    for key in [
        "detected_license_expression",
        "copyrights",
        "holders",
        "authors",
        "programming_language",
    ] {
        let normalized_entries = object
            .get(key)
            .map(|entries| canonicalize_tally_entry_array(entries, key))
            .unwrap_or_else(|| Value::Array(Vec::new()));
        normalized.insert(key.to_string(), normalized_entries);
    }

    let normalized_value = Value::Object(normalized);
    (!is_empty_tallies_value(&normalized_value)).then_some(normalized_value)
}

fn canonical_tallies_by_facet_section(value: &Value) -> Option<Value> {
    let Some(array) = value.as_array() else {
        return Some(canonicalize_json_value(value));
    };

    let mut normalized: Vec<Value> = array
        .iter()
        .map(|entry| {
            let facet = entry
                .get("facet")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let tallies = canonical_tallies_section(entry.get("tallies").unwrap_or(&Value::Null))
                .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
            json!({
                "facet": facet,
                "tallies": tallies,
            })
        })
        .collect();
    normalized.sort_by_cached_key(canonical_value_string);
    Some(Value::Array(normalized))
}

fn canonicalize_tally_entry_array(value: &Value, kind: &str) -> Value {
    let Some(array) = value.as_array() else {
        return Value::Array(Vec::new());
    };

    let mut normalized: Vec<Value> = array
        .iter()
        .map(|entry| {
            let count = entry.get("count").and_then(Value::as_u64).unwrap_or(0);
            let normalized_value = entry
                .get("value")
                .and_then(Value::as_str)
                .map(|text| normalize_tally_value(kind, text));
            json!({
                "count": count,
                "value": normalized_value,
            })
        })
        .collect();
    normalized.sort_by_cached_key(canonical_value_string);
    Value::Array(normalized)
}

fn normalize_tally_value(kind: &str, value: &str) -> String {
    match kind {
        "detected_license_expression" => normalize_license_expression(value),
        "copyrights" => normalize_tally_copyright_value(value),
        "holders" | "authors" | "programming_language" => normalize_text(value),
        _ => normalize_text(value),
    }
}

fn normalize_tally_copyright_value(value: &str) -> String {
    let trimmed = value
        .trim()
        .trim_end_matches(" as indicated by the @authors tag");

    if let Some(rest) = trimmed.strip_prefix("(c) ") {
        let normalized_rest = rest.trim_start_matches(|ch: char| {
            ch.is_ascii_digit() || ch == ' ' || ch == ',' || ch == '-'
        });

        if !normalized_rest.is_empty() && normalized_rest != rest {
            return format!("(c) {}", normalized_rest.trim());
        }
    }

    if let Some(rest) = trimmed.strip_prefix("Copyright (c) ") {
        let normalized_rest = rest.trim_start_matches(|ch: char| {
            ch.is_ascii_digit() || ch == ' ' || ch == ',' || ch == '-'
        });

        if !normalized_rest.is_empty() && normalized_rest != rest {
            return format!("Copyright (c) {}", normalized_rest.trim());
        }
    }

    if let Some(rest) = trimmed.strip_prefix("Copyright ")
        && let Some((yearish, remainder)) = rest.split_once(',')
        && !yearish.is_empty()
        && yearish
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == ' ' || ch == ',' || ch == '-')
    {
        return format!("Copyright {}", remainder.trim());
    }

    if let Some(rest) = trimmed.strip_prefix("Copyright ") {
        let mut parts = rest.rsplitn(2, ' ');
        let trailing = parts.next().unwrap_or_default();
        let leading = parts.next().unwrap_or_default();
        if !leading.is_empty()
            && trailing
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == ',' || ch == '-')
        {
            return format!("Copyright {}", leading.trim());
        }
    }

    trimmed.to_string()
}

fn sample_values(values: &[String]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for value in values {
        set.insert(value.clone());
    }
    set.into_iter().take(10).collect()
}

fn value_counter(values: &[String]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value.clone()).or_insert(0) += 1;
    }
    counts
}

fn subtract_counters(
    left: &BTreeMap<String, usize>,
    right: &BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    let mut result = BTreeMap::new();
    for (key, left_count) in left {
        let right_count = right.get(key).copied().unwrap_or(0);
        if left_count > &right_count {
            result.insert(key.clone(), left_count - right_count);
        }
    }
    result
}

fn counter_entries(counter: &BTreeMap<String, usize>) -> Vec<ValueCountEntry> {
    counter
        .iter()
        .map(|(value, count)| ValueCountEntry {
            value: value.clone(),
            count: *count,
        })
        .collect()
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

fn file_entry_count(value: &Value) -> usize {
    value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("file"))
        .count()
}

fn array_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| values.len())
        .unwrap_or(0)
}

fn file_package_data_count(value: &Value) -> usize {
    value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|entry| {
            entry
                .get("package_data")
                .and_then(Value::as_array)
                .map(|package_data| package_data.len())
                .unwrap_or(0)
        })
        .sum()
}

fn file_package_data_dependency_count(value: &Value) -> usize {
    value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|entry| {
            entry
                .get("package_data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|package_data| {
                    package_data
                        .get("dependencies")
                        .and_then(Value::as_array)
                        .map(|dependencies| dependencies.len())
                        .unwrap_or(0)
                })
                .sum::<usize>()
        })
        .sum()
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

fn top_level_package_identities(value: &Value) -> BTreeSet<String> {
    value
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            package_identity(item)
                .map(str::to_string)
                .or_else(|| package_fallback_identity(item))
                .unwrap_or_else(|| "<unknown>".to_string())
        })
        .collect()
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

fn top_level_dependency_identities_by_path(value: &Value) -> BTreeMap<String, BTreeSet<String>> {
    let mut output: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for item in value
        .get("dependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let path = item
            .get("datafile_path")
            .or_else(|| item.get("path"))
            .and_then(Value::as_str)
            .map(normalize_compare_path)
            .unwrap_or_else(|| "<unknown>".to_string());
        let identity = dependency_identity(item).unwrap_or_else(|| "<unknown>".to_string());
        output.entry(path).or_default().insert(identity);
    }
    output
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

fn raw_dependency_identities_by_path(value: &Value) -> BTreeMap<String, BTreeSet<String>> {
    let mut output: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let file_path = file
            .get("path")
            .and_then(Value::as_str)
            .map(normalize_compare_path)
            .unwrap_or_else(|| "<unknown>".to_string());

        for package_data in file
            .get("package_data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for item in package_data
                .get("dependencies")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let path = item
                    .get("datafile_path")
                    .or_else(|| item.get("path"))
                    .and_then(Value::as_str)
                    .map(normalize_compare_path)
                    .unwrap_or_else(|| file_path.clone());
                let identity = dependency_identity(item).unwrap_or_else(|| "<unknown>".to_string());
                output.entry(path).or_default().insert(identity);
            }
        }
    }
    output
}

fn difference_entries(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<ValueCountEntry> {
    left.difference(right)
        .map(|value| ValueCountEntry {
            value: value.clone(),
            count: 1,
        })
        .collect()
}

fn dependency_identity(item: &Value) -> Option<String> {
    for key in ["purl", "package_url", "dependency_uid"] {
        if let Some(value) = item.get(key).and_then(Value::as_str) {
            let normalized = normalize_text(value);
            if !normalized.is_empty() {
                return Some(normalized);
            }
        }
    }
    let mut parts = Vec::new();
    for key in [
        "datafile_path",
        "scope",
        "namespace",
        "name",
        "version",
        "version_requirement",
        "is_runtime",
        "is_optional",
    ] {
        if let Some(value) = item.get(key) {
            let normalized = if let Some(text) = value.as_str() {
                normalize_text(text)
            } else {
                value.to_string()
            };
            if !normalized.is_empty() {
                parts.push(format!("{key}={normalized}"));
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("|"))
    }
}

fn top_level_directional_differences(
    left: &TopLevelCounts,
    right: &TopLevelCounts,
) -> BTreeMap<String, i64> {
    let mut output = BTreeMap::new();
    for key in [
        "packages",
        "dependencies",
        "license_detections",
        "license_references",
        "license_rule_references",
    ] {
        if !count_delta_is_directly_comparable(key, left, right) {
            continue;
        }
        let left_value = left.count(key);
        let right_value = right.count(key);
        if left_value > right_value {
            output.insert(key.to_string(), left_value - right_value);
        }
    }
    output
}

fn skipped_comparisons(left: &TopLevelCounts, right: &TopLevelCounts) -> BTreeMap<String, String> {
    ["packages", "dependencies"]
        .into_iter()
        .filter(|metric| !count_delta_is_directly_comparable(metric, left, right))
        .map(|metric| {
            (
                metric.to_string(),
                mixed_source_skip_reason(metric, left, right),
            )
        })
        .collect()
}

fn count_delta_is_directly_comparable(
    key: &str,
    left: &TopLevelCounts,
    right: &TopLevelCounts,
) -> bool {
    match key {
        "packages" => {
            left.source(key) == PACKAGES_COUNT_SOURCE && right.source(key) == PACKAGES_COUNT_SOURCE
        }
        "dependencies" => {
            left.source(key) == DEPENDENCIES_COUNT_SOURCE
                && right.source(key) == DEPENDENCIES_COUNT_SOURCE
        }
        _ => true,
    }
}

fn mixed_source_skip_reason(
    metric: &str,
    scancode: &TopLevelCounts,
    provenant: &TopLevelCounts,
) -> String {
    format!(
        "top-level {metric} comparison skipped: ScanCode {}; Provenant {}",
        scancode.source(metric),
        provenant.source(metric)
    )
}

fn top_level_count_note(
    metric: &str,
    scancode: &TopLevelCounts,
    provenant: &TopLevelCounts,
) -> String {
    if !matches!(metric, "packages" | "dependencies") {
        return "top-level count".to_string();
    }

    if count_delta_is_directly_comparable(metric, scancode, provenant) {
        return "top-level count".to_string();
    }

    mixed_source_skip_reason(metric, scancode, provenant)
}

fn compare_uses_only_findings(scan_args: &[String], scancode: &Value, provenant: &Value) -> bool {
    scan_args.iter().any(|arg| arg == "--only-findings")
        || json_output_uses_only_findings(scancode)
        || json_output_uses_only_findings(provenant)
}

fn json_output_uses_only_findings(value: &Value) -> bool {
    value
        .get("headers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|header| {
            header
                .get("options")
                .and_then(Value::as_object)
                .is_some_and(|options| {
                    option_value_is_truthy(options.get("--only-findings"))
                        || option_value_is_truthy(options.get("only_findings"))
                })
        })
}

fn option_value_is_truthy(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(true)))
        || matches!(value, Some(Value::String(text)) if text.eq_ignore_ascii_case("true"))
}

fn output_only_path_note(tool_name: &str, path_kind: &str, only_findings_active: bool) -> String {
    let mut note = format!("{path_kind} paths present only in {tool_name} final output");
    if only_findings_active {
        note.push_str(
            "; with --only-findings, the other output may have filtered these paths away after finding nothing",
        );
    }
    note
}

fn tsv_row(metric: &str, scancode: i64, provenant: i64, delta: i64, notes: &str) -> Vec<String> {
    vec![
        metric.to_string(),
        scancode.to_string(),
        provenant.to_string(),
        delta.to_string(),
        notes.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn metric_values_normalize_raw_copyrights_for_compare() {
        let entry = json!({
            "copyrights": [
                {
                    "copyright": "Copyright 2024 Example Corp. All rights reserved."
                }
            ]
        });

        assert_eq!(
            metric_values(&entry, "copyrights"),
            vec!["Copyright 2024 Example Corp".to_string()]
        );
    }

    #[test]
    fn metric_values_normalize_punctuation_only_copyright_differences() {
        let entry = json!({
            "copyrights": [
                {
                    "copyright": "Copyright 2024 Example Corp.;"
                }
            ]
        });

        assert_eq!(
            metric_values(&entry, "copyrights"),
            vec!["Copyright 2024 Example Corp".to_string()]
        );
    }

    #[test]
    fn metric_values_normalize_confidentiality_tail_copyright_differences() {
        let entry = json!({
            "copyrights": [
                {
                    "copyright": "(c) foo platforms, inc. and affiliates. confidential and proprietary."
                }
            ]
        });

        assert_eq!(
            metric_values(&entry, "copyrights"),
            vec!["(c) foo platforms, inc. and affiliates".to_string()]
        );
    }
}
