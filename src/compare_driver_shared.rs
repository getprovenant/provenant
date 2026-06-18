// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::compare_normalization::{
    normalize_compare_path, normalize_license_expression, normalize_text,
    package_fallback_identity, package_identity,
};

/// Package content fields whose values are compared by the
/// package-field-content axis (see [`package_field_content_differences`]).
///
/// This axis is intentionally separate from the identity-only `package_data`
/// metric: two outputs can agree on every package identity yet disagree on the
/// declared-license/holder content those packages carry. The post-extraction
/// declared-license/holder population hook fills exactly these fields, so a
/// regression that drops or corrupts their content shows up here even though
/// the identity bucket stays clean.
pub const PACKAGE_CONTENT_FIELDS: &[&str] = &[
    "declared_license_expression",
    "declared_license_expression_spdx",
    "holder",
];

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
pub struct PackageFieldContentDifferenceEntry {
    pub path: String,
    pub identity: String,
    pub field: String,
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

/// Normalizes one package content field for comparison.
///
/// License expressions go through [`normalize_license_expression`] so operand
/// order and trivial parenthesization differences do not register as deltas;
/// other fields (e.g. `holder`) are whitespace-normalized only. Returns `None`
/// for absent, null, or whitespace-only values so a missing field and an empty
/// string compare equal.
pub fn normalize_package_content_field(item: &Value, field: &str) -> Option<String> {
    let raw = item.get(field).and_then(Value::as_str)?;
    let normalized = match field {
        "declared_license_expression" | "declared_license_expression_spdx" => {
            normalize_license_expression(raw)
        }
        _ => normalize_text(raw),
    };
    (!normalized.is_empty()).then_some(normalized)
}

/// Collects every package row keyed by `(path, identity)` for the
/// package-field-content axis, drawing from both top-level assembled
/// `packages[]` and file-level `files[].package_data[]`.
///
/// Top-level packages are bucketed under the synthetic `<top-level>` path so
/// they line up across the two compared outputs regardless of which file
/// produced them; file-level rows keep their owning file path. Within a path,
/// rows are keyed by package identity (purl, else the
/// type|name|version|datasource_id fallback). When two rows in the same path
/// share an identity, the first one wins, which is sufficient for the declared
/// license/holder content this axis tracks.
pub fn package_content_rows_by_key(value: &Value) -> BTreeMap<(String, String), Value> {
    let mut output: BTreeMap<(String, String), Value> = BTreeMap::new();

    for item in value
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let identity = package_identity(item)
            .map(str::to_string)
            .or_else(|| package_fallback_identity(item))
            .unwrap_or_else(|| "<unknown>".to_string());
        output
            .entry(("<top-level>".to_string(), identity))
            .or_insert_with(|| item.clone());
    }

    for file in value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let path = file
            .get("path")
            .and_then(Value::as_str)
            .map(normalize_compare_path)
            .unwrap_or_else(|| "<unknown>".to_string());
        for item in file
            .get("package_data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let identity = package_identity(item)
                .map(str::to_string)
                .or_else(|| package_fallback_identity(item))
                .unwrap_or_else(|| "<unknown>".to_string());
            output
                .entry((path.clone(), identity))
                .or_insert_with(|| item.clone());
        }
    }

    output
}

