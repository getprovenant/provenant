// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde_json::{Map as JsonMap, Value as JsonValue};
use std::path::{Path, PathBuf};

use crate::app::request::{InputMode, ScanRequest};
use crate::app::scan_pipeline::execute_request;
use crate::license_detection::DEFAULT_LICENSEDB_URL_TEMPLATE;
use crate::progress::ProgressMode;
use crate::scanner::MemoryMode;
use crate::{Output, ProcessMode};

#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("{0}")]
    InvalidOptions(String),
    #[error(transparent)]
    Pipeline(#[from] anyhow::Error),
}

/// Selects how the workflow facade sources license rules.
#[derive(Debug, Clone)]
pub enum LicenseSource {
    /// Skip license detection entirely.
    Disabled,
    /// Use the embedded Provenant license dataset.
    Embedded,
    /// Load a custom dataset from a directory containing the expected rules and licenses layout.
    Directory(PathBuf),
}

/// High-level configuration for in-process scans through [`scan_path`] and [`scan_paths`].
///
/// Defaults stay intentionally conservative: progress is quiet, no scan dimensions are enabled,
/// input headers are omitted, and ambient `PROVENANT_CACHE` is ignored unless you set
/// [`ScanOptions::cache_dir`].
#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub progress_mode: ProgressMode,
    pub process_mode: ProcessMode,
    pub timeout_seconds: f64,
    pub max_depth: usize,
    pub max_in_memory: MemoryMode,
    pub collect_info: bool,
    pub detect_license: LicenseSource,
    pub detect_packages: bool,
    pub detect_system_packages: bool,
    pub detect_packages_in_compiled: bool,
    pub package_only: bool,
    pub no_assemble: bool,
    pub detect_copyrights: bool,
    pub detect_emails: bool,
    pub detect_urls: bool,
    pub detect_generated: bool,
    pub max_emails: usize,
    pub max_urls: usize,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub include_input_header: bool,
    pub cache_dir: Option<PathBuf>,
    pub cache_clear: bool,
    pub incremental: bool,
    pub reindex: bool,
    pub no_license_index_cache: bool,
    pub license_text: bool,
    pub license_text_diagnostics: bool,
    pub license_diagnostics: bool,
    pub unknown_licenses: bool,
    pub no_sequence_matching: bool,
    pub license_score: u8,
    pub filter_clues: bool,
    pub ignore_author_patterns: Vec<String>,
    pub ignore_copyright_holder_patterns: Vec<String>,
    pub only_findings: bool,
    pub mark_source: bool,
    pub classify: bool,
    pub summary: bool,
    pub license_clarity_score: bool,
    pub license_references: bool,
    pub license_url_template: String,
    pub license_policy: Option<PathBuf>,
    pub tallies: bool,
    pub tallies_key_files: bool,
    pub tallies_with_details: bool,
    pub facets: Vec<String>,
    pub tallies_by_facet: bool,
    pub strip_root: bool,
    pub full_root: bool,
    pub header_options: JsonMap<String, JsonValue>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            progress_mode: ProgressMode::Quiet,
            process_mode: ProcessMode::default(),
            timeout_seconds: 120.0,
            max_depth: 0,
            max_in_memory: MemoryMode::Limit(10_000),
            collect_info: false,
            detect_license: LicenseSource::Disabled,
            detect_packages: false,
            detect_system_packages: false,
            detect_packages_in_compiled: false,
            package_only: false,
            no_assemble: false,
            detect_copyrights: false,
            detect_emails: false,
            detect_urls: false,
            detect_generated: false,
            max_emails: 50,
            max_urls: 50,
            include: Vec::new(),
            exclude: Vec::new(),
            include_input_header: false,
            cache_dir: None,
            cache_clear: false,
            incremental: false,
            reindex: false,
            no_license_index_cache: false,
            license_text: false,
            license_text_diagnostics: false,
            license_diagnostics: false,
            unknown_licenses: false,
            no_sequence_matching: false,
            license_score: 0,
            filter_clues: false,
            ignore_author_patterns: Vec::new(),
            ignore_copyright_holder_patterns: Vec::new(),
            only_findings: false,
            mark_source: false,
            classify: false,
            summary: false,
            license_clarity_score: false,
            license_references: false,
            license_url_template: DEFAULT_LICENSEDB_URL_TEMPLATE.to_string(),
            license_policy: None,
            tallies: false,
            tallies_key_files: false,
            tallies_with_details: false,
            facets: Vec::new(),
            tallies_by_facet: false,
            strip_root: false,
            full_root: false,
            header_options: JsonMap::new(),
        }
    }
}

