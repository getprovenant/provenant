// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Map, Value, json};

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

pub(crate) fn compare_json_files(
    scancode_source: &Path,
    provenant_source: &Path,
    artifact_dir: &Path,
) -> Result<CompareCommandResult> {
    validate_json_input(scancode_source, "--scancode-json")?;
    validate_json_input(provenant_source, "--provenant-json")?;

    let layout = prepare_layout(artifact_dir)?;
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
    let only_scancode_paths: Vec<String> = scancode_paths
        .difference(&provenant_paths)
        .cloned()
        .collect();
    let only_provenant_paths: Vec<String> = provenant_paths
        .difference(&scancode_paths)
        .cloned()
        .collect();
    let common_resource_paths: Vec<String> = scancode_resource_paths
        .intersection(&provenant_resource_paths)
        .cloned()
        .collect();
    let only_scancode_resource_paths: Vec<String> = scancode_resource_paths
        .difference(&provenant_resource_paths)
        .cloned()
        .collect();
    let only_provenant_resource_paths: Vec<String> = provenant_resource_paths
        .difference(&scancode_resource_paths)
        .cloned()
        .collect();

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
    let top_level_regressions_map = top_level_regressions(&sc_top, &pr_top, true);
    let top_level_higher_counts = top_level_regressions(&pr_top, &sc_top, false);

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
            sc_top[key],
            pr_top[key],
            pr_top[key] - sc_top[key],
            "top-level count",
        ));
    }
    rows.push(tsv_row(
        "common_file_paths",
        common_paths.len() as i64,
        common_paths.len() as i64,
        0,
        "paths present in both outputs",
    ));
    rows.push(tsv_row(
        "only_scancode_file_paths",
        only_scancode_paths.len() as i64,
        0,
        -(only_scancode_paths.len() as i64),
        "paths seen only in ScanCode output",
    ));
    rows.push(tsv_row(
        "only_provenant_file_paths",
        0,
        only_provenant_paths.len() as i64,
        only_provenant_paths.len() as i64,
        "paths seen only in Provenant output",
    ));
    rows.push(tsv_row(
        "common_resource_paths",
        common_resource_paths.len() as i64,
        common_resource_paths.len() as i64,
        0,
        "resource paths present in both outputs",
    ));
    rows.push(tsv_row(
        "only_scancode_resource_paths",
        only_scancode_resource_paths.len() as i64,
        0,
        -(only_scancode_resource_paths.len() as i64),
        "resource paths seen only in ScanCode output",
    ));
    rows.push(tsv_row(
        "only_provenant_resource_paths",
        0,
        only_provenant_resource_paths.len() as i64,
        only_provenant_resource_paths.len() as i64,
        "resource paths seen only in Provenant output",
    ));

    let mut potential_regressions = only_scancode_paths.len() + top_level_regressions_map.len();
    let mut potential_higher = only_provenant_paths.len() + top_level_higher_counts.len();
    if info_mode {
        potential_regressions += only_scancode_resource_paths.len();
        potential_higher += only_provenant_resource_paths.len();
    }
    if row2_mode {
        potential_regressions += row2_top_level_differences.len();
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
            potential_regressions += higher_counts[metric].len();
            potential_regressions += extra;
            potential_higher += missing;
        } else {
            potential_regressions += lower_counts[metric].len();
            potential_higher += higher_counts[metric].len();
            potential_regressions += missing;
            potential_higher += extra;
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
            potential_regressions += differences;
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
            potential_regressions += differences;
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
            potential_regressions += differences;
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

    let dependency_value_differences = dependency_differences(&scancode, &provenant);
    let dependency_missing = dependency_value_differences
        .iter()
        .filter(|entry| !entry.missing_in_provenant.is_empty())
        .count();
    let dependency_extra = dependency_value_differences
        .iter()
        .filter(|entry| !entry.extra_in_provenant.is_empty())
        .count();
    file_metric_summary.insert(
        "dependencies".to_string(),
        json!({
            "missing_in_provenant": dependency_missing,
            "extra_in_provenant": dependency_extra,
        }),
    );
    potential_regressions += dependency_missing;
    potential_higher += dependency_extra;
    rows.push(tsv_row(
        "dependencies_missing_in_provenant",
        dependency_missing as i64,
        0,
        -(dependency_missing as i64),
        "dependency identities present only in ScanCode output",
    ));
    rows.push(tsv_row(
        "dependencies_extra_in_provenant",
        0,
        dependency_extra as i64,
        dependency_extra as i64,
        "dependency identities present only in Provenant output",
    ));
    rows.push(tsv_row(
        "top_level_license_expression_deltas",
        license_deltas.len() as i64,
        license_deltas.len() as i64,
        0,
        "expressions with different top-level detection counts",
    ));

    let comparison_status = if potential_regressions > 0 {
        "potential_regressions_detected"
    } else if potential_higher > 0 || !license_deltas.is_empty() {
        "differences_detected"
    } else {
        "no_detected_differences"
    };

    let sample_paths = [
        (
            "only_scancode_paths",
            layout.samples_dir.join("only_scancode_paths.json"),
        ),
        (
            "only_provenant_paths",
            layout.samples_dir.join("only_provenant_paths.json"),
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
            "dependency_value_differences",
            layout.samples_dir.join("dependency_value_differences.json"),
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

    write_pretty_json(&sample_paths[0].1, &only_scancode_paths)?;
    write_pretty_json(&sample_paths[1].1, &only_provenant_paths)?;
    write_pretty_json(&sample_paths[2].1, &lower_counts)?;
    write_pretty_json(&sample_paths[3].1, &higher_counts)?;
    write_pretty_json(&sample_paths[4].1, &value_differences)?;
    write_pretty_json(&sample_paths[5].1, &license_deltas)?;
    write_pretty_json(&sample_paths[6].1, &dependency_value_differences)?;
    write_pretty_json(&sample_paths[7].1, &info_value_differences)?;
    write_pretty_json(&sample_paths[8].1, &classify_value_differences)?;
    write_pretty_json(&sample_paths[9].1, &row2_value_differences)?;
    write_pretty_json(&sample_paths[10].1, &row2_top_level_differences)?;

    let summary = json!({
        "comparison_status": comparison_status,
        "top_level_counts": {
            "scancode": sc_top,
            "provenant": pr_top,
            "delta": {
                "files": pr_top["files"] - sc_top["files"],
                "packages": pr_top["packages"] - sc_top["packages"],
                "dependencies": pr_top["dependencies"] - sc_top["dependencies"],
                "license_detections": pr_top["license_detections"] - sc_top["license_detections"],
                "license_references": pr_top["license_references"] - sc_top["license_references"],
                "license_rule_references": pr_top["license_rule_references"] - sc_top["license_rule_references"],
            }
        },
        "file_path_comparison": {
            "common_paths": common_paths.len(),
            "only_scancode_paths": only_scancode_paths.len(),
            "only_provenant_paths": only_provenant_paths.len(),
        },
        "resource_path_comparison": {
            "common_paths": common_resource_paths.len(),
            "only_scancode_paths": only_scancode_resource_paths.len(),
            "only_provenant_paths": only_provenant_resource_paths.len(),
        },
        "file_metric_summary": file_metric_summary,
        "info_metric_summary": info_metric_summary,
        "classify_metric_summary": classify_metric_summary,
        "row2_metric_summary": row2_metric_summary,
        "row2_top_level_section_difference_count": row2_top_level_differences.len(),
        "top_level_regressions": top_level_regressions_map,
        "top_level_higher_counts": top_level_higher_counts,
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
            entry
                .get("path")
                .and_then(Value::as_str)
                .map(|path| (normalize_compare_path(path), entry.clone()))
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
                "package_data" => package_identity(item)
                    .map(str::to_string)
                    .or_else(|| package_fallback_identity(item)),
                "copyrights" => item
                    .get("copyright")
                    .and_then(Value::as_str)
                    .map(str::to_string),
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
    if normalized.contains(" OR ")
        || normalized.contains(" or ")
        || normalized.contains(" WITH ")
        || normalized.contains(" with ")
    {
        normalized
    } else if normalized.contains(" AND ") {
        let stripped = normalized.replace(['(', ')'], "");
        let mut parts: Vec<_> = stripped
            .split(" AND ")
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect();
        parts.sort_unstable();
        parts.join(" AND ")
    } else {
        normalized.replace(['(', ')'], "")
    }
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

fn top_level_counts(value: &Value) -> HashMap<&'static str, i64> {
    HashMap::from([
        ("files", file_entry_count(value) as i64),
        ("packages", array_len(value, "packages") as i64),
        ("dependencies", array_len(value, "dependencies") as i64),
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
    ])
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
            let count = item
                .get("detection_count")
                .and_then(Value::as_i64)
                .unwrap_or(1);
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

fn dependency_differences(scancode: &Value, provenant: &Value) -> Vec<ValueDifferenceEntry> {
    let sc_by_path = dependency_identities_by_path(scancode);
    let pr_by_path = dependency_identities_by_path(provenant);
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

fn dependency_identities_by_path(value: &Value) -> BTreeMap<String, BTreeSet<String>> {
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

fn top_level_regressions(
    left: &HashMap<&'static str, i64>,
    right: &HashMap<&'static str, i64>,
    left_is_scancode: bool,
) -> BTreeMap<String, i64> {
    let mut output = BTreeMap::new();
    for key in [
        "packages",
        "dependencies",
        "license_detections",
        "license_references",
        "license_rule_references",
    ] {
        let left_value = left[key];
        let right_value = right[key];
        if left_is_scancode {
            if right_value < left_value {
                output.insert(key.to_string(), left_value - right_value);
            }
        } else if left_value > right_value {
            output.insert(key.to_string(), left_value - right_value);
        }
    }
    output
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
