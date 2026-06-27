---
name: verify-benchmark-target
description: Iterate on compare-outputs for a concrete repository or artifact until only justified Provenant advantages remain, then record benchmark-backed results and open a PR.
---

# Verify a Benchmark Target

This skill drives the repeated compare-review-fix-rerun workflow used to verify Provenant against ScanCode on one concrete repository or artifact at a time. Use it when the goal is to run `compare-outputs`, inspect where ScanCode is genuinely better, make general fixes until those gaps are closed, look for false-positive or junk-reduction opportunities, and then record durable results in `docs/BENCHMARKS.md` when the target belongs in the maintained benchmark set.

It covers the full scan pipeline: package and dependency extraction, license detection, copyright detection, author/holder cleanup, URL/email normalization, assembly behavior, and other common-profile output differences that show up during benchmark verification.

This is the **parity-and-advantage vs ScanCode** lane. If instead you want to prove that a specific Provenant **code change** made the scanner faster (before/after on the same code, profile-first, regression-guarded), use [`benchmark-perf-change`](../benchmark-perf-change/SKILL.md) and its `xtask perf-ab` harness. Self before/after timings from that lane do **not** belong in `docs/BENCHMARKS.md`; this skill owns that file.

## Best fit

Use this skill when the task sounds like:

- Verify `https://github.com/org/repo` following `docs/BENCHMARKS.md`
- Run `compare-outputs`, fix regressions, and keep iterating until only Provenant advantages remain
- Review author / holder / copyright junk or false positives while validating a benchmark target
- Add or refresh a benchmark entry and open a PR for the verification work

## Source documents

- **Benchmarks**: `docs/BENCHMARKS.md` — the maintained reference for recorded compare-outputs runs, timing, and end-state advantages
- **xtask commands**: `xtask/README.md` — CLI reference for `compare-outputs`, `update-parser-golden`, `update-copyright-golden`, `update-license-golden`
- **PR template**: `.github/pull_request_template.md` — required structure for agent-authored PRs
- **AGENTS.md**: repo-level contributor guardrails

## Workflow

### Step 1: Ground on the benchmark methodology and select the target

Before running anything:

- Read the opening methodology in `docs/BENCHMARKS.md`, not just a few example entries.
- Read a few nearby benchmark entries from the same target family (or the closest comparable section) so the eventual wording and scope match the maintained style.
- Decide whether this target is likely to earn a durable benchmark entry or is better treated as one-off PR evidence only.

Choose the target's verification inputs from the current task context:

- Prefer user-provided or issue-linked repositories/artifacts when available.
- Reuse existing `docs/BENCHMARKS.md` targets from the same target family when they are still representative.
- Prefer stable repository snapshots (commit SHA or tag), not moving branches.
- Use artifact/rootfs or compiled-binary targets only when the target meaningfully depends on those surfaces.

### Step 2: Run compare-outputs for each selected target, in sequence

Start with `compare-outputs`, not with existing Provenant test suites. The benchmark-verification loop is the source of truth here. Existing tests become relevant only after a concrete regression or improvement has been identified and needs focused coverage.

Choose the `compare-outputs` build mode intentionally:

- **Default `--build-mode optimized`**: use this for any run whose timing may be quoted, compared against ScanCode, or recorded in `docs/BENCHMARKS.md`.
- **Optional `--build-mode fast-iteration`**: use this for quicker local reruns when you only need to check whether code changes affected output while you are still triaging deltas.
- Before adding or refreshing a benchmark entry, rerun the target in the default optimized mode so the recorded timing is benchmark-grade.

**Timeout discipline for agent-run compares**:

- When invoking `compare-outputs` through the Bash tool, use a **5-hour timeout** (`18000000` ms), especially for 10k+ file targets.
- If the Bash call times out, do **not** immediately rerun `compare-outputs`. First check whether the ScanCode Docker container is still alive (usually via the printed ScanCode image, e.g. `docker ps --filter ancestor=<image>`).
- If the container is still running, wait for it with `docker wait <container>`.
- Only salvage the finished ScanCode artifacts when `docker wait` exits `0`. If the container exits non-zero, do **not** populate the cache from that run.
- For a successful `docker wait`, salvage the finished ScanCode artifacts into the printed `ScanCode cache` directory: copy `raw/scancode.json` there as `scancode.json`, and copy `raw/scancode-stdout.txt` there as `scancode-stdout.txt` if it exists. `compare-outputs` prewrites the cache metadata, so a rerun can reuse that salvaged ScanCode output as a cache hit instead of rescanning.

