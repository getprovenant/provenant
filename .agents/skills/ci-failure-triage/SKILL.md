---
name: ci-failure-triage
description: Triage Provenant CI failures and map GitHub jobs to local reproduction commands, owning docs, and existing skills. Use for CI failed, check.yml, clippy, cargo deny, unused deps, license headers, generated docs drift, golden shard, or workflow failure.
---

# CI Failure Triage

Use this skill when a Provenant GitHub Actions check, hook, or local validation command fails and you need to identify the owning surface before fixing anything.

## Best Fit

Use this skill when the task says:

- CI failed or a GitHub job is red
- reproduce a check locally
- clippy, rustfmt, cargo deny, cargo machete, license headers, crate size, or generated docs failed
- a test shard, golden suite, or docs check failed
- decide which existing skill owns the fix

## High-Signal Gotchas

- Start from the failing GitHub job and step; do not run broad local checks first.
- Do not create a standalone compare-output triage path; benchmark compare work belongs to `verify-benchmark-target`.
- Do not create a standalone changed-file scan CI path; CLI `--paths-file` and `--incremental` usage belongs to `provenant-cli`.
- CI skips several jobs for Renovate branches; account for branch context before comparing runs.
- Generated docs failures should be fixed at the source metadata, then regenerated.
- Golden expected files should change only when the new output is intentionally correct and documented.

For the full CI job-to-command map, read `references/check-map.md` after identifying the failed job.

## Source Documents

- `.github/workflows/check.yml` - CI job matrix
- `CONTRIBUTING.md` - local setup and validation defaults
- `docs/TESTING_STRATEGY.md` - narrow test selection
- `package.json` - docs, formatting, headers, and hooks scripts
- `scripts/README.md` - repo helper scripts
- `xtask/README.md` - maintainer commands
- `AGENTS.md` - repo guardrails and frequent gotchas

## Triage Workflow

### 1. Start from the failing job and step

Do not run broad local checks first. Identify the exact job and step name, then map it to the smallest local reproduction.

Use `references/check-map.md` for the detailed job-to-command map. Keep this entrypoint focused on routing and escalation.

### 2. Route domain-specific failures

- Parser implementation or parser metadata drift: load `add-parser`.
- CLI flag usage or scan command construction: load `provenant-cli`.
- Benchmark compare-review-fix-rerun work: load `verify-benchmark-target`.
- License index artifact or overlay failures: load `license-index-curation`.
- Generated docs drift: load `generated-docs-maintenance`.
- Serve API or OpenAPI failures: load `serve-api-verifier`.
- Rust dependency policy failures: load `dependency-policy-maintenance`.

### 3. Reproduce narrowly, then widen

Follow the repo default: use the smallest command that proves the failed surface. Widen only after the narrow owner passes or when the CI failure is clearly cross-cutting.

For tests, prefer:

```bash
cargo test --doc
cargo test --test <suite_name> <filter>
cargo test --lib <filter>
cargo test --features golden-tests <narrow_filter>
```

Avoid unfiltered `cargo test`, `cargo test --all`, or broad golden suites unless there is no narrower reproduction.

## Boundaries

This skill diagnoses and routes CI failures. It should not grow into a parser guide, CLI reference, benchmark workflow, or dependency-governance guide; load the owning skill once the domain is clear.
