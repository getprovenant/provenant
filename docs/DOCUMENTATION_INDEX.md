# Documentation Index

This index helps you find the right documentation for your needs.

## For Users

- **[README.md](../README.md)** - Installation, usage, and quick start
- **[LIBRARY_GUIDE.md](LIBRARY_GUIDE.md)** - Programmatic embedding guidance for using Provenant from Rust
- **[SERVE_API_GUIDE.md](SERVE_API_GUIDE.md)** - HTTP API usage guide for `provenant serve`
- **[CLI_GUIDE.md](CLI_GUIDE.md)** - Command-line workflows and important flag combinations
- **[EVALUATING_WITH_SCANCODE_WORKFLOWS.md](EVALUATING_WITH_SCANCODE_WORKFLOWS.md)** - Compatibility notes and workflow differences for ScanCode users evaluating Provenant
- **[BENCHMARKS.md](BENCHMARKS.md)** - Maintained package-detection compare-run record, timing methodology, and Provenant-vs-ScanCode outcomes
- **[OUTPUT_FIELD_REFERENCE.md](OUTPUT_FIELD_REFERENCE.md)** - Generated reference for public output records, fields, and presence rules
- **[SUPPORTED_FORMATS.md](SUPPORTED_FORMATS.md)** - Generated support matrix for package and package-adjacent detection surfaces
- **[NOTICE](../NOTICE)** - Upstream attribution and licensing details for included ScanCode-derived materials
- **[ACKNOWLEDGEMENTS.md](../ACKNOWLEDGEMENTS.md)** - Project support and acknowledgements, including employer and infrastructure support
- **[SECURITY.md](../SECURITY.md)** - Security reporting guidance

## For Contributors

### Getting Started

- **[CONTRIBUTING.md](../CONTRIBUTING.md)** - Contributor workflow, local setup, testing guidance, and pull request conventions
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - System design and components
- **[LICENSE_DETECTION_ARCHITECTURE.md](LICENSE_DETECTION_ARCHITECTURE.md)** - Detailed license-detection engine architecture and rule-loading flow
- **[HOW_TO_ADD_A_PARSER.md](HOW_TO_ADD_A_PARSER.md)** - Step-by-step parser implementation guide
- **[TESTING_STRATEGY.md](TESTING_STRATEGY.md)** - Five-layer testing approach

### Design Decisions

- **[adr/README.md](adr/README.md)** - Architectural Decision Records index and guidance

### Beyond-Parity Features

- **[improvements/README.md](improvements/README.md)** - Beyond-parity improvements index and per-area links

## For Maintainers

