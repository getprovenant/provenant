// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use liblzma::read::XzDecoder;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use tar::Archive;
use tempfile::TempDir;
use url::Url;
use zip::ZipArchive;

use crate::ProcessMode;
use crate::serve_api::{ServeLicenseSource, ServeScanInput, ServeScanOptions, ServeScanRequest};
use crate::workflow::{LicenseSource, ScanOptions, WorkflowError, scan_paths};

impl From<ServeLicenseSource> for LicenseSource {
    fn from(source: ServeLicenseSource) -> Self {
        match source {
            ServeLicenseSource::Disabled => LicenseSource::Disabled,
            ServeLicenseSource::Embedded => LicenseSource::Embedded,
            ServeLicenseSource::Directory { path } => LicenseSource::Directory(PathBuf::from(path)),
        }
    }
}

impl From<ServeScanOptions> for ScanOptions {
    fn from(options: ServeScanOptions) -> Self {
        Self {
            collect_info: options.collect_info,
            detect_license: LicenseSource::from(options.detect_license),
            detect_packages: options.detect_packages,
            detect_system_packages: options.detect_system_packages,
            detect_packages_in_compiled: options.detect_packages_in_compiled,
            detect_copyrights: options.detect_copyrights,
            detect_emails: options.detect_emails,
            detect_urls: options.detect_urls,
            detect_generated: options.detect_generated,
            include: options.include,
            exclude: options.exclude,
            strip_root: options.strip_root,
            full_root: options.full_root,
            license_text: options.license_text,
            license_text_diagnostics: options.license_text_diagnostics,
            license_diagnostics: options.license_diagnostics,
            unknown_licenses: options.unknown_licenses,
            no_sequence_matching: options.no_sequence_matching,
            license_score: options.license_score,
            only_findings: options.only_findings,
            mark_source: options.mark_source,
            classify: options.classify,
            summary: options.summary,
            license_clarity_score: options.license_clarity_score,
            license_references: options.license_references,
            tallies: options.tallies,
            tallies_key_files: options.tallies_key_files,
            tallies_with_details: options.tallies_with_details,
            facets: options.facets,
            tallies_by_facet: options.tallies_by_facet,
            ..Self::default()
        }
    }
}

type Result<T> = std::result::Result<T, IngestError>;

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REDIRECTS: usize = 5;

// Overall per-scan ceilings for the untrusted serve lane. These are
// intentionally generous so realistic projects scan unchanged, while still
// bounding hostile inputs (e.g. a repository of millions of tiny files) that
// would otherwise run effectively unbounded on the single sync-scan worker.
const SERVE_MAX_SCAN_FILES: usize = 500_000;
const SERVE_MAX_SCAN_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const SERVE_SCAN_DEADLINE_SECONDS: f64 = 300.0;

