# xtask Maintainer Commands

`xtask/` is the home for Provenant's Rust-based maintainer workflows that are
intentionally coupled to Provenant internals or to the repo-built `provenant`
binary. Small, self-contained hot-path tools that benefit from package-boundary
isolation live as separate workspace crates under `tools/`; the current
example is [`tools/license-headers/`](../tools/license-headers/README.md).

Run these commands directly with:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin <command> -- ...
```

## Command Index

| Command                           | Purpose                                                                                                                                     |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `benchmark-target`                | Measure Provenant against an explicit local or remote benchmark target.                                                                     |
| `compare-outputs`                 | Compare Provenant and ScanCode raw outputs, either by running both scanners on one target or by comparing two existing JSON files directly. |
| `perf-ab`                         | A/B-time a scan across two Provenant git refs (self before/after) and report the head-vs-base speedup.                                      |
| `update-parser-golden`            | Regenerate parser `.expected.json` fixtures from current Rust parser output.                                                                |
| `update-copyright-golden`         | Maintain copyright golden YAML fixtures with parity-gated or Rust-owned update modes.                                                       |
| `update-license-golden`           | Maintain license golden YAML fixtures with parity-gated or Rust-owned update modes.                                                         |
| `validate-urls`                   | Validate URLs in production docs and Rust docstrings.                                                                                       |
| `generate-output-field-reference` | Regenerate `docs/OUTPUT_FIELD_REFERENCE.md` from output-schema documentation metadata.                                                      |
| `generate-serve-openapi`          | Regenerate the checked-in OpenAPI document for `provenant serve`.                                                                           |
| `generate-supported-formats`      | Regenerate `docs/SUPPORTED_FORMATS.md` from parser metadata.                                                                                |
| `generate-benchmark-chart`        | Regenerate the benchmark duration-vs-files SVG from timing rows in `docs/BENCHMARKS.md`.                                                    |
| `generate-index-artifact`         | Regenerate the embedded license index artifact from ScanCode rules and licenses.                                                            |
| `classify-rule-overmatch`         | Classify upstream license rules by overmatch-risk class and rank un-covered overlay candidates.                                             |

## `benchmark-target`

### Purpose

`benchmark-target` measures Provenant against an explicitly supplied benchmark
target and reports a repeated-run matrix for:

- uncached runs
- incremental runs

This makes it useful for checking repeated-run speedups on unchanged input.

### Usage

```bash
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --help
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --repo-url https://github.com/org/repo.git --repo-ref main --profile common
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --target-path /path/to/local/directory --profile common-with-compiled
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --repo-url https://github.com/org/repo.git --repo-ref v1.2.3 --profile licenses
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --target-path /path/to/local/directory --profile packages
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --repo-url https://github.com/org/repo.git --repo-ref <sha> --profile common
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --target-path /path/to/local/directory --profile common
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --repo-url https://github.com/org/repo.git --repo-ref <sha> -- -clupe
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --target-path /path/to/local/directory -- --timeout 300 --license-text
cargo run --manifest-path xtask/Cargo.toml --bin benchmark-target -- --target-path /path/to/local/directory -- --license --package
```

CLI arguments:

- Exactly one of `--repo-url` or one-or-more `--target-path` values is required.
- `--repo-url URL`: benchmark the given repository URL via the shared repo cache.
- `--repo-ref REF`: required with `--repo-url`; commit SHA, tag, or branch to resolve and benchmark.
- `--target-path PATH`: benchmark an existing local directory in place.
- `--profile common`: convenience shorthand for `-clupe --system-package --strip-root --processes 4`.
- `--profile common-with-compiled`: convenience shorthand for `-clupe --system-package --package-in-compiled --strip-root`.
- `--profile licenses`: convenience shorthand for `-l --strip-root`.
- `--profile packages`: convenience shorthand for `-p --strip-root`.
- Pass either a supported `--profile` or explicit benchmark scan flags after `--`.
- A common explicit profile is `-clupe` (`--copyright --license --url --package --email`).

### What It Does

1. Either scans a local directory passed via `--target-path` or resolves `--repo-url` + `--repo-ref` through a shared repo cache.
2. Builds Provenant in release mode.
3. Updates or creates a shared shallow repo cache under `.provenant/repo-cache/`, fetches only the requested ref at depth 1, resolves it to a full commit SHA, and materializes a detached checkout for the run.
4. Runs cold/warm scenarios with isolated cache roots while forwarding the requested Provenant scan flags unchanged.
5. Writes a run manifest plus benchmark results under `.provenant/benchmarks/`.
6. Prints a summary table with wall time, key phase timings, peak RSS, and incremental reuse signals.

### Output

For each scenario, the command writes:

- `results/<scenario>/scan-output.json`
- `results/<scenario>/provenant-stdout.txt`
- `run-manifest.json`

It also writes a tab-separated summary file at:

- `results/summary.tsv`

### Notes

- The command uses an explicit per-scenario `--cache-dir` so incremental manifest results do not leak across scenarios.
- The command also adds `--no-license-index-cache` to every Provenant invocation so repeated benchmark runs do not inherit warmed license-index state from earlier repositories.
- `--target-path` mode scans the directory in place; it does not reset, stash, or otherwise mutate that path.
- `--repo-url` mode requires `--repo-ref`; the command resolves that ref to a full commit SHA and records the exact SHA in `run-manifest.json`.
- Warm-run comparisons are meaningful only within one invocation because the command recreates `.provenant/benchmarks` on every run.
- Benchmark artifacts are kept in the repo-local `.provenant/` developer artifact directory rather than `/tmp`, so they stay near future comparison runs and are easier to inspect before cleanup.
- Repo URL runs reuse cached git objects from `.provenant/repo-cache/` instead of recloning the upstream repository on every invocation.
- `run-manifest.json` records the Provenant binary version plus the current Provenant repository revision, dirty state, and diff hash for the run, so benchmark snapshots stay attributable as the scanner evolves.
- On macOS, the command falls back to `/usr/bin/time -l`; on systems with GNU `time`, it uses verbose memory reporting automatically.

## `compare-outputs`

### Purpose

`compare-outputs` compares Provenant and ScanCode raw outputs and produces
reduced comparison artifacts for later manual or agent review.

It supports two input modes:

- **scan-target mode**: run both scanners on the same repository or local target
- **direct-json mode**: compare one existing ScanCode JSON file and one existing Provenant JSON file without rerunning either scanner

### Requirements

- Scan-target mode requires Docker on ScanCode cache misses.
- Scan-target mode builds a local ScanCode Docker image from the bundled
  `reference/scancode-toolkit` submodule automatically when the matching image
  is missing and a ScanCode run is required.
- Direct-json mode only requires readable input JSON files.

### Usage

```bash
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --help
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --repo-url https://github.com/org/repo.git --repo-ref main --profile common
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --repo-url https://github.com/org/repo.git --repo-ref <sha> --profile common --build-mode fast-iteration
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --target-path /path/to/local/directory --profile common-with-compiled
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --repo-url https://github.com/org/repo.git --repo-ref v1.2.3 --profile licenses
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --target-path /path/to/local/directory --profile packages
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --repo-url https://github.com/org/repo.git --repo-ref <sha> --profile common
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --target-path /path/to/local/directory --profile common
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --target-path /path/to/local/directory -- --license --package --strip-root
cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- --scancode-json /path/to/scancode.json --provenant-json /path/to/provenant.json
```

CLI arguments:

- Choose exactly one input mode: either `--repo-url`, one-or-more `--target-path` values, or the pair `--scancode-json` + `--provenant-json`.
- `--repo-url URL`: compare the given repository URL via the shared repo cache.
- `--repo-ref REF`: required with `--repo-url`; commit SHA, tag, or branch to resolve and compare.
- `--target-path PATH`: compare an existing local directory in place, or repeat the flag to stage multiple local files into one compare run.
- `--scancode-json PATH`: compare an existing ScanCode output JSON directly, without rerunning ScanCode.
- `--provenant-json PATH`: compare an existing Provenant output JSON directly, without rerunning Provenant.
- `--scancode-cache-identity ID`: optional with `--target-path`; opt in to shared ScanCode cache reuse for a caller-asserted local snapshot identity.
- `--build-mode optimized|fast-iteration`: choose how scan-target mode builds the local Provenant binary. The default `optimized` mode uses `cargo build --release` for benchmark-grade timing. `fast-iteration` uses `cargo build --profile ci-release` for quicker output-focused reruns while you are still triaging changes.
- `--profile common`: convenience shorthand for `-clupe --system-package --strip-root --processes 4`.
- `--profile common-with-compiled`: convenience shorthand for `-clupe --system-package --package-in-compiled --strip-root`.
- `--profile licenses`: convenience shorthand for `-l --strip-root`.
- `--profile packages`: convenience shorthand for `-p --strip-root`.
- In scan-target mode, pass either a supported `--profile` or explicit shared scan flags after `--`.
- Direct-json mode compares the provided JSON files as-is and does not accept `--profile` or explicit scan flags after `--`.
- Build mode only matters in scan-target mode because direct-json mode compares already-produced raw outputs without rebuilding Provenant.
- Compare reduction still applies compatibility-aware in-memory normalization for known intentional differences such as Provenant's raw-default file-level copyright rendering, so parity review does not fail noisily just because Provenant preserved `All rights reserved` or punctuation in the saved JSON.

### What It Does

1. Creates a per-run artifact directory under `.provenant/compare-runs/`.
2. In scan-target mode, either scans the local directory in place or resolves `--repo-url` + `--repo-ref` through a shared repo cache.
3. In scan-target mode, builds Provenant in the selected build mode (`optimized` by default via `cargo build --release`; `fast-iteration` via `cargo build --profile ci-release`).
4. In scan-target mode, updates or creates a shared shallow repo cache under `.provenant/repo-cache/`, fetches only the requested ref at depth 1, resolves it to a full commit SHA, and materializes a detached checkout for the run.
5. In scan-target mode, resolves the ScanCode runtime identity and, on cache misses, ensures a local Docker-backed ScanCode runtime exists by building the image from `reference/scancode-toolkit` if needed.
6. In scan-target mode, reuses cached ScanCode raw artifacts when available, otherwise runs ScanCode alongside Provenant with the same shared scan profile and ephemeral license-cache directories.
7. In direct-json mode, copies the provided raw JSON files into the run artifact directory without rerunning either scanner.
8. Saves raw outputs and logs under `raw/`.
9. Produces reduced comparison artifacts under `comparison/` and prints the absolute artifact paths at the end.

### Output

Each run writes artifacts under:

- `.provenant/compare-runs/<run-id>/`

Core files:

- `run-manifest.json`
- `raw/scancode.json`
- `raw/provenant.json`
- `comparison/summary.json`
- `comparison/summary.tsv`
- `comparison/samples/*.json`

`comparison/summary.json` includes a neutral `comparison_status`. `review_required`
means the reduced compare found deltas that still need reviewer or agent
classification; it is not itself a regression verdict. The summary also exposes
directional signal buckets under `comparison_signal_summary` plus top-level
directional maps for the ScanCode-favored and Provenant-favored sides.

Alongside the identity-only `package_data` bucket, the summary also reports a
`package_field_content` axis (mirrored in `package_field_content_summary` and
the `package_field_content_value_differences.json` sample). For packages that
both outputs agree exist (matched by purl, else the
`type|name|version|datasource_id` fallback, across both top-level `packages[]`
and file-level `files[].package_data[]`), it diffs the **content** of
`declared_license_expression`, `declared_license_expression_spdx`, and `holder`.
This catches a change that drops or corrupts declared-license/holder content
even when every package identity still matches. Each difference is counted in
exactly one of three reconcilable buckets: `missing_in_provenant` (content only
on the ScanCode side), `extra_in_provenant` (content only on the Provenant
side), and `value_vs_value_mismatch` (both sides carry content but the values
differ), so `missing + extra + value_vs_value_mismatch` always equals the total
number of differences and `sum(by_field.values())`. Because it compares content
for the first time, enabling it can surface pre-existing ScanCode-vs-Provenant
declared-license deltas that have nothing to do with any recent change; that is
expected, valuable parity signal to classify, not automatically a regression.

Optional diagnostic logs when available:

- `raw/scancode-stdout.txt`
- `raw/provenant-stdout.txt`

### Notes

- The command keeps the full raw scanner outputs; it does **not** stream giant machine-readable payloads to stdout.
- Stdout is reserved for progress, a reduced summary table, and the saved artifact paths.
- ScanCode currently runs via Docker on all platforms for this workflow because that is the reproducible runtime path verified in this repository.
- Direct-json mode skips Docker, Provenant binary builds, cache preparation, and scanner execution. It compares the provided raw JSON files exactly as supplied.
- Use the default `--build-mode optimized` whenever the run's timing may be recorded in `docs/BENCHMARKS.md` or compared seriously against ScanCode wall-clock numbers.
- Use `--build-mode fast-iteration` when you only need a quicker compare rerun to see whether a code change affected output. Before recording benchmark timing or refreshing a benchmark entry, rerun the target in the default optimized mode.
- When ScanCode caching is enabled, `compare-outputs` prewrites the cache manifest before the Docker run starts. If an outer wrapper times out after ScanCode has begun but the container keeps running, wait for that container first and only salvage the finished `raw/scancode.json` (and `raw/scancode-stdout.txt` when present) into the printed ScanCode cache directory when `docker wait` exits `0`. If the container exits non-zero, do **not** populate the cache from that run.
- When `compare-outputs` actually executes either scanner, it disables persistent license-cache reuse for fairness: Provenant runs with `--no-license-index-cache`, and ScanCode uses container-local ephemeral cache directories.
- `compare-outputs` passes the same shared scan args to both scanners. The `common` profile includes installed package database coverage and fixes the shared worker count at `--processes 4`, which is usually a no-op on ordinary source repositories but matters for extracted rootfs/container trees and other artifact targets. Use `common-with-compiled` when you also want Go/Rust compiled-binary package extraction in the shared scan profile.
- Direct-json mode infers whether info/classify/summary-style sections should be compared from the JSON contents themselves, so it does not require replaying the original scan flags just to unlock those diffs.
- For `--profile common`, the ScanCode Docker invocation also adds `--memory 12g --memory-swap 12g`, and that runtime cap is part of ScanCode cache validation.
- `--repo-url` mode requires `--repo-ref`; the command records both the requested ref and the resolved full commit SHA in `run-manifest.json`.
- In scan-target mode, `run-manifest.json` also records the Provenant binary version plus the current Provenant repository revision, dirty state, and diff hash, alongside the ScanCode runtime identity. Direct-json mode records placeholder command/runtime fields because it compares the supplied raw JSON files without executing either scanner.
- Repo URL runs reuse cached git objects from `.provenant/repo-cache/`, fetch only the requested ref shallowly, and remove the temporary detached checkout after the run so compare artifacts do not retain duplicate full repository trees.
- Repo URL runs also reuse cached raw ScanCode artifacts from `.provenant/scancode-cache/` when the resolved target commit, ScanCode runtime identity, and effective ScanCode scan args are unchanged.
- Local `--target-path` runs rerun ScanCode by default. Pass `--scancode-cache-identity <id>` to opt into shared ScanCode raw-artifact reuse for a local snapshot you have identified explicitly.
- For local target-path cache hits, the **path itself is not the cache identity**. The cache key is derived from your explicit `--scancode-cache-identity` plus the effective ScanCode runtime/args, so reusing the same identity across different local paths will intentionally hit the same cache entry when the staged snapshot is meant to be the same.
- For local target-path cache hits, keep all of these stable between runs: the explicit `--scancode-cache-identity`, the effective ScanCode scan args/profile, the ScanCode runtime identity (`image`, runtime revision/dirty state/diff hash, Docker platform), and the ScanCode memory limits that `compare-outputs` applies.
- Treat `--scancode-cache-identity` as a caller-owned snapshot label, not a convenience name. If the local artifact contents changed in any meaningful way, bump the identity yourself; xtask validates the manifest/runtime/args, but it does **not** hash local target contents for you.
- Repeated `--target-path` values currently support **files only** and are mainly intended for multi-input `--from-json` replay compares. The harness stages those files under one temporary input directory and passes them explicitly to both scanners in the same order.
- For repeated local `--target-path` file inputs, cache hits also depend on keeping the same file list order, because xtask stages them as ordered numbered filenames under one temporary input tree before invoking ScanCode.
- When shared scan args reference a local auxiliary file for both scanners (for example `--license-policy <path>`, `--license-rules-path <path>`, or `--custom-template <path>`), `compare-outputs` stages that file separately and rewrites the ScanCode Docker path automatically so both scanners see the same auxiliary input.
- Cache hits now require a cached `scancode.json` plus cache `manifest.json`; `scancode-stdout.txt` is reused when available but is no longer required for cache completeness.
- A quick target-path rerun checklist for expected ScanCode cache hits is: same `--scancode-cache-identity`, same `--profile` or explicit scan args, same auxiliary inputs, same local file order when repeating `--target-path`, and no ScanCode runtime change since the cache was written.
- `scancode-stdout.txt` and `provenant-stdout.txt` are best-effort diagnostic logs. The compare pipeline only requires the JSON outputs, so a log-write failure no longer makes the command fail.
- Path-only buckets in `comparison/summary.json`, `comparison/summary.tsv`, and `comparison/samples/*.json` describe **final output membership**, not proven scan coverage. When the compared outputs used `--only-findings`, a path shown only on one side can simply mean the other side scanned it but filtered it away because no findings remained.
- The command adds Git control-path ignore rules (`.git`, nested `.git`, and their contents) on the ScanCode side so repository metadata does not dominate the comparison artifacts without hiding package-adjacent files such as `.gitmodules`. Provenant already excludes `.git` directories during path collection by default, so xtask does not need to restate those ignores for Provenant. The harness no longer injects a blanket `target/*` ignore because some upstream repositories legitimately use `target/` as source content.

## `perf-ab`

### Purpose

`perf-ab` answers "did my code change actually make Provenant faster?". It
builds two Provenant git refs in release mode and **interleaved-A/B-times** a
scan against the same target, reporting per-side medians and the head-vs-base
speedup.

This is a **self before/after** comparison and is intentionally different from
`benchmark-target` and `compare-outputs`, which measure **Provenant against
ScanCode**. `perf-ab` never runs ScanCode and its timings do not belong in
`docs/BENCHMARKS.md`; it is for attributing a wall-clock change to your own
edit. The companion `benchmark-perf-change` skill describes the full
profile-first, regression-guarded workflow this command supports.

### Usage

```bash
cargo run --manifest-path xtask/Cargo.toml --bin perf-ab -- --help
cargo run --manifest-path xtask/Cargo.toml --bin perf-ab -- --base origin/main --head HEAD --target-path /path/to/repo -- -c -n 1
cargo run --manifest-path xtask/Cargo.toml --bin perf-ab -- --base origin/main --head HEAD --repo-url https://github.com/org/repo.git --repo-ref <sha> --rounds 7 --profile licenses
cargo run --manifest-path xtask/Cargo.toml --bin perf-ab -- --base <ref> --head <ref> --target-path /path/to/repo --check-output -- -clupe -n 1
cargo run --manifest-path xtask/Cargo.toml --bin perf-ab -- --base-bin ./base/provenant --head-bin ./head/provenant --target-path /path/to/repo -- -c -n 1
```

CLI arguments:

- `--base REF`: base git ref to build and time (the "before" side). Default `origin/main`.
- `--head REF`: head git ref to build and time (the "after" side). Default `HEAD`.
- Choose exactly one target: either `--repo-url` (with `--repo-ref`) via the shared repo cache, or `--target-path PATH` for a local directory scanned in place.
- `--repo-ref REF`: required with `--repo-url`; commit SHA, tag, or branch of the target repo.
- `--rounds N`: number of interleaved timed rounds per side after the discarded warmup. Default `5`.
- `--base-bin PATH` / `--head-bin PATH`: use a prebuilt binary for that side and skip building the ref.
- `--check-output`: scan with both binaries to JSON and assert byte-identical output after normalizing the volatile header. The correctness gate for pure-refactor perf changes.
- `--profile common|common-with-compiled|licenses|packages`: scan-flag shorthand shared by both sides. Mutually exclusive with explicit flags after `--`.
- Pass either a supported `--profile` or explicit scan flags after `--` (for example `-- -c -n 1`). The same flags are forwarded to both binaries.

### What It Does

1. Resolves the scan target: either a local `--target-path` scanned in place, or `--repo-url` + `--repo-ref` materialized through the shared repo cache under `.provenant/repo-cache/` and removed after the run.
2. Builds each ref in release (`cargo build --release --bin provenant`) into a stable per-ref worktree under `.provenant/perf-ab-build/`, backed by a dedicated bare clone of the current repo so the developer's working tree is untouched. `--base-bin` / `--head-bin` skip the build for that side.
3. Optionally runs the `--check-output` correctness gate before timing.
4. Runs one discarded warmup scan per binary, then `--rounds` interleaved timed scans (base, head, base, head, …) so machine-wide drift is shared evenly.
5. Reports per-round real times, the per-side median, and the speedup as both a percent reduction and a factor (a factor below `1.0` is flagged as a regression).
6. Writes a run manifest under `.provenant/perf-ab/<run-id>/run-manifest.json` recording both refs, their resolved build revisions, the binaries used, the per-round timings, and the computed speedup.

### Output

Each run writes artifacts under:

- `.provenant/perf-ab/<run-id>/run-manifest.json`
- `.provenant/perf-ab/<run-id>/<side>-<phase>-scan.json` (scratch scan outputs from timed runs)

### Notes

- Both sides always run with `--no-license-index-cache` so a warmed license index from one round does not skew the next, matching `benchmark-target` fairness.
- The warmup run is discarded; only the `--rounds` interleaved timed runs feed the median.
- Build each ref in release on a stable checkout; do not point `--base-bin`/`--head-bin` at debug or differently-profiled binaries, which would make the comparison meaningless.
- `--check-output` normalizes only the volatile top-level `headers` block (version, timestamps, duration) before comparing; any remaining diff fails the run, because a behavior-preserving perf change must be byte-identical.
- Building two release binaries is slow. Use `--base-bin`/`--head-bin` when you already have both binaries, or `--repo-ref`/refs that share most compiled artifacts.

## `update-parser-golden`

`update-parser-golden` regenerates parser `.expected.json` fixtures directly from current Rust parser output.

Show CLI help:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin update-parser-golden -- --help
```

CLI arguments:

- `<ParserType>`: parser struct name (for example `NpmParser`)
- `<input_file>`: fixture input file to parse
- `<output_file>`: `.expected.json` file to write
- `--list`: list all registered parser types

Example:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin update-parser-golden -- <ParserType> <input_file> <output_file>
```

`update-parser-golden` rewrites a fixture's whole package object and only emits a
single package, so it cannot refresh multi-package goldens or goldens that wrap
the package in a richer document shape. For those, and to refresh only the
fields the central post-extraction step owns (`declared_license_expression`,
`declared_license_expression_spdx`, `license_detections`, `holder`) while
leaving the rest of the fixture byte-stable, run the parser golden suite with
the in-place refresh switch:

```bash
PROVENANT_UPDATE_PARSER_GOLDEN=1 cargo test --features golden-tests --test parsers_golden
```

The switch makes the golden comparators surgically patch those fields in place
and report success without comparing; unset, the comparators behave normally.
Always review the resulting diff before committing.

## `update-copyright-golden`

`update-copyright-golden` syncs and updates copyright golden YAML fixtures (authors / ics / copyrights).

Show CLI help:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin update-copyright-golden -- --help
```

CLI arguments:

- `<authors|ics|copyrights>`: fixture suite to process
- `--list-mismatches`: print files where Python reference expectations differ from current Rust detector output (parity precheck)
- `--show-diff`: print missing/extra summary for those Python-reference parity mismatches (plus samples with `--filter`)
- `--filter PATTERN`: limit processing to paths containing `PATTERN`
- `--sync-actual`: write expected values from current Rust detector output
- `--write`: apply file updates (without it, command is dry-run)

`ics` here refers to the Android Ice Cream Sandwich (Android 4.0) fixture corpus from ScanCode reference tests.

Important distinction: this command is a maintenance/sync tool. Golden tests compare Rust detector output to local Rust-owned fixture YAMLs; `--list-mismatches` compares Rust detector output to Python reference expectations to decide whether a sync is parity-safe. This remains detector-level parity work; the newer file-level output rendering difference is handled separately in output and compare tests.

Expected workflow:

1. Check Python-reference parity impact first:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-copyright-golden -- copyrights --list-mismatches --show-diff
   ```

2. If parity is acceptable for a fixture, sync from Python reference:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-copyright-golden -- copyrights --filter <pattern> --write
   ```

3. If divergence is intentional or Rust-specific, update to Rust actuals:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-copyright-golden -- copyrights --sync-actual --filter <pattern> --write
   ```

## `update-license-golden`

`update-license-golden` syncs and updates license golden YAML fixtures.

Show CLI help:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin update-license-golden -- --help
```

CLI arguments:

- `--list-mismatches` (`--list-diffs` alias): print files where Python reference expectations differ from current Rust detector output (parity precheck)
- `--show-diff`: print detailed diff for those mismatches
- `--filter PATTERN`: limit processing to paths containing `PATTERN`
- `--suite SUITE`: process only one suite (lic1, lic2, lic3, lic4, external, unknown)
- `--sync-actual`: write expected values from current Rust detector output
- `--write`: apply file updates (without it, command is dry-run)

Expected workflow:

1. Check Python-reference parity impact first:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-license-golden -- --list-mismatches --show-diff
   ```

2. If parity is acceptable for a fixture, sync from Python reference:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-license-golden -- --suite lic1 --filter <pattern> --write
   ```

3. If divergence is intentional or Rust-specific, update to Rust actuals:

   ```bash
   cargo run --manifest-path xtask/Cargo.toml --bin update-license-golden -- --sync-actual --suite unknown --filter <pattern> --write
   ```

## `validate-urls`

`validate-urls` systematically validates all URLs in production documentation and Rust docstrings.

Manual run:

```bash
cargo run --quiet --manifest-path xtask/Cargo.toml --bin validate-urls -- --root .
```

Exit codes:

- `0`: all URLs valid
- `1`: some URLs failed validation

This command is informational in CI and does not block PRs.

## `generate-supported-formats`

`generate-supported-formats` regenerates `docs/SUPPORTED_FORMATS.md` from parser metadata.

Examples:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin generate-supported-formats
cargo run --manifest-path xtask/Cargo.toml --bin generate-supported-formats -- --check
```

## `generate-output-field-reference`

`generate-output-field-reference` regenerates the checked-in public output field reference from semantic metadata stored under `src/output_schema/`.

Examples:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin generate-output-field-reference
cargo run --manifest-path xtask/Cargo.toml --bin generate-output-field-reference -- --check
```

## `generate-serve-openapi`

`generate-serve-openapi` regenerates the checked-in OpenAPI document for the current `provenant serve` API surface.

Examples:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin generate-serve-openapi
cargo run --manifest-path xtask/Cargo.toml --bin generate-serve-openapi -- --check
```

## `generate-benchmark-chart`

`generate-benchmark-chart` regenerates `docs/scan-duration-vs-files.svg` and refreshes the headline benchmark summary stats in `docs/BENCHMARKS.md` from the benchmark timing rows in that document.

The command computes and prints:

- number of recorded runs
- number of runs where Provenant is faster
- median speedup
- geometric-mean speedup
- median speedup on sub-100-file targets
- median speedup on 10k+-file targets

`--check` verifies both the checked-in SVG and the BENCHMARKS headline summary line.

Examples:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin generate-benchmark-chart
cargo run --manifest-path xtask/Cargo.toml --bin generate-benchmark-chart -- --check
```

## `generate-index-artifact`

`generate-index-artifact` regenerates the embedded license index artifact from ScanCode rules and licenses.
The generated artifact reflects the checked-in build policy at
`resources/license_detection/index_build_policy.toml`, so policy changes should
be committed alongside the regenerated `license_index.zst` artifact.
Downstream add/replace overlays live as regular `.RULE` / `.LICENSE` files under
`resources/license_detection/overlay/`, and the generated artifact embeds
provenance that surfaces in structured scan output headers.
If upstream absorbs one of these local curations, artifact generation now fails
fast so maintainers remove the redundant ignore/overlay instead of silently
shipping dead policy.

Examples:

```bash
cargo run --manifest-path xtask/Cargo.toml --bin generate-index-artifact
cargo run --manifest-path xtask/Cargo.toml --bin generate-index-artifact -- --check
```

## `classify-rule-overmatch`

`classify-rule-overmatch` scores every upstream ScanCode rule against the same
overmatch-risk signals the bundled overlays in
`resources/license_detection/index_build_policy.toml` already encode, then ranks
the rules that share a systematic root cause but are not yet covered by an
overlay. It is a read-only reporting tool: it never edits the dataset or policy.

The risk classes mirror the existing overlay clusters:

- `BareWeakWord`: short bare GPL-family shorthand mapped to an unversioned bucket
  (GPL → `gpl-1.0-plus`, LGPL → `lgpl-2.0-plus`, AGPL → `agpl-3.0-plus`); too
  weak to assert a concrete license, so a clue-only treatment fits.
- `VersionMismatch`: a short notice whose expression asserts a _specific
  elevated_ GPL/LGPL/AGPL version but whose text carries no matching version
  anchor, so it can overmatch a neighbouring version.
- `BareReferencedFilename`: a short notice that asserts a license via a bare
  COPYING/LICENSE reference with no independent version anchor.
- `BsdEndorsement`: a short `bsd-new` text rule that contains the endorsement
  clause but is neither continuous nor full coverage.

Rules already guarded by an inline required phrase (`{{...}}`), and false
positive, clue, and deprecated rules, are skipped.

Use it to triage candidates before curating overlays; the highest-signal,
systematic class in practice is `BareWeakWord`. Treat the other classes as
review candidates rather than blanket-apply targets: many of the flagged rules
encode ScanCode's deliberate unversioned-bucket mappings, and demoting them
would trade false positives for recall.

Examples:

```bash
# Text report ranked by risk score (defaults to the bundled ScanCode corpus).
cargo run --manifest-path xtask/Cargo.toml --bin classify-rule-overmatch

# Machine-readable output for further analysis.
cargo run --manifest-path xtask/Cargo.toml --bin classify-rule-overmatch -- --json
```
