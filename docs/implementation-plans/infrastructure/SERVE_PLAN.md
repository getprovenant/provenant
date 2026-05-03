# `provenant serve` Plan and Contract

> **Status**: 🟡 Active implementation plan — this document defines the intended end-state `provenant serve` contract
> **Related docs**: [`../../SERVE_API_GUIDE.md`](../../SERVE_API_GUIDE.md) for the implemented HTTP surface and [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) for runtime placement
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
- make the service meaningfully better than the CLI for remote callers by prioritizing repository, URL, and upload-driven ingestion rather than requiring pre-mounted server-local paths
- support API-triggered and automation-friendly scanning for CI, webhooks, cron, and internal platform workflows

## Non-goals

- a ScanCode.io-style web product with users, projects, runs, and orchestration state
- a Lambda-first request model
- a second independent scan implementation path that bypasses `src/app/*` and `execute_request(...)`
- immediate inclusion of auth, tenancy, quotas, billing, or persistence concerns in the core scanner contract
- presenting server-visible filesystem paths as the primary client contract for cloud-hosted deployments
- building provider-specific object-storage adapters into the first user-facing contract when generic uploads and fetchable URLs cover the same need more simply
- bundling a built-in scheduler or workflow orchestrator into the service instead of letting CI, cron, or webhooks drive the API externally

## Intended end-to-end service surface

### Deployment model

The intended operator model is a **long-lived container or host process**.

Likely deployment shapes:

- ECS/Fargate + ALB
- EC2 + systemd or container runtime
- Kubernetes deployment behind ingress/load balancer

API Gateway is an optional front door; it should sit in front of a real long-lived service, not replace it.

Lambda is not the primary target for `provenant serve`. If a Lambda adapter is added later, it should wrap this service contract rather than redefine it.

For ECS/Fargate specifically, server-local mounted paths are a narrow fit because the task runtime is centered around task-scoped storage rather than a stable host filesystem. In that environment, the main client contract should prefer remote-friendly inputs such as repository references, uploaded payloads, and fetchable URLs. Filesystem-visible paths remain useful when the operator intentionally stages data into the task, shared volume, or remote filesystem, but that is an operator deployment choice rather than the primary cloud API story.

### Automation model

The service should be easy to drive from external automation without becoming an orchestrator itself.

Typical automation shapes include:

- CI jobs such as GitHub Actions, GitLab CI, or Jenkins
- webhook-triggered internal platform calls
- cron or scheduled jobs managed outside the service

The API should therefore be explicit, scriptable, and stable for create-submit-poll-fetch workflows, while scheduling policy and workflow orchestration remain outside the core service.

### Versioning model

- health and operator endpoints are unversioned: `/livez`, `/readyz`, `/version`
- scanner API endpoints live under `/v1/...`

### Intended scan API families

The intended scanner surface includes:

- `POST /v1/scans` — synchronous scan request for bounded workloads
- `POST /v1/scans:async` or equivalent async submission endpoint — for long-running or larger scans
- `GET /v1/jobs/{id}` — async job status lookup
- `GET /v1/jobs/{id}/result` — result retrieval for persisted async jobs

Synchronous scan execution is the minimum required service capability. Async jobs are part of the intended end-state contract for larger or longer-running workloads and are the natural fit for repository scans, larger uploads, and slower remote fetches where staging and scan execution may exceed a bounded request/response envelope.

### Intended input modes

The final service should support these request shapes, with clear product roles:

1. **repository input** for hosted source repositories and source snapshots, typically expressed as a repository URL plus ref, or a stable archive or release-asset URL
2. **upload input** for bounded archives, source snapshots, SBOMs, and binaries pushed directly to the service
3. **remote URL input** for downloadable artifacts or remote text resources when fetch-over-HTTPS is the simplest integration path
4. **trusted local-path scanning** only where the operator intentionally deploys the service with local filesystem authority

Repository input and upload input should be the primary remote-service contract because they map most directly to the repeated demand for “scan this repo,” “scan this artifact,” and “run this scan from CI or a persistent service.” Generic HTTPS URL fetches are the fallback remote-reference mode when that is simpler than pushing a direct upload. Raw cloud-provider object references should not be the first contract surface; if operators use object storage, they should provide fetchable URLs or front the service with an upload flow rather than forcing Provenant itself to become a storage-adapter hub.

Trusted local paths remain valuable, but only as a narrow operator mode. They cover cases such as same-host integrations, shared-volume or EFS-backed workers, CI agents that already own a checkout, or internal services scanning data that has already been staged locally. They should not be treated as the default remote-service contract for ECS/Fargate-style clients.

The intended priority is therefore:

- make repository input a first-class remote-service workflow
- make upload input a first-class bounded-workload workflow
- support remote URL fetches where they are simpler than uploads or repository-specific handling
- keep `paths` as a supported operator transport

### Sync vs async intent

- synchronous scans provide the baseline request/response interaction for bounded workloads
- synchronous scans should cover only inputs that comfortably fit within the request, fetch, stage, and scan envelope of a single bounded HTTP transaction
- async jobs provide the scalable path for larger workloads and remote repository or URL-driven processing
- the API contract should remain explicit about which routes are sync-only and which require persisted job state

In practice, this means path-based operator scans, some small uploads, and some bounded remote URL scans may fit the synchronous route, while repository scans and larger remote fetches are expected to lean heavily on the async route.

### CLI vs service boundary

`provenant serve` should not exist merely as “the CLI, but over HTTP.”

The CLI remains the best fit when the caller already has local files and can invoke a binary directly. The service becomes meaningfully distinct when it offers one or more of these advantages:

- a warm long-lived process for repeated requests
- an HTTP integration surface for non-Rust or non-shell callers
- centralized scanner lifecycle, health checks, rollout, and observability
- remote-friendly ingestion modes that remove the need for callers to pre-stage server-visible paths themselves
- an async and automation-friendly contract that external CI and platform systems can call repeatedly without shelling out to the CLI on every request

The plan should therefore optimize the end-state contract for real service use cases, not just for same-host path delegation.

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
- the route must not require server-visible filesystem paths as the default remote client contract
- path-based requests remain allowed only as one explicit operator-mode transport among several input modes
- the route should be able to represent repository, upload, and remote URL-driven requests explicitly rather than hiding them behind filesystem assumptions

### `POST /v1/scans:async`

This route should accept scan requests that exceed the desired synchronous execution envelope and return a job handle.

This is the preferred route family for repository scans, larger uploads, slower remote URL fetches, and other workloads where fetch and staging are part of the request lifecycle.

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
- `POST /v1/scans` supports at least one repository, upload, or remote-URL-driven bounded workflow without requiring server-visible paths
- the service continues to support trusted local-path scans only as an explicit operator-mode transport
- `POST /v1/scans:async` returns a durable job handle for larger workloads
- `GET /v1/jobs/{id}` and `GET /v1/jobs/{id}/result` return stable async status/result contracts
- at least one repository-driven or remote-URL-driven input flow works cleanly for ECS/Fargate-style deployments without requiring callers to pre-mount files onto the scanner host
- the async API is straightforward to drive from external CI, webhook, or cron-based automation without requiring a built-in scheduler
- starting a second shell on an occupied port exits non-zero with a deterministic bind failure