const MAX_REMOTE_INPUT_BYTES: u64 = 100 * 1024 * 1024;
const MAX_UPLOADED_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARCHIVE_ENTRY_BYTES: u64 = 50 * 1024 * 1024;
const MAX_ARCHIVE_TOTAL_BYTES: u64 = 100 * 1024 * 1024;
const MAX_ARCHIVE_ENTRY_COUNT: usize = 10_000;
const ARCHIVE_LIMITS: ArchiveExtractionLimits = ArchiveExtractionLimits {
    max_entry_bytes: MAX_ARCHIVE_ENTRY_BYTES,
    max_total_bytes: MAX_ARCHIVE_TOTAL_BYTES,
    max_entry_count: MAX_ARCHIVE_ENTRY_COUNT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct IngestPolicy {
    privileged_inputs: PrivilegedInputPolicy,
}

impl IngestPolicy {
    pub(super) fn allow_privileged_inputs() -> Self {
        Self {
            privileged_inputs: PrivilegedInputPolicy::Allowed,
        }
    }

    pub(super) fn upload_only() -> Self {
        Self {
            privileged_inputs: PrivilegedInputPolicy::UploadOnly,
        }
    }

    pub(super) fn privileged_inputs_allowed(self) -> bool {
        self.privileged_inputs == PrivilegedInputPolicy::Allowed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivilegedInputPolicy {
    Allowed,
    UploadOnly,
}

#[derive(Debug, Clone, Copy)]
struct ArchiveExtractionLimits {
    max_entry_bytes: u64,
    max_total_bytes: u64,
    max_entry_count: usize,
}

#[derive(Debug, Default)]
struct ArchiveExtractionState {
    extracted_files: usize,
    extracted_bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum IngestError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    PayloadTooLarge(String),
    #[error("{message}")]
    Upstream {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },
    #[error("{message}")]
    Internal {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },
}

impl IngestError {
    fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    fn upstream(msg: impl Into<String>) -> Self {
        Self::Upstream {
            message: msg.into(),
            source: None,
        }
    }

    fn upstream_with_source(
        source: impl std::error::Error + Send + Sync + 'static,
        msg: impl Into<String>,
    ) -> Self {
        Self::Upstream {
            message: msg.into(),
            source: Some(anyhow::Error::new(source)),
        }
    }

    fn internal_with_source(
        source: impl std::error::Error + Send + Sync + 'static,
        msg: impl Into<String>,
    ) -> Self {
        Self::Internal {
            message: msg.into(),
            source: Some(anyhow::Error::new(source)),
        }
    }
}

impl ServeScanInput {
    fn prepare(self, policy: IngestPolicy) -> Result<(Vec<PathBuf>, Option<TempDir>)> {
        validate_input_policy(&self, policy)?;
        match self {
            Self::Paths { paths } => prepare_paths_input(paths),
            Self::Repository { url, reference } => prepare_repository_input(&url, &reference),
            Self::Url { url } => prepare_url_input(&url),
            Self::Upload {
                filename,
                content_base64,
            } => prepare_upload_input(&filename, &content_base64),
        }
    }
}

pub(super) fn validate_input_policy(input: &ServeScanInput, policy: IngestPolicy) -> Result<()> {
    if policy.privileged_inputs_allowed() {
        return Ok(());
    }

    let input_type = match input {
        ServeScanInput::Paths { .. } => "paths",
        ServeScanInput::Repository { .. } => "repository",
        ServeScanInput::Url { .. } => "url",
        ServeScanInput::Upload { .. } => return Ok(()),
    };

    Err(IngestError::validation(format!(
        "input.type = \"{input_type}\" requires --allow-privileged-inputs when provenant serve is bound beyond localhost"
    )))
}

fn prepare_paths_input(paths: Vec<String>) -> Result<(Vec<PathBuf>, Option<TempDir>)> {
    if paths.is_empty() {
        return Err(IngestError::validation(
            "input.paths must contain at least one path",
        ));
    }

    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    for path in &paths {
        if !path.exists() {
            return Err(IngestError::validation(format!(
                "input path does not exist: {}",
                path.display()
            )));
        }
    }

    Ok((paths, None))
}

fn prepare_repository_input(url: &str, reference: &str) -> Result<(Vec<PathBuf>, Option<TempDir>)> {
    if url.trim().is_empty() {
        return Err(IngestError::validation("repository.url must not be empty"));
    }
    if reference.trim().is_empty() {
        return Err(IngestError::validation("repository.ref must not be empty"));
    }

    let staging_dir = TempDir::new().map_err(|e| {
        IngestError::internal_with_source(e, "failed to create repository staging directory")
    })?;
    let repo_dir = staging_dir.path().join("repository");

    run_git(
        Command::new("git").arg("init").arg(&repo_dir),
        "failed to initialize repository staging checkout",
    )?;
    run_git(
        Command::new("git")
            .current_dir(&repo_dir)
            .args(["remote", "add", "origin", url]),
        "failed to configure repository staging remote",
    )?;
    run_git(
        Command::new("git")
            .current_dir(&repo_dir)
            .args(["fetch", "--depth", "1", "origin", reference]),
        "failed to fetch repository ref for remote ingestion",
    )?;
    run_git(
        Command::new("git")
            .current_dir(&repo_dir)
            .args(["checkout", "--detach", "FETCH_HEAD"]),
        "failed to checkout fetched repository ref",
    )?;

    Ok((vec![repo_dir], Some(staging_dir)))
}

fn prepare_url_input(url: &str) -> Result<(Vec<PathBuf>, Option<TempDir>)> {
    if url.trim().is_empty() {
        return Err(IngestError::validation("url.url must not be empty"));
    }

    let parsed_url =
        Url::parse(url).map_err(|_| IngestError::validation("url.url must be a valid URL"))?;
    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err(IngestError::validation("url.url must use http or https"));
    }

    let staging_dir = TempDir::new().map_err(|e| {
        IngestError::internal_with_source(e, "failed to create URL staging directory")
    })?;
    let download_dir = staging_dir.path().join("download");
    fs::create_dir_all(&download_dir).map_err(|e| {
        IngestError::internal_with_source(e, format!("failed to create {}", download_dir.display()))
    })?;

    let artifact_path = download_remote_input(url, &download_dir)?;
    materialize_downloaded_artifact(staging_dir, artifact_path)
}

fn prepare_upload_input(
    filename: &str,
    content_base64: &str,
) -> Result<(Vec<PathBuf>, Option<TempDir>)> {
    let normalized_filename = validate_upload_filename(filename)?;
    let decoded = STANDARD
        .decode(content_base64)
        .map_err(|_| IngestError::validation("upload.content_base64 must be valid base64"))?;

    if decoded.is_empty() {
        return Err(IngestError::validation(
            "upload.content_base64 must not decode to an empty payload",
        ));
    }

    if decoded.len() > MAX_UPLOADED_INPUT_BYTES {
        return Err(IngestError::PayloadTooLarge(format!(
            "upload payload exceeds max size of {} bytes",
            MAX_UPLOADED_INPUT_BYTES
        )));
    }

    let staging_dir = TempDir::new().map_err(|e| {
        IngestError::internal_with_source(e, "failed to create upload staging directory")
    })?;
    let upload_dir = staging_dir.path().join("upload");
    fs::create_dir_all(&upload_dir).map_err(|e| {
        IngestError::internal_with_source(e, format!("failed to create {}", upload_dir.display()))
    })?;

    let artifact_path = upload_dir.join(normalized_filename);
    fs::write(&artifact_path, decoded).map_err(|e| {
        IngestError::internal_with_source(e, format!("failed to write {}", artifact_path.display()))
    })?;

    materialize_downloaded_artifact(staging_dir, artifact_path)
}

fn materialize_downloaded_artifact(
    staging_dir: TempDir,
    artifact_path: PathBuf,
) -> Result<(Vec<PathBuf>, Option<TempDir>)> {
    let artifact_name = artifact_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("downloaded");

    if let Some(format) = detect_archive_format(artifact_name) {
        let extract_dir = staging_dir.path().join("extracted");
        fs::create_dir_all(&extract_dir).map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to create {}", extract_dir.display()),
            )
        })?;
        extract_archive(&artifact_path, &extract_dir, format)?;
        return Ok((vec![extract_dir], Some(staging_dir)));
    }

    Ok((vec![artifact_path], Some(staging_dir)))
}

