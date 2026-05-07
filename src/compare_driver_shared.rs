// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;
use serde_json::Value;

use crate::compare_normalization::{
    normalize_compare_path, normalize_text, package_fallback_identity, package_identity,
};

pub const FILES_COUNT_SOURCE: &str = "files[]";
pub const PACKAGES_COUNT_SOURCE: &str = "packages[]";
pub const PACKAGE_DATA_COUNT_SOURCE: &str = "packages[] empty; files[].package_data present";
pub const DEPENDENCIES_COUNT_SOURCE: &str = "dependencies[]";
pub const PACKAGE_DATA_DEPENDENCIES_COUNT_SOURCE: &str =
    "dependencies[] empty; files[].package_data[].dependencies present";
pub const LICENSE_DETECTIONS_COUNT_SOURCE: &str = "license_detections[]";
pub const LICENSE_REFERENCES_COUNT_SOURCE: &str = "license_references[]";
pub const LICENSE_RULE_REFERENCES_COUNT_SOURCE: &str = "license_rule_references[]";

#[derive(Debug, Serialize, Clone)]
pub struct ValueCountEntry {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct CountDeltaEntry {
    pub path: String,
    pub scancode: usize,
    pub provenant: usize,
    pub delta: isize,
    pub scancode_sample_values: Vec<String>,
    pub provenant_sample_values: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValueDifferenceEntry {
    pub path: String,
    pub scancode: usize,
    pub provenant: usize,
    pub missing_in_provenant: Vec<ValueCountEntry>,
    pub extra_in_provenant: Vec<ValueCountEntry>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ScalarDifferenceEntry {
    pub path: String,
    pub scancode: Option<String>,
    pub provenant: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TopLevelSectionDifferenceEntry {
    pub section: String,
    pub scancode: Option<Value>,
    pub provenant: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct TopLevelCounts {
    pub counts: HashMap<&'static str, i64>,
    pub sources: HashMap<&'static str, &'static str>,
}

impl TopLevelCounts {
    pub fn count(&self, key: &str) -> i64 {
        *self.counts.get(key).expect("top-level count exists")
    }

    pub fn source(&self, key: &str) -> &'static str {
        self.sources
            .get(key)
            .copied()
            .expect("top-level count source exists")
    }

    pub fn counts_json(&self) -> BTreeMap<String, i64> {
        self.counts
            .iter()
            .map(|(key, value)| ((*key).to_string(), *value))
            .collect()
    }

    pub fn sources_json(&self) -> BTreeMap<String, String> {
        self.sources
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }
}

pub fn resources_contain_any_field(resources: &BTreeMap<String, Value>, fields: &[&str]) -> bool {
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

pub fn value_contains_any_section(value: &Value, sections: &[&str]) -> bool {
    sections
        .iter()
        .any(|section| value.get(section).is_some_and(|entry| !entry.is_null()))
}

pub fn files_by_path(value: &Value) -> BTreeMap<String, Value> {
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

pub fn resources_by_path(value: &Value) -> BTreeMap<String, Value> {
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

pub fn metric_count(entry: &Value, key: &str) -> usize {
    entry
        .get(key)
        .and_then(Value::as_array)
        .map(|values| values.len())
        .unwrap_or(0)
}

pub fn sample_values(values: &[String]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for value in values {
        set.insert(value.clone());
    }
    set.into_iter().take(10).collect()
}

pub fn value_counter(values: &[String]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value.clone()).or_insert(0) += 1;
    }
    counts
}

pub fn metric_uses_distinct_signal_values(metric: &str) -> bool {
    matches!(metric, "copyrights" | "holders" | "authors")
}

pub fn metric_signal_counter(metric: &str, values: &[String]) -> BTreeMap<String, usize> {
    if !metric_uses_distinct_signal_values(metric) {
        return value_counter(values);
    }

    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|value| (value, 1))
        .collect()
}

pub fn counter_total(counter: &BTreeMap<String, usize>) -> usize {
    counter.values().copied().sum()
}

pub fn subtract_counters(
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

pub fn filter_counter_to_signal_keys(
    counter: &BTreeMap<String, usize>,
    signal_counter: &BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    counter
        .iter()
        .filter(|(key, _)| signal_counter.contains_key(*key))
        .map(|(key, value)| (key.clone(), *value))
        .collect()
}

pub fn counter_entries(counter: &BTreeMap<String, usize>) -> Vec<ValueCountEntry> {
    counter
        .iter()
        .map(|(value, count)| ValueCountEntry {
            value: value.clone(),
            count: *count,
        })
        .collect()
}

pub fn file_entry_count(value: &Value) -> usize {
    value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("file"))
        .count()
}

pub fn array_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| values.len())
        .unwrap_or(0)
}

pub fn file_package_data_count(value: &Value) -> usize {
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

pub fn file_package_data_dependency_count(value: &Value) -> usize {
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

pub fn top_level_package_identities(value: &Value) -> BTreeSet<String> {
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

pub fn top_level_dependency_identities_by_path(
    value: &Value,
) -> BTreeMap<String, BTreeSet<String>> {
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

pub fn raw_dependency_identities_by_path(value: &Value) -> BTreeMap<String, BTreeSet<String>> {
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

pub fn difference_entries(
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
) -> Vec<ValueCountEntry> {
    left.difference(right)
        .map(|value| ValueCountEntry {
            value: value.clone(),
            count: 1,
        })
        .collect()
}

pub fn dependency_identity(item: &Value) -> Option<String> {
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

pub fn top_level_directional_differences(
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

pub fn skipped_comparisons(
    left: &TopLevelCounts,
    right: &TopLevelCounts,
) -> BTreeMap<String, String> {
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

pub fn count_delta_is_directly_comparable(
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

pub fn mixed_source_skip_reason(
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

pub fn top_level_count_note(
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

pub fn compare_uses_only_findings(
    scan_args: &[String],
    scancode: &Value,
    provenant: &Value,
) -> bool {
    scan_args.iter().any(|arg| arg == "--only-findings")
        || json_output_uses_only_findings(scancode)
        || json_output_uses_only_findings(provenant)
}

pub fn json_output_uses_only_findings(value: &Value) -> bool {
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

pub fn option_value_is_truthy(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(true)))
        || matches!(value, Some(Value::String(text)) if text.eq_ignore_ascii_case("true"))
}

pub fn output_only_path_note(
    tool_name: &str,
    path_kind: &str,
    only_findings_active: bool,
) -> String {
    let mut note = format!("{path_kind} paths present only in {tool_name} final output");
    if only_findings_active {
        note.push_str(
            "; with --only-findings, the other output may have filtered these paths away after finding nothing",
        );
    }
    note
}

pub fn tsv_row(
    metric: &str,
    scancode: i64,
    provenant: i64,
    delta: i64,
    notes: &str,
) -> Vec<String> {
    vec![
        metric.to_string(),
        scancode.to_string(),
        provenant.to_string(),
        delta.to_string(),
        notes.to_string(),
    ]
}
