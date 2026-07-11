# ADR 0011: License Compliance Gating and SARIF Output

**Status**: Accepted
**Authors**: Provenant team
**Supersedes**: None

> **Current contract owner**: [`../CLI_GUIDE.md`](../CLI_GUIDE.md) owns the live flag and policy-file documentation; [`../OUTPUT_FIELD_REFERENCE.md`](../OUTPUT_FIELD_REFERENCE.md) owns the output-field contract. This ADR records why compliance severity, build gating, and SARIF output were added and how they fit together.

## Context

Provenant is increasingly run in CI (directly, in containers, and via the GitHub Action). A scan that only emits a JSON/SBOM report is useful as an audit artifact, but it does not, by itself, act as a guardrail: nothing decides whether a result is acceptable, and nobody reads a JSON artifact on every run. The two behaviors teams actually want from license scanning in CI are:

1. **Gating** — fail the build when a disallowed license appears (for example copyleft in a proprietary product).
2. **Inline surfacing** — show violations where reviewers already look (pull-request checks and the code-scanning UI), via SARIF.

The existing `--license-policy` feature (ported from ScanCode) attaches policy entries `{license_key, label, color_code, icon}` to each file. `label` is free text, so there is **no machine-readable severity**: the output cannot drive a gate or a SARIF level. ScanCode has no severity concept here either, so there is no compatibility contract to preserve — we are free to define one.

The blocker for SARIF was never lines of code (a serializer is comparable in size to the existing CycloneDX emitter). It was semantics: emitting every license detection as a SARIF result produces thousands of informational alerts (alert fatigue) and is worse than no SARIF. Meaningful SARIF requires a notion of which findings are violations and at what level — the same severity a gate needs. Gating, SARIF, and severity are therefore one decision, not three.

## Decision

**Introduce a Provenant-defined compliance severity on license-policy entries, and build both the CI gate and SARIF output on top of it.**

### 1. Policy severity (`compliance_alert`)

Extend the policy-file entry with an optional `compliance_alert` field:

```yaml
license_policies:
  - license_key: gpl-3.0
    label: Prohibited License
    compliance_alert: error
  - license_key: gpl-2.0
    label: Copyleft License
    compliance_alert: warning
  - license_key: mit
    label: Approved License
    # no compliance_alert => informational only
```

- Allowed values: `error`, `warning`, or absent (informational).
- The field is optional and additive: existing ScanCode-style policy files keep working unchanged and simply carry no severity.
- Severity ordering: `error` > `warning` > none.
- `compliance_alert` is surfaced on each file's `license_policy` entries in the output, so downstream tooling can read it without re-deriving anything.

### 2. Build gate (`--fail-on <LEVEL>`)

`--fail-on <error|warning>` makes a scan exit non-zero when any scanned file matches a policy whose `compliance_alert` is at or above `LEVEL` (`warning` fails on warning and error; `error` fails only on error). It requires `--license-policy` (nothing to evaluate otherwise).

- A policy violation exits with a **dedicated exit code (3)**, distinct from a scan/runtime error (1), so CI can tell "policy failed" from "the tool broke."
- Output is still written first: the report (and SARIF, if requested) is produced, then the process exits non-zero. The artifact is never lost to the gate.

### 3. SARIF output (`--sarif <FILE>`)

A SARIF 2.1.0 emitter (`src/output/sarif.rs`, wired like the other output formats) turns policy-flagged findings into code-scanning results:

- One SARIF rule per policy license key that carries a `compliance_alert`; `ruleId` is the license key, description is the `label`.
- One result per file whose detected license matches such a policy; `level` maps `error -> error`, `warning -> warning`; the location is the file path plus the matching detection's line region when available.
- With no policy (or no severities) the run has zero results — no noise by default. SARIF is therefore meaningful only alongside `--license-policy`, which the docs state explicitly.

### 4. GitHub Action surface

The action exposes `license-policy` and `fail-on` inputs that map to the CLI flags, so the gate and SARIF are reachable from a workflow without hand-writing `args`.

## Consequences

- **Positive**: CI can gate on license policy with a single flag; SARIF makes violations visible in pull requests and the code-scanning UI; the policy file gains a documented, machine-readable severity; all three features share one model and one source of truth. Non-gating, informational use is unchanged (severity is optional).
- **Negative / cost**: a new output format and a new exit-code path to maintain and test; a small, Provenant-specific extension to the policy schema that ScanCode does not understand (harmless — ScanCode ignores unknown keys, and Provenant ignores the field when no gate/SARIF is requested).
- **Scope boundary**: severity is evaluated against **file-level** detected license expressions (matching the existing `--license-policy` behavior). Extending gating to package/dependency declared licenses is a natural follow-up and is intentionally out of scope here.
- Using the code-scanning UI for license findings is slightly off-label (it targets security), but it is an established pattern (Trivy/Grype upload license and misconfiguration findings the same way).

## Alternatives Considered

- **Action-only gate (parse JSON in the wrapper).** Rejected as the primary mechanism: it hides the decision in shell, only helps Action users, and duplicates policy semantics outside the engine. The CLI gate is reusable everywhere; the Action just forwards to it.
- **SARIF of every detection.** Rejected: informational-level noise that buries real violations. SARIF is scoped to severity-carrying policy matches.
- **Deriving severity from the free-text `label`.** Rejected as brittle and locale-dependent; an explicit `compliance_alert` field is unambiguous.
- **A separate policy engine / config format.** Rejected: extending the existing, already-shipped policy file keeps one concept and preserves ScanCode-file compatibility.