For repository-backed targets:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- \
  --repo-url https://github.com/org/repo.git --repo-ref <ref> --profile common
```

Fast output-only iteration reruns can opt into the lighter Provenant build:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- \
  --repo-url https://github.com/org/repo.git --repo-ref <ref> --profile common --build-mode fast-iteration
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

**Loop exit condition**:

Keep iterating on compare → triage → fix → rerun until both of these are true:

1. No unresolved ScanCode-better deltas remain for the target.
2. The remaining Provenant-better differences are justified, user-visible advantages rather than unreviewed noise.

**False-positive / junk-reduction checklist**:

- Review author, holder, and copyright deltas for weak matches, placeholder strings, generated-text bleed, or repeated notice spam.
- Prefer reducing junk and false positives over matching ScanCode's noisy output.
- Treat README prose, changelog prose, generated docs, fixture blobs, and machine-expanded headers as likely noise sources unless the underlying text clearly supports the finding.
- Watch for malformed holder capture that appends punctuation, labels, URLs, or surrounding prose to an otherwise valid party name.

**Classification categories**:

| Category                                            | Action                                           |
| --------------------------------------------------- | ------------------------------------------------ |
| Provenant is better                                 | Document in the benchmark entry's outcome bullet |
| ScanCode is better                                  | Fix in Provenant (see Step 4)                    |
| Both wrong / cosmetic difference                    | Accept, do not fix, do not regress               |
| Provenant more correct (e.g. Unicode normalization) | Accept as advantage, do not treat as regression  |

Do **not** treat normalization improvements as regressions when Provenant is more correct (e.g. preserving `René` instead of degrading to `Rene`).

### Step 4: Fix regressions

When ScanCode produces better output than Provenant:

1. **Identify the root cause** — is it a package-extraction bug, a missing feature, a license-detection gap, a copyright-detection issue, an assembly problem, or broader scan-pipeline behavior?
2. **Make generic scanner improvements** — fixes must improve general scan quality, not just tune one benchmark target. Reject target-specific workarounds. This includes generic false-positive reduction and junk filtering when the benchmark exposes noisy author / holder / copyright behavior.
3. **Add focused tests** — every fixed regression or accepted behavior change should gain adequate automated coverage (owning unit tests for extraction or detection logic, scanner/assembly contract tests when applicable, integration tests, and golden tests as appropriate). Do not substitute pre-existing passing tests for benchmark verification; add or rerun only the narrow coverage that owns the changed behavior.
4. **Rerun affected regression suites** when a fix touches shared detection logic. Keep local validation tightly scoped and prefer the narrowest owning test target/filter:
   - Copyright-detection changes → rerun copyright goldens
   - License-detection changes → rerun license goldens
   - Package-extraction or assembly fixes → rerun the narrow owning tests, scanner/assembly contract tests where applicable, and relevant integration coverage
5. **Rerun the compare-outputs** for the target to confirm the fix.

### Step 5: Record the benchmark entry

For each target that belongs in the durable benchmark record, add or refresh an entry in `docs/BENCHMARKS.md`:

**Repository-backed targets** go in the "Repository-backed targets" section.
**Artifact/rootfs-backed targets** go in the "Artifact/rootfs-backed targets" section.

Within each section, keep entries **alphabetically ordered by target label**.

**Entry format**:

Each benchmark entry should use the same visible structure as the current document:

1. A heading of the form `##### [org/repo @ short_sha](link) — **N× faster**`
2. A `- Files: N` bullet
3. A `- Run context: ...` bullet
4. A `- Timing: Provenant \`Xs\`; ScanCode \`Ys\`` bullet
5. A final outcome bullet written as a present-tense end-state comparison (see writing rules below)

**Run context**: Record the run date and machine information (OS, CPU, RAM, arch, process count), formatted as `<date> · <OS> · <CPU> · <RAM> · <arch> · <N> proc`. Do not include the `run_id`/process-id suffix — it only ever pointed at a local, uncommitted `compare-runs/` directory and is meaningless to other readers.

**Timing**: Record same-host wall-clock timings for Provenant and ScanCode from the compare-outputs run. Compute relative speedup. If `run-manifest.json` reports `scancode.cache_hit: true`, use the cached ScanCode raw timing.

**Outcome bullet writing rules**:

