# Agent Guidelines for Provenant

This file is for evergreen repo-specific guardrails and recurring agent gotchas. It intentionally duplicates a small set of high-value facts from the canonical docs so those facts stay in agent context even when an agent does not read further. Keep long procedures and fast-changing workflow detail in the canonical docs instead of fully restating them here.

## Start Here

- [`README.md`](README.md) for the project overview, user-facing setup, and CLI entry points.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) for the main contributor workflow, local setup, hooks, commit/PR conventions, DCO, and license-header policy.
- [`docs/DOCUMENTATION_INDEX.md`](docs/DOCUMENTATION_INDEX.md) when you need to find which document owns a topic.

Important baseline facts to remember on every run:

- `README.md` is user-facing; `CONTRIBUTING.md` is the main contributor workflow document.
- Contributor setup is dual-stack: Rust toolchain plus Node.js `>=24` with `npm`.
- The usual local bootstrap is `npm run setup`.
- `npm run hooks:run` runs the full pre-commit hook suite on all files.
- `npm run check:docs` is the default validation entry point for documentation-only changes.

Before making non-trivial changes, read the document that owns the surface you are touching:

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for overall system design, parser/assembly boundaries, and scanner-owned exceptions.
- [`docs/LICENSE_DETECTION_ARCHITECTURE.md`](docs/LICENSE_DETECTION_ARCHITECTURE.md) for license-index, dataset, cache, and detection-pipeline details.
- [`docs/HOW_TO_ADD_A_PARSER.md`](docs/HOW_TO_ADD_A_PARSER.md) for parser workflow, registration, datasource wiring, and assembly/file-reference integration.
- [`docs/TESTING_STRATEGY.md`](docs/TESTING_STRATEGY.md) for test layers and validation expectations.
- [`xtask/README.md`](xtask/README.md) for maintainer workflows such as compare runs, golden maintenance, generated artifacts, and benchmark helpers.

## Project Context

Provenant's primary goal is to produce the best practical scan result for users: accurate, explainable, bounded, and maintainable. ScanCode compatibility is an important compatibility lane and regression signal, not the end goal. Preserve parity where users depend on it, but do not treat ScanCode output or implementation choices as automatically correct when Provenant can provide a better-supported result.

Routine scans use the embedded license index. The `reference/scancode-toolkit/` submodule is mainly needed for parity research, embedded-license-data maintenance, and maintainer workflows that depend on upstream material.

Use the Python ScanCode codebase as a behavioral reference, not an implementation template or an unquestioned source of truth. When comparing with ScanCode code or scan output, first identify the user-facing contract it demonstrates, then ask whether Provenant can solve the case more accurately, clearly, or safely before adopting the same behavior.

When an upstream test fixture is needed for Provenant tests, copy it into Provenant-owned `testdata/` and reference that local copy. Do **not** make tests or golden fixtures depend directly on paths under `reference/scancode-toolkit/`. Prefer synthetic or truncated fixtures over wholesale copies. When you do add a **substantial, wholesale** copy of an identifiable third-party file or package — especially under a copyleft or notice-required license — record its source and license in a short co-located note (a `README`/`SOURCE` in the containing directory), not in a central index. See [`testdata/PROVENANCE.md`](testdata/PROVENANCE.md) for the overall provenance policy.

## Run-Every-Time Guardrails

- Keep parsing static and bounded. Do not execute package-manager code, project code, or shell commands to recover metadata.
- Preserve behavior and parity where users depend on it. If Provenant intentionally diverges, preserve and test the explicit Provenant contract and any documented compatibility lane.
- Treat ScanCode deltas as evidence to investigate, not automatic Provenant bugs. When a difference improves correctness, precision, safety, explainability, or boundedness, keep the Provenant behavior and document or test the contract as appropriate.
- Keep package extraction separate from broader detection work. Parsers may normalize trustworthy declared package-license metadata, but file-content license and copyright detection belong to the detection pipeline.
- Parsers are file-local extractors. Cross-file ownership, topology-aware workspace handling, and file-reference resolution belong in assembly unless an existing documented scanner-owned exception says otherwise.
- Prefer honest unknowns over guessed compatibility defaults. If a datasource does not prove dependency intent such as `is_runtime`, `is_optional`, `is_direct`, or `is_pinned`, leave it unset.
- Use `cargo add`, `cargo remove`, and targeted `cargo update` instead of editing Rust dependencies by hand. Do not add dependencies lightly, and check maintenance health before introducing a new one.
- Treat contributor tooling as dual-stack: Rust plus Node/npm-managed hooks and doc tooling. See [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`package.json`](package.json) for the current bootstrap and helper commands.

