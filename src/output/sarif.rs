// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//
// SARIF 2.1.0 output. Results come from license-policy entries that carry a
// `compliance_alert` severity (see ADR 0011): each such policy match on a file
// becomes a code-scanning result. With no policy (or no severities) the run has
// zero results, so SARIF stays quiet unless the user opts into policy gating.

use std::collections::BTreeMap;
use std::io::{self, Write};

use serde_json::{Value, json};

use crate::models::ComplianceAlert;
use crate::output_schema::{Output, OutputFileInfo};

use super::shared::io_other;

const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const INFORMATION_URI: &str = "https://github.com/getprovenant/provenant";

pub(crate) fn write_sarif(output: &Output, writer: &mut dyn Write) -> io::Result<()> {
    let sarif = build_sarif(output);
    serde_json::to_writer_pretty(&mut *writer, &sarif).map_err(io_other)?;
    writer.write_all(b"\n")
}

fn level_str(alert: ComplianceAlert) -> &'static str {
    match alert {
        ComplianceAlert::Error => "error",
        ComplianceAlert::Warning => "warning",
    }
}

fn build_sarif(output: &Output) -> Value {
    let tool_version = output
        .headers
        .first()
        .map(|header| header.tool_version.clone())
        .unwrap_or_default();

    // license_key -> (label, highest severity seen), kept sorted for stable output.
    let mut rule_map: BTreeMap<String, (String, ComplianceAlert)> = BTreeMap::new();
    let mut results: Vec<Value> = Vec::new();

    for file in &output.files {
        let Some(policy_entries) = &file.license_policy else {
            continue;
        };
        for entry in policy_entries {
            let Some(alert) = entry.compliance_alert else {
                continue;
            };

            rule_map
                .entry(entry.license_key.clone())
                .and_modify(|(_, level)| {
                    if alert > *level {
                        *level = alert;
                    }
                })
                .or_insert((entry.label.clone(), alert));

            let mut physical_location = json!({
                "artifactLocation": { "uri": file.path }
            });
            if let Some((start_line, end_line)) = region_for_key(file, &entry.license_key) {
                physical_location["region"] = json!({
                    "startLine": start_line,
                    "endLine": end_line,
                });
            }

            let label = display_label(&entry.label, &entry.license_key);
            results.push(json!({
                "ruleId": entry.license_key,
                "level": level_str(alert),
                "message": {
                    "text": format!("{label}: license `{}` detected in {}", entry.license_key, file.path)
                },
                "locations": [ { "physicalLocation": physical_location } ]
            }));
        }
    }

    let rules: Vec<Value> = rule_map
        .iter()
        .map(|(key, (label, level))| {
            json!({
                "id": key,
                "name": key,
                "shortDescription": { "text": display_label(label, key) },
                "defaultConfiguration": { "level": level_str(*level) }
            })
        })
        .collect();

    json!({
        "$schema": SARIF_SCHEMA,
        "version": "2.1.0",
        "runs": [ {
            "tool": {
                "driver": {
                    "name": "Provenant",
                    "informationUri": INFORMATION_URI,
                    "version": tool_version,
                    "rules": rules
                }
            },
            "results": results
        } ]
    })
}

fn display_label<'a>(label: &'a str, license_key: &'a str) -> &'a str {
    if label.is_empty() { license_key } else { label }
}

/// Find a line region to anchor a policy result: the first license-detection match
/// on the file whose expression contains this exact license key. Tokenizing avoids
/// substring false positives (e.g. `gpl-3.0` inside `lgpl-3.0`).
fn region_for_key(file: &OutputFileInfo, license_key: &str) -> Option<(u64, u64)> {
    for detection in &file.license_detections {
        for license_match in &detection.matches {
            if expression_has_key(&license_match.license_expression, license_key) {
                return Some((license_match.start_line, license_match.end_line));
            }
        }
    }
    None
}

fn expression_has_key(expression: &str, license_key: &str) -> bool {
    expression
        .split(|character: char| character.is_whitespace() || character == '(' || character == ')')
        .any(|token| token == license_key)
}

#[cfg(test)]
mod tests {
    use super::expression_has_key;

    #[test]
    fn expression_has_key_matches_whole_tokens_only() {
        assert!(expression_has_key("gpl-3.0", "gpl-3.0"));
        assert!(expression_has_key("gpl-3.0 OR mit", "mit"));
        assert!(expression_has_key("(apache-2.0 AND gpl-3.0)", "gpl-3.0"));
        assert!(expression_has_key(
            "gpl-2.0 WITH classpath-exception-2.0",
            "gpl-2.0"
        ));
        // Must not match a substring of a different key.
        assert!(!expression_has_key("lgpl-3.0", "gpl-3.0"));
        assert!(!expression_has_key("mit OR apache-2.0", "gpl-3.0"));
    }
}