/// Diffs the content of [`PACKAGE_CONTENT_FIELDS`] for packages matched by
/// `(path, identity)` across the two outputs.
///
/// Only identity-matched rows are compared: rows present on a single side are
/// an identity-level signal already surfaced by the `package_data` and
/// top-level package buckets, so they are skipped here to keep this axis
/// focused on content drift for packages both outputs agree exist.
///
/// Enabling this axis can surface pre-existing ScanCode-vs-Provenant
/// declared-license deltas that predate the post-extraction population hook.
/// That is expected and valuable signal, not necessarily a regression to fix.
pub fn package_field_content_differences(
    scancode: &Value,
    provenant: &Value,
) -> Vec<PackageFieldContentDifferenceEntry> {
    let sc_rows = package_content_rows_by_key(scancode);
    let pr_rows = package_content_rows_by_key(provenant);

    let mut differences = Vec::new();
    for ((path, identity), sc_item) in &sc_rows {
        let Some(pr_item) = pr_rows.get(&(path.clone(), identity.clone())) else {
            continue;
        };
        for field in PACKAGE_CONTENT_FIELDS {
            let sc_value = normalize_package_content_field(sc_item, field);
            let pr_value = normalize_package_content_field(pr_item, field);
            if sc_value != pr_value {
                differences.push(PackageFieldContentDifferenceEntry {
                    path: path.clone(),
                    identity: identity.clone(),
                    field: (*field).to_string(),
                    scancode: sc_value,
                    provenant: pr_value,
                });
            }
        }
    }
    differences
}

/// Self-consistent tally of a [`package_field_content_differences`] result.
///
/// Each difference entry lands in exactly one of the three buckets, so the
/// invariant `missing_in_provenant + extra_in_provenant + value_vs_value_mismatch
/// == sum(by_field.values()) == total entries` always holds. Value-vs-value
/// mismatches (both sides non-null but different) get their own bucket instead
/// of being counted into both directional totals, which keeps the summary
/// numbers reconcilable by any downstream consumer.
#[derive(Debug, Default, Clone)]
pub struct PackageFieldContentTally {
    pub missing_in_provenant: usize,
    pub extra_in_provenant: usize,
    pub value_vs_value_mismatch: usize,
    pub by_field: BTreeMap<String, usize>,
}

