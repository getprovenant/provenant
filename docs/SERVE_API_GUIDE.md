# Serve API Guide

This guide is for people who want to call Provenant over HTTP instead of invoking the CLI directly or embedding the Rust library in-process.

Use it when you want to:

- run Provenant as a long-lived local or self-hosted service
- integrate from another language or process without managing CLI subprocesses for every scan
- call a machine-readable scan endpoint over HTTP

For command-line scanning workflows, start with the [CLI Guide](CLI_GUIDE.md). For in-process Rust embedding, start with the [Library Guide](LIBRARY_GUIDE.md).

## Start the service

```sh
provenant serve --bind 127.0.0.1:8080
```

The current service surface includes:

- `GET /livez`
- `GET /readyz`
- `GET /version`
- synchronous `POST /v1/scans`

## Check service health

```sh
curl -sS http://127.0.0.1:8080/livez
curl -sS http://127.0.0.1:8080/readyz
curl -sS http://127.0.0.1:8080/version
```

Expected shapes:

```json
{ "status": "ok" }
```

```json
{"status":"ready","api_version":"v1",...}
```

```json
{ "service": "provenant-serve", "api_version": "v1", "tool_version": "..." }
```

## Run a synchronous scan

Send JSON to `POST /v1/scans` with `Content-Type: application/json`.

The currently supported input mode is:

- `input.type = "paths"`

That means the service reads one or more trusted local paths visible to the host or container running `provenant serve`.

Example:

```sh
curl -sS \
  -X POST http://127.0.0.1:8080/v1/scans \
  -H 'Content-Type: application/json' \
  -d '{
    "input": {
      "type": "paths",
      "paths": ["/absolute/path/to/checkout"]
    },
    "options": {
      "detect_license": { "type": "embedded" },
      "detect_packages": true,
      "detect_copyrights": true,
      "detect_emails": true,
      "detect_urls": true
    }
  }'
```

The response body is the same ScanCode-compatible JSON shape Provenant already exposes through its existing output schema.

## Common request options

The request body currently supports scan options that map onto the shared Provenant scan pipeline, including:

- `collect_info`
- `detect_license`
- `detect_packages`
- `detect_system_packages`
- `detect_packages_in_compiled`
- `detect_copyrights`
- `detect_emails`
- `detect_urls`
- `detect_generated`
- `include` / `exclude`
- `strip_root` / `full_root`
- `license_text`, `license_text_diagnostics`, `license_diagnostics`, `unknown_licenses`, `license_score`
- `only_findings`, `mark_source`
- `classify`, `summary`, `license_clarity_score`, `license_references`
- `tallies`, `tallies_key_files`, `tallies_with_details`, `facets`, `tallies_by_facet`

The first practical default is usually a local-path equivalent of CLI `-clupe`:

```json
{
  "input": {
    "type": "paths",
    "paths": ["/absolute/path/to/checkout"]
  },
  "options": {
    "detect_license": { "type": "embedded" },
    "detect_packages": true,
    "detect_copyrights": true,
    "detect_emails": true,
    "detect_urls": true
  }
}
```

## Current limits

The current API surface is intentionally narrow:

- only synchronous `POST /v1/scans` is implemented
- only local-path input mode (`input.type = "paths"`) is implemented
- async routes are not implemented yet
- object-store inputs, uploads, auth, and job persistence are not implemented yet

## Machine-readable contract

The current OpenAPI document is generated from the implementation-coupled API contract and checked in at:

- [`openapi/provenant-serve.openapi.json`](openapi/provenant-serve.openapi.json)

That file is the best source if you want to inspect the current route, request, and response contract programmatically.