## Frequent Agent Gotchas

- Scan commands need at least one explicit output flag such as `--json-pp -` or `--json out.json`.
- Detections are opt-in. Flags such as `--license`, `--package`, `--copyright`, `--info`, `--email`, and `--url` change what the scan actually collects.
- Default file-level copyright output is more source-faithful than historic ScanCode rendering; use `--compat-mode scancode` when a parity-sensitive workflow needs the ScanCode-style rendered value.
- `--package-only` is not a synonym for `--package`; it is a narrower fast path with different output semantics.
- Prefer `--paths-file` when you already have an explicit rooted file list. Use `--include` for glob-style filtering inside a scan root. `--paths-file` requires exactly one native scan root and is not the same as `--from-json` reshaping.
- The shared cache root is used for both incremental scan state and the license-index cache. Cache flags can materially change repeat-run behavior and benchmark fairness.
- [`docs/SUPPORTED_FORMATS.md`](docs/SUPPORTED_FORMATS.md) is auto-generated and must not be edited manually.

Use [`docs/CLI_GUIDE.md`](docs/CLI_GUIDE.md) for current CLI workflows and flag combinations.

## Testing and Validation Defaults

Keep local validation tightly scoped. This repository has many slow and specialized tests, so default to the smallest command that proves your change and let CI handle the broader matrix.

- Prefer focused commands such as `cargo test --doc`, `cargo test --test <suite_name>`, `cargo test --lib <filter>`, or `cargo test --features golden-tests <filter>` when those match the change.
- Prefer exact test paths or narrowly owned suites over broad substring filters.
- Avoid broad local commands such as `cargo test`, `cargo test --all`, `cargo test --lib`, or unfiltered golden suites unless there is no narrower way to validate the change.
- For documentation-only changes, use the docs checks from [`CONTRIBUTING.md`](CONTRIBUTING.md) / [`package.json`](package.json).
- Do not update golden expected files just to make a failing test pass; fix the implementation unless the new output is intentional and documented.
- For parser and golden-covered work, make sure new or updated `.expected` fixtures are actually generated and committed when the change is intentional.
- All checks must pass before merging, even if CI is the place that runs the full matrix.

For parser work, Layer 3 scanner/assembly contract tests are the default expectation when downstream package, dependency, assembly, or file-link behavior matters. Parser-only tests and parser goldens do not cover the full scanner-wired contract by themselves. See [`docs/TESTING_STRATEGY.md`](docs/TESTING_STRATEGY.md).

## Code Quality Guardrails

- Avoid `.unwrap()` in library code unless a panic is genuinely intended.
- Do not use `#[allow(dead_code)]` or clippy suppressions as a shortcut. Suppressions should be rare, permanent, and justified in comments.
- Use comments to explain non-obvious intent or tradeoffs, not to restate the code. Keep them sparse and short; do not add unnecessary or verbose comments, and never narrate what the code plainly already says.
- Use `Path` and `PathBuf` for filesystem paths instead of string concatenation, and watch for `\n` vs `\r\n` sensitivity in tests.
- When touching scanner concurrency or shared-state code, preserve thread safety and parallel-processing assumptions.

## Parser and Detection Work

Treat [`docs/HOW_TO_ADD_A_PARSER.md`](docs/HOW_TO_ADD_A_PARSER.md) as the canonical guide for parser work. In particular, remember these recurring failure modes:

