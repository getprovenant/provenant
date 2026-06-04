---
name: license-index-curation
description: Maintain Provenant license-detection overlays, index build policy, embedded license index artifacts, and related verification. Use for license index, overlay, .RULE, .LICENSE, build policy, stale curation, generate-index-artifact, or embedded license data work.
---

# License Index Curation

Use this skill when changing Provenant's bundled license-detection dataset, downstream ScanCode-format overlays, or embedded license-index artifact. It is a maintainer workflow skill, not a general license matcher implementation guide.

## Best Fit

Use this skill when the task says:

- update or regenerate the embedded license index
- add, replace, or remove a downstream `.RULE` or `.LICENSE` overlay
- change `resources/license_detection/index_build_policy.toml`
- fix stale overlay reasons, stale ignore IDs, or upstream-absorbed curations
- export, inspect, or compare license datasets for curation work

Do not use this skill for ordinary `provenant scan --license` usage; load `provenant-cli` for that.

## High-Signal Gotchas

- The embedded artifact reflects both upstream ScanCode data and Provenant's checked-in build policy.
- Artifact generation intentionally fails on stale ignore IDs, missing overlay reasons, stale reasons, and overlays that become identical to upstream.
- Normal scans use the embedded index by default; `--license-dataset-path` is an advanced exported/custom dataset workflow.
- The shared cache root also stores license-index cache files, so cache flags can affect repeat-run observations.
- `reference/scancode-toolkit/` is a behavioral/data source, not a test fixture dependency for Provenant tests.

For detailed curation failure modes, read `references/overlay-gotchas.md` before editing overlays or policy.

## Source Documents

- `docs/LICENSE_DETECTION_ARCHITECTURE.md` - license dataset, cache, overlay, and artifact architecture
- `xtask/README.md` - `generate-index-artifact` command reference
- `resources/license_detection/index_build_policy.toml` - checked-in curation policy
- `resources/license_detection/overlay/` - downstream ScanCode-format overlay files
- `.github/actions/verify-embedded-license-index/action.yml` - CI verification path
- `docs/CLI_GUIDE.md` - exported/custom dataset user workflows

## Workflow

### 1. Classify the issue

Before editing code, decide whether the problem is data curation or engine behavior.

Prefer an overlay or policy change when the fix is about one or a few rule/license semantics:

- rule reclassification
- minimum coverage or required phrase tuning
- adding or replacing a downstream `.RULE` / `.LICENSE`
- ignoring a stale or harmful upstream rule/license ID with rationale

Reach for matcher/refinement code only when the problem spans rule families or exposes an engine-level bug.

### 2. Edit curation inputs

- Put add/replace curations under `resources/license_detection/overlay/` using normal ScanCode `.RULE` / `.LICENSE` syntax.
- Keep every overlay documented in `resources/license_detection/index_build_policy.toml` with a rationale.
- Remove stale ignore IDs and stale overlay reasons when upstream changes make them redundant.
- If upstream absorbs a local overlay, remove the now-identical downstream overlay instead of keeping dead policy.

### 3. Regenerate and check the artifact

Run the owning generator after any overlay, policy, or upstream license-data change:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin generate-index-artifact
```

For verification-only work, use:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin generate-index-artifact -- --check
```

Commit the generated `resources/license_detection/license_index.zst` only when the curation change is intentional.

### 4. Add focused coverage

Use the narrowest detector test that proves the curation. Typical options:

```bash
cargo test --lib license_detection::<filter>
cargo test --features golden-tests <narrow_license_filter>
```

If a golden expected file changes, explain why the new output is correct. Do not update goldens just to make CI green.

## Boundaries

- Parser-declared license normalization belongs in parser work; use `add-parser` when adding parser surfaces.
- Full benchmark compare-review-fix loops belong to `verify-benchmark-target`.
- CLI scan command selection belongs to `provenant-cli`.
