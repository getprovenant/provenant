// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::cli::ProcessMode;
use crate::output::OutputFormat;
use crate::progress::ProgressMode;
use crate::scanner::MemoryMode;
use serde_json::{Map as JsonMap, Value as JsonValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputMode {
    Native,
    FromJson,
}

#[derive(Debug, Clone)]
pub(crate) struct OutputTarget {
    pub(crate) format: OutputFormat,
    pub(crate) file: String,
    pub(crate) custom_template: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ScanRequest {
    pub(crate) input_paths: Vec<String>,
    pub(crate) input_mode: InputMode,
    pub(crate) output_targets: Vec<OutputTarget>,
    pub(crate) output_header_options: JsonMap<String, JsonValue>,
    pub(crate) progress_mode: ProgressMode,
    pub(crate) process_mode: ProcessMode,
    pub(crate) timeout_seconds: f64,
    pub(crate) quiet: bool,
    pub(crate) verbose: bool,
    pub(crate) strip_root: bool,
    pub(crate) full_root: bool,
    pub(crate) exclude: Vec<String>,
    pub(crate) include: Vec<String>,
    pub(crate) paths_files: Vec<String>,
    pub(crate) respect_process_cache_env: bool,
    pub(crate) cache_dir: Option<String>,
    pub(crate) cache_clear: bool,
    pub(crate) incremental: bool,
    pub(crate) max_depth: usize,
    pub(crate) max_in_memory: MemoryMode,
    pub(crate) info: bool,
    pub(crate) package: bool,
    pub(crate) system_package: bool,
    pub(crate) package_in_compiled: bool,
    pub(crate) package_only: bool,
    pub(crate) no_assemble: bool,
    pub(crate) license_dataset_path: Option<String>,
    pub(crate) reindex: bool,
    pub(crate) no_license_index_cache: bool,
    pub(crate) license_text: bool,
    pub(crate) license_text_diagnostics: bool,
    pub(crate) license_diagnostics: bool,
    pub(crate) unknown_licenses: bool,
    pub(crate) no_sequence_matching: bool,
    pub(crate) license_score: u8,
    pub(crate) license_url_template: String,
    pub(crate) filter_clues: bool,
    pub(crate) ignore_author: Vec<String>,
    pub(crate) ignore_copyright_holder: Vec<String>,
    pub(crate) only_findings: bool,
    pub(crate) mark_source: bool,
    pub(crate) classify: bool,
    pub(crate) summary: bool,
    pub(crate) license_clarity_score: bool,
    pub(crate) license_references: bool,
    pub(crate) license_policy: Option<String>,
    pub(crate) tallies: bool,
    pub(crate) tallies_key_files: bool,
    pub(crate) tallies_with_details: bool,
    pub(crate) facet: Vec<String>,
    pub(crate) tallies_by_facet: bool,
    pub(crate) generated: bool,
    pub(crate) license: bool,
    pub(crate) copyright: bool,
    pub(crate) email: bool,
    pub(crate) max_email: usize,
    pub(crate) url: bool,
    pub(crate) max_url: usize,
    /// Bounds applied to untrusted input trees (set by `provenant serve`).
    /// Trusted CLI and library scans leave these unset for unchanged behavior.
    pub(crate) scan_bounds: ScanBounds,
}

/// Optional collector ceilings for untrusted scans.
///
/// Defaults are fully permissive so trusted CLI and library scans behave
/// exactly as before; `provenant serve` opts into finite values.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScanBounds {
    pub(crate) max_files: Option<usize>,
    pub(crate) max_total_bytes: Option<u64>,
    pub(crate) deadline_seconds: Option<f64>,
    pub(crate) restrict_out_of_tree_symlinks: bool,
}
