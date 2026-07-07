// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

mod ingest;
mod job_controller;

use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::json;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::cli::ServeArgs;
use crate::license_detection::LicenseDetectionEngine;
use crate::serve::ingest::{
    IngestError, IngestPolicy, ScanError, SyncScanExecution, validate_input_policy,
};
use crate::serve::job_controller::{
    AsyncJobController, AsyncSubmitError, DispatchedAsyncJob, JobOutcome, JobResult,
};
use crate::serve_api::{API_VERSION, ServeScanRequest};
use crate::version::BUILD_VERSION;
use crate::workflow::WorkflowError;

const MAX_REQUEST_BODY_BYTES: usize = 24 * 1024 * 1024;
const DEFAULT_SYNC_SCAN_WORKERS: usize = 1;
const DEFAULT_SYNC_SCAN_QUEUE_CAPACITY: usize = 4;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ServeError {
    #[error(transparent)]
    Ingest(#[from] IngestError),
    #[error(transparent)]
    Scan(#[from] ScanError),
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
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ServeConfig {
    bind: String,
    ingest_policy: IngestPolicy,
}

impl TryFrom<&ServeArgs> for ServeConfig {
    type Error = anyhow::Error;

    fn try_from(args: &ServeArgs) -> Result<Self> {
        if args.bind.trim().is_empty() {
            return Err(anyhow!("--bind must not be empty"));
        }

        let loopback_bind = bind_allows_privileged_inputs_by_default(&args.bind);
        // An explicit --allow-privileged-inputs means the operator trusts these
        // inputs, including local/private fetch targets. A loopback bind only
        // enables privileged input *types*; it keeps SSRF protection on so a
        // default localhost server cannot be tricked into reaching internal or
        // cloud-metadata addresses.
        let ingest_policy = if args.allow_privileged_inputs {
            IngestPolicy::trust_local_targets()
        } else if loopback_bind {
            IngestPolicy::allow_privileged_inputs()
        } else {
            IngestPolicy::upload_only()
        };

        Ok(Self {
            bind: args.bind.clone(),
            ingest_policy,
        })
    }
}

fn bind_allows_privileged_inputs_by_default(bind: &str) -> bool {
    if let Ok(addr) = bind.parse::<SocketAddr>() {
        return addr.ip().is_loopback();
    }

    let Some((host, _port)) = bind.rsplit_once(':') else {
        return false;
    };
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

#[derive(Debug, Clone)]
struct ServeState {
    spdx_license_list_version: String,
    jobs: AsyncJobController,
    ingest_policy: IngestPolicy,
}

#[derive(Debug, Clone, Copy)]
struct RequestWorkerConfig {
    general_workers: usize,
    sync_scan_workers: usize,
    general_queue_capacity: usize,
    sync_scan_queue_capacity: usize,
}

impl RequestWorkerConfig {
    fn for_parallelism(available_parallelism: usize) -> Self {
        let general_workers = available_parallelism.clamp(2, 4);
        Self {
            general_workers,
            sync_scan_workers: DEFAULT_SYNC_SCAN_WORKERS,
            general_queue_capacity: general_workers * 16,
            sync_scan_queue_capacity: DEFAULT_SYNC_SCAN_QUEUE_CAPACITY,
        }
    }
}

impl Default for RequestWorkerConfig {
    fn default() -> Self {
        let cpus = thread::available_parallelism().map_or(1, |count| count.get());
        Self::for_parallelism(cpus)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestLane {
    General,
    SyncScan,
}

struct RequestDispatcher {
    general: RequestQueue,
    sync_scan: RequestQueue,
}

struct RequestQueue {
    label: &'static str,
    busy_message: &'static str,
    sender: SyncSender<tiny_http::Request>,
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

    let spdx_license_list_version = LicenseDetectionEngine::embedded_spdx_license_list_version()
        .map_err(|e| anyhow!("license detection engine failed to initialize: {e}"))?;

    let server = Server::http(&config.bind)
        .map_err(|e| anyhow!("Failed to bind provenant serve to {}: {e}", config.bind))?;
    let local_addr = server.server_addr();

    log::info!("Starting provenant serve on http://{local_addr} (api {API_VERSION})");
    if !config.ingest_policy.privileged_inputs_allowed() {
        // Warn (not info) so this security-boundary notice still surfaces under
        // `serve --quiet`, which raises the log level to warn.
        log::warn!(
            "Privileged serve inputs (paths, url, repository) are disabled for this non-loopback bind; use --allow-privileged-inputs only for trusted deployments."
        );
    }

    let state = ServeState {
        spdx_license_list_version,
        jobs: AsyncJobController::new(),
        ingest_policy: config.ingest_policy,
    };
    serve_forever(server, state)
}

fn serve_forever(server: Server, state: ServeState) -> Result<()> {
    let dispatcher = RequestDispatcher::start(state, RequestWorkerConfig::default());
    for request in server.incoming_requests() {
        if let Err(error) = dispatcher.dispatch(request) {
            log::error!("serve request dispatch error: {error}");
        }
    }

    Ok(())
}

impl RequestDispatcher {
    fn start(state: ServeState, config: RequestWorkerConfig) -> Self {
        let state = Arc::new(state);
        Self {
            general: RequestQueue::start(
                "general",
                "request queue is full; try again later",
                config.general_workers,
                config.general_queue_capacity,
                Arc::clone(&state),
            ),
            sync_scan: RequestQueue::start(
                "sync-scan",
                "synchronous scan queue is full; try again later",
                config.sync_scan_workers,
                config.sync_scan_queue_capacity,
                state,
            ),
        }
    }

    fn dispatch(&self, request: tiny_http::Request) -> Result<()> {
        match request_lane(request.method(), request.url()) {
            RequestLane::General => self.general.try_dispatch(request),
            RequestLane::SyncScan => self.sync_scan.try_dispatch(request),
        }
    }
}

impl RequestQueue {
    fn start(
        label: &'static str,
        busy_message: &'static str,
        worker_count: usize,
        queue_capacity: usize,
        state: Arc<ServeState>,
    ) -> Self {
        let (sender, receiver) = sync_channel(queue_capacity.max(1));
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..worker_count.max(1) {
            let worker_receiver = Arc::clone(&receiver);
            let worker_state = Arc::clone(&state);
            thread::spawn(move || {
                request_worker_loop(label, index, worker_receiver, worker_state);
            });
        }

        Self {
            label,
            busy_message,
            sender,
        }
    }

    fn try_dispatch(&self, request: tiny_http::Request) -> Result<()> {
        match self.sender.try_send(request) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(request)) => respond_server_busy(request, self.busy_message),
            Err(TrySendError::Disconnected(request)) => {
                respond_server_busy(request, self.busy_message)?;
                Err(anyhow!("serve {} request workers stopped", self.label))
            }
        }
    }
}

fn request_worker_loop(
    label: &'static str,
    index: usize,
    receiver: Arc<Mutex<Receiver<tiny_http::Request>>>,
    state: Arc<ServeState>,
) {
    loop {
        let request = {
            let receiver = receiver
                .lock()
                .expect("serve request queue lock should not be poisoned");
            receiver.recv()
        };

        match request {
            Ok(request) => {
                if let Err(error) = handle_request(request, &state) {
                    log::error!("serve {label} worker {index} request handling error: {error}");
                }
            }
            Err(_) => break,
        }
    }
}

// Synchronous scans can run for a long time, so keep them out of the
// general request lane used by health, job polling, and async submission.
fn request_lane(method: &Method, url: &str) -> RequestLane {
    if *method == Method::Post && url == "/v1/scans" {
        RequestLane::SyncScan
    } else {
        RequestLane::General
    }
}

fn respond_server_busy(request: tiny_http::Request, message: &'static str) -> Result<()> {
    let response = HttpResponse::error(StatusCode::from(503), "server_busy", message.to_string());
    request.respond(response.into_tiny_response())?;
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

    // Early-out: reject oversized Content-Length before reading the body.
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

    // Second check: catches chunked transfer encoding where body_length()
    // returns 0 and the actual size only becomes known after reading.
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
        (m, "/readyz") if *m == Method::Get => HttpResponse::json(
            StatusCode::from(200),
            &crate::serve_api::ServeReadinessResponse {
                status: "ready".to_string(),
                api_version: Some(API_VERSION.to_string()),
                spdx_license_list_version: Some(state.spdx_license_list_version.clone()),
                message: None,
            },
        ),
        (m, "/version") if *m == Method::Get => HttpResponse::json(
            StatusCode::from(200),
            &crate::serve_api::ServeVersionResponse {
                service: "provenant-serve".to_string(),
                api_version: API_VERSION.to_string(),
                tool_version: BUILD_VERSION.to_string(),
            },
        ),
        (m, "/v1/scans") if *m == Method::Post => handle_sync_scan_request(request, state),
        (_, "/v1/scans") => HttpResponse::error(
            StatusCode::from(405),
            "method_not_allowed",
            "use POST /v1/scans for synchronous scan execution".to_string(),
        ),
        (m, "/v1/scans:async") if *m == Method::Post => handle_async_scan_request(request, state),
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

impl From<ServeError> for HttpResponse {
    fn from(error: ServeError) -> HttpResponse {
        HttpResponse::error(
            error.http_status_code(),
            error.error_type(),
            error.to_string(),
        )
    }
}

fn handle_sync_scan_request(request: &ParsedRequest, state: &ServeState) -> HttpResponse {
    let sync_request = match decode_scan_request_from_http(request, "POST /v1/scans") {
        Ok(req) => req,
        Err(resp) => return resp,
    };
    let execution = match SyncScanExecution::new(sync_request, state.ingest_policy) {
        Ok(e) => e,
        Err(e) => return HttpResponse::from(ServeError::from(e)),
    };
    match execution.execute() {
        Ok(body) => HttpResponse {
            status: StatusCode::from(200),
            body,
        },
        Err(error) => HttpResponse::from(ServeError::from(error)),
    }
}

fn handle_async_scan_request(request: &ParsedRequest, state: &ServeState) -> HttpResponse {
    let sync_request = match decode_scan_request_from_http(request, "POST /v1/scans:async") {
        Ok(req) => req,
        Err(resp) => return resp,
    };
    if let Err(error) = validate_input_policy(&sync_request.input, state.ingest_policy) {
        return HttpResponse::from(ServeError::from(error));
    }
    let (response, dispatches) = match state.jobs.submit(sync_request) {
        Ok(result) => result,
        Err(AsyncSubmitError::QueueFull) => {
            return HttpResponse::error(
                StatusCode::from(503),
                "server_busy",
                "async job queue is full; try again later".to_string(),
            );
        }
    };
    spawn_dispatches(state.jobs.clone(), dispatches, state.ingest_policy);
    HttpResponse::json(StatusCode::from(202), &response)
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
    let Some(result) = state.jobs.get_job_result(job_id) else {
        return HttpResponse::error(
            StatusCode::from(404),
            "job_not_found",
            format!("async job {job_id} was not found"),
        );
    };

    match result {
        JobResult::Succeeded { result_body } => HttpResponse {
            status: StatusCode::from(200),
            body: result_body,
        },
        JobResult::Pending => HttpResponse::error(
            StatusCode::from(409),
            "job_not_ready",
            format!("async job {job_id} is currently pending"),
        ),
        JobResult::Running => HttpResponse::error(
            StatusCode::from(409),
            "job_not_ready",
            format!("async job {job_id} is currently running"),
        ),
        JobResult::Failed {
            message,
            status_code,
        } => {
            let status = if (400..600).contains(&status_code) {
                StatusCode::from(status_code)
            } else {
                StatusCode::from(500)
            };
            HttpResponse::error(
                status,
                "job_failed",
                if message.is_empty() {
                    format!("async job {job_id} failed")
                } else {
                    message
                },
            )
        }
    }
}

fn decode_scan_request_from_http(
    request: &ParsedRequest,
    route_label: &str,
) -> std::result::Result<ServeScanRequest, HttpResponse> {
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

    ServeScanRequest::decode(&request.body).map_err(|error| {
        HttpResponse::error(StatusCode::from(400), "invalid_request", error.to_string())
    })
}

fn spawn_dispatches(
    controller: AsyncJobController,
    dispatches: Vec<DispatchedAsyncJob>,
    ingest_policy: IngestPolicy,
) {
    for dispatched in dispatches {
        let controller = controller.clone();
        thread::spawn(move || {
            let result = SyncScanExecution::new(dispatched.request, ingest_policy)
                .map_err(ServeError::from)
                .and_then(|e| {
                    e.run_async(dispatched.allocated_processors)
                        .map_err(ServeError::from)
                });

            let outcome = match result {
                Ok(result_body) => {
                    log::info!("serve async job {} succeeded", dispatched.job_id);
                    JobOutcome::Succeeded { result_body }
                }
                Err(error) => {
                    log::error!("serve async job {} failed: {error}", dispatched.job_id);
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
            spawn_dispatches(controller, follow_up_dispatches, ingest_policy);
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

    #[test]
    fn request_lane_routes_only_sync_scan_posts_to_sync_pool() {
        assert_eq!(
            request_lane(&Method::Post, "/v1/scans"),
            RequestLane::SyncScan
        );
        assert_eq!(
            request_lane(&Method::Post, "/v1/scans?foo=bar"),
            RequestLane::General
        );
        assert_eq!(
            request_lane(&Method::Get, "/v1/scans"),
            RequestLane::General
        );
        assert_eq!(
            request_lane(&Method::Post, "/v1/scans:async"),
            RequestLane::General
        );
        assert_eq!(request_lane(&Method::Get, "/readyz"), RequestLane::General);
        assert_eq!(
            request_lane(&Method::Get, "/v1/jobs/job-1"),
            RequestLane::General
        );
        assert_eq!(
            request_lane(&Method::Get, "/v1/jobs/job-1/result"),
            RequestLane::General
        );
    }

    #[test]
    fn request_worker_config_keeps_general_capacity_separate_from_sync_scans() {
        let single_cpu = RequestWorkerConfig::for_parallelism(1);
        assert_eq!(single_cpu.sync_scan_workers, 1);
        assert!(single_cpu.general_workers >= 2);
        assert!(single_cpu.general_queue_capacity >= single_cpu.general_workers);
        assert!(single_cpu.sync_scan_queue_capacity >= single_cpu.sync_scan_workers);

        let many_cpus = RequestWorkerConfig::for_parallelism(64);
        assert_eq!(many_cpus.sync_scan_workers, 1);
        assert_eq!(many_cpus.general_workers, 4);
    }

    fn ready_state() -> ServeState {
        ServeState {
            spdx_license_list_version: "3.28".to_string(),
            jobs: AsyncJobController::with_limits(2, 2, 8),
            ingest_policy: IngestPolicy::allow_privileged_inputs(),
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
            allow_privileged_inputs: false,
            verbosity: crate::cli::VerbosityFlags::default(),
        };

        let error = ServeConfig::try_from(&args).expect_err("empty bind should fail");
        assert!(error.to_string().contains("--bind must not be empty"));
    }

    #[test]
    fn serve_config_allows_privileged_inputs_for_loopback_bind() {
        let args = ServeArgs {
            bind: "127.0.0.1:8080".to_string(),
            allow_privileged_inputs: false,
            verbosity: crate::cli::VerbosityFlags::default(),
        };

        let config = ServeConfig::try_from(&args).expect("loopback bind should configure");
        assert!(config.ingest_policy.privileged_inputs_allowed());
    }

    #[test]
    fn serve_config_allows_privileged_inputs_for_localhost_hostname() {
        let args = ServeArgs {
            bind: "localhost:8080".to_string(),
            allow_privileged_inputs: false,
            verbosity: crate::cli::VerbosityFlags::default(),
        };

        let config = ServeConfig::try_from(&args).expect("localhost bind should configure");
        assert!(config.ingest_policy.privileged_inputs_allowed());
    }

    #[test]
    fn serve_config_restricts_privileged_inputs_for_non_loopback_bind() {
        let args = ServeArgs {
            bind: "0.0.0.0:8080".to_string(),
            allow_privileged_inputs: false,
            verbosity: crate::cli::VerbosityFlags::default(),
        };

        let config = ServeConfig::try_from(&args).expect("non-loopback bind should configure");
        assert!(!config.ingest_policy.privileged_inputs_allowed());
    }

    #[test]
    fn serve_config_explicitly_allows_privileged_inputs_for_non_loopback_bind() {
        let args = ServeArgs {
            bind: "0.0.0.0:8080".to_string(),
            allow_privileged_inputs: true,
            verbosity: crate::cli::VerbosityFlags::default(),
        };

        let config = ServeConfig::try_from(&args).expect("non-loopback bind should configure");
        assert!(config.ingest_policy.privileged_inputs_allowed());
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
    fn sync_scan_requires_json_content_type() {
        let state = ready_state();
        let response = handle_sync_scan_request(&test_request(Method::Post, "/v1/scans"), &state);
        assert_status(&response, 415);
        assert!(response.body.contains("unsupported_media_type"));
    }

    #[test]
    fn unknown_path_returns_not_found() {
        let response =
            response_for_request(&test_request(Method::Get, "/nonexistent"), &ready_state());
        assert_status(&response, 404);
        assert!(response.body.contains("not_found"));
    }

    #[test]
    fn parse_job_route_rejects_empty_job_id() {
        assert!(parse_job_route("/v1/jobs/").is_none());
    }

    #[test]
    fn parse_job_route_rejects_embedded_slashes() {
        assert!(parse_job_route("/v1/jobs/abc/def").is_none());
        assert!(parse_job_route("/v1/jobs/abc/def/result").is_none());
    }

    #[test]
    fn failed_job_result_with_invalid_status_code_falls_back_to_500() {
        let state = ready_state_with_job(
            "job-5",
            crate::serve::job_controller::AsyncJobRecord::failed("bad status".to_string(), 999, 1),
        );
        let response =
            response_for_request(&test_request(Method::Get, "/v1/jobs/job-5/result"), &state);
        assert_status(&response, 500);
        assert!(response.body.contains("job_failed"));
    }
}