fn download_remote_input(url: &str, output_dir: &Path) -> Result<PathBuf> {
    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(DOWNLOAD_TIMEOUT)
        .redirect(Policy::limited(MAX_REDIRECTS))
        .build()
        .map_err(|e| {
            IngestError::internal_with_source(e, "failed to build remote-ingestion HTTP client")
        })?;

    let mut response = client
        .get(url)
        .header("User-Agent", "provenant-serve/remote-ingestion")
        .send()
        .map_err(|e| {
            IngestError::upstream_with_source(e, format!("failed to fetch remote input from {url}"))
        })?
        .error_for_status()
        .map_err(|e| {
            IngestError::upstream_with_source(
                e,
                format!("remote input fetch returned an error status for {url}"),
            )
        })?;

    if response
        .content_length()
        .is_some_and(|content_length| content_length > MAX_REMOTE_INPUT_BYTES)
    {
        return Err(IngestError::PayloadTooLarge(format!(
            "remote input exceeds max size of {} bytes",
            MAX_REMOTE_INPUT_BYTES
        )));
    }

    let filename = derive_download_filename(response.url());
    let output_path = output_dir.join(filename);
    let mut output_file = File::create(&output_path).map_err(|e| {
        IngestError::internal_with_source(e, format!("failed to create {}", output_path.display()))
    })?;

    let mut total_bytes = 0u64;
    let mut buffer = [0u8; 8192];
    loop {
        let read_bytes = response.read(&mut buffer).map_err(|e| {
            IngestError::upstream_with_source(e, format!("failed to read remote input from {url}"))
        })?;
        if read_bytes == 0 {
            break;
        }
        total_bytes += read_bytes as u64;
        if total_bytes > MAX_REMOTE_INPUT_BYTES {
            return Err(IngestError::PayloadTooLarge(format!(
                "remote input exceeds max size of {} bytes",
                MAX_REMOTE_INPUT_BYTES
            )));
        }
        output_file.write_all(&buffer[..read_bytes]).map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to write {}", output_path.display()),
            )
        })?;
    }

    Ok(output_path)
}

