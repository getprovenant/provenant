// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::models::PackageData;
use crate::output_schema::OutputPackageData;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Fields the central post-extraction step (`populate_declared_license_and_holder`)
/// can newly populate. When the `PROVENANT_UPDATE_PARSER_GOLDEN` maintenance
/// switch is set, the golden comparators surgically refresh only these fields in
/// place so unrelated golden structure stays byte-stable.
const POST_EXTRACTION_GOLDEN_FIELDS: &[&str] = &[
    "declared_license_expression",
    "declared_license_expression_spdx",
    "license_detections",
    "holder",
];

fn golden_update_enabled() -> bool {
    std::env::var_os("PROVENANT_UPDATE_PARSER_GOLDEN").is_some()
}

pub fn compare_package_data_parser_only(
    actual: &PackageData,
    expected_path: &Path,
) -> Result<(), String> {
    let expected_content = fs::read_to_string(expected_path)
        .map_err(|e| format!("Failed to read expected file: {}", e))?;

    let mut expected_value: Value = serde_json::from_str(&expected_content)
        .map_err(|e| format!("Failed to parse expected JSON: {}", e))?;

    let output: OutputPackageData = actual.into();
    let actual_json = serde_json::to_value(&output)
        .map_err(|e| format!("Failed to serialize actual PackageData: {}", e))?;

    if golden_update_enabled() {
        let actual_objects = [&actual_json];
        refresh_post_extraction_fields(&mut expected_value, &actual_objects);
        return write_golden(expected_path, &expected_value);
    }

    let expected_json = unwrap_expected_parser_package(&expected_value)?;
    compare_json_values_parser_only(&actual_json, expected_json, "")
}

pub fn compare_package_data_collection_parser_only(
    actual: &[PackageData],
    expected_path: &Path,
) -> Result<(), String> {
    let expected_content = fs::read_to_string(expected_path)
        .map_err(|e| format!("Failed to read expected file: {}", e))?;

    let mut expected_value: Value = serde_json::from_str(&expected_content)
        .map_err(|e| format!("Failed to parse expected JSON: {}", e))?;

    let output: Vec<OutputPackageData> = actual.iter().map(|pd| pd.into()).collect();
    let actual_json = serde_json::to_value(&output)
        .map_err(|e| format!("Failed to serialize actual PackageData collection: {}", e))?;

    if golden_update_enabled() {
        let actual_objects: Vec<&Value> = actual_json.as_array().into_iter().flatten().collect();
        refresh_post_extraction_fields(&mut expected_value, &actual_objects);
        return write_golden(expected_path, &expected_value);
    }

    let expected_json = unwrap_expected_parser_package_collection(&expected_value)?;
    compare_json_values_parser_only(&actual_json, expected_json, "")
}

fn write_golden(expected_path: &Path, value: &Value) -> Result<(), String> {
    let mut serialized = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize updated golden: {}", e))?;
    serialized.push('\n');
    fs::write(expected_path, serialized)
        .map_err(|e| format!("Failed to write updated golden: {}", e))
}

/// Copies the post-extraction-owned fields from freshly produced package
/// objects into the matching package objects of an existing expected document,
/// preserving every other field and the document's overall shape.
fn refresh_post_extraction_fields(expected: &mut Value, actual_objects: &[&Value]) {
    for (target, source) in collect_expected_package_objects(expected)
        .into_iter()
        .zip(actual_objects.iter())
    {
        let Some(target_map) = target.as_object_mut() else {
            continue;
        };
        for field in POST_EXTRACTION_GOLDEN_FIELDS {
            match source.get(*field) {
                Some(value) => {
                    target_map.insert((*field).to_string(), value.clone());
                }
                None => {
                    target_map.remove(*field);
                }
            }
        }
    }
}

enum ExpectedShape {
    Array,
    Packages,
    Files,
    SingleObject,
    Unknown,
}

fn collect_expected_package_objects(expected: &mut Value) -> Vec<&mut Value> {
    let shape = match expected {
        Value::Array(_) => ExpectedShape::Array,
        Value::Object(map) if map.get("packages").is_some_and(Value::is_array) => {
            ExpectedShape::Packages
        }
        Value::Object(map) if map.get("files").is_some_and(Value::is_array) => ExpectedShape::Files,
        Value::Object(_) => ExpectedShape::SingleObject,
        _ => ExpectedShape::Unknown,
    };

    match shape {
        ExpectedShape::Array => expected
            .as_array_mut()
            .map(|array| array.iter_mut().collect())
            .unwrap_or_default(),
        ExpectedShape::Packages => expected
            .get_mut("packages")
            .and_then(Value::as_array_mut)
            .map(|packages| packages.iter_mut().collect())
            .unwrap_or_default(),
        ExpectedShape::Files => expected
            .get_mut("files")
            .and_then(Value::as_array_mut)
            .map(|files| {
                files
                    .iter_mut()
                    .filter_map(|file| file.get_mut("package_data"))
                    .filter_map(Value::as_array_mut)
                    .flat_map(|package_data| package_data.iter_mut())
                    .collect()
            })
            .unwrap_or_default(),
        ExpectedShape::SingleObject => vec![expected],
        ExpectedShape::Unknown => Vec::new(),
    }
}

