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

By default, `provenant serve` is intended for same-host use and binds to loopback. On loopback binds, all input modes listed below are enabled.

When the service is bound beyond localhost, requests may come from other machines. In that mode, local-path, remote-URL, and repository inputs are disabled unless the operator explicitly starts the service with `--allow-privileged-inputs`. Upload input remains available because the caller supplies the content to scan instead of asking the service host to read local paths, fetch URLs, or run `git fetch`.

Remote-URL and repository fetches are protected against server-side request forgery (SSRF). Regardless of bind, the service refuses to fetch targets that resolve to private, loopback, link-local (including the `169.254.169.254` cloud-metadata address), unique-local, or other non-public addresses, and it re-validates the target on every HTTP redirect. Repository URLs are restricted to the `https`, `git`, and `ssh` transports, and `git` runs with a deny-by-default transport allowlist so remote helpers such as `ext::` cannot execute. This SSRF protection stays on by default even on a loopback bind. Passing `--allow-privileged-inputs` additionally trusts the operator to reach local or private targets (for example internal mirrors), relaxing the address filter and permitting the local `file://` git transport.

Use `--allow-privileged-inputs` only for trusted deployments with their own network access controls:

```sh
provenant serve --bind 0.0.0.0:8080 --allow-privileged-inputs
```

The current service surface includes:

- `GET /livez`
- `GET /readyz`
- `GET /version`
- synchronous `POST /v1/scans`
- asynchronous `POST /v1/scans:async`
- `GET /v1/jobs/{id}`
- `GET /v1/jobs/{id}/result`

## Check service health

For the shell examples below, set a base URL for the service you are querying:

```sh
HOST=127.0.0.1
PORT=8080
SERVE_BASE_URL="http://${HOST}:${PORT}"
```

```sh
curl -sS "${SERVE_BASE_URL}/livez"
curl -sS "${SERVE_BASE_URL}/readyz"
curl -sS "${SERVE_BASE_URL}/version"
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

The currently supported input modes are:

- `input.type = "paths"`
- `input.type = "repository"`
- `input.type = "url"`
- `input.type = "upload"`

That means the service can:

- read one or more trusted local paths visible to the host or container running `provenant serve`
- shallow-fetch a repository ref into temporary local staging before scanning it
- download a bounded remote HTTP(S) artifact or text resource into temporary local staging before scanning it
- accept a bounded JSON upload payload and materialize it locally before scanning it

Local-path, repository, and remote URL inputs are privileged input modes: they use the service host's filesystem, network, or `git` client on behalf of the caller. They are best suited to same-host or operator-controlled deployments where callers are already trusted to make those host-side accesses.

### Local-path input

Example:

```sh
curl -sS \
  -X POST "${SERVE_BASE_URL}/v1/scans" \
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

### Repository input

Repository input is the simplest way to say “scan this repo at this ref” over the current sync API.

```sh
curl -sS \
  -X POST "${SERVE_BASE_URL}/v1/scans" \
  -H 'Content-Type: application/json' \
  -d '{
    "input": {
      "type": "repository",
      "url": "https://github.com/aboutcode-org/scancode.io.git",
      "ref": "main"
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

### Remote URL input

Remote URL input fetches a bounded HTTP(S) resource into temporary local staging before scanning it.

For supported archive URLs such as `.zip`, `.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, or `.tar.xz`, the service extracts the archive before scanning it. Other URLs are scanned as the downloaded file as-is.

```sh
curl -sS \
  -X POST "${SERVE_BASE_URL}/v1/scans" \
  -H 'Content-Type: application/json' \
  -d '{
    "input": {
      "type": "url",
      "url": "https://github.com/aboutcode-org/scancode.io/archive/refs/heads/main.zip"
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

### Upload input

Upload input is a bounded JSON upload path for smaller archives, SBOMs, or other artifacts when pushing content directly is simpler than exposing a URL.

The payload is base64 encoded and identified by a file name. Archive uploads use the same archive extraction behavior as remote URL inputs.

One practical shell workflow is:

```sh
CONTENT_BASE64=$(base64 < snapshot.zip | tr -d '\n')

