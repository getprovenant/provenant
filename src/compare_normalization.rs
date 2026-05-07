// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::utils::spdx::combine_license_expressions;
use serde_json::{Map, Value, json};

pub fn normalize_text(value: &str) -> String {
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

fn normalize_compare_url_value(value: &str) -> String {
    let normalized = normalize_text(value);
    if normalized.is_empty() {
        return normalized;
    }

    let Ok(parsed) = url::Url::parse(&normalized) else {
        return normalized;
    };

    if parsed.cannot_be_a_base() {
        return normalized;
    }

    let prefix_end = normalized
        .find('?')
        .or_else(|| normalized.find('#'))
        .unwrap_or(normalized.len());
    let prefix = &normalized[..prefix_end];
    let suffix = &normalized[prefix_end..];

    let trimmed_prefix = prefix.trim_end_matches('/');
    if trimmed_prefix.is_empty() {
        normalized
    } else {
        format!("{trimmed_prefix}{suffix}")
    }
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

pub fn metric_values(entry: &Value, metric: &str) -> Vec<String> {
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
                "urls" => item
                    .get("url")
                    .and_then(Value::as_str)
                    .map(normalize_compare_url_value),
                "scan_errors" => scan_error_identity(item).map(str::to_string),
                _ => None,
            }?;
            let normalized = normalize_text(&value);
            (!normalized.is_empty()).then_some(normalized)
        })
        .collect()
}

pub fn package_identity(item: &Value) -> Option<&str> {
    item.get("purl")
        .and_then(Value::as_str)
        .or_else(|| item.get("package_url").and_then(Value::as_str))
}

fn package_metric_identity(item: &Value) -> Option<String> {
    package_identity(item)
        .map(str::to_string)
        .or_else(|| package_fallback_identity(item))
}

pub fn package_fallback_identity(item: &Value) -> Option<String> {
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

pub fn normalize_compare_path(path: &str) -> String {
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

pub fn normalize_license_expression(value: &str) -> String {
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

pub fn scalar_field_value(entry: &Value, key: &str) -> Option<String> {
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

pub fn structured_field_value(entry: &Value, key: &str) -> Option<String> {
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

pub fn classify_scalar_value(entry: &Value, key: &str) -> Option<String> {
    match entry.get(key) {
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        Some(Value::Null) | None => Some("false".to_string()),
        Some(other) => scalar_field_value(&json!({ key: other }), key),
    }
}

pub fn canonical_section_value(value: &Value, key: &str) -> Option<Value> {
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

    let mut normalized = Map::new();
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

    let mut normalized = Map::new();
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
                .unwrap_or_else(|| Value::Object(Map::new()));
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

    #[test]
    fn metric_values_ignore_trailing_slash_only_url_differences() {
        let entry = json!({
            "urls": [
                {"url": "http://mozilla.org/MPL/2.0/"},
                {"url": "https://example.com/foo/?a=1"}
            ]
        });

        assert_eq!(
            metric_values(&entry, "urls"),
            vec![
                "http://mozilla.org/MPL/2.0".to_string(),
                "https://example.com/foo?a=1".to_string()
            ]
        );
    }

    #[test]
    fn normalize_license_expression_ignores_trivial_outer_parentheses() {
        assert_eq!(
            normalize_license_expression("(MIT OR Apache-2.0)"),
            normalize_license_expression("MIT OR Apache-2.0")
        );
        assert_eq!(
            normalize_license_expression("MIT OR Apache-2.0"),
            normalize_license_expression("Apache-2.0 OR MIT")
        );
        assert_eq!(
            normalize_license_expression("MIT AND Apache-2.0"),
            normalize_license_expression("Apache-2.0 AND MIT")
        );
        assert_eq!(
            normalize_license_expression("((MIT))"),
            normalize_license_expression("MIT")
        );
    }
}
