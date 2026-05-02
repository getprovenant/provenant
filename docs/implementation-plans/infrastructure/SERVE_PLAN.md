# `provenant serve` Plan and Contract

> **Status**: 🟡 Active implementation plan — this document defines the current `provenant serve` contract and the intended end-to-end service surface
> **Current contract owner**: this plan for the in-flight service contract, with future migration to evergreen runtime and user-facing docs once the surface stabilizes
> **Priority**: P1 — define the full self-hosted service shape up front and implement it incrementally against that contract
> **Tracking issue**: [`#834`](https://github.com/mstykow/provenant/issues/834)

## Overview

`provenant serve` is the planned **self-hosted long-lived HTTP scanner service** built on top of the shared app/workflow pipeline introduced in the stacked base PR.

This document defines the **end-state service contract** first. Implementation can land in smaller stacked slices, but the contract described here is the target those slices are working toward.

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

## Scope of this slice

### Included now

- `provenant serve --bind <HOST:PORT>`
- a persistent HTTP listener suitable for local development and container-style hosting
- `GET /livez`
- `GET /readyz`
- `GET /version`
- explicit `501 Not Implemented` placeholder responses for `/v1/scans...`

### Explicit exclusions

- no scan submission endpoint yet
- no upload transport or object-store integration yet
- no auth, tenancy, quotas, or API keys
- no async job queue or persistence model
- no AWS-specific infrastructure code

## Intended end-to-end service surface

This section describes the intended final shape of `provenant serve`, not just the currently implemented subset.

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
- the current implementation slice already reserves `/v1/scans...` so the shell matches the intended stable namespace from the outset

### Intended scan API families

The intended scanner surface includes:

- `POST /v1/scans` — synchronous scan request for bounded workloads
- `POST /v1/scans:async` or equivalent async submission endpoint — for long-running or larger scans
- `GET /v1/jobs/{id}` — async job status lookup if an async model is added
- `GET /v1/jobs/{id}/result` — result retrieval for persisted async jobs if that model is added

Synchronous scan execution is the minimum required service capability. Async jobs remain part of the intended end-state contract, but can follow after the synchronous route exists.

### Intended input modes

The final service should support these request shapes:

1. **direct content submission** for small text/files
2. **archive or payload upload** for bounded package/repo snapshots
3. **object reference input** (for example S3-backed references) for larger hosted workflows
4. **trusted local-path scanning** only where the operator intentionally deploys the service with local filesystem authority

The first real scan endpoint should choose the smallest safe subset explicitly rather than implying support for all of them.

### Sync vs async intent

- synchronous scans should be the first real scan mode added
- async jobs are a planned extension, not a prerequisite for the first executable scan endpoint
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

Current warm initialization is intentionally lightweight and only proves the shell can initialize the embedded license-detection metadata needed by the future scan API.

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

Purpose: the stable scanner namespace, currently reserved during the shell-only slice.

Current behavior in this slice:

- any request under `/v1/scans...` returns `501 Not Implemented`
- response body makes it explicit that scan routes are not implemented in this shell yet

This keeps the shell aligned with the intended end-state namespace from the start instead of introducing a throwaway version prefix.

### Intended future `POST /v1/scans`

This route is part of the intended end-state contract. Its design should follow these rules:

- request body names the scan mode and input transport explicitly
- response body returns the same Provenant/ScanCode-compatible scan result model already produced by the shared pipeline
- the route is a thin transport adapter over the shared app/workflow execution path

The first implementation should keep this route synchronous and narrow in transport support.

## Current implemented subset

The code in the current stacked slice implements only the service shell portion of the end-state contract:

- `provenant serve --bind <HOST:PORT>`
- `GET /livez`
- `GET /readyz`
- `GET /version`
- `501 Not Implemented` responses under `/v1/scans...`

The full scan routes described above remain part of the contract, but are not yet wired in this slice.

## Acceptance checks for this shell

- `provenant serve --bind 127.0.0.1:0` starts successfully
- `GET /livez` returns `200`
- `GET /readyz` eventually returns `200`
- `GET /version` returns `200` and includes `api_version` plus `tool_version`
- requests under `/v1/scans` return `501`
- starting a second shell on an occupied port exits non-zero with a deterministic bind failure
