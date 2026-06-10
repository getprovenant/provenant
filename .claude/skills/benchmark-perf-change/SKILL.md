---
name: benchmark-perf-change
description: Measure whether a Provenant code change actually improves performance — profile-first, before/after on the same code, regression-guarded, with a byte-identical correctness gate. Use for "is this optimization actually faster", "benchmark my change", "profile a copyright/license hotspot", "before/after perf", "did my refactor regress performance", or "prove this speedup".
---

# Benchmark a Performance Change

This skill drives the disciplined workflow for proving that a Provenant **code change** makes the scanner faster (or for honestly reporting that it does not). It is profile-first, measures before/after on the same code paths, guards against regressions, and gates pure refactors on byte-identical output.

This is **distinct** from [`verify-benchmark-target`](../verify-benchmark-target/SKILL.md). That skill compares **Provenant vs ScanCode** for parity and user-visible advantage and records durable entries in `docs/BENCHMARKS.md`. This skill compares **Provenant against itself** (one ref vs another) to attribute a wall-clock change to your edit. Keep the two lanes separate: a `perf-ab` self-comparison is not a ScanCode parity result and does not belong in `docs/BENCHMARKS.md`; a `compare-outputs`/`benchmark-target` ScanCode run does not prove that your specific edit moved the needle.

## Best fit

Use this skill when the task sounds like:

- Is this optimization actually faster? Prove it.
- Benchmark my change / before-and-after my refactor.
- Profile the copyright (or license, or assembly) hotspot and speed it up.
- Did my refactor regress performance?

## Source documents

- **xtask commands**: `xtask/README.md` — CLI reference for `perf-ab` (the A/B harness this skill drives) and `benchmark-target`.
- **Benchmarks**: `docs/BENCHMARKS.md` — the ScanCode-comparison record. Read it to understand what lives there and why a self-comparison does **not**.
- **PR template**: `.github/pull_request_template.md` — required structure for agent-authored PRs.
- **AGENTS.md**: repo-level contributor guardrails (testing defaults, code-quality rules).

## Tooling

- **`xtask perf-ab`** is the harness for the A/B measurement step (step 4). It builds two git refs in release, interleaves timed rounds, reports per-side medians and the head-vs-base speedup, and can gate on byte-identical output with `--check-output`.
- **`samply`** (macOS) or **`perf` / `cargo flamegraph`** (Linux) is the profiler for step 1. Build with the `profiling` Cargo profile (`cargo build --profile profiling`), which inherits `release` but keeps debug symbols.

## Workflow

### Step 1: Profile FIRST — never optimize on assumption

Do not touch code until profiling proves where the time goes.

1. Build release with debug symbols: `cargo build --profile profiling --bin provenant` (the `profiling` profile inherits `release` and keeps `debug = true`, `strip = false`). If that profile is ever removed, fall back to `--release` with symbols re-enabled.
2. Profile a representative scan that actually exercises the suspected path:
   - macOS: `samply record ./target/profiling/provenant scan --json /tmp/out.json -c -n 1 /path/to/repo`
   - Linux: `perf record -g ./target/profiling/provenant scan ...` then `perf report`, or `cargo flamegraph`.
3. Attribute cost to concrete functions. Confirm the hypothesized hotspot is genuinely hot **before** editing it.

**Cautionary example (real):** a proposed optimization targeted an ~1100-regex tagger. Profiling showed that tagger was **0% of the profile** — the real cost was a string-replace chain in a different function. Optimizing the regex tagger would have cost effort and risk for zero gain. Profile first, then optimize the function profiling actually blames.

### Step 2: Set a STOP CONDITION before implementing — and honor it

Before writing the optimization, write down the threshold that makes it worth shipping (for example: ">= 10% median wall-clock reduction on a copyright-dense repo with `-c -n 1`, output byte-identical"). Then measure honestly against it:

- If the change does not beat the threshold **while staying correct**, report the negative result and do **not** ship it.
- A flat or slower result is a valid, valuable finding. Record it; do not quietly keep a change that does not pay for its complexity.

**Cautionary examples (real):** a `RegexSet` rewrite measured a **2–3.5x regression** and was correctly rejected. A single-pass collapse of a multi-pass transform measured **flat** and was also correctly rejected. Both decisions were right because the stop condition was set up front.

### Step 3: Byte-identical correctness gate (pure-refactor perf changes)

A perf change that is supposed to preserve behavior must produce identical output. Prove it two ways:

1. **Run the relevant golden INTEGRATION suite**, not `--lib`:

   ```bash
   cargo test --test <area>_golden --features golden-tests   # e.g. copyright_golden, parsers_golden, license_golden
   ```

   Golden suites are integration test targets (`tests/`). `cargo test --lib` does **not compile or run them**, so a passing `cargo test --lib` is **not sufficient** to prove behavior is unchanged. Pick the suite that owns the code you touched.

