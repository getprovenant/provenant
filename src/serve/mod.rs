// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

mod ingest;
mod job_controller;

use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde_json::json;
use tempfile::TempDir;
use tiny_http::{Header, Response, Server, StatusCode};

use crate::ProcessMode;
use crate::cli::ServeArgs;
use crate::license_detection::LicenseDetectionEngine;
use crate::serve::ingest::{IngestError, prepare_sync_input};
use crate::serve::job_controller::{
    AsyncJobController, AsyncSubmitError, DispatchedAsyncJob, JobOutcome,
};
use crate::serve_api::{API_VERSION, AsyncJobState, SyncLicenseSource, SyncScanRequest};
use crate::version::BUILD_VERSION;
use crate::workflow::{LicenseSource, ScanOptions, WorkflowError, scan_paths};

const MAX_REQUEST_BODY_BYTES: usize = 24 * 1024 * 1024;

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
    pub(crate) fn http_status_code(&self) -> StatusCode {
        match self {
            Self::Ingest(IngestError::Validation(_)) => StatusCode::from(422),
            Self::Ingest(IngestError::PayloadTooLarge(_)) => StatusCode::from(413),
            Self::Ingest(IngestError::Upstream { .. }) => StatusCode::from(502),
            Self::Ingest(IngestError::Internal { .. }) => StatusCode::from(500),
            Self::Workflow(WorkflowError::InvalidOptions(_)) => StatusCode::from(422),
            Self::Workflow(WorkflowError::Pipeline(_)) => StatusCode::from(500),
            Self::Serialization(_) => StatusCode::from(500),
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

#[derive(Debug)]
struct SyncScanExecution {
    paths: Vec<PathBuf>,
    options: ScanOptions,
    _staging_dir: Option<TempDir>,
}

struct ParsedRequest {
    method: tiny_http::Method,
    path: String,
    headers: Vec<Header>,
    body: Vec<u8>,
}

struct HttpResponse {
    status: StatusCode,
    body: String,
}

fn json_response<T: Serialize>(status: StatusCode, body: &T) -> HttpResponse {
    let serialized = serde_json::to_string(body).expect("response body should serialize");
    HttpResponse {
        status,
        body: serialized,
    }
}

fn error_response(status: StatusCode, status_str: &'static str, message: String) -> HttpResponse {
    json_response(
        status,
        &crate::serve_api::ServeErrorResponse {
            status: status_str.to_string(),
            message,
            api_version: API_VERSION.to_string(),
        },
    )
}

fn into_tiny_response(response: HttpResponse) -> Response<std::io::Cursor<Vec<u8>>> {
    let content_type = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("Content-Type header should be valid ASCII");

    let body_bytes = response.body.into_bytes();
    let len = body_bytes.len();

    Response::new(
        response.status,
        vec![content_type],
        std::io::Cursor::new(body_bytes),
        Some(len),
        None,
    )
}

pub(crate) fn run(args: &ServeArgs) -> Result<()> {
    let config = ServeConfig::try_from(args)?;

    let server = Server::http(&config.bind)
        .map_err(|e| anyhow!("Failed to bind provenant serve to {}: {e}", config.bind))?;
    let local_addr = server.server_addr();

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
    serve_forever(server, state)
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

fn serve_forever(server: Server, state: ServeState) -> Result<()> {
    for request in server.incoming_requests() {
        if let Err(error) = handle_request(request, &state) {
            eprintln!("serve request handling error: {error}");
        }
    }

    Ok(())
}

fn handle_request(mut request: tiny_http::Request, state: &ServeState) -> Result<()> {
    let parsed = match parse_request(&mut request) {
        Ok(parsed) => parsed,
        Err(ServeError::Ingest(IngestError::PayloadTooLarge(_))) => {
            let response = error_response(
                StatusCode::from(413),
                "payload_too_large",
                "request body exceeds max size".to_string(),
            );
            request.respond(into_tiny_response(response))?;
            return Ok(());
        }
        Err(error) => {
            let response =
                error_response(StatusCode::from(400), "invalid_request", error.to_string());
            request.respond(into_tiny_response(response))?;
            return Ok(());
        }
    };

    let response = response_for_request(&parsed, state);
    request.respond(into_tiny_response(response))?;
    Ok(())
}

fn parse_request(request: &mut tiny_http::Request) -> Result<ParsedRequest, ServeError> {
    let content_length = request.body_length().unwrap_or(0);

    if content_length > MAX_REQUEST_BODY_BYTES {
        return Err(ServeError::Ingest(IngestError::PayloadTooLarge(format!(
            "request body exceeds max size of {} bytes",
            MAX_REQUEST_BODY_BYTES
        ))));
    }

    let mut body = Vec::with_capacity(content_length);
    request
        .as_reader()
        .take(MAX_REQUEST_BODY_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|e| {
            ServeError::Ingest(IngestError::Internal {
                message: "failed to read request body".to_string(),
                source: Some(anyhow::Error::new(e)),
            })
        })?;

    if body.len() > MAX_REQUEST_BODY_BYTES {
        return Err(ServeError::Ingest(IngestError::PayloadTooLarge(format!(
            "request body exceeds max size of {} bytes",
            MAX_REQUEST_BODY_BYTES
        ))));
    }

    Ok(ParsedRequest {
        method: request.method().clone(),
        path: request.url().to_string(),
        headers: request.headers().to_vec(),
        body,
    })
}

fn response_for_request(request: &ParsedRequest, state: &ServeState) -> HttpResponse {
    let readiness = state
        .readiness
        .lock()
        .expect("serve readiness lock should not be poisoned")
        .clone();

    if let Some(job_route) = parse_job_route(&request.path) {
        return match job_route {
            JobRoute::Status(job_id) => {
                if request.method == tiny_http::Method::Get {
                    handle_job_status_request(job_id, state)
                } else {
                    error_response(
                        StatusCode::from(405),
                        "method_not_allowed",
                        format!("use GET /v1/jobs/{job_id} to inspect async job state"),
                    )
                }
            }
            JobRoute::Result(job_id) => {
                if request.method == tiny_http::Method::Get {
                    handle_job_result_request(job_id, state)
                } else {
                    error_response(
                        StatusCode::from(405),
                        "method_not_allowed",
                        format!("use GET /v1/jobs/{job_id}/result to fetch async job output"),
                    )
                }
            }
        };
    }

    match (&request.method, request.path.as_str()) {
        (m, "/livez") if *m == tiny_http::Method::Get => json_response(
            StatusCode::from(200),
            &crate::serve_api::ServeLivenessResponse {
                status: "ok".to_string(),
            },
        ),
        (m, "/readyz") if *m == tiny_http::Method::Get => match readiness {
            ReadinessState::Pending => json_response(
                StatusCode::from(503),
                &crate::serve_api::ServeReadinessResponse {
                    status: "warming".to_string(),
                    api_version: None,
                    spdx_license_list_version: None,
                    message: None,
                },
            ),
            ReadinessState::Ready {
                spdx_license_list_version,
            } => json_response(
                StatusCode::from(200),
                &crate::serve_api::ServeReadinessResponse {
                    status: "ready".to_string(),
                    api_version: Some(API_VERSION.to_string()),
                    spdx_license_list_version: Some(spdx_license_list_version),
                    message: None,
                },
            ),
            ReadinessState::Failed { message } => json_response(
                StatusCode::from(503),
                &crate::serve_api::ServeReadinessResponse {
                    status: "failed".to_string(),
                    api_version: None,
                    spdx_license_list_version: None,
                    message: Some(message),
                },
            ),
        },
        (m, "/version") if *m == tiny_http::Method::Get => json_response(
            StatusCode::from(200),
            &crate::serve_api::ServeVersionResponse {
                service: "provenant-serve".to_string(),
                api_version: API_VERSION.to_string(),
                tool_version: BUILD_VERSION.to_string(),
            },
        ),
        (m, "/v1/scans") if *m == tiny_http::Method::Post => {
            handle_sync_scan_request(request).unwrap_or_else(|e| e)
        }
        (_, "/v1/scans") => error_response(
            StatusCode::from(405),
            "method_not_allowed",
            "use POST /v1/scans for synchronous scan execution".to_string(),
        ),
        (m, "/v1/scans:async") if *m == tiny_http::Method::Post => {
            handle_async_scan_request(request, state).unwrap_or_else(|e| e)
        }
        (_, "/v1/scans:async") => error_response(
            StatusCode::from(405),
            "method_not_allowed",
            "use POST /v1/scans:async for asynchronous scan submission".to_string(),
        ),
        _ => HttpResponse {
            status: StatusCode::from(404),
            body: json!({ "status": "not_found", "path": request.path }).to_string(),
        },
    }
}

type HandlerResult = Result<HttpResponse, HttpResponse>;

impl From<ServeError> for HttpResponse {
    fn from(error: ServeError) -> HttpResponse {
        error_response(
            error.http_status_code(),
            error.error_type(),
            error.to_string(),
        )
    }
}

fn handle_sync_scan_request(request: &ParsedRequest) -> HandlerResult {
    let sync_request = decode_scan_request_from_http(request, "POST /v1/scans")?;
    let execution = build_sync_scan_execution(sync_request)?;
    match execute_scan_execution(execution) {
        Ok(body) => Ok(HttpResponse {
            status: StatusCode::from(200),
            body,
        }),
        Err(error) => Err(error.into()),
    }
}

fn handle_async_scan_request(request: &ParsedRequest, state: &ServeState) -> HandlerResult {
    let sync_request = decode_scan_request_from_http(request, "POST /v1/scans:async")?;
    let (response, dispatches) =
        state
            .jobs
            .submit(sync_request)
            .map_err(|AsyncSubmitError::QueueFull| {
                error_response(
                    StatusCode::from(503),
                    "server_busy",
                    "async job queue is full; try again later".to_string(),
                )
            })?;
    spawn_dispatches(state.jobs.clone(), dispatches);
    Ok(json_response(StatusCode::from(202), &response))
}

fn handle_job_status_request(job_id: &str, state: &ServeState) -> HttpResponse {
    match state.jobs.status_snapshot(job_id) {
        Some(snapshot) => json_response(StatusCode::from(200), &snapshot.into_status_response()),
        None => error_response(
            StatusCode::from(404),
            "job_not_found",
            format!("async job {job_id} was not found"),
        ),
    }
}

fn handle_job_result_request(job_id: &str, state: &ServeState) -> HttpResponse {
    let Some(snapshot) = state.jobs.result_snapshot(job_id) else {
        return error_response(
            StatusCode::from(404),
            "job_not_found",
            format!("async job {job_id} was not found"),
        );
    };

    match snapshot.state {
        AsyncJobState::Succeeded => HttpResponse {
            status: StatusCode::from(200),
            body: snapshot
                .result_body
                .expect("successful async job should retain result body"),
        },
        AsyncJobState::Pending | AsyncJobState::Running => error_response(
            StatusCode::from(409),
            "job_not_ready",
            format!(
                "async job {job_id} is currently {}",
                match snapshot.state {
                    AsyncJobState::Pending => "pending",
                    AsyncJobState::Running => "running",
                    _ => unreachable!(),
                }
            ),
        ),
        AsyncJobState::Failed => {
            let status_code = snapshot
                .error_status_code
                .and_then(|code| {
                    let status = StatusCode::from(code);
                    if status.0 >= 400 && status.0 < 600 {
                        Some(status)
                    } else {
                        None
                    }
                })
                .unwrap_or(StatusCode::from(500));
            error_response(
                status_code,
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
    request: &ParsedRequest,
    route_label: &str,
) -> std::result::Result<SyncScanRequest, HttpResponse> {
    let has_json_content_type = request
        .headers
        .iter()
        .any(|h| h.field.equiv("Content-Type") && h.value.as_str().starts_with("application/json"));

    if !has_json_content_type {
        return Err(error_response(
            StatusCode::from(415),
            "unsupported_media_type",
            format!("{route_label} requires Content-Type: application/json"),
        ));
    }

    decode_sync_scan_request(&request.body).map_err(|error| {
        error_response(StatusCode::from(400), "invalid_request", error.to_string())
    })
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

fn spawn_dispatches(controller: AsyncJobController, dispatches: Vec<DispatchedAsyncJob>) {
    for dispatched in dispatches {
        let controller = controller.clone();
        thread::spawn(move || {
            let outcome =
                match run_async_scan_request(dispatched.request, dispatched.allocated_processors) {
                    Ok(result_body) => {
                        eprintln!("serve async job {} succeeded", dispatched.job_id);
                        JobOutcome::Succeeded { result_body }
                    }
                    Err(error) => {
                        eprintln!("serve async job {} failed: {error}", dispatched.job_id);
                        JobOutcome::Failed {
                            message: "async scan job failed".to_string(),
                            status_code: error.http_status_code().0,
                        }
                    }
                };
            let follow_up_dispatches = controller.complete_job(
                dispatched.job_id,
                dispatched.allocated_processors,
                outcome,
            );
            spawn_dispatches(controller, follow_up_dispatches);
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serve::job_controller::AsyncJobController;
    use crate::serve_api::{SyncScanInput, SyncScanOptions};

    fn test_request(method: tiny_http::Method, path: &str) -> ParsedRequest {
        ParsedRequest {
            method,
            path: path.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn assert_status(response: &HttpResponse, expected: u16) {
        assert_eq!(response.status.0, expected);
    }

    fn ready_state() -> ServeState {
        ServeState {
            readiness: Arc::new(Mutex::new(ReadinessState::Ready {
                spdx_license_list_version: "3.28".to_string(),
            })),
            jobs: AsyncJobController::with_limits(2, 2, 8),
        }
    }

    fn ready_state_with_job(
        job_id: &str,
        record: crate::serve::job_controller::AsyncJobRecord,
    ) -> ServeState {
        let state = ready_state();
        state.jobs.insert_job(job_id.to_string(), record);
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
        let response = response_for_request(
            &test_request(tiny_http::Method::Get, "/readyz"),
            &ready_state(),
        );
        assert_status(&response, 200);
        assert!(response.body.contains("\"status\":\"ready\""));
        assert!(response.body.contains("\"api_version\":\"v1\""));
    }

    #[test]
    fn async_scan_route_requires_post() {
        let response = response_for_request(
            &test_request(tiny_http::Method::Get, "/v1/scans:async"),
            &ready_state(),
        );
        assert_status(&response, 405);
        assert!(response.body.contains("method_not_allowed"));
    }

    #[test]
    fn pending_job_status_reports_pending_state() {
        let state = ready_state_with_job(
            "job-1",
            crate::serve::job_controller::AsyncJobRecord::pending(),
        );
        let response = response_for_request(
            &test_request(tiny_http::Method::Get, "/v1/jobs/job-1"),
            &state,
        );
        assert_status(&response, 200);
        assert!(response.body.contains("\"state\":\"pending\""));
        assert!(response.body.contains("\"result_ready\":false"));
    }

    #[test]
    fn pending_job_result_reports_not_ready() {
        let state = ready_state_with_job(
            "job-2",
            crate::serve::job_controller::AsyncJobRecord::pending(),
        );
        let response = response_for_request(
            &test_request(tiny_http::Method::Get, "/v1/jobs/job-2/result"),
            &state,
        );
        assert_status(&response, 409);
        assert!(response.body.contains("job_not_ready"));
    }

    #[test]
    fn completed_job_result_returns_stored_body() {
        let state = ready_state_with_job(
            "job-3",
            crate::serve::job_controller::AsyncJobRecord::succeeded(
                "{\"status\":\"ok\"}".to_string(),
                2,
            ),
        );
        let response = response_for_request(
            &test_request(tiny_http::Method::Get, "/v1/jobs/job-3/result"),
            &state,
        );
        assert_status(&response, 200);
        assert_eq!(response.body, "{\"status\":\"ok\"}");
    }

    #[test]
    fn failed_job_result_returns_failure_contract() {
        let state = ready_state_with_job(
            "job-4",
            crate::serve::job_controller::AsyncJobRecord::failed(
                "async scan job failed".to_string(),
                500,
                1,
            ),
        );
        let response = response_for_request(
            &test_request(tiny_http::Method::Get, "/v1/jobs/job-4/result"),
            &state,
        );
        assert_status(&response, 500);
        assert!(response.body.contains("job_failed"));
        assert!(response.body.contains("async scan job failed"));
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
        let response =
            handle_sync_scan_request(&test_request(tiny_http::Method::Post, "/v1/scans"))
                .unwrap_or_else(|e| e);
        assert_status(&response, 415);
        assert!(response.body.contains("unsupported_media_type"));
    }
}
