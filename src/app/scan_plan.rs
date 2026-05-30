// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::app::request::ScanRequest;
use crate::cli::ProcessMode;
use crate::progress::ProgressMode;
use crate::scanner::{LicenseScanOptions, TextDetectionOptions};

#[derive(Debug, Clone)]
pub(crate) struct ScanPlan {
    pub(crate) progress_mode: ProgressMode,
    pub(crate) scan_names: String,
    pub(crate) text_options: TextDetectionOptions,
    pub(crate) license_options: LicenseScanOptions,
}

impl ScanPlan {
    pub(crate) fn from_request(request: &ScanRequest) -> Self {
        let enable_application_packages = request.package || request.package_only;
        let enable_system_packages = request.system_package || request.package_only;
        let enable_packages =
            enable_application_packages || enable_system_packages || request.package_in_compiled;
        let (detect_copyrights, detect_emails, detect_urls, detect_generated) =
            if request.package_only {
                (false, request.email, request.url, request.generated)
            } else {
                (
                    request.copyright,
                    request.email,
                    request.url,
                    request.generated,
                )
            };

        Self {
            progress_mode: request.progress_mode,
            scan_names: configured_scan_names(request),
            text_options: TextDetectionOptions {
                collect_info: request.info,
                detect_packages: enable_packages,
                detect_application_packages: enable_application_packages,
                detect_system_packages: enable_system_packages,
                detect_packages_in_compiled: request.package_in_compiled,
                detect_copyrights,
                detect_generated,
                detect_emails,
                detect_urls,
                max_emails: request.max_email,
                max_urls: request.max_url,
                timeout_seconds: effective_timeout_seconds(
                    request.process_mode,
                    request.timeout_seconds,
                ),
            },
            license_options: LicenseScanOptions {
                include_text: request.license_text,
                include_text_diagnostics: request.license_text_diagnostics,
                include_diagnostics: request.license_diagnostics,
                unknown_licenses: request.unknown_licenses,
                enable_sequence_matching: !request.no_sequence_matching,
                min_score: request.license_score,
            },
        }
    }
}

pub(crate) fn effective_timeout_seconds(process_mode: ProcessMode, timeout_seconds: f64) -> f64 {
    match process_mode {
        ProcessMode::SequentialWithoutTimeouts => 0.0,
        ProcessMode::Parallel(_) | ProcessMode::SequentialWithTimeouts => timeout_seconds,
    }
}

pub(crate) fn configured_scan_names(request: &ScanRequest) -> String {
    let mut names = Vec::new();
    if request.license {
        names.push("licenses");
    }
    if request.info {
        names.push("info");
    }
    if request.package {
        names.push("packages");
    }
    if (request.system_package || request.package_in_compiled || request.package_only)
        && !names.contains(&"packages")
    {
        names.push("packages");
    }
    if request.copyright {
        names.push("copyrights");
    }
    if request.email {
        names.push("emails");
    }
    if request.url {
        names.push("urls");
    }
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_timeout_seconds_supports_reference_compat_values() {
        assert_eq!(
            effective_timeout_seconds(ProcessMode::SequentialWithoutTimeouts, 30.0),
            0.0
        );
        assert_eq!(
            effective_timeout_seconds(ProcessMode::SequentialWithTimeouts, 30.0),
            30.0
        );
    }

    #[test]
    fn progress_mode_for_scan_args_maps_quiet_verbose_default() {
        let default_cli =
            crate::cli::Cli::try_parse_from(["provenant", "--json-pp", "scan.json", "sample-dir"])
                .unwrap();
        let default_request = crate::app::request::ScanRequest::from(
            default_cli
                .scan_args()
                .expect("scan args should be present"),
        );
        assert_eq!(
            default_request.progress_mode,
            crate::progress::ProgressMode::Default
        );

        let quiet_cli = crate::cli::Cli::try_parse_from([
            "provenant",
            "--json-pp",
            "scan.json",
            "--quiet",
            "sample-dir",
        ])
        .unwrap();
        let quiet_request = crate::app::request::ScanRequest::from(
            quiet_cli.scan_args().expect("scan args should be present"),
        );
        assert_eq!(
            quiet_request.progress_mode,
            crate::progress::ProgressMode::Quiet
        );

        let verbose_cli = crate::cli::Cli::try_parse_from([
            "provenant",
            "--json-pp",
            "scan.json",
            "--verbose",
            "sample-dir",
        ])
        .unwrap();
        let verbose_request = crate::app::request::ScanRequest::from(
            verbose_cli
                .scan_args()
                .expect("scan args should be present"),
        );
        assert_eq!(
            verbose_request.progress_mode,
            crate::progress::ProgressMode::Verbose
        );
    }

    #[test]
    fn configured_scan_names_keeps_license_first_and_lists_enabled_scans() {
        let cli = crate::cli::Cli::try_parse_from([
            "provenant",
            "--json-pp",
            "scan.json",
            "--license",
            "--package",
            "README.md",
        ])
        .unwrap();

        let request = crate::app::request::ScanRequest::from(
            cli.scan_args().expect("scan args should be present"),
        );
        assert_eq!(configured_scan_names(&request), "licenses, packages");
    }

    #[test]
    fn scan_plan_propagates_no_sequence_matching_into_license_options() {
        let cli = crate::cli::Cli::try_parse_from([
            "provenant",
            "--json-pp",
            "scan.json",
            "--license",
            "--no-sequence-matching",
            "sample-dir",
        ])
        .unwrap();

        let request = crate::app::request::ScanRequest::from(
            cli.scan_args().expect("scan args should be present"),
        );
        let plan = ScanPlan::from_request(&request);

        assert!(!plan.license_options.enable_sequence_matching);
    }

    #[test]
    fn scan_plan_derives_runtime_scan_configuration_from_args() {
        let cli = crate::cli::Cli::try_parse_from([
            "provenant",
            "--json-pp",
            "scan.json",
            "--license",
            "--package",
            "--email",
            "--timeout",
            "45",
            "sample-dir",
        ])
        .unwrap();

        let request = crate::app::request::ScanRequest::from(
            cli.scan_args().expect("scan args should be present"),
        );
        let plan = ScanPlan::from_request(&request);
        assert_eq!(plan.progress_mode, crate::progress::ProgressMode::Default);
        assert_eq!(plan.scan_names, "licenses, packages, emails");
        assert!(plan.text_options.detect_packages);
        assert!(plan.text_options.detect_emails);
        assert_eq!(plan.text_options.timeout_seconds, 45.0);
        assert_eq!(plan.license_options.min_score, 0);
    }
}