pub fn tally_package_field_content_differences(
    differences: &[PackageFieldContentDifferenceEntry],
) -> PackageFieldContentTally {
    let mut tally = PackageFieldContentTally::default();
    for entry in differences {
        *tally.by_field.entry(entry.field.clone()).or_insert(0) += 1;
        match (entry.scancode.is_some(), entry.provenant.is_some()) {
            (true, false) => tally.missing_in_provenant += 1,
            (false, true) => tally.extra_in_provenant += 1,
            _ => tally.value_vs_value_mismatch += 1,
        }
    }
    tally
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

/// Maximum number of values retained per field/direction in the cross-file
/// frequency rollup. The rollup is a triage aid, so a bounded top-N keeps the
/// artifact readable while still surfacing the systematic patterns.
pub const FIELD_VALUE_FREQUENCY_TOP_N: usize = 50;

#[derive(Debug, Serialize, Clone)]
pub struct FieldValueFrequencyEntry {
    pub value: String,
    /// Total occurrences of this value summed across every common-path file.
    pub total_count: usize,
    /// Number of distinct files contributing at least one occurrence.
    pub file_count: usize,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct FieldValueFrequencyDirections {
    /// Values present only in Provenant output (PV-only).
    pub extra_in_provenant: Vec<FieldValueFrequencyEntry>,
    /// Values present only in ScanCode output (SC-only).
    pub extra_in_scancode: Vec<FieldValueFrequencyEntry>,
}

/// Roll the per-file value-level diffs up into a cross-file, frequency-ranked
/// view so systematic patterns become visible (e.g. one author value appearing
/// hundreds of times across files instead of as scattered one-count entries).
///
/// This is a pure aggregation of `value_differences`, which both compare drivers
/// already compute per file. It is a neutral diagnostic only: PV-only values are
/// not necessarily junk (much is legitimate source-faithful or richer output),
/// so the rollup is framed by direction and never feeds pass/fail or signal
/// counts.
pub fn field_value_frequency_rollup(
    value_differences: &BTreeMap<String, Vec<ValueDifferenceEntry>>,
    top_n: usize,
) -> BTreeMap<String, FieldValueFrequencyDirections> {
    value_differences
        .iter()
        .map(|(field, entries)| {
            let pv_only = aggregate_direction(entries, |entry| &entry.extra_in_provenant, top_n);
            let sc_only = aggregate_direction(entries, |entry| &entry.missing_in_provenant, top_n);
            (
                field.clone(),
                FieldValueFrequencyDirections {
                    extra_in_provenant: pv_only,
                    extra_in_scancode: sc_only,
                },
            )
        })
        .collect()
}

fn aggregate_direction(
    entries: &[ValueDifferenceEntry],
    select: impl Fn(&ValueDifferenceEntry) -> &Vec<ValueCountEntry>,
    top_n: usize,
) -> Vec<FieldValueFrequencyEntry> {
    let mut totals: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for entry in entries {
        for value_count in select(entry) {
            let slot = totals.entry(value_count.value.clone()).or_insert((0, 0));
            slot.0 += value_count.count;
            slot.1 += 1;
        }
    }
    let mut ranked: Vec<FieldValueFrequencyEntry> = totals
        .into_iter()
        .map(
            |(value, (total_count, file_count))| FieldValueFrequencyEntry {
                value,
                total_count,
                file_count,
            },
        )
        .collect();
    // Sort by total count descending, then by value ascending for a stable,
    // deterministic order across runs. Starting from a BTreeMap keeps the
    // by-value tie-break deterministic regardless of input ordering.
    ranked.sort_by(|a, b| {
        b.total_count
            .cmp(&a.total_count)
            .then_with(|| a.value.cmp(&b.value))
    });
    ranked.truncate(top_n);
    ranked
}

/// How many top values per field/direction to surface inline in `summary.json`.
/// The standalone samples file carries the full top-N; this is just a glanceable
/// preview. A field with values in only one direction still appears, with the
/// empty direction rendered as `[]`; only fields empty in both directions are
/// omitted.
pub const FIELD_VALUE_FREQUENCY_SUMMARY_TOP_N: usize = 5;

pub fn field_value_frequency_summary(
    rollup: &BTreeMap<String, FieldValueFrequencyDirections>,
    top_n: usize,
) -> Map<String, Value> {
    let mut summary = Map::new();
    for (field, directions) in rollup {
        let pv_only = &directions.extra_in_provenant;
        let sc_only = &directions.extra_in_scancode;
        if pv_only.is_empty() && sc_only.is_empty() {
            continue;
        }
        summary.insert(
            field.clone(),
            json!({
                "extra_in_provenant": field_value_frequency_preview(pv_only, top_n),
                "extra_in_scancode": field_value_frequency_preview(sc_only, top_n),
            }),
        );
    }
    summary
}

fn field_value_frequency_preview(entries: &[FieldValueFrequencyEntry], top_n: usize) -> Vec<Value> {
    entries
        .iter()
        .take(top_n)
        .map(|entry| {
            json!({
                "value": entry.value,
                "total_count": entry.total_count,
                "file_count": entry.file_count,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entries_for(
        differences: &[PackageFieldContentDifferenceEntry],
        field: &str,
    ) -> Vec<PackageFieldContentDifferenceEntry> {
        differences
            .iter()
            .filter(|entry| entry.field == field)
            .cloned()
            .collect()
    }

    #[test]
    fn normalize_package_content_field_treats_empty_and_absent_as_none() {
        let item = json!({ "declared_license_expression": "", "holder": "   " });
        assert_eq!(
            normalize_package_content_field(&item, "declared_license_expression"),
            None
        );
        assert_eq!(normalize_package_content_field(&item, "holder"), None);
        assert_eq!(normalize_package_content_field(&item, "missing"), None);
    }

    #[test]
    fn normalize_package_content_field_canonicalizes_license_expressions() {
        let item = json!({ "declared_license_expression": "(MIT OR Apache-2.0)" });
        assert_eq!(
            normalize_package_content_field(&item, "declared_license_expression").as_deref(),
            Some("Apache-2.0 OR MIT")
        );
    }

    #[test]
    fn package_field_content_reports_declared_license_gained() {
        let scancode = json!({
            "files": [{
                "path": "metadata.rb",
                "type": "file",
                "package_data": [{
                    "purl": "pkg:chef/example@1.0.0",
                    "declared_license_expression": null
                }]
            }]
        });
        let provenant = json!({
            "files": [{
                "path": "metadata.rb",
                "type": "file",
                "package_data": [{
                    "purl": "pkg:chef/example@1.0.0",
                    "declared_license_expression": "apache-2.0",
                    "declared_license_expression_spdx": "Apache-2.0"
                }]
            }]
        });

        let differences = package_field_content_differences(&scancode, &provenant);
        let license = entries_for(&differences, "declared_license_expression");
        assert_eq!(license.len(), 1);
        assert_eq!(license[0].scancode, None);
        assert_eq!(license[0].provenant.as_deref(), Some("apache-2.0"));
        assert_eq!(license[0].path, "metadata.rb");
        assert_eq!(license[0].identity, "pkg:chef/example@1.0.0");
        // The SPDX form gained content too.
        assert_eq!(
            entries_for(&differences, "declared_license_expression_spdx").len(),
            1
        );
    }

    #[test]
    fn package_field_content_matches_top_level_packages_by_identity() {
        let scancode = json!({
            "packages": [{ "purl": "pkg:chef/example@1.0.0" }]
        });
        let provenant = json!({
            "packages": [{ "purl": "pkg:chef/example@1.0.0", "holder": "Example Corp" }]
        });

        let differences = package_field_content_differences(&scancode, &provenant);
        let holder = entries_for(&differences, "holder");
        assert_eq!(holder.len(), 1);
        assert_eq!(holder[0].path, "<top-level>");
        assert_eq!(holder[0].provenant.as_deref(), Some("Example Corp"));
    }

    #[test]
    fn package_field_content_ignores_operand_order_only_differences() {
        let scancode = json!({
            "packages": [{
                "purl": "pkg:npm/example@1.0.0",
                "declared_license_expression": "MIT OR Apache-2.0"
            }]
        });
        let provenant = json!({
            "packages": [{
                "purl": "pkg:npm/example@1.0.0",
                "declared_license_expression": "Apache-2.0 OR MIT"
            }]
        });

        assert!(package_field_content_differences(&scancode, &provenant).is_empty());
    }

    #[test]
    fn package_field_content_skips_identity_only_present_on_one_side() {
        let scancode = json!({
            "packages": [{ "purl": "pkg:npm/only-scancode@1.0.0", "holder": "A" }]
        });
        let provenant = json!({
            "packages": [{ "purl": "pkg:npm/only-provenant@1.0.0", "holder": "B" }]
        });

        // Identity-only deltas are surfaced by the package_data buckets, not here.
        assert!(package_field_content_differences(&scancode, &provenant).is_empty());
    }

    #[test]
    fn package_field_content_reports_value_mismatch_on_both_sides() {
        let scancode = json!({
            "packages": [{
                "purl": "pkg:npm/example@1.0.0",
                "declared_license_expression": "mit"
            }]
        });
        let provenant = json!({
            "packages": [{
                "purl": "pkg:npm/example@1.0.0",
                "declared_license_expression": "apache-2.0"
            }]
        });

        let differences = package_field_content_differences(&scancode, &provenant);
        let license = entries_for(&differences, "declared_license_expression");
        assert_eq!(license.len(), 1);
        assert_eq!(license[0].scancode.as_deref(), Some("mit"));
        assert_eq!(license[0].provenant.as_deref(), Some("apache-2.0"));
    }

    /// Asserts the tally invariant: every entry lands in exactly one of the
    /// three directional buckets, so they sum to both the total entry count and
    /// the sum of `by_field`.
    fn assert_tally_reconciles(tally: &PackageFieldContentTally, total_entries: usize) {
        let bucket_total =
            tally.missing_in_provenant + tally.extra_in_provenant + tally.value_vs_value_mismatch;
        assert_eq!(
            bucket_total, total_entries,
            "directional buckets must sum to the number of difference entries"
        );
        let by_field_total: usize = tally.by_field.values().sum();
        assert_eq!(
            bucket_total, by_field_total,
            "directional buckets must reconcile with by_field"
        );
    }

    #[test]
    fn tally_reconciles_for_one_sided_missing() {
        // Content present only on the ScanCode side -> missing_in_provenant.
        let scancode = json!({
            "packages": [{ "purl": "pkg:npm/example@1.0.0", "holder": "Example Corp" }]
        });
        let provenant = json!({
            "packages": [{ "purl": "pkg:npm/example@1.0.0" }]
        });

        let differences = package_field_content_differences(&scancode, &provenant);
        let tally = tally_package_field_content_differences(&differences);
        assert_eq!(tally.missing_in_provenant, 1);
        assert_eq!(tally.extra_in_provenant, 0);
        assert_eq!(tally.value_vs_value_mismatch, 0);
        assert_tally_reconciles(&tally, differences.len());
    }

    #[test]
    fn tally_reconciles_for_one_sided_extra() {
        // Content present only on the Provenant side -> extra_in_provenant.
        let scancode = json!({
            "packages": [{ "purl": "pkg:npm/example@1.0.0" }]
        });
        let provenant = json!({
            "packages": [{ "purl": "pkg:npm/example@1.0.0", "holder": "Example Corp" }]
        });

        let differences = package_field_content_differences(&scancode, &provenant);
        let tally = tally_package_field_content_differences(&differences);
        assert_eq!(tally.missing_in_provenant, 0);
        assert_eq!(tally.extra_in_provenant, 1);
        assert_eq!(tally.value_vs_value_mismatch, 0);
        assert_tally_reconciles(&tally, differences.len());
    }

    #[test]
    fn tally_reconciles_for_value_vs_value_mismatch() {
        // Both sides non-null but different -> its own bucket, NOT double-counted
        // into the directional totals.
        let scancode = json!({
            "packages": [{
                "purl": "pkg:npm/example@1.0.0",
                "declared_license_expression": "mit"
            }]
        });
        let provenant = json!({
            "packages": [{
                "purl": "pkg:npm/example@1.0.0",
                "declared_license_expression": "apache-2.0"
            }]
        });

        let differences = package_field_content_differences(&scancode, &provenant);
        let tally = tally_package_field_content_differences(&differences);
        assert_eq!(tally.missing_in_provenant, 0);
        assert_eq!(tally.extra_in_provenant, 0);
        assert_eq!(tally.value_vs_value_mismatch, 1);
        assert_tally_reconciles(&tally, differences.len());
    }

    #[test]
    fn tally_reconciles_for_mixed_entries() {
        // A package gaining content (extra) plus a package with a value mismatch:
        // the totals must still reconcile across all three buckets and by_field.
        let scancode = json!({
            "packages": [
                { "purl": "pkg:npm/gains@1.0.0" },
                { "purl": "pkg:npm/mismatch@1.0.0", "declared_license_expression": "mit" }
            ]
        });
        let provenant = json!({
            "packages": [
                { "purl": "pkg:npm/gains@1.0.0", "holder": "Example Corp" },
                { "purl": "pkg:npm/mismatch@1.0.0", "declared_license_expression": "apache-2.0" }
            ]
        });

        let differences = package_field_content_differences(&scancode, &provenant);
        let tally = tally_package_field_content_differences(&differences);
        assert_eq!(tally.extra_in_provenant, 1);
        assert_eq!(tally.value_vs_value_mismatch, 1);
        assert_eq!(tally.missing_in_provenant, 0);
        assert_tally_reconciles(&tally, differences.len());
    }

    fn value_diff(
        path: &str,
        missing: &[(&str, usize)],
        extra: &[(&str, usize)],
    ) -> ValueDifferenceEntry {
        let to_entries = |pairs: &[(&str, usize)]| {
            pairs
                .iter()
                .map(|(value, count)| ValueCountEntry {
                    value: (*value).to_string(),
                    count: *count,
                })
                .collect::<Vec<_>>()
        };
        ValueDifferenceEntry {
            path: path.to_string(),
            scancode: 0,
            provenant: 0,
            missing_in_provenant: to_entries(missing),
            extra_in_provenant: to_entries(extra),
        }
    }

    #[test]
    fn field_value_frequency_rollup_aggregates_counts_across_files() {
        let mut value_differences: BTreeMap<String, Vec<ValueDifferenceEntry>> = BTreeMap::new();
        value_differences.insert(
            "authors".to_string(),
            vec![
                value_diff("a.rs", &[], &[("Adam Jacob", 2), ("rare", 1)]),
                value_diff("b.rs", &[], &[("Adam Jacob", 1)]),
                value_diff("c.rs", &[("Only In SC", 4)], &[("Adam Jacob", 1)]),
            ],
        );

        let rollup = field_value_frequency_rollup(&value_differences, 50);
        let authors = &rollup["authors"];

        // Extra (PV-only) values roll up across all three files.
        assert_eq!(authors.extra_in_provenant.len(), 2);
        let adam = &authors.extra_in_provenant[0];
        assert_eq!(adam.value, "Adam Jacob");
        assert_eq!(adam.total_count, 4);
        assert_eq!(adam.file_count, 3);
        // Sorted by total_count descending: Adam (4) before rare (1).
        assert_eq!(authors.extra_in_provenant[1].value, "rare");
        assert_eq!(authors.extra_in_provenant[1].total_count, 1);
        assert_eq!(authors.extra_in_provenant[1].file_count, 1);

        // SC-only values aggregate independently.
        assert_eq!(authors.extra_in_scancode.len(), 1);
        assert_eq!(authors.extra_in_scancode[0].value, "Only In SC");
        assert_eq!(authors.extra_in_scancode[0].total_count, 4);
    }

    #[test]
    fn field_value_frequency_rollup_is_deterministic_and_capped() {
        let entries: Vec<ValueDifferenceEntry> = (0..10)
            .map(|i| value_diff(&format!("f{i}.rs"), &[], &[("tie-a", 3), ("tie-b", 3)]))
            .collect();
        let mut value_differences = BTreeMap::new();
        value_differences.insert("holders".to_string(), entries);

        let rollup = field_value_frequency_rollup(&value_differences, 1);
        let holders = &rollup["holders"];
        // Cap honored: only the single top-ranked value survives.
        assert_eq!(holders.extra_in_provenant.len(), 1);
        // Equal totals break ties by value ascending, so "tie-a" wins.
        assert_eq!(holders.extra_in_provenant[0].value, "tie-a");
        assert_eq!(holders.extra_in_provenant[0].total_count, 30);
        assert_eq!(holders.extra_in_provenant[0].file_count, 10);
    }

    #[test]
    fn field_value_frequency_summary_omits_empty_fields() {
        let mut rollup: BTreeMap<String, FieldValueFrequencyDirections> = BTreeMap::new();
        rollup.insert(
            "copyrights".to_string(),
            FieldValueFrequencyDirections::default(),
        );
        rollup.insert(
            "authors".to_string(),
            FieldValueFrequencyDirections {
                extra_in_provenant: vec![FieldValueFrequencyEntry {
                    value: "Adam Jacob".to_string(),
                    total_count: 9,
                    file_count: 4,
                }],
                extra_in_scancode: Vec::new(),
            },
        );

        let summary = field_value_frequency_summary(&rollup, 5);
        assert!(!summary.contains_key("copyrights"));
        assert_eq!(
            summary["authors"]["extra_in_provenant"][0]["value"],
            "Adam Jacob"
        );
        assert_eq!(
            summary["authors"]["extra_in_provenant"][0]["total_count"],
            9
        );
        // Field non-empty in one direction still appears, with the empty
        // direction rendered as `[]`.
        assert_eq!(summary["authors"]["extra_in_scancode"], json!([]));
    }
}