fn unwrap_expected_parser_package(expected_value: &Value) -> Result<&Value, String> {
    if let Some(expected_array) = expected_value.as_array() {
        if expected_array.is_empty() {
            return Err("Expected file contains empty array".to_string());
        }
        return Ok(&expected_array[0]);
    }

    if let Some(package_data) = expected_value
        .get("files")
        .and_then(Value::as_array)
        .and_then(|files| files.first())
        .and_then(|file| file.get("package_data"))
        .and_then(Value::as_array)
    {
        if package_data.is_empty() {
            return Err("Expected file contains empty files[0].package_data array".to_string());
        }
        return Ok(&package_data[0]);
    }

    Ok(expected_value)
}

fn unwrap_expected_parser_package_collection(expected_value: &Value) -> Result<&Value, String> {
    if expected_value.is_array() {
        return Ok(expected_value);
    }

    if let Some(packages) = expected_value.get("packages") {
        return Ok(packages);
    }

    if let Some(package_data) = expected_value
        .get("files")
        .and_then(Value::as_array)
        .and_then(|files| files.first())
        .and_then(|file| file.get("package_data"))
    {
        return Ok(package_data);
    }

    Err("Expected file does not contain a package collection".to_string())
}

fn compare_json_values_parser_only(
    actual: &Value,
    expected: &Value,
    path: &str,
) -> Result<(), String> {
    const SKIP_FIELDS: &[&str] = &[
        "identifier",
        "matched_text",
        "matcher",
        "matched_length",
        "match_coverage",
        "rule_relevance",
        "rule_identifier",
        "rule_url",
        "start_line",
        "end_line",
        "extra_data",
        "package_uid",
        "datafile_paths",
        "datasource_ids",
    ];

    if SKIP_FIELDS.iter().any(|&field| path.ends_with(field)) {
        return Ok(());
    }

    fn is_tolerable_default_field(key: &str, value: &Value) -> bool {
        match value {
            Value::Null => true,
            Value::Bool(false) => true,
            Value::Array(arr) if arr.is_empty() => true,
            Value::Object(obj) if obj.is_empty() => true,
            Value::String(s) if key == "namespace" && s.is_empty() => true,
            _ => false,
        }
    }

    fn is_nullable_bool_field(path: &str) -> bool {
        path.ends_with("is_runtime")
            || path.ends_with("is_optional")
            || path.ends_with("is_pinned")
            || path.ends_with("is_direct")
            || path.ends_with("is_private")
            || path.ends_with("is_virtual")
    }

    match (actual, expected) {
        (Value::Null, Value::Null) => Ok(()),
        (Value::Null, Value::Object(obj)) if obj.is_empty() => Ok(()),
        (Value::Object(obj), Value::Null) if obj.is_empty() => Ok(()),
        (Value::Null, Value::Bool(false)) if is_nullable_bool_field(path) => Ok(()),
        (Value::Bool(false), Value::Null) if is_nullable_bool_field(path) => Ok(()),
        (Value::Null, Value::String(s)) if path.ends_with("namespace") && s.is_empty() => Ok(()),
        (Value::String(s), Value::Null) if path.ends_with("namespace") && s.is_empty() => Ok(()),
        (Value::Bool(a), Value::Bool(e)) if a == e => Ok(()),
        (Value::Number(a), Value::Number(e)) if a == e => Ok(()),
        (Value::String(a), Value::String(e)) if a == e => Ok(()),

        (Value::Array(a), Value::Array(e)) => {
            if a.len() != e.len() {
                return Err(format!(
                    "Array length mismatch at {}: actual={}, expected={}",
                    path,
                    a.len(),
                    e.len()
                ));
            }
            for (i, (actual_item, expected_item)) in a.iter().zip(e.iter()).enumerate() {
                let item_path = format!("{}[{}]", path, i);
                compare_json_values_parser_only(actual_item, expected_item, &item_path)?;
            }
            Ok(())
        }

        (Value::Object(a), Value::Object(e)) => {
            if e.is_empty() && path.ends_with("resolved_package") {
                return Ok(());
            }

            let all_keys: std::collections::HashSet<_> = a.keys().chain(e.keys()).collect();

            for key in all_keys {
                let field_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", path, key)
                };

                if SKIP_FIELDS.contains(&key.as_str()) {
                    continue;
                }

                match (a.get(key), e.get(key)) {
                    (Some(actual_val), Some(expected_val)) => {
                        compare_json_values_parser_only(actual_val, expected_val, &field_path)?;
                    }
                    (None, Some(expected_val)) => match expected_val {
                        _ if is_tolerable_default_field(key, expected_val) => continue,
                        _ => {
                            if key == "license_detections"
                                || key == "declared_license_expression"
                                || key == "declared_license_expression_spdx"
                                || key == "other_license_detections"
                                || key == "other_license_expression"
                                || key == "other_license_expression_spdx"
                            {
                                continue;
                            }
                            if !SKIP_FIELDS.contains(&key.as_str()) {
                                return Err(format!("Missing field in actual: {}", field_path));
                            }
                        }
                    },
                    (Some(_), None) => {
                        if a.get(key)
                            .is_some_and(|actual_val| is_tolerable_default_field(key, actual_val))
                        {
                            continue;
                        }
                        return Err(format!("Extra field in actual: {}", field_path));
                    }
                    (None, None) => unreachable!(),
                }
            }
            Ok(())
        }

        _ => Err(format!(
            "Type mismatch at {}: actual={:?}, expected={:?}",
            path, actual, expected
        )),
    }
}