/// Scan a single native filesystem input through the supported high-level workflow facade.
///
/// ```
/// use provenant::workflow::{scan_path, ScanOptions};
/// use std::fs;
/// use tempfile::tempdir;
///
/// let root = tempdir()?;
/// let root = root.path();
/// fs::write(root.join("README.txt"), "hello from doctest\n")?;
///
/// let output = scan_path(&root, &ScanOptions::default())?;
/// assert!(output.files.iter().any(|file| file.path.ends_with("README.txt")));
/// assert_eq!(output.headers.len(), 1);
/// assert!(!output.headers[0].options.contains_key("input"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn scan_path(path: impl AsRef<Path>, options: &ScanOptions) -> Result<Output, WorkflowError> {
    scan_paths([path.as_ref()], options)
}

/// Scan multiple native filesystem inputs in one in-process workflow run.
///
/// Absolute paths are supported as long as they can be resolved through a shared scan root by the
/// internal pipeline.
///
/// ```
/// use provenant::workflow::{scan_paths, ScanOptions};
/// use std::fs;
/// use tempfile::tempdir;
///
/// let root = tempdir()?;
/// let root = root.path();
/// let left = root.join("left");
/// let right = root.join("right");
/// fs::create_dir_all(&left)?;
/// fs::create_dir_all(&right)?;
/// fs::write(left.join("one.txt"), "left\n")?;
/// fs::write(right.join("two.txt"), "right\n")?;
///
/// let output = scan_paths([left.as_path(), right.as_path()], &ScanOptions::default())?;
/// let paths: Vec<_> = output.files.iter().map(|file| file.path.as_str()).collect();
/// assert!(paths.iter().any(|path| path.ends_with("one.txt")));
/// assert!(paths.iter().any(|path| path.ends_with("two.txt")));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn scan_paths<'a>(
    paths: impl IntoIterator<Item = &'a Path>,
    options: &ScanOptions,
) -> Result<Output, WorkflowError> {
    let input_paths: Vec<String> = paths
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    if input_paths.is_empty() {
        return Err(WorkflowError::InvalidOptions(
            "At least one input path is required".to_string(),
        ));
    }

    let request = request_for_native_paths(input_paths, options);
    validate_workflow_request(&request)?;

    execute_request(&request)
        .map(|executed| executed.output)
        .map_err(WorkflowError::Pipeline)
}