fn derive_download_filename(url: &Url) -> String {
    let candidate = url
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .filter(|segment| !segment.trim().is_empty())
        .unwrap_or("downloaded");
    validate_upload_filename(candidate).unwrap_or_else(|_| "downloaded".to_string())
}

fn validate_upload_filename(filename: &str) -> Result<String> {
    let path = Path::new(filename);
    let mut components = path.components();
    let Some(Component::Normal(component)) = components.next() else {
        return Err(IngestError::validation(
            "upload.filename must be a simple file name",
        ));
    };
    if components.next().is_some() {
        return Err(IngestError::validation(
            "upload.filename must be a simple file name",
        ));
    }
    let normalized = component
        .to_str()
        .ok_or_else(|| IngestError::validation("upload.filename must be valid UTF-8"))?;
    if normalized.trim().is_empty() {
        return Err(IngestError::validation("upload.filename must not be empty"));
    }
    Ok(normalized.to_string())
}

enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
}

fn detect_archive_format(filename: &str) -> Option<ArchiveFormat> {
    let filename = filename.to_ascii_lowercase();
    if filename.ends_with(".zip") {
        return Some(ArchiveFormat::Zip);
    }
    if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
        return Some(ArchiveFormat::TarGz);
    }
    if filename.ends_with(".tar.bz2") {
        return Some(ArchiveFormat::TarBz2);
    }
    if filename.ends_with(".tar.xz") {
        return Some(ArchiveFormat::TarXz);
    }
    if filename.ends_with(".tar") {
        return Some(ArchiveFormat::Tar);
    }
    None
}

fn extract_archive(archive_path: &Path, output_dir: &Path, format: ArchiveFormat) -> Result<()> {
    let file = File::open(archive_path).map_err(|e| {
        IngestError::internal_with_source(e, format!("failed to open {}", archive_path.display()))
    })?;

    match format {
        ArchiveFormat::Zip => extract_zip_archive(archive_path, file, output_dir),
        ArchiveFormat::TarGz => extract_tar_archive(archive_path, GzDecoder::new(file), output_dir),
        ArchiveFormat::TarBz2 => {
            extract_tar_archive(archive_path, BzDecoder::new(file), output_dir)
        }
        ArchiveFormat::TarXz => extract_tar_archive(archive_path, XzDecoder::new(file), output_dir),
        ArchiveFormat::Tar => extract_tar_archive(archive_path, file, output_dir),
    }
}

