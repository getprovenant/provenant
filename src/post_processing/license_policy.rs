// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::license_detection::expression::{LicenseExpression, parse_expression};
use crate::models::{ComplianceAlert, FileInfo, LicensePolicyEntry, Package, TopLevelDependency};

#[derive(Debug, Deserialize)]
struct LicensePolicyFile {
    license_policies: Vec<LicensePolicyEntry>,
}

enum PolicyFileStatus {
    Ready(Vec<LicensePolicyEntry>),
    SoftError(String),
}

pub(crate) fn apply_license_policy_from_file(
    files: &mut [FileInfo],
    policy_path: &Path,
) -> Result<Vec<String>> {
    match load_license_policy(policy_path)? {
        PolicyFileStatus::Ready(policies) => {
            apply_license_policy(files, &policies)?;
            Ok(Vec::new())
        }
        PolicyFileStatus::SoftError(error) => {
            for file in files {
                file.license_policy = Some(vec![]);
            }
            Ok(vec![error])
        }
    }
}

fn load_license_policy(policy_path: &Path) -> Result<PolicyFileStatus> {
    let policy_text = fs::read_to_string(policy_path).map_err(|err| {
        anyhow!(
            "Failed to read license policy file {:?}: {err}",
            policy_path
        )
    })?;
    let policy_file: LicensePolicyFile = yaml_serde::from_str(&policy_text).map_err(|err| {
        anyhow!(
            "Failed to parse license policy file {:?}: {err}",
            policy_path
        )
    })?;

    if policy_file.license_policies.is_empty() {
        return Ok(PolicyFileStatus::SoftError(format!(
            "License policy file {:?} is empty",
            policy_path
        )));
    }

    let mut seen = BTreeSet::new();
    for policy in &policy_file.license_policies {
        if !seen.insert(policy.license_key.clone()) {
            return Ok(PolicyFileStatus::SoftError(format!(
                "License policy file {:?} contains duplicate license key {:?}",
                policy_path, policy.license_key
            )));
        }
    }

    Ok(PolicyFileStatus::Ready(policy_file.license_policies))
}

fn apply_license_policy(files: &mut [FileInfo], policies: &[LicensePolicyEntry]) -> Result<()> {
    for file in files {
        if file.file_type != crate::models::FileType::File {
            file.license_policy = Some(vec![]);
            continue;
        }
        let license_keys = file_license_keys(file)?;
        let mut matched_policies: Vec<_> = policies
            .iter()
            .filter(|policy| license_keys.contains(&policy.license_key))
            .cloned()
            .collect();
        matched_policies.sort_by(|left, right| left.license_key.cmp(&right.license_key));
        file.license_policy = Some(matched_policies);
    }

    Ok(())
}

fn file_license_keys(file: &FileInfo) -> Result<BTreeSet<String>> {
    let mut keys = BTreeSet::new();
    for detection in &file.license_detections {
        collect_license_keys(&detection.license_expression, &mut keys)?;
    }
    Ok(keys)
}

fn collect_license_keys(expression: &str, keys: &mut BTreeSet<String>) -> Result<()> {
    if expression.trim().is_empty() {
        return Ok(());
    }

    let parsed = parse_expression(expression)
        .map_err(|err| anyhow!("Failed to parse license expression {:?}: {err}", expression))?;
    collect_expression_keys(&parsed, keys);
    Ok(())
}

fn collect_expression_keys(expression: &LicenseExpression, keys: &mut BTreeSet<String>) {
    match expression {
        LicenseExpression::License(key) | LicenseExpression::LicenseRef(key) => {
            keys.insert(key.clone());
        }
        LicenseExpression::And { left, right }
        | LicenseExpression::Or { left, right }
        | LicenseExpression::With { left, right } => {
            collect_expression_keys(left, keys);
            collect_expression_keys(right, keys);
        }
    }
}