fn request_for_native_paths(input_paths: Vec<String>, options: &ScanOptions) -> ScanRequest {
    let mut header_options = options.header_options.clone();
    if options.include_input_header {
        header_options.insert(
            "input".to_string(),
            JsonValue::Array(input_paths.iter().cloned().map(JsonValue::String).collect()),
        );
    }

    let (license, license_dataset_path) = match &options.detect_license {
        LicenseSource::Disabled => (false, None),
        LicenseSource::Embedded => (true, None),
        LicenseSource::Directory(path) => (true, Some(path.to_string_lossy().to_string())),
    };

    ScanRequest {
        input_paths,
        input_mode: InputMode::Native,
        output_targets: Vec::new(),
        output_header_options: header_options,
        progress_mode: options.progress_mode,
        process_mode: options.process_mode,
        timeout_seconds: options.timeout_seconds,
        quiet: matches!(options.progress_mode, ProgressMode::Quiet),
        verbose: matches!(options.progress_mode, ProgressMode::Verbose),
        strip_root: options.strip_root,
        full_root: options.full_root,
        include: options.include.clone(),
        exclude: options.exclude.clone(),
        paths_files: Vec::new(),
        respect_process_cache_env: false,
        cache_dir: options
            .cache_dir
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        cache_clear: options.cache_clear,
        incremental: options.incremental,
        max_depth: options.max_depth,
        max_in_memory: options.max_in_memory,
        info: options.collect_info,
        package: options.detect_packages,
        system_package: options.detect_system_packages,
        package_in_compiled: options.detect_packages_in_compiled,
        package_only: options.package_only,
        no_assemble: options.no_assemble,
        license_dataset_path,
        reindex: options.reindex,
        no_license_index_cache: options.no_license_index_cache,
        license_text: options.license_text,
        license_text_diagnostics: options.license_text_diagnostics,
        license_diagnostics: options.license_diagnostics,
        unknown_licenses: options.unknown_licenses,
        no_sequence_matching: options.no_sequence_matching,
        license_score: options.license_score,
        license_url_template: options.license_url_template.clone(),
        filter_clues: options.filter_clues,
        ignore_author: options.ignore_author_patterns.clone(),
        ignore_copyright_holder: options.ignore_copyright_holder_patterns.clone(),
        only_findings: options.only_findings,
        mark_source: options.mark_source,
        classify: options.classify,
        summary: options.summary,
        license_clarity_score: options.license_clarity_score,
        license_references: options.license_references,
        license_policy: options
            .license_policy
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        tallies: options.tallies,
        tallies_key_files: options.tallies_key_files,
        tallies_with_details: options.tallies_with_details,
        facet: options.facets.clone(),
        tallies_by_facet: options.tallies_by_facet,
        generated: options.detect_generated,
        license,
        copyright: options.detect_copyrights,
        email: options.detect_emails,
        max_email: options.max_emails,
        url: options.detect_urls,
        max_url: options.max_urls,
    }
}