fn extract_entry(
    output_dir: &Path,
    relative_path: &Path,
    declared_entry_size: u64,
    state: &mut ArchiveExtractionState,
    entry_reader: &mut dyn std::io::Read,
    limits: ArchiveExtractionLimits,
    archive_context: &str,
) -> Result<()> {
    enforce_archive_entry_start(state, declared_entry_size, limits)?;

    let destination = output_dir.join(relative_path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            IngestError::internal_with_source(e, format!("failed to create {}", parent.display()))
        })?;
    }
    let mut output = File::create(&destination).map_err(|e| {
        IngestError::internal_with_source(e, format!("failed to create {}", destination.display()))
    })?;
    copy_archive_entry_with_limits(
        entry_reader,
        &mut output,
        &mut state.extracted_bytes,
        limits,
        archive_context,
        relative_path,
    )?;

    Ok(())
}

fn extract_zip_archive(archive_path: &Path, file: File, output_dir: &Path) -> Result<()> {
    let mut archive = ZipArchive::new(file).map_err(|e| {
        IngestError::internal_with_source(
            e,
            format!("failed to read zip archive {}", archive_path.display()),
        )
    })?;

    let mut state = ArchiveExtractionState::default();
    let archive_context = format!("{}/", archive_path.display());

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| {
            IngestError::internal_with_source(e, format!("failed to read zip entry {index}"))
        })?;
        let Some(relative_path) = normalize_archive_path(Path::new(entry.name())) else {
            continue;
        };
        if entry.is_dir() {
            fs::create_dir_all(output_dir.join(relative_path)).map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!(
                        "failed to create archive directory in {}",
                        output_dir.display()
                    ),
                )
            })?;
            continue;
        }

        extract_entry(
            output_dir,
            &relative_path,
            entry.size(),
            &mut state,
            &mut entry,
            ARCHIVE_LIMITS,
            &archive_context,
        )?;
    }

    if state.extracted_files == 0 {
        return Err(IngestError::validation(
            "archive did not contain any safe files to scan",
        ));
    }

    Ok(())
}

fn extract_tar_archive<R: Read>(archive_path: &Path, reader: R, output_dir: &Path) -> Result<()> {
    let mut archive = Archive::new(reader);
    let mut state = ArchiveExtractionState::default();
    let archive_context = format!("{}/", archive_path.display());

    for entry in archive.entries().map_err(|e| {
        IngestError::internal_with_source(
            e,
            format!("failed to enumerate tar archive {}", archive_path.display()),
        )
    })? {
        let mut entry = entry.map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to read tar entry in {}", archive_path.display()),
            )
        })?;
        let entry_path = entry.path().map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to read tar path in {}", archive_path.display()),
            )
        })?;
        let Some(relative_path) = normalize_archive_path(&entry_path) else {
            continue;
        };

        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(output_dir.join(relative_path)).map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!(
                        "failed to create archive directory in {}",
                        output_dir.display()
                    ),
                )
            })?;
            continue;
        }

        if !entry.header().entry_type().is_file() {
            continue;
        }

        let entry_size = entry.header().size().map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to read tar size in {}", archive_path.display()),
            )
        })?;

        extract_entry(
            output_dir,
            &relative_path,
            entry_size,
            &mut state,
            &mut entry,
            ARCHIVE_LIMITS,
            &archive_context,
        )?;
    }

    if state.extracted_files == 0 {
        return Err(IngestError::validation(
            "archive did not contain any safe files to scan",
        ));
    }

    Ok(())
}

