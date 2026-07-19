# CI Check Map

Use this reference after identifying the failed GitHub job and step. Start with the smallest matching local command.

| CI symptom                      | First local check                                                                                                                        | Likely owner                                         |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| License headers                 | `npm run headers:check`                                                                                                                  | first-party file headers                             |
| Release version sync            | `./scripts/check_release_version_sync.sh`                                                                                                | release/version metadata                             |
| ScanCode output format sync     | `./scripts/check_scancode_output_format_sync.sh`                                                                                         | ScanCode reference compatibility                     |
| Embedded license index          | `.github/actions/verify-embedded-license-index` or `cargo run --manifest-path xtask/Cargo.toml --bin generate-index-artifact -- --check` | `license-index-curation`                             |
| Supported formats drift         | `cargo run --quiet --locked --manifest-path xtask/Cargo.toml --bin generate-supported-formats -- --check`                                | `generated-docs-maintenance` or `add-parser`         |
| Serve OpenAPI drift             | `cargo run --quiet --locked --manifest-path xtask/Cargo.toml --bin generate-serve-openapi -- --check`                                    | `serve-api-verifier` or `generated-docs-maintenance` |
| Output field reference drift    | `cargo run --quiet --locked --manifest-path xtask/Cargo.toml --bin generate-output-field-reference -- --check`                           | `generated-docs-maintenance`                         |
| Packaged crate size             | `./scripts/check_crate_size.sh`                                                                                                          | `dependency-policy-maintenance`                      |
| Rust formatting                 | `cargo fmt --all -- --check`                                                                                                             | changed Rust files                                   |
| Docs/YAML/JSON formatting       | `npm run format:check` (under `Code Quality` in CI)                                                                                      | changed docs/config files                            |
| Cargo manifest sorting          | `npm run format:manifests:check`                                                                                                         | `dependency-policy-maintenance`                      |
| Clippy                          | `cargo clippy --all-targets --all-features -- -D warnings`                                                                               | changed Rust code                                    |
| Rust compilation                | `cargo check --all --verbose`                                                                                                            | changed Rust code                                    |
| Dependency policy               | `cargo deny check advisories bans licenses sources`                                                                                      | `dependency-policy-maintenance`                      |
| Unused dependencies             | `./scripts/check_unused_deps.sh`                                                                                                         | `dependency-policy-maintenance`                      |
| Library tests                   | `cargo test -p provenant-cli --lib --profile ci-release --verbose -- --skip _scan_test::`                                                | changed library code                                 |
| Doctests                        | `cargo test -p provenant-cli --doc --profile ci-release --verbose`                                                                       | public API docs/examples                             |
| No-default-features smoke       | `cargo check --lib --no-default-features` plus the exact failing no-default-features test                                                | feature-gated parser/scanner code                    |
| Golden Tests                    | `cargo test --test <suite> --features golden-tests` (add `--profile ci-release` for copyright/license/post-processing)                   | owning detector/parser/output surface                |
| Scanner/assembly contract shard | `cargo test --test <suite_or_filter>` or parser-local scan tests                                                                         | parser/assembly/scanner contract code                |
| Integration shard               | `cargo test --test <suite_name> <filter>`                                                                                                | end-to-end user-visible behavior                     |
| Docs lint/format                | `npm run check:docs` (under `Code Quality` in CI)                                                                                        | markdown/docs formatting                             |

## Escalation Rules

- If the failure is a parser implementation issue, load `add-parser` rather than duplicating parser workflow here.
- If the failure is command construction or scan flag semantics, load `provenant-cli`.
- If the failure comes from benchmark compare verification, load `verify-benchmark-target`.
- If the first local check passes but CI failed, compare OS, feature flags, branch filters, and whether CI runs generated files in `--locked` mode.