2. **Diff full scan output of the before/after binaries on a real repo** and require **0 diffs**. `xtask perf-ab --check-output` does exactly this: it scans with both binaries to JSON and asserts byte-identical output after normalizing only the volatile header (version/timestamp/duration). Use it, or diff manually after normalizing the `headers` block.

3. **Lock any order- or cascade-dependent behavior with a unit test.** If your change reorders passes, collapses a cascade, or changes iteration order, add a focused unit test that pins the observable result so a future refactor cannot silently drift.

Any diff means a **bug in your change**. Fix the code — never edit the goldens to make a perf refactor "pass".

### Step 4: A/B measurement hygiene

Use `xtask perf-ab` and respect these rules (the harness already enforces most of them):

- **Build BOTH refs in release.** Never compare a debug build to a release build, or two different cargo profiles.
- **INTERLEAVE runs** (base, head, base, head, …) rather than all-base-then-all-head, so machine-wide thermal/scheduling drift is shared evenly. `perf-ab` does this automatically.
- **Warm up and discard the cold run** per binary. `perf-ab` discards one warmup per side.
- **Take the MEDIAN of >= 5 rounds.** `perf-ab` defaults to `--rounds 5` and reports the median.
- **Run in a STABLE location, outside git worktrees.** Worktree churn and artifact deletion during a measurement corrupt timings. `perf-ab` builds each ref into its own stable checkout under `.provenant/` and times from there.
- **Re-measure independently when numbers conflict.** A bad measurement once reported a bogus **1.5%** improvement that was actually **~16%** — caused by measuring in a churning location. If two runs disagree materially, repeat from a clean state before trusting either.

Typical invocation:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin perf-ab -- \
  --base origin/main --head HEAD \
  --target-path /path/to/representative/repo \
  --rounds 7 --check-output -- -c -n 1
```

### Step 5: Isolate the component and pick a representative target

Make the measurement attribute cost to the thing you changed:

- **Isolate per-file CPU with `-n 1`** (single worker). Parallelism hides and reshuffles per-file cost; single-thread makes a detector's own work visible.
- **Attribute one detector's cost as a delta vs a no-detection baseline.** Time the same scan with and without the detector flag (for example `-c` vs no `-c`) and compare the deltas before and after your change, rather than reading a whole-scan number that is dominated by unrelated work.
- **Choose a target whose content actually exercises the path** — a copyright-dense repo for copyright work, a license-text-heavy tree for license work, a manifest-heavy repo for parser/assembly work — and **verify it does** (e.g. confirm the output actually contains many copyright findings) before trusting the numbers. A target that barely touches your code path will report flat regardless of whether the change helped.

### Step 6: Decide, then report honestly

- If the change beats the stop condition **and** the correctness gate is clean, keep it and write up the measured median reduction/factor, the target, the round count, and the `-n 1`/baseline-delta methodology.
- If it does not, report the negative result plainly and drop the change.
- Either way, attach the `perf-ab` run manifest path (`.provenant/perf-ab/<run-id>/run-manifest.json`) as evidence. Do **not** record self-comparison timings in `docs/BENCHMARKS.md`; that file is for Provenant-vs-ScanCode results.

### Step 7: Open a PR

1. Branch with a descriptive name (for example `perf/<area>-<what>`).
2. Use a Conventional Commit type that matches the change (`perf` for a genuine speedup, `refactor` for a behavior-preserving cleanup that happens to be measured) with DCO sign-off (`git commit -s`).
3. Use `.github/pull_request_template.md` for the body. In **How to verify**, give the exact `perf-ab` command, the representative target, and the measured median/factor — this is the extra-beyond-CI evidence reviewers cannot reproduce from the suite alone.
4. State the stop condition and whether the change met it. If you rejected a tempting-but-flat/slower alternative, say so.
5. List any golden or unit-test changes and why the new expectations are correct.

## Common failure modes

- Optimizing a function that profiling shows is cold (the 1100-regex-tagger trap). Profile first.
- Trusting `cargo test --lib` to prove byte-identical behavior — it never compiles the golden integration suites. Run `cargo test --test <area>_golden --features golden-tests`.
- Measuring base-debug vs head-release, or two different cargo profiles. Build both in release.
- All-base-then-all-head timing instead of interleaving, so drift loads one side.
- Reporting the single fastest (or a single cold) run instead of the median of >= 5 interleaved rounds.
- Measuring inside a churning git worktree, producing a bogus number (the 1.5%-that-was-really-16% trap).
- Using a target whose content does not exercise the changed path, then concluding "no change".
- Keeping a change that misses its stop condition because it "feels" faster.
- Editing goldens to absorb an output diff from a pure-refactor perf change instead of fixing the code.
- Recording a self before/after `perf-ab` result in `docs/BENCHMARKS.md` (that file is the ScanCode-comparison record; see `verify-benchmark-target`).
