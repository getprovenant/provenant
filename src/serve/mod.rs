// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

mod ingest;

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

use crate::ProcessMode;
use crate::cli::ServeArgs;
use crate::license_detection::LicenseDetectionEngine;
use crate::serve_api::{
    API_VERSION, AsyncJobState, AsyncJobStatusResponse, AsyncScanAcceptedResponse,
    ServeErrorResponse, ServeLivenessResponse, ServeReadinessResponse, ServeVersionResponse,
    SyncLicenseSource, SyncScanRequest,
};
use crate::version::BUILD_VERSION;
use crate::workflow::{LicenseSource, ScanOptions, WorkflowError, scan_paths};
use ingest::{IngestError, prepare_sync_input};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REQUEST_BODY_BYTES: usize = 24 * 1024 * 1024;
const DEFAULT_ASYNC_MAX_PROCESSORS_PER_JOB: usize = 4;
const DEFAULT_ASYNC_RETAINED_TERMINAL_JOBS: usize = 64;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ServeError {
    #[error(transparent)]
    Ingest(#[from] IngestError),
    #[error(transparent)]
    Workflow(#[from] WorkflowError),
    #[error("{0}")]
    Serialization(String),
}

impl ServeError {
    pub(crate) fn http_status_code(&self) -> u16 {
        match self {
            Self::Ingest(IngestError::Validation(_)) => 422,
            Self::Ingest(IngestError::PayloadTooLarge(_)) => 413,
            Self::Ingest(IngestError::Upstream { .. }) => 502,
            Self::Ingest(IngestError::Internal { .. }) => 500,
            Self::Workflow(WorkflowError::InvalidOptions(_)) => 422,
            Self::Workflow(WorkflowError::Pipeline(_)) => 500,
            Self::Serialization(_) => 500,
        }
    }

    pub(crate) fn error_type(&self) -> &'static str {
        match self {
            Self::Ingest(IngestError::Validation(_)) => "invalid_scan_request",
            Self::Ingest(IngestError::PayloadTooLarge(_)) => "payload_too_large",
            Self::Ingest(IngestError::Upstream { .. }) => "upstream_error",
            Self::Ingest(IngestError::Internal { .. }) => "internal_error",
            Self::Workflow(WorkflowError::InvalidOptions(_)) => "invalid_scan_request",
            Self::Workflow(WorkflowError::Pipeline(_)) => "scan_failed",
            Self::Serialization(_) => "scan_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ServeConfig {
    bind: String,
}

impl TryFrom<&ServeArgs> for ServeConfig {
    type Error = anyhow::Error;

    fn try_from(args: &ServeArgs) -> Result<Self> {
        if args.bind.trim().is_empty() {
            return Err(anyhow!("--bind must not be empty"));
        }

        Ok(Self {
            bind: args.bind.clone(),
        })
    }
}

#[derive(Debug, Clone)]
enum ReadinessState {
    Pending,
    Ready { spdx_license_list_version: String },
    Failed { message: String },
}

#[derive(Debug)]
struct ServeState {
    readiness: Arc<Mutex<ReadinessState>>,
    jobs: AsyncJobController,
}

#[derive(Debug, Clone)]
struct AsyncJobController {
    inner: Arc<Mutex<AsyncJobControllerState>>,
    processor_budget: usize,
    max_processors_per_job: usize,
    max_retained_terminal_jobs: usize,
}

#[derive(Debug)]
struct AsyncJobControllerState {
    active_processors: usize,
    jobs: HashMap<String, AsyncJobRecord>,
    pending: VecDeque<PendingAsyncJob>,
    completed: VecDeque<String>,
}

#[derive(Debug)]
struct PendingAsyncJob {
    job_id: String,
    request: SyncScanRequest,
}

#[derive(Debug)]
struct AsyncJobRecord {
    state: AsyncJobState,
    allocated_processors: Option<usize>,
    result_body: Option<String>,
    error_message: Option<String>,
    error_status_code: Option<u16>,
}

#[derive(Debug)]
struct DispatchedAsyncJob {
    job_id: String,
    request: SyncScanRequest,
    allocated_processors: usize,
}

#[derive(Debug, Clone)]
struct AsyncJobSnapshot {
    job_id: String,
    state: AsyncJobState,
    allocated_processors: Option<usize>,
    error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct AsyncJobResultSnapshot {
    state: AsyncJobState,
    result_body: Option<String>,
    error_message: Option<String>,
    error_status_code: Option<u16>,
}

#[derive(Debug, Clone, Copy)]
enum AsyncSubmitError {
    QueueFull,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl AsyncJobController {
    fn new() -> Self {
        let processor_budget = default_async_processor_budget();
        Self::with_limits(
            processor_budget,
            processor_budget.clamp(1, DEFAULT_ASYNC_MAX_PROCESSORS_PER_JOB),
            DEFAULT_ASYNC_RETAINED_TERMINAL_JOBS,
        )
    }

    fn with_limits(
        processor_budget: usize,
        max_processors_per_job: usize,
        max_retained_terminal_jobs: usize,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AsyncJobControllerState {
                active_processors: 0,
                jobs: HashMap::new(),
                pending: VecDeque::new(),
                completed: VecDeque::new(),
            })),
            processor_budget: processor_budget.max(1),
            max_processors_per_job: max_processors_per_job.max(1),
            max_retained_terminal_jobs: max_retained_terminal_jobs.max(1),
        }
    }

    fn submit(
        &self,
        request: SyncScanRequest,
    ) -> std::result::Result<AsyncScanAcceptedResponse, AsyncSubmitError> {
        let (response, dispatches) = {
            let mut inner = self
                .inner
                .lock()
                .expect("serve async job lock should not be poisoned");
            if inner.non_terminal_jobs() >= self.max_non_terminal_jobs() {
                return Err(AsyncSubmitError::QueueFull);
            }

            let job_id = format!("job-{}", Uuid::new_v4().simple());
            inner.jobs.insert(job_id.clone(), AsyncJobRecord::pending());
            inner.pending.push_back(PendingAsyncJob {
                job_id: job_id.clone(),
                request,
            });
            let dispatches = self.schedule_locked(&mut inner);
            let snapshot = inner
                .status_snapshot(&job_id)
                .expect("submitted async job should be present");
            (
                AsyncScanAcceptedResponse {
                    status: "accepted".to_string(),
                    job_id: job_id.clone(),
                    state: snapshot.state,
                    status_url: format!("/v1/jobs/{job_id}"),
                    result_url: format!("/v1/jobs/{job_id}/result"),
                },
                dispatches,
            )
        };

        self.spawn_dispatches(dispatches);
        Ok(response)
    }

    fn status_snapshot(&self, job_id: &str) -> Option<AsyncJobSnapshot> {
        self.inner
            .lock()
            .expect("serve async job lock should not be poisoned")
            .status_snapshot(job_id)
    }

    fn result_snapshot(&self, job_id: &str) -> Option<AsyncJobResultSnapshot> {
        self.inner
            .lock()
            .expect("serve async job lock should not be poisoned")
            .result_snapshot(job_id)
    }

    fn max_non_terminal_jobs(&self) -> usize {
        self.processor_budget.saturating_mul(4).max(4)
    }

    fn schedule_locked(&self, inner: &mut AsyncJobControllerState) -> Vec<DispatchedAsyncJob> {
        let mut dispatches = Vec::new();

        while !inner.pending.is_empty() {
            let available_processors = self
                .processor_budget
                .saturating_sub(inner.active_processors);
            if available_processors == 0 {
                break;
            }

            let allocated_processors = available_processors.min(self.max_processors_per_job).max(1);
            let PendingAsyncJob { job_id, request } = inner
                .pending
                .pop_front()
                .expect("pending async job should still exist");
            let record = inner
                .jobs
                .get_mut(&job_id)
                .expect("queued async job should have metadata");
            record.state = AsyncJobState::Running;
            record.allocated_processors = Some(allocated_processors);
            record.error_message = None;
            inner.active_processors += allocated_processors;
            dispatches.push(DispatchedAsyncJob {
                job_id,
                request,
                allocated_processors,
            });
        }

        dispatches
    }

    fn spawn_dispatches(&self, dispatches: Vec<DispatchedAsyncJob>) {
        for dispatched in dispatches {
            let controller = self.clone();
            thread::spawn(move || controller.run_job(dispatched));
        }
    }

    fn run_job(&self, dispatched: DispatchedAsyncJob) {
        let result = run_async_scan_request(dispatched.request, dispatched.allocated_processors);
        let follow_up_dispatches = {
            let mut inner = self
                .inner
                .lock()
                .expect("serve async job lock should not be poisoned");
            inner.active_processors = inner
                .active_processors
                .saturating_sub(dispatched.allocated_processors);

            let record = inner
                .jobs
                .get_mut(&dispatched.job_id)
                .expect("running async job should have metadata");
            match result {
                Ok(result_body) => {
                    record.state = AsyncJobState::Succeeded;
                    record.result_body = Some(result_body);
                    record.error_message = None;
                }
                Err(error) => {
                    eprintln!("serve async job {} failed: {error}", dispatched.job_id);
                    record.state = AsyncJobState::Failed;
                    record.result_body = None;
                    record.error_message = Some(error.to_string());
                    record.error_status_code = Some(error.http_status_code());
                }
            }

            inner.completed.push_back(dispatched.job_id.clone());
            inner.evict_completed_jobs(self.max_retained_terminal_jobs);

            self.schedule_locked(&mut inner)
        };

        self.spawn_dispatches(follow_up_dispatches);
    }
}

impl AsyncJobControllerState {
    fn non_terminal_jobs(&self) -> usize {
        self.jobs
            .values()
            .filter(|job| matches!(job.state, AsyncJobState::Pending | AsyncJobState::Running))
            .count()
    }

    fn status_snapshot(&self, job_id: &str) -> Option<AsyncJobSnapshot> {
        self.jobs.get(job_id).map(|job| AsyncJobSnapshot {
            job_id: job_id.to_string(),
            state: job.state,
            allocated_processors: job.allocated_processors,
            error_message: job.error_message.clone(),
        })
    }

    fn result_snapshot(&self, job_id: &str) -> Option<AsyncJobResultSnapshot> {
        self.jobs.get(job_id).map(|job| AsyncJobResultSnapshot {
            state: job.state,
            result_body: job.result_body.clone(),
            error_message: job.error_message.clone(),
            error_status_code: job.error_status_code,
        })
    }

    fn evict_completed_jobs(&mut self, max_retained_terminal_jobs: usize) {
        while self.completed.len() > max_retained_terminal_jobs {
            let Some(job_id) = self.completed.pop_front() else {
                break;
            };
            self.jobs.remove(&job_id);
        }
    }
}

impl AsyncJobRecord {
    fn pending() -> Self {
        Self {
            state: AsyncJobState::Pending,
            allocated_processors: None,
            result_body: None,
            error_message: None,
            error_status_code: None,
        }
    }
}

impl AsyncJobSnapshot {
    fn into_status_response(self) -> AsyncJobStatusResponse {
        AsyncJobStatusResponse {
            job_id: self.job_id,
            state: self.state,
            result_ready: matches!(self.state, AsyncJobState::Succeeded),
            allocated_processors: self.allocated_processors,
            message: self.error_message,
        }
    }
}

pub(crate) fn run(args: &ServeArgs) -> Result<()> {
    let config = ServeConfig::try_from(args)?;
    let bind_addr = resolve_bind_addr(&config.bind)?;
    let listener = TcpListener::bind(bind_addr)
        .with_context(|| format!("Failed to bind provenant serve to {}", config.bind))?;
    let local_addr = listener
        .local_addr()
        .context("Failed to read provenant serve local address after bind")?;

    let readiness = Arc::new(Mutex::new(ReadinessState::Pending));
    start_warm_init(Arc::clone(&readiness));

    eprintln!(
        "Starting provenant serve on http://{} (api {API_VERSION})",
        local_addr
    );

    let state = ServeState {
        readiness,
        jobs: AsyncJobController::new(),
    };
    serve_forever(listener, state)
}

fn resolve_bind_addr(bind: &str) -> Result<SocketAddr> {
    bind.to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow!("Could not resolve bind address {bind}"))
}

fn start_warm_init(readiness: Arc<Mutex<ReadinessState>>) {
    thread::spawn(move || {
        let next_state = match LicenseDetectionEngine::embedded_spdx_license_list_version() {
            Ok(version) => ReadinessState::Ready {
                spdx_license_list_version: version,
            },
            Err(error) => ReadinessState::Failed {
                message: error.to_string(),
            },
        };

        let mut current = readiness
            .lock()
            .expect("serve readiness lock should not be poisoned");
        *current = next_state;
    });
}

fn serve_forever(listener: TcpListener, state: ServeState) -> Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &state) {
                    eprintln!("serve request handling error: {error}");
                }
            }
            Err(error) => return Err(error.into()),
        }
    }

    Ok(())
}