- **[RELEASING.md](RELEASING.md)** - Release prerequisites, workflow, and verification steps
- **[BENCHMARKS.md](BENCHMARKS.md)** - Maintained verification records and recorded package-detection compare runs
- **[Package-detection issue tracker](https://github.com/getprovenant/provenant/issues?q=is%3Aissue%20state%3Aopen%20label%3Apackage-parsing)** - Open future package-detection work
- **[../xtask/README.md](../xtask/README.md)** - Maintainer commands for compare runs, golden maintenance, and generated artifacts

### Document Organization

```text
docs/
├── BENCHMARKS.md                      # Evergreen: Benchmark methodology and recorded compare runs
├── CLI_GUIDE.md                       # Evergreen: User-facing CLI workflows
├── LIBRARY_GUIDE.md                   # Evergreen: User-facing Rust embedding guide
├── OUTPUT_FIELD_REFERENCE.md          # Generated: Public output field reference
├── SERVE_API_GUIDE.md                 # Evergreen: User-facing HTTP API guide
├── ARCHITECTURE.md                    # Evergreen: System design
├── LICENSE_DETECTION_ARCHITECTURE.md  # Evergreen: License-detection subsystem
├── RELEASING.md                       # Evergreen: Maintainer release process
├── HOW_TO_ADD_A_PARSER.md             # Evergreen: Parser guide
├── TESTING_STRATEGY.md                # Evergreen: Testing philosophy
├── SUPPORTED_FORMATS.md               # Generated: CI-checked support matrix
├── DOCUMENTATION_INDEX.md             # This file
│
├── adr/                               # Historical decision records + current-contract notes
│
└── improvements/                      # Evergreen: Beyond-parity features
```

## Quick Links by Task

### I want to

**...understand the overall architecture**
→ [ARCHITECTURE.md](ARCHITECTURE.md)

**...understand license detection internals**
→ [LICENSE_DETECTION_ARCHITECTURE.md](LICENSE_DETECTION_ARCHITECTURE.md)

**...add a new package parser**
→ [HOW_TO_ADD_A_PARSER.md](HOW_TO_ADD_A_PARSER.md)

**...see future package-detection work**
→ [Package-detection issue tracker](https://github.com/getprovenant/provenant/issues?q=is%3Aissue%20state%3Aopen%20label%3Apackage-parsing)

**...understand testing strategy**
→ [TESTING_STRATEGY.md](TESTING_STRATEGY.md)

**...see what formats are supported**
→ [SUPPORTED_FORMATS.md](SUPPORTED_FORMATS.md)

**...look up an output field**
→ [OUTPUT_FIELD_REFERENCE.md](OUTPUT_FIELD_REFERENCE.md)

**...figure out which document owns a topic**
→ [README.md](../README.md), [CLI_GUIDE.md](CLI_GUIDE.md), [LIBRARY_GUIDE.md](LIBRARY_GUIDE.md), [SERVE_API_GUIDE.md](SERVE_API_GUIDE.md), [ARCHITECTURE.md](ARCHITECTURE.md), [LICENSE_DETECTION_ARCHITECTURE.md](LICENSE_DETECTION_ARCHITECTURE.md), [HOW_TO_ADD_A_PARSER.md](HOW_TO_ADD_A_PARSER.md), and [TESTING_STRATEGY.md](TESTING_STRATEGY.md) for workflow ownership

**...learn CLI usage and flag combinations**
→ [CLI_GUIDE.md](CLI_GUIDE.md)

**...use Provenant as an HTTP API**
→ [SERVE_API_GUIDE.md](SERVE_API_GUIDE.md)

**...use Provenant as a Rust library**
→ [LIBRARY_GUIDE.md](LIBRARY_GUIDE.md)

**...understand Provenant's compatibility relationship with ScanCode Toolkit**
→ [README.md](../README.md) for high-level positioning, then [EVALUATING_WITH_SCANCODE_WORKFLOWS.md](EVALUATING_WITH_SCANCODE_WORKFLOWS.md) for practical differences

**...evaluate Provenant against an existing ScanCode workflow**
→ [EVALUATING_WITH_SCANCODE_WORKFLOWS.md](EVALUATING_WITH_SCANCODE_WORKFLOWS.md)

**...review upstream attribution or the code/data licensing split**
→ [NOTICE](../NOTICE)

**...review project support and acknowledgements**
→ [ACKNOWLEDGEMENTS.md](../ACKNOWLEDGEMENTS.md)

**...review security reporting guidance**
→ [SECURITY.md](../SECURITY.md)

**...understand a design decision**
→ [adr/README.md](adr/README.md)

**...see where Provenant intentionally differs from or improves on the Python reference**
→ [improvements/README.md](improvements/README.md)

**...track implementation quality and behavior**
→ [TESTING_STRATEGY.md](TESTING_STRATEGY.md) for testing philosophy, plus [BENCHMARKS.md](BENCHMARKS.md) for the canonical package-detection verification record, compare-run timing references, and maintained Provenant-vs-ScanCode outcomes

**...cut a release**
→ [RELEASING.md](RELEASING.md)

## Document Lifecycle

### Evergreen Documents (Permanent)

- **ARCHITECTURE.md** - Updated as architecture evolves
- **CLI_GUIDE.md** - Updated as the public CLI workflows evolve
- **LIBRARY_GUIDE.md** - Updated as the supported Rust embedding surface evolves
- **SERVE_API_GUIDE.md** - Updated as the current HTTP API surface evolves
- **EVALUATING_WITH_SCANCODE_WORKFLOWS.md** - Updated as the ScanCode workflow evaluation surface evolves
- **BENCHMARKS.md** - Updated as maintained benchmark examples and methodology evolve
- **OUTPUT_FIELD_REFERENCE.md** - Auto-generated and CI-checked for drift
- **LICENSE_DETECTION_ARCHITECTURE.md** - Updated as the license-detection subsystem evolves
- **RELEASING.md** - Updated as the release workflow changes
- **HOW_TO_ADD_A_PARSER.md** - Updated as parser patterns change
- **TESTING_STRATEGY.md** - Updated as testing approach evolves
- **SUPPORTED_FORMATS.md** - Auto-generated and CI-checked for drift
- **adr/README.md** - ADR index; accepted ADRs record design decisions and may receive limited maintenance notes to prevent broken or misleading references
- **improvements/README.md** - Landing page for intentional differences and improvements documents

### Canonical Ownership Rules

- **Current user-facing CLI behavior** lives in `README.md` and `CLI_GUIDE.md`.
- **Current user-facing Rust embedding guidance** lives in `README.md` and `LIBRARY_GUIDE.md`.
- **Current user-facing HTTP API guidance** lives in `README.md` and `SERVE_API_GUIDE.md`.
- **Current architecture and maintainer contracts** live in evergreen docs such as `ARCHITECTURE.md`, `LICENSE_DETECTION_ARCHITECTURE.md`, `HOW_TO_ADD_A_PARSER.md`, and `TESTING_STRATEGY.md`.
- **Generated support coverage** lives in `SUPPORTED_FORMATS.md`.
- **Generated output-field semantics** live in `OUTPUT_FIELD_REFERENCE.md`.
- **Design decisions and rationale** live in `adr/`.
- **Current verification records and maintainer workflows** live in evergreen docs such as `BENCHMARKS.md`, `TESTING_STRATEGY.md`, and `xtask/README.md`.
- **Future package-detection work** lives in the GitHub issue tracker under the `package-parsing` label.

## Contributing

When adding documentation:

1. **Evergreen docs** go in `docs/` root or subdirectories (`adr/`, `improvements/`)
2. **ADRs** document accepted design decisions - create new ADRs for substantive decision changes, but allow narrowly scoped maintenance notes or link fixes that prevent stale guidance
3. **Intentional differences and improvements** get documented in `improvements/` with examples
4. **Auto-generated docs** (like `SUPPORTED_FORMATS.md` and `OUTPUT_FIELD_REFERENCE.md`) should not be edited manually

## Maintenance

- **SUPPORTED_FORMATS.md**: Regenerate with `cargo run --manifest-path xtask/Cargo.toml --bin generate-supported-formats` and keep it passing `-- --check` in CI
- **OUTPUT_FIELD_REFERENCE.md**: Regenerate with `cargo run --manifest-path xtask/Cargo.toml --bin generate-output-field-reference` and keep it passing `-- --check` in CI
- **ADRs**: Add new ADRs for significant design decisions
- **Improvements**: Document intentional differences and improvements as they're implemented
