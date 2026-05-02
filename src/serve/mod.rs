// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result, anyhow};
use serde_json::json;

use crate::cli::ServeArgs;
use crate::license_detection::LicenseDetectionEngine;
use crate::version::BUILD_VERSION;

const API_VERSION: &str = "v1";

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
    let (method, path) = parse_request_line(&mut stream)?;
    let response = response_for_request(&method, &path, state);
    write_http_response(&mut stream, response)
}

fn parse_request_line(stream: &mut TcpStream) -> Result<(String, String)> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    if request_line.trim().is_empty() {
        return Err(anyhow!("received empty HTTP request"));
    }

    loop {
        let mut header_line = String::new();
        reader.read_line(&mut header_line)?;
        if header_line == "\r\n" || header_line.is_empty() {
            break;
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| anyhow!("missing HTTP method in request line"))?;
    let path = parts
        .next()
        .ok_or_else(|| anyhow!("missing HTTP path in request line"))?;
    Ok((method.to_string(), path.to_string()))
}

#[derive(Debug)]
struct HttpResponse {
    status_code: u16,
    reason: &'static str,
    body: String,
}

fn response_for_request(method: &str, path: &str, state: &ServeState) -> HttpResponse {
    let readiness = state
        .readiness
        .lock()
        .expect("serve readiness lock should not be poisoned")
        .clone();

    match (method, path) {
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
        (_, path) if path.starts_with("/v1/scans") => json_response(
            501,
            "Not Implemented",
            json!({
                "status": "not_implemented",
                "message": "scan routes are not implemented yet",
                "api_version": API_VERSION,
            }),
        ),
        _ => json_response(
            404,
            "Not Found",
            json!({ "status": "not_found", "path": path }),
        ),
    }
}

fn json_response(status_code: u16, reason: &'static str, body: serde_json::Value) -> HttpResponse {
    HttpResponse {
        status_code,
        reason,
        body: body.to_string(),
    }
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
        let response = response_for_request("GET", "/readyz", &ready_state());
        assert_eq!(response.status_code, 200);
        assert!(response.body.contains("\"status\":\"ready\""));
        assert!(response.body.contains("\"api_version\":\"v1\""));
    }

    #[test]
    fn scan_routes_return_not_implemented_contract() {
        let response = response_for_request("POST", "/v1/scans", &ready_state());
        assert_eq!(response.status_code, 501);
        assert!(response.body.contains("not_implemented"));
    }
}