fn enforce_archive_entry_start(
    state: &mut ArchiveExtractionState,
    declared_entry_size: u64,
    limits: ArchiveExtractionLimits,
) -> Result<()> {
    if declared_entry_size > limits.max_entry_bytes {
        return Err(IngestError::PayloadTooLarge(format!(
            "archive entry exceeds max size of {} bytes",
            limits.max_entry_bytes
        )));
    }

    state.extracted_files += 1;
    if state.extracted_files > limits.max_entry_count {
        return Err(IngestError::PayloadTooLarge(format!(
            "archive exceeds max entry count of {}",
            limits.max_entry_count
        )));
    }

    Ok(())
}

fn copy_archive_entry_with_limits(
    entry_reader: &mut dyn std::io::Read,
    output: &mut dyn Write,
    extracted_bytes: &mut u64,
    limits: ArchiveExtractionLimits,
    archive_context: &str,
    relative_path: &Path,
) -> Result<u64> {
    let mut entry_bytes = 0u64;
    let mut buffer = [0u8; 8192];

    loop {
        let read_bytes = entry_reader.read(&mut buffer).map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!(
                    "failed to extract {}{}",
                    archive_context,
                    relative_path.display()
                ),
            )
        })?;
        if read_bytes == 0 {
            break;
        }

        let read_bytes = read_bytes as u64;
        let next_entry_bytes = entry_bytes
            .checked_add(read_bytes)
            .ok_or_else(|| archive_entry_too_large(limits))?;
        if next_entry_bytes > limits.max_entry_bytes {
            return Err(archive_entry_too_large(limits));
        }

        let next_extracted_bytes = extracted_bytes
            .checked_add(read_bytes)
            .ok_or_else(|| archive_total_too_large(limits))?;
        if next_extracted_bytes > limits.max_total_bytes {
            return Err(archive_total_too_large(limits));
        }

        output
            .write_all(&buffer[..read_bytes as usize])
            .map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!(
                        "failed to extract {}{}",
                        archive_context,
                        relative_path.display()
                    ),
                )
            })?;
        entry_bytes = next_entry_bytes;
        *extracted_bytes = next_extracted_bytes;
    }

    Ok(entry_bytes)
}

fn archive_entry_too_large(limits: ArchiveExtractionLimits) -> IngestError {
    IngestError::PayloadTooLarge(format!(
        "archive entry exceeds max size of {} bytes",
        limits.max_entry_bytes
    ))
}

fn archive_total_too_large(limits: ArchiveExtractionLimits) -> IngestError {
    IngestError::PayloadTooLarge(format!(
        "archive exceeds max extracted size of {} bytes",
        limits.max_total_bytes
    ))
}

fn normalize_archive_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => continue,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