- A parser that is not registered in `src/parsers/mod.rs` will never be called by scanner dispatch.
- `datasource_id` must be set on every production path, including parse-error and fallback returns.
- Use `crate::parser_warn!` for parser failures so diagnostics land in structured scan output.
- Every new datasource must be classified for assembly accounting.
- If a parser emits `PackageData.file_references`, assembly ownership for resolution must also be wired.
- Parser changes that affect supported-surface metadata must keep parser metadata registration and generated docs in sync.
- One `PackageType` can map to multiple datasource IDs; use datasource IDs for file-format-level assembly behavior.

For license-detection dataset curation, prefer checked-in overlays under `resources/license_detection/overlay/` over matcher-code changes when the issue is a rule/license semantic problem rather than an engine problem. Typical overlay-worthy fixes are rule reclassification, minimum-coverage tuning, required-phrase tightening, or other upstream-compatible `.RULE` / `.LICENSE` adjustments. After changing overlays or `resources/license_detection/index_build_policy.toml`, regenerate the embedded artifact with `cargo run --manifest-path xtask/Cargo.toml --bin generate-index-artifact` and rerun the narrow owning license tests.

Overlays are for **file-content** detection semantics. For a bare or informal name in a package's **declared** license field that is not valid SPDX (for example a manifest `license: "Apache"` or `"PSF"`), add it to [`resources/license_detection/declared_license_aliases.toml`](resources/license_detection/declared_license_aliases.toml) instead of an overlay or matcher code. That config maps the name to a canonical license key only in the bounded declared field, never in file content, so it cannot cause file-content false positives; it is compiled in via `include_str!` and needs no index regeneration. Litmus test: if the bare word also appears as ordinary prose or inside its own license notice (e.g. `Apache`, `Boost`, `Python`), an index rule for it would misfire on file content, so use the alias — not an overlay. Every entry carries a required justification, enforced by tests that also verify the target key resolves in the index.

For parity-sensitive parser work, use the compare and benchmark workflows in [`xtask/README.md`](xtask/README.md) instead of relying on ad hoc raw diffs.

## Architecture Reminder

- Internal domain types and the public ScanCode-compatible output schema are intentionally separate. Put domain semantics in `src/models/` and output-shaping concerns in `src/output_schema/`.

## Contributor Compliance Reminders

- Sign off authored commits with `git commit -s` to satisfy the DCO policy.
- Use Conventional Commits format for commit messages and pull request titles.
- Keep PR scope disciplined. For ecosystem or parser work, prefer one ecosystem family per PR.
- Follow [`.github/pull_request_template.md`](.github/pull_request_template.md) for agent-authored PRs and omit sections that do not apply.
- When creating PRs with `gh`, do not combine `--template` with `--body` or `--body-file`; if you script the PR body, render the template structure manually.
- License-header scope and repair/check commands are owned by [`CONTRIBUTING.md`](CONTRIBUTING.md), [`tools/license-headers/README.md`](tools/license-headers/README.md), and [`.license-headers.toml`](.license-headers.toml).
- Files whose Rust code is derived from the ScanCode Toolkit (ported/adapted, not just behavior-compatible) must carry dual `nexB Inc. and others` + `Provenant contributors` headers with a "Derived from ScanCode Toolkit" change notice. The authoritative list is the `derived` key in [`.license-headers.toml`](.license-headers.toml). When you add or convert a file that ports ScanCode code, add its path to that list and run the header `--fix`; the header check enforces it.

## Documentation Ownership Notes

- Keep evergreen contributor and architecture docs under [`docs/`](docs/).
- Do not edit generated docs such as [`docs/SUPPORTED_FORMATS.md`](docs/SUPPORTED_FORMATS.md) by hand; use the owning generation command.
- Parser changes can require regenerating `docs/SUPPORTED_FORMATS.md`; the pre-commit hook checks this and stages the updated file when applicable.
- For release, benchmark, compare-output, and artifact-generation workflows, use [`xtask/README.md`](xtask/README.md).
- If you are unsure which document owns a topic, start with [`docs/DOCUMENTATION_INDEX.md`](docs/DOCUMENTATION_INDEX.md).

If you believe you found a security issue, follow [`SECURITY.md`](SECURITY.md) and avoid public disclosure first.
