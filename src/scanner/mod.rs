// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

mod collect;
pub(crate) mod process;

use crate::license_detection::LicenseDetectionEngine;
use crate::models::FileInfo;

pub struct ProcessResult {
    pub files: Vec<FileInfo>,
    pub excluded_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct LicenseScanOptions {
    pub include_text: bool,
    pub include_text_diagnostics: bool,
    pub include_diagnostics: bool,
    pub unknown_licenses: bool,
    pub enable_sequence_matching: bool,
    pub min_score: u8,
}

impl Default for LicenseScanOptions {
    fn default() -> Self {
        Self {
            include_text: false,
            include_text_diagnostics: false,
            include_diagnostics: false,
            unknown_licenses: false,
            enable_sequence_matching: true,
            min_score: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextDetectionOptions {
    pub collect_info: bool,
    pub detect_packages: bool,
    pub detect_application_packages: bool,
    pub detect_system_packages: bool,
    pub detect_packages_in_compiled: bool,
    pub detect_copyrights: bool,
    pub detect_generated: bool,
    pub detect_emails: bool,
    pub detect_urls: bool,
    pub max_emails: usize,
    pub max_urls: usize,
    pub timeout_seconds: f64,
}

impl Default for TextDetectionOptions {
    fn default() -> Self {
        Self {
            collect_info: false,
            detect_packages: false,
            detect_application_packages: false,
            detect_system_packages: false,
            detect_packages_in_compiled: false,
            detect_copyrights: true,
            detect_generated: false,
            detect_emails: false,
            detect_urls: false,
            max_emails: 50,
            max_urls: 50,
            timeout_seconds: 120.0,
        }
    }
}

pub fn scan_options_fingerprint(
    text_options: &TextDetectionOptions,
    license_options: LicenseScanOptions,
    license_engine: Option<&LicenseDetectionEngine>,
) -> String {
    let (license_enabled, rules_count, first_rule_id, last_rule_id) = match license_engine {
        Some(engine) => {
            let rules = &engine.index().rules_by_rid;
            (
                true,
                rules.len(),
                rules
                    .first()
                    .map(|rule| rule.identifier.as_str())
                    .unwrap_or(""),
                rules
                    .last()
                    .map(|rule| rule.identifier.as_str())
                    .unwrap_or(""),
            )
        }
        None => (false, 0, "", ""),
    };

    format!(
        "tool_version={};info={};packages={};app_packages={};system_packages={};compiled_packages={};copyrights={};generated={};emails={};urls={};max_emails={};max_urls={};timeout={:.6};license_enabled={};rules_count={};first_rule_id={};last_rule_id={};license_text={};license_text_diagnostics={};license_diagnostics={};unknown_licenses={};sequence_matching={};license_score={}",
        crate::version::BUILD_VERSION,
        text_options.collect_info,
        text_options.detect_packages,
        text_options.detect_application_packages,
        text_options.detect_system_packages,
        text_options.detect_packages_in_compiled,
        text_options.detect_copyrights,
        text_options.detect_generated,
        text_options.detect_emails,
        text_options.detect_urls,
        text_options.max_emails,
        text_options.max_urls,
        text_options.timeout_seconds,
        license_enabled,
        rules_count,
        first_rule_id,
        last_rule_id,
        license_options.include_text,
        license_options.include_text_diagnostics,
        license_options.include_diagnostics,
        license_options.unknown_licenses,
        license_options.enable_sequence_matching,
        license_options.min_score,
    )
}

pub use self::collect::{
    CollectedPaths, CollectionFrontier, collect_paths, collect_selected_paths,
};
#[allow(unused_imports)]
pub use self::process::{
    MemoryMode, process_collected, process_collected_sequential,
    process_collected_with_memory_limit, process_collected_with_memory_limit_sequential,
};

#[cfg(test)]
mod tests;