- Write as a **present-tense end-state comparison**, not implementation history.
- Contrast **Provenant vs ScanCode**, never Provenant-now vs the bug you just fixed. Drop any "instead of / rather than `<old Provenant behavior>`" clause — that narrates the fix; a reader who never saw the bug has no referent. If the change only reached parity with ScanCode (no Provenant edge), say so plainly or omit it rather than dressing the fix as an advantage.
- Lead with what Provenant does better today: broader coverage, richer identity, safer handling, cleaner normalization, more correct classification, or faster runtime.
- Do **not** use process/history wording: `fixed`, `restored`, `aligned`, `added support`, `after`, `now that`, `triaged`, `reviewed tail`, `remaining deltas`.
- If a reviewed non-regression difference matters, rewrite it as a **user-visible advantage**.
- When claiming much broader package/dependency counts, include a **short causal explanation** naming the main surfaces driving the gap.
- Preferred sentence shape: **"Broader/richer/safer/more correct X ..., plus Y ..., with Z ..."**.

### Step 6: Record the verification outcome

When representative targets have been verified:

1. Add or refresh the relevant `docs/BENCHMARKS.md` entry when the target materially improves the maintained package-detection evidence.
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

3. If fixes touched a specific package-extraction surface, rerun the owning parser golden tests and any relevant scanner/assembly contract tests for that target family.

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

   For package-extraction golden JSON fixtures maintained through `update-parser-golden`, regenerate them directly from current Rust output:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-parser-golden -- <ParserType> <input> <output>
   ```

### Step 8: Open a PR

1. Create a branch such as `verify/<target-slug>`.
2. Commit with an appropriate Conventional Commits type that matches the actual change (`fix`, `test`, `docs`, etc.), optionally scoped to the affected target family or subsystem, and include the required DCO sign-off.
3. Use `.github/pull_request_template.md` for the PR body.
4. Include the compare-run artifact paths, the verified target snapshot, and a summary of what was fixed versus what was accepted as a justified Provenant advantage.
5. List any golden test changes with justification.
6. If a benchmark entry was added or refreshed, call that out explicitly in the PR summary.

## Recommended invocation pattern

Use prompts shaped like this:

```text
Read `docs/BENCHMARKS.md` and verify <repo-or-artifact> following the documented methodology. Run `compare-outputs` rather than starting from existing Provenant tests. Use `--build-mode fast-iteration` for quick local output-check reruns if helpful, but finish with the default optimized mode before recording timing or refreshing a benchmark entry. Make whatever general fixes are needed until only justified Provenant advantages remain over the compared ScanCode output. Also review false-positive and junk-reduction opportunities for authors, holders, and copyrights. When the target is benchmark-worthy, add or refresh the corresponding `docs/BENCHMARKS.md` entry. At the end, check whether any changes require new goldens, then open a PR with all resulting changes.
```

## Common failure modes

- Using `--profile packages` instead of `--profile common` — misses license/copyright/author/email/URL regressions.
- Using a branch name instead of a commit SHA for `--repo-ref` — not reproducible.
- Recording timing from a `--build-mode fast-iteration` run instead of rerunning in the default optimized mode first.
- Treating ScanCode-better output as "acceptable noise" without inspecting the underlying file text.
- Stopping once package deltas look good while author / holder / copyright junk still needs review.
- Making target-specific fixes that only improve one benchmark without addressing the general root cause.
- Starting from existing tests instead of from `compare-outputs`, then mistaking already-covered behavior for benchmark verification.
- Forgetting to run regression suites after fixing shared detection logic.
- Writing BENCHMARKS.md advantages as implementation history instead of present-tense end-state comparison.
- Updating golden expected files just to make tests pass without documenting why the new output is correct.

## Per-target-family watch points

Use the target's current issue/PR context plus the existing BENCHMARKS examples as guidance. Common cross-target patterns to watch:

- **Package count deltas**: Verify whether extra/missing packages are real extraction or assembly regressions or just fixture noise.
- **License-expression deltas**: ScanCode often collapses compound expressions; Provenant may be more specific.
- **Copyright/author noise**: Large doc/test trees generate many weak detections. Focus on genuine regressions.
- **Dependency scope**: Lockfile vs manifest precedence differences are often target-family-specific.
- **Vendored/generated files**: Exclude from triage unless they expose a real scan-pipeline bug.
- **Compiled-binary lanes**: Only use `common-with-compiled` when the selected target actually depends on compiled-binary package extraction.
