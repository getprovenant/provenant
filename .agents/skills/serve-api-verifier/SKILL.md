---
name: serve-api-verifier
description: Verify Provenant's `provenant serve` HTTP API, health endpoints, sync and async scan flows, upload/url/repository inputs, and OpenAPI drift. Use for serve API, /v1/scans, async jobs, health endpoints, OpenAPI, or HTTP service changes.
---

# Serve API Verifier

Use this skill when changing or validating the long-lived `provenant serve` HTTP surface. It is a product-verification skill for HTTP behavior, not a general CLI scan reference.

## Best Fit

Use this skill when the task mentions:

- `provenant serve`
- `/livez`, `/readyz`, or `/version`
- `POST /v1/scans` or `POST /v1/scans:async`
- `/v1/jobs/{id}` or `/v1/jobs/{id}/result`
- repository, URL, upload, or local-path service inputs
- generated OpenAPI drift

## High-Signal Gotchas

- A successful async submission is not the final result; verify job status and result retrieval.
- Verify response shapes and key fields, not just HTTP 200.
- Repository and URL inputs stage temporary local content; verify the resulting scan output, not only ingestion.
- Upload input uses base64 JSON payloads and should be checked with bounded fixture sizes.
- OpenAPI JSON is generated; fix source API metadata/types and regenerate rather than hand-editing.
- General scan flag selection belongs to `provenant-cli`; this skill owns HTTP request/response verification.

For repeatable smoke checks, use `scripts/serve_smoke.sh`. For payload examples, read `references/requests.md`.

## Source Documents

- `docs/SERVE_API_GUIDE.md` - user-facing service workflow
- generated serve OpenAPI document - machine-readable API contract
- `xtask/README.md` - `generate-serve-openapi` command
- `.github/workflows/check.yml` - OpenAPI drift check
- `docs/CLI_GUIDE.md` - high-level serve entry point

## Verification Workflow

### 1. Build or run the service intentionally

Use the current binary or run through Cargo, depending on the task context:

```bash
cargo run --bin provenant -- serve --bind 127.0.0.1:8080
```

Use a stable base URL in follow-up commands:

```bash
SERVE_BASE_URL="http://127.0.0.1:8080"
```

### 2. Check health and version endpoints

```bash
curl -sS "${SERVE_BASE_URL}/livez"
curl -sS "${SERVE_BASE_URL}/readyz"
curl -sS "${SERVE_BASE_URL}/version"
```

Verify the response shapes, not just HTTP 200. `/readyz` should expose readiness and API-version context; `/version` should identify the service and tool version. Prefer the bundled smoke script when you need a repeatable assertion path:

```bash
.opencode/skills/serve-api-verifier/scripts/serve_smoke.sh "${SERVE_BASE_URL}"
```

### 3. Exercise the changed scan mode

For sync scans, post to `/v1/scans` with the input mode affected by the change: `paths`, `repository`, `url`, or `upload`.

For async scans, post the same request shape to `/v1/scans:async`, then poll:

```bash
curl -sS "${SERVE_BASE_URL}/v1/jobs/<job_id>"
curl -sS "${SERVE_BASE_URL}/v1/jobs/<job_id>/result"
```

Assert the state transition and final ScanCode-compatible result shape. Do not stop at job acceptance.

### 4. Check OpenAPI drift

When routes, request/response types, schemas, or examples change, regenerate and verify OpenAPI:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin generate-serve-openapi
cargo run --manifest-path xtask/Cargo.toml --bin generate-serve-openapi -- --check
```

If only checking CI drift, run the `-- --check` form.

### 5. Run narrow automated tests

Prefer the smallest owning test target or filter. If no direct test exists for the changed path, add focused coverage before relying on manual curl checks alone.

## Boundaries

- Browser/UI verification is out of scope.
- Benchmark comparisons belong to `verify-benchmark-target`.
- Generated OpenAPI maintenance overlaps with `generated-docs-maintenance`; use this skill when the API behavior itself changed, and the docs skill when the failure is pure generated-doc drift.