fn run_git(command: &mut Command, context_message: &str) -> Result<()> {
    let output = command
        .output()
        .map_err(|e| IngestError::upstream_with_source(e, context_message))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(IngestError::upstream(format!(
            "{}: {}",
            context_message,
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ScanError {
    #[error(transparent)]
    Workflow(#[from] WorkflowError),
    #[error("{0}")]
    Serialization(String),
}

#[derive(Debug)]
pub(super) struct SyncScanExecution {
    paths: Vec<PathBuf>,
    options: ScanOptions,
    _staging_dir: Option<TempDir>,
}

impl SyncScanExecution {
    pub(super) fn new(request: ServeScanRequest, policy: IngestPolicy) -> Result<Self> {
        let (paths, _staging_dir) = request.input.prepare(policy)?;

        let mut options = ScanOptions::from(request.options);
        if _staging_dir.is_some() {
            options.strip_root = true;
            options.full_root = false;
        }

        // Every serve request handles untrusted input, so bound the walk and
        // refuse to dereference symlinks that escape the scan root.
        options.max_files = Some(SERVE_MAX_SCAN_FILES);
        options.max_total_bytes = Some(SERVE_MAX_SCAN_TOTAL_BYTES);
        options.scan_deadline_seconds = Some(SERVE_SCAN_DEADLINE_SECONDS);
        options.restrict_out_of_tree_symlinks = true;

        Ok(Self {
            paths,
            options,
            _staging_dir,
        })
    }

    pub(super) fn execute(self) -> std::result::Result<String, ScanError> {
        let output = scan_paths(self.paths.iter().map(|p| p.as_path()), &self.options)?;
        serde_json::to_string(&crate::output_schema::Output::from(&output))
            .map_err(|e| ScanError::Serialization(format!("scan result should serialize: {e}")))
    }

    pub(super) fn run_async(
        mut self,
        allocated_processors: usize,
    ) -> std::result::Result<String, ScanError> {
        self.options.process_mode = if allocated_processors <= 1 {
            ProcessMode::SequentialWithTimeouts
        } else {
            ProcessMode::Parallel(allocated_processors)
        };
        self.execute()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_filename_must_be_simple() {
        let error = validate_upload_filename("nested/file.txt")
            .expect_err("nested upload filename should fail");
        assert!(error.to_string().contains("simple file name"));
    }

    #[test]
    fn url_input_rejects_non_http_scheme() {
        let error =
            prepare_url_input("file:///tmp/input.txt").expect_err("non-http URL input should fail");
        assert!(error.to_string().contains("http or https"));
    }

    #[test]
    fn sync_scan_execution_rejects_empty_paths() {
        use crate::serve_api::{ServeScanInput, ServeScanOptions, ServeScanRequest};

        let error = SyncScanExecution::new(
            ServeScanRequest {
                input: ServeScanInput::Paths { paths: Vec::new() },
                options: ServeScanOptions::default(),
            },
            IngestPolicy::allow_privileged_inputs(),
        )
        .expect_err("empty paths should fail");

        assert!(
            error
                .to_string()
                .contains("input.paths must contain at least one path")
        );
    }

    #[test]
    fn sync_scan_execution_applies_untrusted_scan_bounds() {
        use crate::serve_api::{ServeScanInput, ServeScanOptions, ServeScanRequest};

        let temp_dir = TempDir::new().expect("temp dir");
        fs::write(temp_dir.path().join("file.txt"), "content\n").expect("write file");

        let execution = SyncScanExecution::new(
            ServeScanRequest {
                input: ServeScanInput::Paths {
                    paths: vec![temp_dir.path().to_string_lossy().to_string()],
                },
                options: ServeScanOptions::default(),
            },
            IngestPolicy::allow_privileged_inputs(),
        )
        .expect("scan execution should build");

        assert_eq!(execution.options.max_files, Some(SERVE_MAX_SCAN_FILES));
        assert_eq!(
            execution.options.max_total_bytes,
            Some(SERVE_MAX_SCAN_TOTAL_BYTES)
        );
        assert_eq!(
            execution.options.scan_deadline_seconds,
            Some(SERVE_SCAN_DEADLINE_SECONDS)
        );
        assert!(execution.options.restrict_out_of_tree_symlinks);
    }

    #[cfg(unix)]
    #[test]
    fn serve_scan_does_not_emit_out_of_tree_symlink_target() {
        use crate::serve_api::{ServeScanInput, ServeScanOptions, ServeScanRequest};
        use std::os::unix::fs::symlink;

        let outside_dir = TempDir::new().expect("outside temp dir");
        let secret = outside_dir.path().join("secret.txt");
        fs::write(&secret, "SERVE_SECRET_MARKER\n").expect("write out-of-tree secret");

        let scan_dir = TempDir::new().expect("scan temp dir");
        fs::write(scan_dir.path().join("inside.txt"), "ordinary\n").expect("write inside file");
        symlink(&secret, scan_dir.path().join("creds")).expect("create escaping symlink");

        let execution = SyncScanExecution::new(
            ServeScanRequest {
                input: ServeScanInput::Paths {
                    paths: vec![scan_dir.path().to_string_lossy().to_string()],
                },
                options: ServeScanOptions {
                    collect_info: true,
                    ..ServeScanOptions::default()
                },
            },
            IngestPolicy::allow_privileged_inputs(),
        )
        .expect("scan execution should build");

        let body = execution.execute().expect("serve scan should succeed");

        // The escaping symlink and the out-of-tree target name must not surface.
        assert!(
            !body.contains("creds"),
            "escaping symlink should not be scanned"
        );
        assert!(
            body.contains("inside.txt"),
            "in-tree file should be scanned"
        );
    }

    #[test]
    fn upload_input_rejects_invalid_base64() {
        let error = prepare_upload_input("input.txt", "%%%%")
            .expect_err("invalid base64 upload should fail");
        assert!(error.to_string().contains("valid base64"));
    }

    #[test]
    fn normalize_archive_path_rejects_parent_dirs() {
        assert!(normalize_archive_path(Path::new("../escape.txt")).is_none());
    }

    #[test]
    fn normalize_archive_path_strips_cur_dir_components() {
        let result = normalize_archive_path(Path::new("./foo.txt"));
        assert_eq!(result.as_deref(), Some(Path::new("foo.txt")));
    }

    #[test]
    fn detect_archive_format_returns_none_for_unsupported() {
        assert!(detect_archive_format("data.rar").is_none());
        assert!(detect_archive_format("readme.txt").is_none());
    }

    #[test]
    fn restricted_policy_rejects_privileged_input_types() {
        let policy = IngestPolicy::upload_only();
        let paths = ServeScanInput::Paths {
            paths: vec![".".to_string()],
        };
        let url = ServeScanInput::Url {
            url: "https://example.com/archive.zip".to_string(),
        };
        let repository = ServeScanInput::Repository {
            url: "https://example.com/repo.git".to_string(),
            reference: "main".to_string(),
        };

        for input in [paths, url, repository] {
            let error = validate_input_policy(&input, policy)
                .expect_err("privileged input should require explicit opt-in");
            assert!(error.to_string().contains("--allow-privileged-inputs"));
        }
    }

    #[test]
    fn restricted_policy_allows_upload_input() {
        let upload = ServeScanInput::Upload {
            filename: "archive.zip".to_string(),
            content_base64: "Zm9v".to_string(),
        };

        validate_input_policy(&upload, IngestPolicy::upload_only())
            .expect("upload input should not require privileged opt-in");
    }

    #[test]
    fn copy_archive_entry_enforces_runtime_entry_bytes() {
        let limits = ArchiveExtractionLimits {
            max_entry_bytes: 4,
            max_total_bytes: 100,
            max_entry_count: 10,
        };
        let mut reader = std::io::Cursor::new(b"12345");
        let mut output = Vec::new();
        let mut extracted_bytes = 0;

        let error = copy_archive_entry_with_limits(
            &mut reader,
            &mut output,
            &mut extracted_bytes,
            limits,
            "archive.zip/",
            Path::new("file.txt"),
        )
        .expect_err("runtime entry bytes should be capped");

        assert!(error.to_string().contains("archive entry exceeds max size"));
    }

    #[test]
    fn copy_archive_entry_enforces_runtime_total_bytes() {
        let limits = ArchiveExtractionLimits {
            max_entry_bytes: 100,
            max_total_bytes: 6,
            max_entry_count: 10,
        };
        let mut reader = std::io::Cursor::new(b"3456");
        let mut output = Vec::new();
        let mut extracted_bytes = 3;

        let error = copy_archive_entry_with_limits(
            &mut reader,
            &mut output,
            &mut extracted_bytes,
            limits,
            "archive.zip/",
            Path::new("file.txt"),
        )
        .expect_err("runtime total bytes should be capped");

        assert!(
            error
                .to_string()
                .contains("archive exceeds max extracted size")
        );
    }
}