/// Count top-level packages and dependencies whose **declared** license matches a
/// policy entry with a `compliance_alert` at or above `threshold`. This lets the
/// `--fail-on` gate act on package/dependency licenses (SCA-style), not just
/// file-level detections. Returns 0 if the policy carries no severities or cannot
/// be loaded (file findings are gated separately, and an unevaluable policy under
/// `--fail-on` already fails the run before this point).
pub(crate) fn count_declared_license_policy_violations(
    policy_path: &Path,
    packages: &[Package],
    dependencies: &[TopLevelDependency],
    threshold: ComplianceAlert,
) -> Result<usize> {
    // Fail closed: a read/parse failure here (e.g. the policy file was replaced or
    // removed mid-scan) propagates as an error rather than being treated as zero
    // violations. A soft error (empty/duplicate keys) under `--fail-on` already
    // failed the run in the pipeline, so treat it as no declared violations here.
    let policies = match load_license_policy(policy_path)? {
        PolicyFileStatus::Ready(policies) => policies,
        PolicyFileStatus::SoftError(_) => return Ok(0),
    };

    let mut severity: BTreeMap<String, ComplianceAlert> = BTreeMap::new();
    for policy in &policies {
        if let Some(alert) = policy.compliance_alert {
            severity
                .entry(policy.license_key.clone())
                .and_modify(|current| {
                    if alert > *current {
                        *current = alert;
                    }
                })
                .or_insert(alert);
        }
    }
    if severity.is_empty() {
        return Ok(0);
    }

    let violates = |expression: &Option<String>| -> bool {
        expression
            .as_deref()
            .is_some_and(|expr| declared_expression_violates(expr, &severity, threshold))
    };

    let package_hits = packages
        .iter()
        .filter(|package| violates(&package.declared_license_expression))
        .count();
    let dependency_hits = dependencies
        .iter()
        .filter(|dependency| {
            dependency
                .resolved_package
                .as_ref()
                .is_some_and(|resolved| violates(&resolved.declared_license_expression))
        })
        .count();

    Ok(package_hits + dependency_hits)
}

/// True when any license key in `expression` maps to a severity at or above `threshold`.
fn declared_expression_violates(
    expression: &str,
    severity: &BTreeMap<String, ComplianceAlert>,
    threshold: ComplianceAlert,
) -> bool {
    let mut keys = BTreeSet::new();
    if collect_license_keys(expression, &mut keys).is_err() {
        return false;
    }
    keys.iter()
        .any(|key| severity.get(key).is_some_and(|alert| *alert >= threshold))
}

#[cfg(test)]
mod tests {
    use super::{apply_license_policy_from_file, declared_expression_violates};
    use crate::models::{ComplianceAlert, FileInfo, FileType, LicenseDetection};
    use std::collections::BTreeMap;

    #[test]
    fn declared_expression_violates_respects_keys_and_threshold() {
        let severity: BTreeMap<String, ComplianceAlert> = [
            ("gpl-3.0".to_string(), ComplianceAlert::Error),
            ("lgpl-2.1".to_string(), ComplianceAlert::Warning),
        ]
        .into_iter()
        .collect();

        // Error-severity key trips both thresholds.
        assert!(declared_expression_violates(
            "gpl-3.0",
            &severity,
            ComplianceAlert::Error
        ));
        // Present in a compound expression.
        assert!(declared_expression_violates(
            "mit OR gpl-3.0",
            &severity,
            ComplianceAlert::Error
        ));
        // Warning key trips `warning` but not `error`.
        assert!(declared_expression_violates(
            "lgpl-2.1",
            &severity,
            ComplianceAlert::Warning
        ));
        assert!(!declared_expression_violates(
            "lgpl-2.1",
            &severity,
            ComplianceAlert::Error
        ));
        // Unlisted / approved license never violates.
        assert!(!declared_expression_violates(
            "mit",
            &severity,
            ComplianceAlert::Warning
        ));
    }

