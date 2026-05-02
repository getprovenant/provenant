# `provenant serve` Plan and Contract

> **Status**: 🟡 Active implementation plan — this document defines the intended end-state `provenant serve` contract
> **Current contract owner**: this plan until the service contract graduates into evergreen runtime and user-facing docs
> **Priority**: P1 — define the full self-hosted service shape up front
> **Tracking issue**: [`#834`](https://github.com/mstykow/provenant/issues/834)

## Overview

`provenant serve` is the planned **self-hosted long-lived HTTP scanner service** built on top of the shared app/workflow pipeline introduced in the stacked base PR.

This document defines the **end-state service contract**.

## Product goals

- provide a self-hosted HTTP surface for systems that do not want to spawn the CLI as a subprocess
- preserve warm in-memory process state across requests in a long-lived service model
- reuse the same shared scan pipeline already used by the CLI and Rust workflow facade
- keep the service focused on scanning and integration ergonomics, not project orchestration or a hosted platform

## Non-goals

- a ScanCode.io-style web product with users, projects, runs, and orchestration state
- a Lambda-first request model
- a second independent scan implementation path that bypasses `src/app/*` and `execute_request(...)`
- immediate inclusion of auth, tenancy, quotas, billing, or persistence concerns in the core scanner contract

## Intended end-to-end service surface

### Deployment model

The intended operator model is a **long-lived container or host process**.

Likely deployment shapes:

- ECS/Fargate + ALB
- EC2 + systemd or container runtime
- Kubernetes deployment behind ingress/load balancer

API Gateway is an optional front door; it should sit in front of a real long-lived service, not replace it.

Lambda is not the primary target for `provenant serve`. If a Lambda adapter is added later, it should wrap this service contract rather than redefine it.

### Versioning model

- health and operator endpoints are unversioned: `/livez`, `/readyz`, `/version`
- scanner API endpoints live under `/v1/...`

### Intended scan API families

The intended scanner surface includes:

- `POST /v1/scans` — synchronous scan request for bounded workloads
- `POST /v1/scans:async` or equivalent async submission endpoint — for long-running or larger scans
- `GET /v1/jobs/{id}` — async job status lookup if an async model is added
- `GET /v1/jobs/{id}/result` — result retrieval for persisted async jobs if that model is added

Synchronous scan execution is the minimum required service capability. Async jobs are part of the intended end-state contract for larger or longer-running workloads.

### Intended input modes

The final service should support these request shapes:

1. **direct content submission** for small text/files
2. **archive or payload upload** for bounded package/repo snapshots
3. **object reference input** (for example S3-backed references) for larger hosted workflows
4. **trusted local-path scanning** only where the operator intentionally deploys the service with local filesystem authority

### Sync vs async intent

- synchronous scans provide the baseline request/response interaction for bounded workloads
- async jobs provide the scalable path for larger workloads and remote object-backed processing
- the API contract should remain explicit about which routes are sync-only and which require persisted job state

### End-state layering

The service should remain a thin transport layer over the shared Provenant scan pipeline:

- HTTP request parsing and response shaping live in the service layer
- scan request normalization should map cleanly onto the shared app/workflow request model
- actual scan execution should continue to flow through shared app/workflow orchestration rather than a separate service-only pipeline
- service-specific lifecycle concerns such as readiness, bind/startup failure, and graceful shutdown stay in the service layer

## HTTP contract

### `GET /livez`

Purpose: liveness check for process health.

Expected response:

```json
{ "status": "ok" }
```

Status code: `200`

### `GET /readyz`

Purpose: readiness check after service warm initialization completes.

Expected states:

- `200` with `{"status":"ready", ...}` once the shell has completed its warm initialization
- `503` with `{"status":"warming"}` while startup warm initialization is still pending
- `503` with `{"status":"failed", ...}` if warm initialization fails

Readiness should only flip to `200` after the service has completed the warm initialization required for serving scan requests reliably.

### `GET /version`

Purpose: machine-readable service metadata.

Expected response shape:

```json
{
  "service": "provenant-serve",
  "api_version": "v1",
  "tool_version": "..."
}
```

Status code: `200`

### `/v1/scans`

Purpose: stable scanner namespace for synchronous scan execution.

### `POST /v1/scans`

This route should follow these rules:

- request body names the scan mode and input transport explicitly
- response body returns the same Provenant/ScanCode-compatible scan result model already produced by the shared pipeline
- the route is a thin transport adapter over the shared app/workflow execution path
- the route should support the bounded synchronous scan contract directly, without forcing async job orchestration for small or medium requests

### `POST /v1/scans:async`

This route should accept scan requests that exceed the desired synchronous execution envelope and return a job handle.

### `GET /v1/jobs/{id}`

This route should expose async job state, including terminal success/failure, without requiring clients to infer job semantics from transport errors.

### `GET /v1/jobs/{id}/result`

This route should return the completed scan result for async jobs using the same output contract as synchronous scans.

## Acceptance checks

- `provenant serve --bind 127.0.0.1:0` starts successfully
- `GET /livez` returns `200`
- `GET /readyz` eventually returns `200`
- `GET /version` returns `200` and includes `api_version` plus `tool_version`
- `POST /v1/scans` accepts a bounded synchronous scan request and returns a scan result using the shared Provenant output contract
- `POST /v1/scans:async` returns a durable job handle for larger workloads
- `GET /v1/jobs/{id}` and `GET /v1/jobs/{id}/result` return stable async status/result contracts
- starting a second shell on an occupied port exits non-zero with a deterministic bind failure