fn default_async_processor_budget() -> usize {
    let cpus = thread::available_parallelism().map_or(1, |count| count.get());
    if cpus > 1 { cpus - 1 } else { 1 }
}

fn handle_connection(mut stream: TcpStream, state: &ServeState) -> Result<()> {
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;

    let request = match parse_http_request(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            return write_http_response(
                &mut stream,
                error_response(400, "Bad Request", "invalid_request", error.to_string()),
            );
        }
    };

    let response = response_for_request(&request, state);
    write_http_response(&mut stream, response)
}

fn parse_http_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    if request_line.trim().is_empty() {
        return Err(anyhow!("received empty HTTP request"));
    }

    let mut headers = HashMap::new();
    loop {
        let mut header_line = String::new();
        reader.read_line(&mut header_line)?;
        if header_line == "\r\n" || header_line.is_empty() {
            break;
        }

        let header = header_line.trim_end_matches("\r\n");
        let Some((name, value)) = header.split_once(':') else {
            return Err(anyhow!("invalid HTTP header line"));
        };
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }

    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| anyhow!("missing HTTP method in request line"))?;
    let path = parts
        .next()
        .ok_or_else(|| anyhow!("missing HTTP path in request line"))?;

    let content_length = headers
        .get("content-length")
        .map(|value| {
            value
                .parse::<usize>()
                .context("invalid Content-Length header")
        })
        .transpose()?
        .unwrap_or(0);

    if content_length > MAX_REQUEST_BODY_BYTES {
        return Err(anyhow!(
            "request body exceeds max size of {} bytes",
            MAX_REQUEST_BODY_BYTES
        ));
    }

    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    Ok(HttpRequest {
        method: method.to_string(),
        path: path.to_string(),
        headers,
        body,
    })
}

