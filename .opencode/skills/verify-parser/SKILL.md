---
name: verify-parser
description: Verify a package parser ecosystem against ScanCode using compare-outputs, fix regressions, and record benchmark-backed results.
---

# Verify a Parser Ecosystem

This skill drives the end-to-end verification workflow for a package parser ecosystem. It runs `compare-outputs` against representative repositories or artifacts, triages and fixes regressions, and records durable results in `docs/BENCHMARKS.md` when the target belongs in the maintained benchmark set.

## Source documents

- **Benchmarks**: `docs/BENCHMARKS.md` — the maintained reference for recorded compare-outputs runs, timing, and end-state advantages
- **xtask commands**: `xtask/README.md` — CLI reference for `compare-outputs`, `update-parser-golden`, `update-copyright-golden`, `update-license-golden`
- **AGENTS.md**: repo-level contributor guardrails

## Workflow

### Step 1: Select representative verification targets

Choose the target ecosystem's verification inputs from the current task context:

- Prefer user-provided or issue-linked repositories/artifacts when available.
- Reuse existing `docs/BENCHMARKS.md` targets for the same ecosystem when they are still representative.
- Prefer stable repository snapshots (commit SHA or tag), not moving branches.
- Use artifact/rootfs or compiled-binary targets only when the ecosystem meaningfully depends on those surfaces.

### Step 2: Run compare-outputs for each selected target, in sequence

For repository-backed targets:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- \
  --repo-url https://github.com/org/repo.git --repo-ref <ref> --profile common
```

For artifact/rootfs-backed targets:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- \
  --target-path /path/to/local/target --profile common
```

For compiled-binary artifact targets:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- \
  --target-path /path/to/local/target --profile common-with-compiled