fn validate_workflow_request(request: &ScanRequest) -> Result<(), WorkflowError> {
    let license_enabled = request.license;

    if request.strip_root && request.full_root {
        return Err(WorkflowError::InvalidOptions(
            "strip_root and full_root are mutually exclusive".to_string(),
        ));
    }

    if request.license_text && !license_enabled {
        return Err(WorkflowError::InvalidOptions(
            "license_text requires detect_license".to_string(),
        ));
    }

    if request.license_text_diagnostics && !request.license_text {
        return Err(WorkflowError::InvalidOptions(
            "license_text_diagnostics requires license_text".to_string(),
        ));
    }

    if request.license_diagnostics && !license_enabled {
        return Err(WorkflowError::InvalidOptions(
            "license_diagnostics requires detect_license".to_string(),
        ));
    }

    if request.unknown_licenses && !license_enabled {
        return Err(WorkflowError::InvalidOptions(
            "unknown_licenses requires detect_license".to_string(),
        ));
    }

    if request.license_references && !license_enabled {
        return Err(WorkflowError::InvalidOptions(
            "license_references requires detect_license".to_string(),
        ));
    }

    if request.license_url_template != DEFAULT_LICENSEDB_URL_TEMPLATE && !license_enabled {
        return Err(WorkflowError::InvalidOptions(
            "license_url_template requires detect_license".to_string(),
        ));
    }

    if request.package_only && license_enabled {
        return Err(WorkflowError::InvalidOptions(
            "package_only cannot be combined with detect_license".to_string(),
        ));
    }

    if request.package_only && request.summary {
        return Err(WorkflowError::InvalidOptions(
            "package_only cannot be combined with summary".to_string(),
        ));
    }

    if request.package_only && request.package {
        return Err(WorkflowError::InvalidOptions(
            "package_only cannot be combined with detect_packages".to_string(),
        ));
    }

    if request.package_only && request.system_package {
        return Err(WorkflowError::InvalidOptions(
            "package_only cannot be combined with detect_system_packages".to_string(),
        ));
    }

    if request.summary && !request.classify {
        return Err(WorkflowError::InvalidOptions(
            "summary requires classify".to_string(),
        ));
    }

    if request.license_clarity_score && !request.classify {
        return Err(WorkflowError::InvalidOptions(
            "license_clarity_score requires classify".to_string(),
        ));
    }

    if request.tallies_key_files && !(request.tallies && request.classify) {
        return Err(WorkflowError::InvalidOptions(
            "tallies_key_files requires tallies and classify".to_string(),
        ));
    }

    if request.tallies_by_facet && request.facet.is_empty() {
        return Err(WorkflowError::InvalidOptions(
            "tallies_by_facet requires at least one facet definition".to_string(),
        ));
    }

    if request.tallies_by_facet && !request.tallies {
        return Err(WorkflowError::InvalidOptions(
            "tallies_by_facet requires tallies".to_string(),
        ));
    }

    if request.mark_source && !request.info {
        return Err(WorkflowError::InvalidOptions(
            "mark_source requires collect_info".to_string(),
        ));
    }

    if request.license_score > 100 {
        return Err(WorkflowError::InvalidOptions(
            "license_score must be between 0 and 100".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scan_path_requires_at_least_one_input() {
        let result = scan_paths(std::iter::empty::<&Path>(), &ScanOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn workflow_request_populates_input_header() {
        let options = ScanOptions {
            include_input_header: true,
            ..ScanOptions::default()
        };
        let request = request_for_native_paths(vec!["src".to_string()], &options);
        assert!(request.output_header_options.contains_key("input"));
    }

    #[test]
    fn workflow_validation_rejects_license_dependent_flags_without_license() {
        let options = ScanOptions {
            license_references: true,
            ..ScanOptions::default()
        };

        let request = request_for_native_paths(vec!["src".to_string()], &options);
        let error = validate_workflow_request(&request).expect_err("validation should fail");
        assert!(matches!(error, WorkflowError::InvalidOptions(_)));
        assert!(
            error
                .to_string()
                .contains("license_references requires detect_license")
        );
    }

    #[test]
    fn workflow_validation_rejects_package_only_with_regular_package_modes() {
        let options = ScanOptions {
            package_only: true,
            detect_packages: true,
            ..ScanOptions::default()
        };

        let request = request_for_native_paths(vec!["src".to_string()], &options);
        let error = validate_workflow_request(&request).expect_err("validation should fail");
        assert!(matches!(error, WorkflowError::InvalidOptions(_)));
        assert!(
            error
                .to_string()
                .contains("package_only cannot be combined with detect_packages")
        );
    }

    #[test]
    fn workflow_validation_rejects_classify_dependent_flags_without_classify() {
        let options = ScanOptions {
            summary: true,
            ..ScanOptions::default()
        };

        let request = request_for_native_paths(vec!["src".to_string()], &options);
        let error = validate_workflow_request(&request).expect_err("validation should fail");
        assert!(matches!(error, WorkflowError::InvalidOptions(_)));
        assert!(error.to_string().contains("summary requires classify"));
    }

    #[test]
    fn scan_path_runs_a_basic_in_process_scan() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(
            temp_dir.path().join("README.txt"),
            "hello from workflow facade\n",
        )
        .expect("write fixture file");

        let options = ScanOptions {
            collect_info: true,
            include_input_header: true,
            ..ScanOptions::default()
        };

        let output = scan_path(temp_dir.path(), &options).expect("workflow scan should succeed");

        assert_eq!(output.headers.len(), 1);
        assert!(!output.files.is_empty());
        assert!(output.headers[0].options.contains_key("input"));
    }

    #[test]
    fn scan_paths_supports_multiple_absolute_inputs() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let left = temp_dir.path().join("left");
        let right = temp_dir.path().join("right");
        fs::create_dir_all(&left).expect("create left dir");
        fs::create_dir_all(&right).expect("create right dir");
        fs::write(left.join("one.txt"), "left\n").expect("write left fixture");
        fs::write(right.join("two.txt"), "right\n").expect("write right fixture");

        let output = scan_paths([left.as_path(), right.as_path()], &ScanOptions::default())
            .expect("workflow scan should succeed for multiple absolute inputs");

        assert!(
            output
                .files
                .iter()
                .any(|file| file.path.ends_with("one.txt"))
        );
        assert!(
            output
                .files
                .iter()
                .any(|file| file.path.ends_with("two.txt"))
        );
    }
}