#[derive(Debug)]
struct HttpResponse {
    status_code: u16,
    reason: &'static str,
    body: String,
}

#[derive(Debug)]
struct SyncScanExecution {
    paths: Vec<PathBuf>,
    options: ScanOptions,
    _staging_dir: Option<TempDir>,
}

fn response_for_request(request: &HttpRequest, state: &ServeState) -> HttpResponse {
    let readiness = state
        .readiness
        .lock()
        .expect("serve readiness lock should not be poisoned")
        .clone();

    if let Some(job_route) = parse_job_route(&request.path) {
        return match job_route {
            JobRoute::Status(job_id) => {
                if request.method == "GET" {
                    handle_job_status_request(job_id, state)
                } else {
                    error_response(
                        405,
                        "Method Not Allowed",
                        "method_not_allowed",
                        format!("use GET /v1/jobs/{job_id} to inspect async job state"),
                    )
                }
            }
            JobRoute::Result(job_id) => {
                if request.method == "GET" {
                    handle_job_result_request(job_id, state)
                } else {
                    error_response(
                        405,
                        "Method Not Allowed",
                        "method_not_allowed",
                        format!("use GET /v1/jobs/{job_id}/result to fetch async job output"),
                    )
                }
            }
        };
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/livez") => serialize_response(
            200,
            "OK",
            &ServeLivenessResponse {
                status: "ok".to_string(),
            },
        ),
        ("GET", "/readyz") => match readiness {
            ReadinessState::Pending => serialize_response(
                503,
                "Service Unavailable",
                &ServeReadinessResponse {
                    status: "warming".to_string(),
                    api_version: None,
                    spdx_license_list_version: None,
                    message: None,
                },
            ),
            ReadinessState::Ready {
                spdx_license_list_version,
            } => serialize_response(
                200,
                "OK",
                &ServeReadinessResponse {
                    status: "ready".to_string(),
                    api_version: Some(API_VERSION.to_string()),
                    spdx_license_list_version: Some(spdx_license_list_version),
                    message: None,
                },
            ),
            ReadinessState::Failed { message } => serialize_response(
                503,
                "Service Unavailable",
                &ServeReadinessResponse {
                    status: "failed".to_string(),
                    api_version: None,
                    spdx_license_list_version: None,
                    message: Some(message),
                },
            ),
        },
        ("GET", "/version") => serialize_response(
            200,
            "OK",
            &ServeVersionResponse {
                service: "provenant-serve".to_string(),
                api_version: API_VERSION.to_string(),
                tool_version: BUILD_VERSION.to_string(),
            },
        ),
        ("POST", "/v1/scans") => handle_sync_scan_request(request),
        (_, "/v1/scans") => error_response(
            405,
            "Method Not Allowed",
            "method_not_allowed",
            "use POST /v1/scans for synchronous scan execution".to_string(),
        ),
        ("POST", "/v1/scans:async") => handle_async_scan_request(request, state),
        (_, "/v1/scans:async") => error_response(
            405,
            "Method Not Allowed",
            "method_not_allowed",
            "use POST /v1/scans:async for asynchronous scan submission".to_string(),
        ),
        _ => json_response(
            404,
            "Not Found",
            json!({ "status": "not_found", "path": request.path }),
        ),
    }
}