curl -sS \
  -X POST "${SERVE_BASE_URL}/v1/scans" \
  -H 'Content-Type: application/json' \
  --data-binary @- <<EOF
{
  "input": {
    "type": "upload",
    "filename": "snapshot.zip",
    "content_base64": "${CONTENT_BASE64}"
  },
  "options": {
    "detect_license": { "type": "embedded" },
    "detect_packages": true
  }
}
EOF
```

The response body is the same ScanCode-compatible JSON shape Provenant already exposes through its existing output schema.
For explanations of public output fields and presence rules on that shared output, see the [Output Field Reference](OUTPUT_FIELD_REFERENCE.md).

## Run an asynchronous scan

Send the same JSON request shape to `POST /v1/scans:async` when you want a durable job handle instead of waiting for the final scan result on the submission request.

Example:

```sh
curl -sS \
  -X POST "${SERVE_BASE_URL}/v1/scans:async" \
  -H 'Content-Type: application/json' \
  -d '{
    "input": {
      "type": "url",
      "url": "https://github.com/aboutcode-org/scancode.io/archive/refs/heads/main.zip"
    },
    "options": {
      "detect_license": { "type": "embedded" },
      "detect_packages": true,
      "collect_info": true
    }
  }'
```

Expected acceptance shape:

```json
{
  "status": "accepted",
  "job_id": "job-7d0f4d8a0d784264a8fe632fe3ffc4fd",
  "state": "pending",
  "status_url": "/v1/jobs/job-7d0f4d8a0d784264a8fe632fe3ffc4fd",
  "result_url": "/v1/jobs/job-7d0f4d8a0d784264a8fe632fe3ffc4fd/result"
}
```

The service may keep the job in `pending` until bounded execution capacity is available. Once it starts work, the job moves to `running`, and then to `succeeded` or `failed`.

Check job state:

```sh
curl -sS "${SERVE_BASE_URL}/v1/jobs/job-7d0f4d8a0d784264a8fe632fe3ffc4fd"
```

Example status response:

```json
{
  "job_id": "job-7d0f4d8a0d784264a8fe632fe3ffc4fd",
  "state": "running",
  "result_ready": false,
  "allocated_processors": 4
}
```

Fetch the completed result:

```sh
curl -sS "${SERVE_BASE_URL}/v1/jobs/job-7d0f4d8a0d784264a8fe632fe3ffc4fd/result"
```

Before completion, `GET /v1/jobs/{id}/result` returns a non-success `job_not_ready` error. After completion, it returns the same ScanCode-compatible JSON shape as the synchronous route.

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
- `license_text`, `license_text_diagnostics`, `license_diagnostics`, `unknown_licenses`, `no_sequence_matching`, `license_score`
- `only_findings`, `mark_source`
- `classify`, `summary`, `license_clarity_score`, `license_references`
- `tallies`, `tallies_key_files`, `tallies_with_details`, `facets`, `tallies_by_facet`

`no_sequence_matching` only matters when `detect_license` is enabled. It disables the approximate sequence matcher so you can compare results with and without that matcher or suppress noisy partial license hits.

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

- synchronous and asynchronous scan submission use the same request body shape
- inputs currently support local paths, repository refs, remote URLs, and bounded JSON uploads
- async jobs run under bounded background execution and may wait in `pending` before starting
- upload input is JSON-only and bounded; multipart upload is not implemented
- remote URL input currently supports only `http` and `https`
- auth is not implemented
- async job state and result retention are bounded in-memory service state, not durable external persistence

## Machine-readable contract

The current OpenAPI document is generated from the implementation-coupled API contract and checked in at:

- [`openapi/provenant-serve.openapi.json`](openapi/provenant-serve.openapi.json)

That file is the best source if you want to inspect the current route, request, and response contract programmatically.
