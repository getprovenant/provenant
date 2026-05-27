// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

mod ingest;
mod job_controller;

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::json;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::cli::ServeArgs;
use crate::license_detection::LicenseDetectionEngine;
use crate::serve::ingest::{IngestError, ScanError, SyncScanExecution};
use crate::serve::job_controller::{
    AsyncJobController, AsyncSubmitError, DispatchedAsyncJob, JobOutcome,
};
use crate::serve_api::{API_VERSION, AsyncJobState, SyncScanRequest};
use crate::version::BUILD_VERSION;
use crate::workflow::WorkflowError;

const MAX_REQUEST_BODY_BYTES: usize = 24 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ServeError {
    #[error(transparent)]
    Ingest(#[from] IngestError),
    #[error(transparent)]
    Scan(#[from] ScanError),
    #[error(transparent)]
    Workflow(#[from] WorkflowError),
}

impl ServeError {
    pub(crate) fn http_status_code(&self) -> StatusCode {
        match self {
            Self::Ingest(IngestError::Validation(_)) => StatusCode::from(422),
            Self::Ingest(IngestError::PayloadTooLarge(_)) => StatusCode::from(413),
            Self::Ingest(IngestError::Upstream { .. }) => StatusCode::from(502),
            Self::Ingest(IngestError::Internal { .. }) => StatusCode::from(500),
            Self::Scan(ScanError::Workflow(WorkflowError::InvalidOptions(_))) => {
                StatusCode::from(422)
            }
            Self::Scan(ScanError::Workflow(WorkflowError::Pipeline(_))) => StatusCode::from(500),
            Self::Scan(ScanError::Serialization(_)) => StatusCode::from(500),
            Self::Workflow(WorkflowError::InvalidOptions(_)) => StatusCode::from(422),
            Self::Workflow(WorkflowError::Pipeline(_)) => StatusCode::from(500),
        }
    }

    pub(crate) fn error_type(&self) -> &'static str {
        match self {
            Self::Ingest(IngestError::Validation(_)) => "invalid_scan_request",
            Self::Ingest(IngestError::PayloadTooLarge(_)) => "payload_too_large",
            Self::Ingest(IngestError::Upstream { .. }) => "upstream_error",
            Self::Ingest(IngestError::Internal { .. }) => "internal_error",
            Self::Scan(ScanError::Workflow(WorkflowError::InvalidOptions(_))) => {
                "invalid_scan_request"
            }
            Self::Scan(ScanError::Workflow(WorkflowError::Pipeline(_))) => "scan_failed",
            Self::Scan(ScanError::Serialization(_)) => "scan_failed",
            Self::Workflow(WorkflowError::InvalidOptions(_)) => "invalid_scan_request",
            Self::Workflow(WorkflowError::Pipeline(_)) => "scan_failed",
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

struct ParsedRequest {
    method: Method,
    path: String,
    headers: Vec<Header>,
    body: Vec<u8>,
}

struct HttpResponse {
    status: StatusCode,
    body: String,
}

impl HttpResponse {
    fn json<T: Serialize>(status: StatusCode, body: &T) -> Self {
        let serialized = serde_json::to_string(body).expect("response body should serialize");
        Self {
            status,
            body: serialized,
        }
    }

    fn error(status: StatusCode, status_str: &'static str, message: String) -> Self {
        Self::json(
            status,
            &crate::serve_api::ServeErrorResponse {
                status: status_str.to_string(),
                message,
                api_version: API_VERSION.to_string(),
            },
        )
    }

    fn into_tiny_response(self) -> Response<std::io::Cursor<Vec<u8>>> {
        let content_type = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
            .expect("Content-Type header should be valid ASCII");

        let body_bytes = self.body.into_bytes();
        let len = body_bytes.len();

        Response::new(
            self.status,
            vec![content_type],
            std::io::Cursor::new(body_bytes),
            Some(len),
            None,
        )
    }
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
            let response = HttpResponse::error(
                StatusCode::from(413),
                "payload_too_large",
                "request body exceeds max size".to_string(),
            );
            request.respond(response.into_tiny_response())?;
            return Ok(());
        }
        Err(error) => {
            let response =
                HttpResponse::error(StatusCode::from(400), "invalid_request", error.to_string());
            request.respond(response.into_tiny_response())?;
            return Ok(());
        }
    };

    let response = response_for_request(&parsed, state);
    request.respond(response.into_tiny_response())?;
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
                if request.method == Method::Get {
                    handle_job_status_request(job_id, state)
                } else {
                    HttpResponse::error(
                        StatusCode::from(405),
                        "method_not_allowed",
                        format!("use GET /v1/jobs/{job_id} to inspect async job state"),
                    )
                }
            }
            JobRoute::Result(job_id) => {
                if request.method == Method::Get {
                    handle_job_result_request(job_id, state)
                } else {
                    HttpResponse::error(
                        StatusCode::from(405),
                        "method_not_allowed",
                        format!("use GET /v1/jobs/{job_id}/result to fetch async job output"),
                    )
                }
            }
        };
    }

    match (&request.method, request.path.as_str()) {
        (m, "/livez") if *m == Method::Get => HttpResponse::json(
            StatusCode::from(200),
            &crate::serve_api::ServeLivenessResponse {
                status: "ok".to_string(),
            },
        ),
        (m, "/readyz") if *m == Method::Get => match readiness {
            ReadinessState::Pending => HttpResponse::json(
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
            } => HttpResponse::json(
                StatusCode::from(200),
                &crate::serve_api::ServeReadinessResponse {
                    status: "ready".to_string(),
                    api_version: Some(API_VERSION.to_string()),
                    spdx_license_list_version: Some(spdx_license_list_version),
                    message: None,
                },
            ),
            ReadinessState::Failed { message } => HttpResponse::json(
                StatusCode::from(503),
                &crate::serve_api::ServeReadinessResponse {
                    status: "failed".to_string(),
                    api_version: None,
                    spdx_license_list_version: None,
                    message: Some(message),
                },
            ),
        },
        (m, "/version") if *m == Method::Get => HttpResponse::json(
            StatusCode::from(200),
            &crate::serve_api::ServeVersionResponse {
                service: "provenant-serve".to_string(),
                api_version: API_VERSION.to_string(),
                tool_version: BUILD_VERSION.to_string(),
            },
        ),
        (m, "/v1/scans") if *m == Method::Post => {
            handle_sync_scan_request(request).unwrap_or_else(|e| e)
        }
        (_, "/v1/scans") => HttpResponse::error(
            StatusCode::from(405),
            "method_not_allowed",
            "use POST /v1/scans for synchronous scan execution".to_string(),
        ),
        (m, "/v1/scans:async") if *m == Method::Post => {
            handle_async_scan_request(request, state).unwrap_or_else(|e| e)
        }
        (_, "/v1/scans:async") => HttpResponse::error(
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
        HttpResponse::error(
            error.http_status_code(),
            error.error_type(),
            error.to_string(),
        )
    }
}

fn handle_sync_scan_request(request: &ParsedRequest) -> HandlerResult {
    let sync_request = decode_scan_request_from_http(request, "POST /v1/scans")?;
    let execution = SyncScanExecution::new(sync_request)
        .map_err(|e| HttpResponse::from(ServeError::from(e)))?;
    match execution.execute() {
        Ok(body) => Ok(HttpResponse {
            status: StatusCode::from(200),
            body,
        }),
        Err(error) => Err(ServeError::from(error).into()),
    }
}

fn handle_async_scan_request(request: &ParsedRequest, state: &ServeState) -> HandlerResult {
    let sync_request = decode_scan_request_from_http(request, "POST /v1/scans:async")?;
    let (response, dispatches) =
        state
            .jobs
            .submit(sync_request)
            .map_err(|AsyncSubmitError::QueueFull| {
                HttpResponse::error(
                    StatusCode::from(503),
                    "server_busy",
                    "async job queue is full; try again later".to_string(),
                )
            })?;
    spawn_dispatches(state.jobs.clone(), dispatches);
    Ok(HttpResponse::json(StatusCode::from(202), &response))
}

fn handle_job_status_request(job_id: &str, state: &ServeState) -> HttpResponse {
    match state.jobs.status_snapshot(job_id) {
        Some(snapshot) => {
            HttpResponse::json(StatusCode::from(200), &snapshot.into_status_response())
        }
        None => HttpResponse::error(
            StatusCode::from(404),
            "job_not_found",
            format!("async job {job_id} was not found"),
        ),
    }
}

fn handle_job_result_request(job_id: &str, state: &ServeState) -> HttpResponse {
    let Some(snapshot) = state.jobs.result_snapshot(job_id) else {
        return HttpResponse::error(
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
        AsyncJobState::Pending | AsyncJobState::Running => HttpResponse::error(
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
            HttpResponse::error(
                status_code,
                "job_failed",
                snapshot
                    .error_message
                    .unwrap_or_else(|| format!("async job {job_id} failed")),
            )
        }
    }
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
        return Err(HttpResponse::error(
            StatusCode::from(415),
            "unsupported_media_type",
            format!("{route_label} requires Content-Type: application/json"),
        ));
    }

    SyncScanRequest::decode(&request.body).map_err(|error| {
        HttpResponse::error(StatusCode::from(400), "invalid_request", error.to_string())
    })
}

fn spawn_dispatches(controller: AsyncJobController, dispatches: Vec<DispatchedAsyncJob>) {
    for dispatched in dispatches {
        let controller = controller.clone();
        thread::spawn(move || {
            let result = SyncScanExecution::new(dispatched.request)
                .map_err(ServeError::from)
                .and_then(|e| {
                    e.run_async(dispatched.allocated_processors)
                        .map_err(ServeError::from)
                });

            let outcome = match result {
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

    fn test_request(method: Method, path: &str) -> ParsedRequest {
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
        let response = response_for_request(&test_request(Method::Get, "/readyz"), &ready_state());
        assert_status(&response, 200);
        assert!(response.body.contains("\"status\":\"ready\""));
        assert!(response.body.contains("\"api_version\":\"v1\""));
    }

    #[test]
    fn async_scan_route_requires_post() {
        let response = response_for_request(
            &test_request(Method::Get, "/v1/scans:async"),
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
        let response = response_for_request(&test_request(Method::Get, "/v1/jobs/job-1"), &state);
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
        let response =
            response_for_request(&test_request(Method::Get, "/v1/jobs/job-2/result"), &state);
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
        let response =
            response_for_request(&test_request(Method::Get, "/v1/jobs/job-3/result"), &state);
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
        let response =
            response_for_request(&test_request(Method::Get, "/v1/jobs/job-4/result"), &state);
        assert_status(&response, 500);
        assert!(response.body.contains("job_failed"));
        assert!(response.body.contains("async scan job failed"));
    }

    #[test]
    fn decode_sync_scan_request_rejects_empty_paths() {
        let error = SyncScanExecution::new(SyncScanRequest {
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
        let error = SyncScanExecution::new(SyncScanRequest {
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
            SyncScanRequest::decode(br#"{"input": }"#).expect_err("malformed JSON should fail");

        assert!(
            error
                .to_string()
                .contains("request body must be valid JSON")
        );
    }

    #[test]
    fn sync_scan_requires_json_content_type() {
        let response = handle_sync_scan_request(&test_request(Method::Post, "/v1/scans"))
            .unwrap_or_else(|e| e);
        assert_status(&response, 415);
        assert!(response.body.contains("unsupported_media_type"));
    }
}