fn handle_sync_scan_request(request: &HttpRequest) -> HttpResponse {
    let sync_request = match decode_scan_request_from_http(request, "POST /v1/scans") {
        Ok(sync_request) => sync_request,
        Err(response) => return response,
    };

    let execution = match build_sync_scan_execution(sync_request) {
        Ok(execution) => execution,
        Err(error) => {
            return serve_error_response(&error);
        }
    };

    match execute_scan_execution(execution) {
        Ok(body) => HttpResponse {
            status_code: 200,
            reason: "OK",
            body,
        },
        Err(error) => serve_error_response(&error),
    }
}

fn serve_error_response(error: &ServeError) -> HttpResponse {
    let status_code = error.http_status_code();
    let reason = http_status_reason(status_code);
    error_response(status_code, reason, error.error_type(), error.to_string())
}

fn http_status_reason(code: u16) -> &'static str {
    match code {
        400 => "Bad Request",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

fn handle_async_scan_request(request: &HttpRequest, state: &ServeState) -> HttpResponse {
    let sync_request = match decode_scan_request_from_http(request, "POST /v1/scans:async") {
        Ok(sync_request) => sync_request,
        Err(response) => return response,
    };

    match state.jobs.submit(sync_request) {
        Ok(response) => serialize_response(202, "Accepted", &response),
        Err(AsyncSubmitError::QueueFull) => error_response(
            503,
            "Service Unavailable",
            "server_busy",
            "async job queue is full; try again later".to_string(),
        ),
    }
}

fn handle_job_status_request(job_id: &str, state: &ServeState) -> HttpResponse {
    match state.jobs.status_snapshot(job_id) {
        Some(snapshot) => serialize_response(200, "OK", &snapshot.into_status_response()),
        None => error_response(
            404,
            "Not Found",
            "job_not_found",
            format!("async job {job_id} was not found"),
        ),
    }
}

fn handle_job_result_request(job_id: &str, state: &ServeState) -> HttpResponse {
    let Some(snapshot) = state.jobs.result_snapshot(job_id) else {
        return error_response(
            404,
            "Not Found",
            "job_not_found",
            format!("async job {job_id} was not found"),
        );
    };

    match snapshot.state {
        AsyncJobState::Succeeded => HttpResponse {
            status_code: 200,
            reason: "OK",
            body: snapshot
                .result_body
                .expect("successful async job should retain result body"),
        },
        AsyncJobState::Pending | AsyncJobState::Running => error_response(
            409,
            "Conflict",
            "job_not_ready",
            format!(
                "async job {job_id} is currently {}",
                async_job_state_label(snapshot.state)
            ),
        ),
        AsyncJobState::Failed => {
            let status_code = snapshot.error_status_code.unwrap_or(500);
            let reason = http_status_reason(status_code);
            error_response(
                status_code,
                reason,
                "job_failed",
                snapshot
                    .error_message
                    .unwrap_or_else(|| format!("async job {job_id} failed")),
            )
        }
    }
}

fn decode_sync_scan_request(body: &[u8]) -> Result<SyncScanRequest> {
    serde_json::from_slice(body).context("request body must be valid JSON")
}

fn decode_scan_request_from_http(
    request: &HttpRequest,
    route_label: &str,
) -> std::result::Result<SyncScanRequest, HttpResponse> {
    if !request
        .headers
        .get("content-type")
        .is_some_and(|value| value.starts_with("application/json"))
    {
        return Err(error_response(
            415,
            "Unsupported Media Type",
            "unsupported_media_type",
            format!("{route_label} requires Content-Type: application/json"),
        ));
    }

    decode_sync_scan_request(&request.body)
        .map_err(|error| error_response(400, "Bad Request", "invalid_request", error.to_string()))
}

fn build_sync_scan_execution(request: SyncScanRequest) -> Result<SyncScanExecution, ServeError> {
    let prepared_input = prepare_sync_input(request.input)?;

    let mut options = ScanOptions::default();
    options.collect_info = request.options.collect_info;
    options.detect_license = match request.options.detect_license {
        SyncLicenseSource::Disabled => LicenseSource::Disabled,
        SyncLicenseSource::Embedded => LicenseSource::Embedded,
        SyncLicenseSource::Directory { path } => LicenseSource::Directory(PathBuf::from(path)),
    };
    options.detect_packages = request.options.detect_packages;
    options.detect_system_packages = request.options.detect_system_packages;
    options.detect_packages_in_compiled = request.options.detect_packages_in_compiled;
    options.detect_copyrights = request.options.detect_copyrights;
    options.detect_emails = request.options.detect_emails;
    options.detect_urls = request.options.detect_urls;
    options.detect_generated = request.options.detect_generated;
    options.include = request.options.include;
    options.exclude = request.options.exclude;
    if prepared_input.strip_staging_root {
        options.strip_root = true;
        options.full_root = false;
    } else {
        options.strip_root = request.options.strip_root;
        options.full_root = request.options.full_root;
    }
    options.license_text = request.options.license_text;
    options.license_text_diagnostics = request.options.license_text_diagnostics;
    options.license_diagnostics = request.options.license_diagnostics;
    options.unknown_licenses = request.options.unknown_licenses;
    options.license_score = request.options.license_score;
    options.only_findings = request.options.only_findings;
    options.mark_source = request.options.mark_source;
    options.classify = request.options.classify;
    options.summary = request.options.summary;
    options.license_clarity_score = request.options.license_clarity_score;
    options.license_references = request.options.license_references;
    options.tallies = request.options.tallies;
    options.tallies_key_files = request.options.tallies_key_files;
    options.tallies_with_details = request.options.tallies_with_details;
    options.facets = request.options.facets;
    options.tallies_by_facet = request.options.tallies_by_facet;

    Ok(SyncScanExecution {
        paths: prepared_input.paths,
        options,
        _staging_dir: prepared_input.staging_dir,
    })
}

fn execute_scan_execution(execution: SyncScanExecution) -> Result<String, ServeError> {
    let output = scan_paths(
        execution.paths.iter().map(|path| path.as_path()),
        &execution.options,
    )?;
    serde_json::to_string(&crate::output_schema::Output::from(&output))
        .map_err(|e| ServeError::Serialization(format!("scan result should serialize: {e}")))
}

fn run_async_scan_request(
    request: SyncScanRequest,
    allocated_processors: usize,
) -> Result<String, ServeError> {
    let mut execution = build_sync_scan_execution(request)?;
    execution.options.process_mode = if allocated_processors <= 1 {
        ProcessMode::SequentialWithTimeouts
    } else {
        ProcessMode::Parallel(allocated_processors)
    };
    execute_scan_execution(execution)
}

fn async_job_state_label(state: AsyncJobState) -> &'static str {
    match state {
        AsyncJobState::Pending => "pending",
        AsyncJobState::Running => "running",
        AsyncJobState::Succeeded => "succeeded",
        AsyncJobState::Failed => "failed",
    }
}

