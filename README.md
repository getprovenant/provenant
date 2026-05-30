# Provenant

[Why Provenant?](#why-provenant) · [Choose a Workflow](#choose-a-workflow) · [Install](#install) · [CLI Guide](docs/CLI_GUIDE.md) · [Benchmarks](docs/BENCHMARKS.md) · [Supported Formats](docs/SUPPORTED_FORMATS.md) · [Architecture](docs/ARCHITECTURE.md)

[![Latest Release](https://img.shields.io/github/v/release/mstykow/provenant?display_name=tag)](https://github.com/mstykow/provenant/releases/latest)
[![Crates.io](https://img.shields.io/crates/v/provenant-cli.svg)](https://crates.io/crates/provenant-cli)
[![CI](https://github.com/mstykow/provenant/actions/workflows/check.yml/badge.svg?branch=main)](https://github.com/mstykow/provenant/actions/workflows/check.yml)
[![License](https://img.shields.io/crates/l/provenant-cli.svg)](LICENSE)

Provenant is a Rust-based code scanner for licenses, copyrights, package metadata, file metadata, and related provenance data. It is a Rust implementation for ScanCode-compatible workflows, focused on correctness, safe static parsing, and native execution.

Provenant is not affiliated with, endorsed by, or sponsored by ScanCode Toolkit, AboutCode, or nexB Inc.

Across documented benchmark targets, Provenant is frequently about an order of magnitude faster than ScanCode while also offering broader package and dependency metadata extraction, documented parser and detection improvements that reduce noisy results, and practical workflows such as incremental rescans, selected-file scans, and long-lived HTTP service use.

Provenant reimplements the scanning engine in Rust and builds on the upstream [ScanCode Toolkit](https://github.com/aboutcode-org/scancode-toolkit) license and rule data.

> [!IMPORTANT]
> **Project status:** production-usable, compatibility-focused, and steadily improving.
> Provenant targets parity for documented ScanCode-compatible workflows and output formats, aside from intentional non-goals such as ScanCode's plugin system. Ongoing work focuses on improving Provenant's performance, quality, and coverage across supported workflows.

## Quick Start

```sh
cargo install provenant-cli
provenant scan --json-pp - --license --package /path/to/repo
```

Prefer release binaries? Download precompiled archives from [GitHub Releases](https://github.com/mstykow/provenant/releases).

## Why Provenant?

- [Benchmark-backed](docs/BENCHMARKS.md) scan speedups that are frequently about an order of magnitude faster than ScanCode on recorded same-host runs
- Broader package and dependency extraction across [many ecosystems](docs/SUPPORTED_FORMATS.md), including parsers with intentional improvements on some surfaces and improvements in overlapping parser families
- [Documented parser and detection fixes](docs/improvements/README.md) that reduce noisy results and false-positive classes, including better bare-word GPL/LGPL clue handling
- Package assembly for sibling, nested, and workspace-style inputs
- Native workflows such as `--incremental` cache reuse, `--paths-file` rooted file lists for CI or changed-file scans, and long-lived HTTP service mode via [`provenant serve`](docs/SERVE_API_GUIDE.md)
- Single self-contained binary with parallel native execution for simpler installation and CI use
- ScanCode-compatible workflows and output formats, including ScanCode-style JSON, SPDX, CycloneDX, YAML, JSON Lines, HTML, and custom templates
- [Security-first](docs/adr/0004-security-first-parsing.md) static parsing with explicit safeguards and compatibility-focused tradeoffs where needed
- Built on upstream ScanCode license and rule data maintained by experts

## Choose a Workflow

| If you need to...                                     | Start here                                                         | Next doc                                                                                   |
| ----------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| Run a one-off CLI scan                                | `provenant scan --json-pp - --license --package /path/to/repo`     | [CLI Guide](docs/CLI_GUIDE.md)                                                             |
| Scan explicit changed files in CI or automation       | Use `--paths-file` with one native scan root                       | [CLI Guide](docs/CLI_GUIDE.md)                                                             |
| Reuse a warm process through HTTP                     | `provenant serve --help`                                           | [Serve API Guide](docs/SERVE_API_GUIDE.md)                                                 |
| Embed Provenant in a Rust application                 | Use the `provenant` library target from `provenant-cli`            | [Library Guide](docs/LIBRARY_GUIDE.md)                                                     |
| Evaluate Provenant with an existing ScanCode workflow | Start from Provenant's compatibility and workflow-difference notes | [Evaluating Provenant with ScanCode workflows](docs/EVALUATING_WITH_SCANCODE_WORKFLOWS.md) |

## Relationship to ScanCode

| Topic              | Provenant                                                                                                                                                                                                         |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Implementation     | Rust implementation for ScanCode-compatible workflows                                                                                                                                                             |
| Compatibility goal | Strong compatibility with ScanCode workflows and output semantics where practical                                                                                                                                 |
| Upstream data      | Uses the upstream ScanCode license and rule data as a foundational dataset                                                                                                                                        |
| Evaluation path    | For teams evaluating Provenant alongside existing ScanCode-compatible workflows, see the [ScanCode workflow guide](docs/EVALUATING_WITH_SCANCODE_WORKFLOWS.md) for compatibility notes and documented differences |

## Install

### From Crates.io

Install the crates.io package `provenant-cli`:

```sh
cargo install provenant-cli
```

This installs the `provenant` command-line binary.

### Download Precompiled Binary

Download the release archive for your platform from the [GitHub Releases](https://github.com/mstykow/provenant/releases) page.

Extract the archive and place the binary somewhere on your `PATH`.

On Linux and macOS:

```sh
tar xzf provenant-*.tar.gz
sudo mv provenant /usr/local/bin/
```

On Windows, extract the `.zip` release and add `provenant.exe` to your `PATH`.

### Build from Source

For a normal source build, you only need the Rust toolchain:

```sh
git clone https://github.com/mstykow/provenant.git
cd provenant
cargo build --release
```

Cargo places the compiled binary under `target/release/`.

> **Note**: The binary includes a built-in compact license index. The `reference/scancode-toolkit/` submodule is only needed for developers updating the embedded license data, using maintainer commands that depend on it, or maintaining Provenant's built-in license dataset.

## Usage

### CLI Scanning

```sh
provenant scan --json-pp <FILE> [OPTIONS] <INPUT>...
```

> [!NOTE]
> Provenant requires at least one explicit output flag, such as `--json-pp -` or `--json scan-results.json`.

For the command tree, run:

```sh
provenant --help
```

For the complete scan-flag surface, run:

```sh
provenant scan --help
```

### Example

```sh
provenant scan --json-pp scan-results.json --license --package ~/projects/my-codebase --ignore "*.git*" --ignore "target/*" --ignore "node_modules/*"
```

Use `-` as `FILE` to write an output stream to stdout, for example `--json-pp -`.
Multiple output flags can be used in a single run, matching ScanCode CLI behavior.
When using `--from-json`, you can pass multiple JSON inputs. Native directory scans also support multiple input paths, matching ScanCode's common-prefix behavior.
For guided workflows, flag combinations, cache controls, and stdin-driven file lists, see the [CLI Guide](docs/CLI_GUIDE.md).

### HTTP Service

For the current service shell surface, run:

```sh
provenant serve --help
```

`provenant serve` runs Provenant as a long-lived HTTP service with warm process reuse, synchronous and asynchronous scan endpoints, and job polling for automation-friendly integrations.

For the HTTP request/response contract and examples, see the [Serve API Guide](docs/SERVE_API_GUIDE.md).

### Rust Library

If you want to embed Provenant in a Rust application instead of invoking the CLI, use the crates.io package `provenant-cli` and import the library target as `provenant`.

For the supported high-level Rust embedding path and dependency setup, see the [Library Guide](docs/LIBRARY_GUIDE.md).

## Output Formats

Implemented output formats include:

- JSON, including ScanCode-compatible output
- YAML
- JSON Lines
- Debian copyright
- SPDX, Tag-Value and RDF/XML
- CycloneDX, JSON and XML
- HTML report
- Custom template rendering

## Documentation

- **[Library Guide](docs/LIBRARY_GUIDE.md)** - Programmatic embedding guidance for using Provenant from Rust
- **[Serve API Guide](docs/SERVE_API_GUIDE.md)** - HTTP API usage, examples, and current service contract for `provenant serve`
- **[Documentation Index](docs/DOCUMENTATION_INDEX.md)** - Best starting point for navigating the docs set
- **[CLI Guide](docs/CLI_GUIDE.md)** - Common workflows and important flag combinations
- **[Evaluating Provenant with ScanCode workflows](docs/EVALUATING_WITH_SCANCODE_WORKFLOWS.md)** - Compatibility notes and workflow differences for ScanCode users evaluating Provenant
- **[Architecture](docs/ARCHITECTURE.md)** - System design, processing pipeline, and design decisions
- **[Output Field Reference](docs/OUTPUT_FIELD_REFERENCE.md)** - Generated reference for public output records, fields, and presence rules
- **[Supported Formats](docs/SUPPORTED_FORMATS.md)** - Generated support matrix for package ecosystems and package-adjacent detection surfaces
- **[How to Add a Parser](docs/HOW_TO_ADD_A_PARSER.md)** - Step-by-step guide for adding new parsers
- **[Testing Strategy](docs/TESTING_STRATEGY.md)** - Testing approach and guidelines
- **[ADRs](docs/adr/)** - Architectural decision records
- **[Intentional Differences and Improvements](docs/improvements/)** - Features where Provenant intentionally differs from or improves on the Python reference

## Contributing

Contributions are welcome. Please feel free to submit a pull request.

For contributor workflow and contribution policy, start with [CONTRIBUTING.md](CONTRIBUTING.md). Inbound contributions use the Developer Certificate of Origin (DCO) 1.1, so commits should be signed off with `git commit -s`; see [`DCO`](DCO) and [`CONTRIBUTING.md`](CONTRIBUTING.md) for the policy details.

For deeper contributor documentation, see the [Documentation Index](docs/DOCUMENTATION_INDEX.md), [How to Add a Parser](docs/HOW_TO_ADD_A_PARSER.md), and [Testing Strategy](docs/TESTING_STRATEGY.md).

## Support and Acknowledgements

Provenant is an independent open source project developed by its contributors. Its development has been made possible in substantial part by support from [TNG Technology Consulting GmbH](https://www.tngtech.com/), including paid contributor time on internal non-client work, compute and inference resources provided by TNG's internal GPU cluster, Skainet, and company-funded usage of third-party AI models. Without that support, Provenant would not have been possible in its current scope and form.

A substantial portion of Provenant's development has been contributed by people working on the project as TNG employees, and work on the project has been done both during TNG-supported work time and during personal unpaid time. For a fuller acknowledgement of project support, see [ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md).

## Upstream Data and Attribution

Provenant is a Rust implementation for ScanCode-compatible workflows. Provenant relies on the upstream ScanCode Toolkit project by nexB Inc. and the AboutCode community for reference behavior, compatibility validation, and the license and rule data maintained by that ecosystem. Provenant code is licensed under Apache-2.0; included ScanCode-derived rule and license data remains subject to upstream attribution and CC-BY-4.0 terms where applicable. We are grateful to nexB Inc. and the AboutCode community for the reference implementation and the extensive license and copyright research behind it. See [`NOTICE`](NOTICE) for preserved upstream attribution notices applicable to materials included in this repository and to distributions that include ScanCode-derived data.

## License

Copyright (c) 2026 Provenant contributors.

The Provenant project code is licensed under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0). See [`NOTICE`](NOTICE) for preserved upstream attribution notices for included ScanCode Toolkit materials.