    #[test]
    fn apply_license_policy_populates_matching_file_entries() {
        let temp = tempfile::tempdir().expect("temp dir");
        let policy_path = temp.path().join("policy.yml");
        std::fs::write(
            &policy_path,
            "license_policies:\n  - license_key: mit\n    label: Approved\n    color_code: '#00ff00'\n    icon: ok\n",
        )
        .expect("policy written");

        let mut files = vec![FileInfo::new(
            "LICENSE".to_string(),
            "LICENSE".to_string(),
            String::new(),
            "LICENSE".to_string(),
            FileType::File,
            None,
            None,
            0,
            None,
            None,
            None,
            None,
            None,
            vec![],
            Some("mit".to_string()),
            vec![LicenseDetection {
                license_expression: "mit".to_string(),
                license_expression_spdx: "MIT".to_string(),
                matches: vec![],
                detection_log: vec![],
                identifier: String::new(),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )];

        apply_license_policy_from_file(&mut files, &policy_path)
            .expect("policy application succeeds");

        let entries = files[0]
            .license_policy
            .as_ref()
            .expect("license policy present");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].license_key, "mit");
        assert_eq!(entries[0].label, "Approved");
    }

    #[test]
    fn apply_license_policy_keeps_scan_running_on_duplicate_license_keys() {
        let temp = tempfile::tempdir().expect("temp dir");
        let policy_path = temp.path().join("policy.yml");
        std::fs::write(
            &policy_path,
            "license_policies:\n  - license_key: mit\n    label: Approved\n    color_code: '#00ff00'\n    icon: ok\n  - license_key: mit\n    label: Duplicate\n    color_code: '#ff0000'\n    icon: stop\n",
        )
        .expect("policy written");

        let mut files = vec![FileInfo::new(
            "LICENSE".to_string(),
            "LICENSE".to_string(),
            String::new(),
            "LICENSE".to_string(),
            FileType::File,
            None,
            None,
            0,
            None,
            None,
            None,
            None,
            None,
            vec![],
            Some("mit".to_string()),
            vec![LicenseDetection {
                license_expression: "mit".to_string(),
                license_expression_spdx: "MIT".to_string(),
                matches: vec![],
                detection_log: vec![],
                identifier: String::new(),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )];

        let errors = apply_license_policy_from_file(&mut files, &policy_path)
            .expect("duplicate policy should not abort scan");

        assert_eq!(files[0].license_policy, Some(vec![]));
        assert!(files[0].scan_diagnostics.is_empty());
        assert!(
            errors
                .iter()
                .any(|error| error.contains("duplicate license key"))
        );
    }

    #[test]
    fn apply_license_policy_sets_empty_entries_for_directory_resources() {
        let temp = tempfile::tempdir().expect("temp dir");
        let policy_path = temp.path().join("policy.yml");
        std::fs::write(
            &policy_path,
            "license_policies:\n  - license_key: mit\n    label: Approved\n    color_code: '#00ff00'\n    icon: ok\n",
        )
        .expect("policy written");

        let mut files = vec![FileInfo::new(
            "src".to_string(),
            "src".to_string(),
            String::new(),
            "src".to_string(),
            FileType::Directory,
            None,
            None,
            0,
            None,
            None,
            None,
            None,
            None,
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )];

        apply_license_policy_from_file(&mut files, &policy_path)
            .expect("policy application succeeds");

        assert_eq!(files[0].license_policy, Some(vec![]));
    }

    #[test]
    fn apply_license_policy_sets_empty_entries_for_directories_on_soft_error() {
        let temp = tempfile::tempdir().expect("temp dir");
        let policy_path = temp.path().join("policy.yml");
        std::fs::write(
            &policy_path,
            "license_policies:\n  - license_key: mit\n    label: Approved\n    color_code: '#00ff00'\n    icon: ok\n  - license_key: mit\n    label: Duplicate\n    color_code: '#ff0000'\n    icon: stop\n",
        )
        .expect("policy written");

        let mut files = vec![FileInfo::new(
            "src".to_string(),
            "src".to_string(),
            String::new(),
            "src".to_string(),
            FileType::Directory,
            None,
            None,
            0,
            None,
            None,
            None,
            None,
            None,
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )];

        let errors = apply_license_policy_from_file(&mut files, &policy_path)
            .expect("duplicate policy should not abort scan");

        assert_eq!(files[0].license_policy, Some(vec![]));
        assert!(
            errors
                .iter()
                .any(|error| error.contains("duplicate license key"))
        );
    }
}