enum JobRoute<'a> {
    Status(&'a str),
    Result(&'a str),
}

fn parse_job_route(path: &str) -> Option<JobRoute<'_>> {
    let suffix = path.strip_prefix("/v1/jobs/")?;
    if let Some(job_id) = suffix.strip_suffix("/result") {
        return is_valid_job_id(job_id).then_some(JobRoute::Result(job_id));
    }

    is_valid_job_id(suffix).then_some(JobRoute::Status(suffix))
}

fn is_valid_job_id(job_id: &str) -> bool {
    !job_id.is_empty() && !job_id.contains('/')
}

fn json_response(status_code: u16, reason: &'static str, body: serde_json::Value) -> HttpResponse {
    HttpResponse {
        status_code,
        reason,
        body: body.to_string(),
    }
}

fn serialize_response<T: serde::Serialize>(
    status_code: u16,
    reason: &'static str,
    body: &T,
) -> HttpResponse {
    HttpResponse {
        status_code,
        reason,
        body: serde_json::to_string(body).expect("response body should serialize"),
    }
}

fn error_response(
    status_code: u16,
    reason: &'static str,
    status: &'static str,
    message: String,
) -> HttpResponse {
    serialize_response(
        status_code,
        reason,
        &ServeErrorResponse {
            status: status.to_string(),
            message,
            api_version: API_VERSION.to_string(),
        },
    )
}