```

**Always use `--profile common`** (not `--profile packages`) so package extraction is evaluated alongside license, copyright, author, email, URL, and other common-profile detection behavior. Use `--profile common-with-compiled` only when the selected target actually requires compiled-binary verification.

Find a recent commit SHA or tag for `--repo-ref`. Do not use branch names — they are not stable.

### Step 3: Triage the comparison output

After each compare-outputs run, inspect the artifacts under `.provenant/compare-runs/<run-id>/`:

- `comparison/summary.json` — high-level delta counts, `comparison_status`, and directional counts in `comparison_signal_summary`
- `comparison/summary.tsv` — tab-separated per-file summary
- `comparison/samples/*.json` — detailed per-field diff samples
- `raw/provenant.json` and `raw/scancode.json` — full scanner outputs

**Triage rules**:

1. Treat `comparison_status: review_required` as a triage-required signal, not an automatic failure.
2. Treat any "more output" from either scanner as a claim to verify — not proof by itself.
3. When scanners disagree, inspect the underlying file text to decide whether the extra or missing finding is justified.
4. Apply the same rigor to license-expression and file-level license-detection deltas as to package, dependency, author, email, or URL deltas.
5. Treat top-level license-expression deltas and repeated file-level license mismatches as first-class regression signals.
6. Do **not** treat a target as verification-complete while any ScanCode-better deltas remain unresolved.

**Classification categories**:

| Category                                            | Action                                          |
| --------------------------------------------------- | ----------------------------------------------- |
| Provenant is better                                 | Document in BENCHMARKS.md advantages column     |
| ScanCode is better                                  | Fix in Provenant (see Step 4)                   |
| Both wrong / cosmetic difference                    | Accept, do not fix, do not regress              |
| Provenant more correct (e.g. Unicode normalization) | Accept as advantage, do not treat as regression |

Do **not** treat normalization improvements as regressions when Provenant is more correct (e.g. preserving `René` instead of degrading to `Rene`).

### Step 4: Fix regressions

When ScanCode produces better output than Provenant:

1. **Identify the root cause** — is it a parser bug, a missing feature, a license-detection gap, a copyright-detection issue, or an assembly problem?
2. **Make generic scanner improvements** — fixes must improve general scan quality, not just tune one benchmark target. Reject target-specific workarounds.
3. **Add focused tests** — every fixed regression or accepted behavior change should gain adequate automated coverage (parser tests, parser-local scanner/assembly contract tests when applicable, integration tests, and golden tests as appropriate).
4. **Rerun affected regression suites** when a fix touches shared detection logic. Keep local validation tightly scoped and prefer the narrowest owning test target/filter:
   - Copyright-detection changes → rerun copyright goldens
   - License-detection changes → rerun license goldens
   - Parser behavior fixes → rerun narrow parser tests, owning scanner/assembly contract tests where applicable, and relevant integration coverage
5. **Rerun the compare-outputs** for the target to confirm the fix.

### Step 5: Record the benchmark row

For each target that belongs in the durable benchmark record, add or refresh a row in `docs/BENCHMARKS.md`:

**Repository-backed targets** go in the "Repository-backed targets" section.
**Artifact/rootfs-backed targets** go in the "Artifact/rootfs-backed targets" section.

Within each section, sort rows **alphabetically by target label**.

**Row format**:

| Column                   | Content                                                                 |
| ------------------------ | ----------------------------------------------------------------------- |
| Target snapshot          | `[org/repo @ short_sha](link)` `<br>` `N files`                         |
| Run context              | `YYYY-MM-DD · <run-id suffix> · <os> · <cpu> · <ram> · <arch> · <proc>` |
| Timing snapshot          | `Provenant: Xs` `<br>` `ScanCode: Ys` `<br>` `N× faster (±N%)`          |
| Advantages over ScanCode | Present-tense end-state comparison (see writing rules below)            |

**Run context**: Copy the `run_id` suffix from `.provenant/compare-runs/<run-id>/run-manifest.json` — it is the portion after the leading UTC timestamp (e.g. `airflow-44518`). Get the date from the same manifest. Record machine information (OS, CPU, RAM, arch, process count).

**Timing**: Record same-host wall-clock timings for Provenant and ScanCode from the compare-outputs run. Compute relative speedup. If `run-manifest.json` reports `scancode.cache_hit: true`, use the cached ScanCode raw timing.

**Advantages column writing rules**:

- Write as a **present-tense end-state comparison**, not implementation history.
- Lead with what Provenant does better today: broader coverage, richer identity, safer handling, cleaner normalization, more correct classification, or faster runtime.
- Do **not** use process/history wording: `fixed`, `restored`, `aligned`, `added support`, `after`, `now that`, `triaged`, `reviewed tail`, `remaining deltas`.
- If a reviewed non-regression difference matters, rewrite it as a **user-visible advantage**.
- When claiming much broader package/dependency counts, include a **short causal explanation** naming the main surfaces driving the gap.
- Preferred sentence shape: **"Broader/richer/safer/more correct X ..., plus Y ..., with Z ..."**.

### Step 6: Record the verification outcome

When representative targets have been verified:

1. Add or refresh the relevant `docs/BENCHMARKS.md` row when the target materially improves the maintained package-detection evidence.
2. If a target is useful for one PR or issue but does not belong in the durable benchmark record, keep the outcome in the PR, issue, or saved compare artifacts instead of creating a new permanent checklist.
3. Do **not** narrate the implementation path in `docs/BENCHMARKS.md`; keep the entry focused on the end-state comparison and user-visible outcome.

### Step 7: Check for golden changes

After all fixes and compare runs are complete:

Keep validation tightly scoped. Prefer the narrowest useful owning test target/filter over broad local golden suites.

1. If fixes touched license detection, run the narrowest relevant license golden coverage and check whether expected files need updating.

   ```bash
   cargo test --features golden-tests <narrow_license_filter>
   ```

2. If fixes touched copyright detection, run the narrowest relevant copyright golden coverage and check whether expected files need updating:

   ```bash
   cargo test --features golden-tests <narrow_copyright_filter>
   ```

3. If fixes touched a specific parser, rerun the parser golden tests and any owning scanner/assembly contract tests for that ecosystem.

4. Only update golden expected files when the new output is genuinely better and the change is documented.

   For license golden YAML fixtures, first do a parity precheck, then choose the update mode that matches the intent:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-license-golden -- --list-mismatches --show-diff --filter <pattern>
   cargo run --manifest-path xtask/Cargo.toml --bin update-license-golden -- --filter <pattern> --write
   cargo run --manifest-path xtask/Cargo.toml --bin update-license-golden -- --sync-actual --filter <pattern> --write
   ```

   Use plain `--write` for parity-safe syncs from the Python reference. Use `--sync-actual --write` only when the Rust-owned expectation is intentionally diverging.

   For copyright golden YAML fixtures, use the same precheck-then-update flow:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-copyright-golden -- copyrights --list-mismatches --show-diff --filter <pattern>
   cargo run --manifest-path xtask/Cargo.toml --bin update-copyright-golden -- copyrights --filter <pattern> --write
   cargo run --manifest-path xtask/Cargo.toml --bin update-copyright-golden -- copyrights --sync-actual --filter <pattern> --write
   ```

   Use plain `--write` for parity-safe syncs from the Python reference. Use `--sync-actual --write` only when the Rust-owned expectation is intentionally diverging.

   For parser golden JSON fixtures, regenerate them directly from current Rust output:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-parser-golden -- <ParserType> <input> <output>
   ```

### Step 8: Open a PR

1. Create a branch: `verify/<ecosystem>-parser`
2. Commit with an appropriate Conventional Commits type that matches the actual change (`fix`, `test`, `docs`, etc.), optionally scoped to the ecosystem.
3. Use `.github/pull_request_template.md` for the PR body.
4. Include the compare-run artifact paths and a summary of what was fixed or accepted.
5. List any golden test changes with justification.

## Common failure modes

- Using `--profile packages` instead of `--profile common` — misses license/copyright/author/email/URL regressions.
- Using a branch name instead of a commit SHA for `--repo-ref` — not reproducible.
- Treating ScanCode-better output as "acceptable noise" without inspecting the underlying file text.
- Making target-specific fixes that only improve one benchmark without addressing the general root cause.
- Forgetting to run regression suites after fixing shared detection logic.
- Writing BENCHMARKS.md advantages as implementation history instead of present-tense end-state comparison.
- Updating golden expected files just to make tests pass without documenting why the new output is correct.

## Per-ecosystem watch points

Use the target ecosystem's current issue/PR context plus the existing BENCHMARKS examples as guidance. Common cross-ecosystem patterns to watch:

- **Package count deltas**: Verify whether extra/missing packages are real parser regressions or just fixture noise.
- **License-expression deltas**: ScanCode often collapses compound expressions; Provenant may be more specific.
- **Copyright/author noise**: Large doc/test trees generate many weak detections. Focus on genuine regressions.
- **Dependency scope**: Lockfile vs manifest precedence differences are ecosystem-specific.
- **Vendored/generated files**: Exclude from triage unless they expose a real parser bug.
- **Compiled-binary lanes**: Only use `common-with-compiled` when the selected target actually depends on compiled-binary package extraction.
