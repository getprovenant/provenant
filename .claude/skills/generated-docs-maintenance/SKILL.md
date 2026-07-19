---
name: generated-docs-maintenance
description: Maintain Provenant generated docs and their xtask checks. Use for supported formats, output field reference, serve OpenAPI, benchmark chart, generated docs drift, validate-urls, or docs check failures.
---

# Generated Docs Maintenance

Use this skill when a checked generated document is stale, a docs CI check reports drift, or a code change requires regenerating Provenant-maintained documentation artifacts.

## Best Fit

Use this skill when the task mentions:

- `docs/SUPPORTED_FORMATS.md`
- `docs/OUTPUT_FIELD_REFERENCE.md`
- serve OpenAPI generation or drift
- benchmark chart or benchmark headline statistics
- URL validation or docs lint/format failures
- generated docs changed by a hook or CI check

## High-Signal Gotchas

- Generated docs should be regenerated from source metadata, not hand-edited.
- `docs/SUPPORTED_FORMATS.md` can drift when parser metadata changes even if parser tests pass.
- `generate-benchmark-chart` reads timing rows from `docs/BENCHMARKS.md`; the SVG and headline are not independent data sources.
- `npm run check:docs` uses `git ls-files`, so untracked new Markdown needs explicit targeted formatting/lint checks before it is staged.
- URL validation is a blocking CI check: a broken/removed production link fails `check.yml`. Re-point rotted benchmark-artifact URLs to a committed `testdata/` fixture, or remove the non-reproducible row.
- Prettier and markdownlint are Node/npm-managed, so docs validation needs the repo's Node toolchain.

## Source Documents

- `docs/DOCUMENTATION_INDEX.md` - document ownership and lifecycle
- `CONTRIBUTING.md` - docs validation defaults
- `xtask/README.md` - generated artifact commands
- `package.json` - npm docs scripts
- `.github/workflows/check.yml` - CI generated-doc checks
- `docs/BENCHMARKS.md` - benchmark chart source rows and methodology

## Generator Map

| Surface                                                        | Owning command                                                                       |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `docs/SUPPORTED_FORMATS.md`                                    | `cargo run --manifest-path xtask/Cargo.toml --bin generate-supported-formats`        |
| `docs/OUTPUT_FIELD_REFERENCE.md`                               | `cargo run --manifest-path xtask/Cargo.toml --bin generate-output-field-reference`   |
| Serve OpenAPI document                                         | `cargo run --manifest-path xtask/Cargo.toml --bin generate-serve-openapi`            |
| `docs/scan-duration-vs-files.svg` and benchmark headline stats | `cargo run --manifest-path xtask/Cargo.toml --bin generate-benchmark-chart`          |
| Production docs and Rust docstring URLs                        | `cargo run --quiet --manifest-path xtask/Cargo.toml --bin validate-urls -- --root .` |
| Markdown lint/format                                           | `npm run check:docs`                                                                 |

Use each generator's `-- --check` mode when you only need to confirm drift.

## Workflow

### 1. Identify the owner

Map the stale file to its generator before editing. Generated docs should be regenerated from source metadata, not hand-edited.

Common sources:

- parser metadata for supported formats
- output schema metadata for output field reference
- serve API schema/code for OpenAPI
- timing rows in `docs/BENCHMARKS.md` for the benchmark chart

### 2. Fix source metadata first

If generated output is wrong, fix the owning source first. Examples:

- parser metadata or registration, not `docs/SUPPORTED_FORMATS.md`
- output schema field docs, not `docs/OUTPUT_FIELD_REFERENCE.md`
- serve API types/routes, not generated OpenAPI JSON
- benchmark timing rows, not only the generated SVG/headline

### 3. Regenerate and validate

Run the generator, inspect the diff, then run the matching check mode. For documentation-only changes, finish with:

```bash
npm run check:docs
```

If code changed, also run the narrow Rust test or check that owns the source metadata.

## Boundaries

- Parser implementation workflow belongs to `add-parser`.
- Benchmark target verification belongs to `verify-benchmark-target`; this skill only maintains generated chart/doc artifacts after benchmark rows are correct.
- General CLI usage belongs to `provenant-cli`.
