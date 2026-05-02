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
use serde::Deserialize;
use serde_json::json;

use crate::cli::ServeArgs;
use crate::license_detection::LicenseDetectionEngine;
use crate::version::BUILD_VERSION;
use crate::workflow::{LicenseSource, ScanOptions, scan_paths};

const API_VERSION: &str = "v1";
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

#[derive(Debug, Deserialize)]
struct SyncScanRequest {
    input: SyncScanInput,
    #[serde(default)]
    options: SyncScanOptions,
}

#[derive(Debug, Deserialize)]
struct SyncScanInput {
    #[serde(rename = "type")]
    transport: SyncInputTransport,
    paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SyncInputTransport {
    Paths,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SyncLicenseSource {
    Disabled,
    Embedded,
    Directory { path: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct SyncScanOptions {
    collect_info: bool,
    detect_license: SyncLicenseSource,
    detect_packages: bool,
    detect_system_packages: bool,
    detect_packages_in_compiled: bool,
    detect_copyrights: bool,
    detect_emails: bool,
    detect_urls: bool,
    detect_generated: bool,
    include: Vec<String>,
    exclude: Vec<String>,
    strip_root: bool,
    full_root: bool,
    license_text: bool,
    license_text_diagnostics: bool,
    license_diagnostics: bool,
    unknown_licenses: bool,
    license_score: u8,
    only_findings: bool,
    mark_source: bool,
    classify: bool,
    summary: bool,
    license_clarity_score: bool,
    license_references: bool,
    tallies: bool,
    tallies_key_files: bool,
    tallies_with_details: bool,
    facets: Vec<String>,
    tallies_by_facet: bool,
}

impl Default for SyncScanOptions {
    fn default() -> Self {
        Self {
            collect_info: false,
            detect_license: SyncLicenseSource::Disabled,
            detect_packages: false,
            detect_system_packages: false,
            detect_packages_in_compiled: false,
            detect_copyrights: false,
            detect_emails: false,
            detect_urls: false,
            detect_generated: false,
            include: Vec::new(),
            exclude: Vec::new(),
            strip_root: false,
            full_root: false,
            license_text: false,
            license_text_diagnostics: false,
            license_diagnostics: false,
            unknown_licenses: false,
            license_score: 0,
            only_findings: false,
            mark_source: false,
            classify: false,
            summary: false,
            license_clarity_score: false,
            license_references: false,
            tallies: false,
            tallies_key_files: false,
            tallies_with_details: false,
            facets: Vec::new(),
            tallies_by_facet: false,
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
        ("GET", "/livez") => json_response(200, "OK", json!({ "status": "ok" })),
        ("GET", "/readyz") => match readiness {
            ReadinessState::Pending => {
                json_response(503, "Service Unavailable", json!({ "status": "warming" }))
            }
            ReadinessState::Ready {
                spdx_license_list_version,
            } => json_response(
                200,
                "OK",
                json!({
                    "status": "ready",
                    "api_version": API_VERSION,
                    "spdx_license_list_version": spdx_license_list_version,
                }),
            ),
            ReadinessState::Failed { message } => json_response(
                503,
                "Service Unavailable",
                json!({ "status": "failed", "message": message }),
            ),
        },
        ("GET", "/version") => json_response(
            200,
            "OK",
            json!({
                "service": "provenant-serve",
                "api_version": API_VERSION,
                "tool_version": BUILD_VERSION,
            }),
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
    json_response(
        status_code,
        reason,
        json!({
            "status": status,
            "message": message,
            "api_version": API_VERSION,
        }),
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
