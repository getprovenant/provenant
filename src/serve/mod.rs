// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde_json::json;

use crate::cli::ServeArgs;
use crate::license_detection::LicenseDetectionEngine;
use crate::serve_api::{
    API_VERSION, ServeErrorResponse, ServeLivenessResponse, ServeReadinessResponse,
    ServeVersionResponse, SyncInputTransport, SyncLicenseSource, SyncScanRequest,
};
use crate::version::BUILD_VERSION;
use crate::workflow::{LicenseSource, ScanOptions, scan_paths};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

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
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
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

    let state = ServeState { readiness };
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

fn response_for_request(request: &HttpRequest, state: &ServeState) -> HttpResponse {
    let readiness = state
        .readiness
        .lock()
        .expect("serve readiness lock should not be poisoned")
        .clone();

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
        (_, "/v1/scans:async") => error_response(
            501,
            "Not Implemented",
            "not_implemented",
            "async scan routes are not implemented yet".to_string(),
        ),
        (_, path) if path.starts_with("/v1/jobs/") => error_response(
            501,
            "Not Implemented",
            "not_implemented",
            "async job routes are not implemented yet".to_string(),
        ),
        _ => json_response(
            404,
            "Not Found",
            json!({ "status": "not_found", "path": request.path }),
        ),
    }
}

fn handle_sync_scan_request(request: &HttpRequest) -> HttpResponse {
    if !request
        .headers
        .get("content-type")
        .is_some_and(|value| value.starts_with("application/json"))
    {
        return error_response(
            415,
            "Unsupported Media Type",
            "unsupported_media_type",
            "POST /v1/scans requires Content-Type: application/json".to_string(),
        );
    }

    let sync_request = match decode_sync_scan_request(&request.body) {
        Ok(sync_request) => sync_request,
        Err(error) => {
            return error_response(400, "Bad Request", "invalid_request", error.to_string());
        }
    };

    let (paths, options) = match build_sync_scan_execution(sync_request) {
        Ok(execution) => execution,
        Err(error) => {
            return error_response(
                422,
                "Unprocessable Entity",
                "invalid_scan_request",
                error.to_string(),
            );
        }
    };

    match scan_paths(paths.iter().map(|path| path.as_path()), &options) {
        Ok(output) => serialize_response(200, "OK", &crate::output_schema::Output::from(&output)),
        Err(error) => error_response(
            422,
            "Unprocessable Entity",
            "scan_failed",
            error.to_string(),
        ),
    }
}

fn decode_sync_scan_request(body: &[u8]) -> Result<SyncScanRequest> {
    serde_json::from_slice(body).context("request body must be valid JSON")
}

fn build_sync_scan_execution(request: SyncScanRequest) -> Result<(Vec<PathBuf>, ScanOptions)> {
    if !matches!(request.input.transport, SyncInputTransport::Paths) {
        return Err(anyhow!("only input.type=paths is currently supported"));
    }

    if request.input.paths.is_empty() {
        return Err(anyhow!("input.paths must contain at least one path"));
    }

    let paths: Vec<PathBuf> = request.input.paths.into_iter().map(PathBuf::from).collect();
    for path in &paths {
        if !path.exists() {
            return Err(anyhow!("input path does not exist: {}", path.display()));
        }
    }

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
    options.strip_root = request.options.strip_root;
    options.full_root = request.options.full_root;
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

    Ok((paths, options))
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

    fn ready_state() -> ServeState {
        ServeState {
            readiness: Arc::new(Mutex::new(ReadinessState::Ready {
                spdx_license_list_version: "3.28".to_string(),
            })),
        }
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
    fn scan_routes_return_not_implemented_contract() {
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/v1/scans:async".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };
        let response = response_for_request(&request, &ready_state());
        assert_eq!(response.status_code, 501);
        assert!(response.body.contains("not_implemented"));
    }

    #[test]
    fn decode_sync_scan_request_rejects_empty_paths() {
        let error = build_sync_scan_execution(SyncScanRequest {
            input: SyncScanInput {
                transport: SyncInputTransport::Paths,
                paths: Vec::new(),
            },
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
}