fn write_http_response(stream: &mut TcpStream, response: HttpResponse) -> Result<()> {
    let payload = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.status_code,
        response.reason,
        response.body.len(),
        response.body,
    );
    stream.write_all(payload.as_bytes())?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serve_api::{SyncScanInput, SyncScanOptions};

    fn dummy_async_request() -> SyncScanRequest {
        SyncScanRequest {
            input: SyncScanInput::Paths {
                paths: vec!["/tmp".to_string()],
            },
            options: SyncScanOptions::default(),
        }
    }

    fn ready_state() -> ServeState {
        ServeState {
            readiness: Arc::new(Mutex::new(ReadinessState::Ready {
                spdx_license_list_version: "3.28".to_string(),
            })),
            jobs: AsyncJobController::with_limits(2, 2, 8),
        }
    }

    fn ready_state_with_job(job_id: &str, record: AsyncJobRecord) -> ServeState {
        let state = ready_state();
        state
            .jobs
            .inner
            .lock()
            .expect("test async job lock should not be poisoned")
            .jobs
            .insert(job_id.to_string(), record);
        state
    }

    #[test]
    fn serve_config_requires_non_empty_bind() {
        let args = ServeArgs {
            bind: String::new(),
        };

        let error = ServeConfig::try_from(&args).expect_err("empty bind should fail");
        assert!(error.to_string().contains("--bind must not be empty"));
    }

    #[test]
    fn readyz_reports_ready_metadata() {
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/readyz".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };
        let response = response_for_request(&request, &ready_state());
        assert_eq!(response.status_code, 200);
        assert!(response.body.contains("\"status\":\"ready\""));
        assert!(response.body.contains("\"api_version\":\"v1\""));
    }

    #[test]
    fn async_scan_route_requires_post() {
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/v1/scans:async".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };
        let response = response_for_request(&request, &ready_state());
        assert_eq!(response.status_code, 405);
        assert!(response.body.contains("method_not_allowed"));
    }

    #[test]
    fn pending_job_status_reports_pending_state() {
        let state = ready_state_with_job("job-1", AsyncJobRecord::pending());
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/v1/jobs/job-1".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };

        let response = response_for_request(&request, &state);
        assert_eq!(response.status_code, 200);
        assert!(response.body.contains("\"state\":\"pending\""));
        assert!(response.body.contains("\"result_ready\":false"));
    }

    #[test]
    fn pending_job_result_reports_not_ready() {
        let state = ready_state_with_job("job-2", AsyncJobRecord::pending());
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/v1/jobs/job-2/result".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };

        let response = response_for_request(&request, &state);
        assert_eq!(response.status_code, 409);
        assert!(response.body.contains("job_not_ready"));
    }

    #[test]
    fn completed_job_result_returns_stored_body() {
        let state = ready_state_with_job(
            "job-3",
            AsyncJobRecord {
                state: AsyncJobState::Succeeded,
                allocated_processors: Some(2),
                result_body: Some("{\"status\":\"ok\"}".to_string()),
                error_message: None,
                error_status_code: None,
            },
        );
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/v1/jobs/job-3/result".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };

        let response = response_for_request(&request, &state);
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, "{\"status\":\"ok\"}");
    }

    #[test]
    fn failed_job_result_returns_failure_contract() {
        let state = ready_state_with_job(
            "job-4",
            AsyncJobRecord {
                state: AsyncJobState::Failed,
                allocated_processors: Some(1),
                result_body: None,
                error_message: Some("async scan job failed".to_string()),
                error_status_code: Some(500),
            },
        );
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/v1/jobs/job-4/result".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };

        let response = response_for_request(&request, &state);
        assert_eq!(response.status_code, 500);
        assert!(response.body.contains("job_failed"));
        assert!(response.body.contains("async scan job failed"));
    }

    #[test]
    fn async_job_controller_rejects_submit_when_queue_is_full() {
        let controller = AsyncJobController::with_limits(1, 1, 8);
        {
            let mut inner = controller
                .inner
                .lock()
                .expect("test async job lock should not be poisoned");
            for index in 0..controller.max_non_terminal_jobs() {
                inner
                    .jobs
                    .insert(format!("job-{index}"), AsyncJobRecord::pending());
            }
        }

        let result = controller.submit(dummy_async_request());
        assert!(matches!(result, Err(AsyncSubmitError::QueueFull)));
    }

    #[test]
    fn scheduler_leaves_extra_jobs_pending_when_budget_is_exhausted() {
        let controller = AsyncJobController::with_limits(4, 2, 8);
        let mut inner = AsyncJobControllerState {
            active_processors: 0,
            jobs: HashMap::new(),
            pending: VecDeque::new(),
            completed: VecDeque::new(),
        };

        for job_id in ["job-a", "job-b", "job-c"] {
            inner
                .jobs
                .insert(job_id.to_string(), AsyncJobRecord::pending());
            inner.pending.push_back(PendingAsyncJob {
                job_id: job_id.to_string(),
                request: dummy_async_request(),
            });
        }

        let dispatches = controller.schedule_locked(&mut inner);
        assert_eq!(dispatches.len(), 2);
        assert_eq!(inner.active_processors, 4);
        assert_eq!(inner.pending.len(), 1);
        assert_eq!(inner.jobs["job-a"].state, AsyncJobState::Running);
        assert_eq!(inner.jobs["job-b"].state, AsyncJobState::Running);
        assert_eq!(inner.jobs["job-c"].state, AsyncJobState::Pending);
    }

    #[test]
    fn completed_jobs_are_evicted_when_retention_limit_is_exceeded() {
        let mut inner = AsyncJobControllerState {
            active_processors: 0,
            jobs: HashMap::from([
                (
                    "job-old".to_string(),
                    AsyncJobRecord {
                        state: AsyncJobState::Succeeded,
                        allocated_processors: Some(1),
                        result_body: Some("old".to_string()),
                        error_message: None,
                        error_status_code: None,
                    },
                ),
                (
                    "job-new".to_string(),
                    AsyncJobRecord {
                        state: AsyncJobState::Succeeded,
                        allocated_processors: Some(1),
                        result_body: Some("new".to_string()),
                        error_message: None,
                        error_status_code: None,
                    },
                ),
            ]),
            pending: VecDeque::new(),
            completed: VecDeque::from(["job-old".to_string(), "job-new".to_string()]),
        };

        inner.evict_completed_jobs(1);

        assert!(!inner.jobs.contains_key("job-old"));
        assert!(inner.jobs.contains_key("job-new"));
        assert_eq!(inner.completed.len(), 1);
    }

    #[test]
    fn decode_sync_scan_request_rejects_empty_paths() {
        let error = build_sync_scan_execution(SyncScanRequest {
            input: SyncScanInput::Paths { paths: Vec::new() },
            options: SyncScanOptions::default(),
        })
        .expect_err("empty paths should fail");

        assert!(
            error
                .to_string()
                .contains("input.paths must contain at least one path")
        );
    }

    #[test]
    fn url_input_requires_http_or_https() {
        let error = build_sync_scan_execution(SyncScanRequest {
            input: SyncScanInput::Url {
                url: "file:///tmp/input.txt".to_string(),
            },
            options: SyncScanOptions::default(),
        })
        .expect_err("unsupported URL scheme should fail");

        assert!(error.to_string().contains("http or https"));
    }

    #[test]
    fn decode_sync_scan_request_requires_valid_json() {
        let error =
            decode_sync_scan_request(br#"{"input": }"#).expect_err("malformed JSON should fail");

        assert!(
            error
                .to_string()
                .contains("request body must be valid JSON")
        );
    }

    #[test]
    fn sync_scan_requires_json_content_type() {
        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/v1/scans".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };

        let response = handle_sync_scan_request(&request);
        assert_eq!(response.status_code, 415);
        assert!(response.body.contains("unsupported_media_type"));
    }

    #[test]
    fn parse_http_request_rejects_oversized_body() {
        let request = format!(
            "POST /v1/scans HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n",
            MAX_REQUEST_BODY_BYTES + 1
        );
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener address");

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept connection");
            let mut stream = stream;
            parse_http_request(&mut stream).expect_err("oversized request should fail")
        });

        let mut client = TcpStream::connect(address).expect("connect client");
        client
            .write_all(request.as_bytes())
            .expect("write request bytes");
        client.flush().expect("flush request bytes");

        let error = handle.join().expect("join parser thread");
        assert!(error.to_string().contains("request body exceeds max size"));
    }
}
