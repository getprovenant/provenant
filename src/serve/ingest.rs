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
use crate::serve_api::{SyncLicenseSource, SyncScanInput, SyncScanOptions, SyncScanRequest};
use crate::workflow::{LicenseSource, ScanOptions, WorkflowError, scan_paths};

impl From<SyncLicenseSource> for LicenseSource {
    fn from(source: SyncLicenseSource) -> Self {
        match source {
            SyncLicenseSource::Disabled => LicenseSource::Disabled,
            SyncLicenseSource::Embedded => LicenseSource::Embedded,
            SyncLicenseSource::Directory { path } => LicenseSource::Directory(PathBuf::from(path)),
        }
    }
}

impl From<SyncScanOptions> for ScanOptions {
    fn from(options: SyncScanOptions) -> Self {
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
const MAX_REMOTE_INPUT_BYTES: u64 = 100 * 1024 * 1024;
const MAX_UPLOADED_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARCHIVE_ENTRY_BYTES: u64 = 50 * 1024 * 1024;
const MAX_ARCHIVE_TOTAL_BYTES: u64 = 100 * 1024 * 1024;
const MAX_ARCHIVE_ENTRY_COUNT: usize = 10_000;

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

impl SyncScanInput {
    fn prepare(self) -> Result<(Vec<PathBuf>, Option<TempDir>)> {
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

    if looks_like_supported_archive(artifact_name) {
        let extract_dir = staging_dir.path().join("extracted");
        fs::create_dir_all(&extract_dir).map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to create {}", extract_dir.display()),
            )
        })?;
        extract_archive(&artifact_path, &extract_dir)?;
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

fn looks_like_supported_archive(filename: &str) -> bool {
    let filename = filename.to_ascii_lowercase();
    [".zip", ".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tar.xz"]
        .iter()
        .any(|suffix| filename.ends_with(suffix))
}

fn extract_archive(archive_path: &Path, output_dir: &Path) -> Result<()> {
    let filename = archive_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if filename.ends_with(".zip") {
        return extract_zip_archive(archive_path, output_dir);
    }
    if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
        return extract_tar_archive(
            archive_path,
            output_dir,
            GzDecoder::new(File::open(archive_path).map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!("failed to open {}", archive_path.display()),
                )
            })?),
        );
    }
    if filename.ends_with(".tar.bz2") {
        return extract_tar_archive(
            archive_path,
            output_dir,
            BzDecoder::new(File::open(archive_path).map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!("failed to open {}", archive_path.display()),
                )
            })?),
        );
    }
    if filename.ends_with(".tar.xz") {
        return extract_tar_archive(
            archive_path,
            output_dir,
            XzDecoder::new(File::open(archive_path).map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!("failed to open {}", archive_path.display()),
                )
            })?),
        );
    }
    if filename.ends_with(".tar") {
        return extract_tar_archive(
            archive_path,
            output_dir,
            File::open(archive_path).map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!("failed to open {}", archive_path.display()),
                )
            })?,
        );
    }

    Err(IngestError::validation(format!(
        "unsupported archive format for remote ingestion: {}",
        archive_path.display()
    )))
}

fn extract_zip_archive(archive_path: &Path, output_dir: &Path) -> Result<()> {
    let file = File::open(archive_path).map_err(|e| {
        IngestError::internal_with_source(e, format!("failed to open {}", archive_path.display()))
    })?;
    let mut archive = ZipArchive::new(file).map_err(|e| {
        IngestError::internal_with_source(
            e,
            format!("failed to read zip archive {}", archive_path.display()),
        )
    })?;

    let mut extracted_files = 0usize;
    let mut extracted_bytes = 0u64;

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

        enforce_archive_limits(&mut extracted_files, &mut extracted_bytes, entry.size())?;
        let destination = output_dir.join(relative_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!("failed to create {}", parent.display()),
                )
            })?;
        }
        let mut output = File::create(&destination).map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to create {}", destination.display()),
            )
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to extract {}", destination.display()),
            )
        })?;
    }

    if extracted_files == 0 {
        return Err(IngestError::validation(
            "archive did not contain any safe files to scan",
        ));
    }

    Ok(())
}

fn extract_tar_archive<R: Read>(archive_path: &Path, output_dir: &Path, reader: R) -> Result<()> {
    let mut archive = Archive::new(reader);
    let mut extracted_files = 0usize;
    let mut extracted_bytes = 0u64;

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

        enforce_archive_limits(
            &mut extracted_files,
            &mut extracted_bytes,
            entry.header().size().map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!("failed to read tar size in {}", archive_path.display()),
                )
            })?,
        )?;
        let destination = output_dir.join(relative_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                IngestError::internal_with_source(
                    e,
                    format!("failed to create {}", parent.display()),
                )
            })?;
        }
        let mut output = File::create(&destination).map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to create {}", destination.display()),
            )
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|e| {
            IngestError::internal_with_source(
                e,
                format!("failed to extract {}", destination.display()),
            )
        })?;
    }

    if extracted_files == 0 {
        return Err(IngestError::validation(
            "archive did not contain any safe files to scan",
        ));
    }

    Ok(())
}

fn enforce_archive_limits(
    extracted_files: &mut usize,
    extracted_bytes: &mut u64,
    entry_size: u64,
) -> Result<()> {
    if entry_size > MAX_ARCHIVE_ENTRY_BYTES {
        return Err(IngestError::PayloadTooLarge(format!(
            "archive entry exceeds max size of {} bytes",
            MAX_ARCHIVE_ENTRY_BYTES
        )));
    }

    *extracted_files += 1;
    if *extracted_files > MAX_ARCHIVE_ENTRY_COUNT {
        return Err(IngestError::PayloadTooLarge(format!(
            "archive exceeds max entry count of {}",
            MAX_ARCHIVE_ENTRY_COUNT
        )));
    }

    *extracted_bytes += entry_size;
    if *extracted_bytes > MAX_ARCHIVE_TOTAL_BYTES {
        return Err(IngestError::PayloadTooLarge(format!(
            "archive exceeds max extracted size of {} bytes",
            MAX_ARCHIVE_TOTAL_BYTES
        )));
    }

    Ok(())
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
    pub(super) fn new(request: SyncScanRequest) -> Result<Self> {
        let (paths, _staging_dir) = request.input.prepare()?;

        let mut options = ScanOptions::from(request.options);
        if _staging_dir.is_some() {
            options.strip_root = true;
            options.full_root = false;
        }

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
    fn upload_input_rejects_invalid_base64() {
        let error = prepare_upload_input("input.txt", "%%%%")
            .expect_err("invalid base64 upload should fail");
        assert!(error.to_string().contains("valid base64"));
    }

    #[test]
    fn normalize_archive_path_rejects_parent_dirs() {
        assert!(normalize_archive_path(Path::new("../escape.txt")).is_none());
    }
}
