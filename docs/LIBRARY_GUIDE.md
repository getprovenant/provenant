# Library Guide

This guide is for people who want to use Provenant from Rust instead of invoking the `provenant` CLI.

Use it when you want to:

- scan a path in-process from Rust
- inspect the resulting Provenant data structures directly
- reuse Provenant as part of another Rust tool or service

If you mainly want shell workflows and output files, start with the [CLI Guide](CLI_GUIDE.md).

This guide covers the Rust embedding surface only. If you want long-lived HTTP access instead of in-process Rust embedding, see the [Serve API Guide](SERVE_API_GUIDE.md).

## Dependency setup

The published package is `provenant-cli`, but the library target is imported as `provenant`.

This guide owns the Rust embedding dependency and feature guidance. The root [README](../README.md) stays at the quick-start level and links here for embedding-specific setup.

If you want the smallest default dependency surface, start with:

```toml
[dependencies]
provenant = { package = "provenant-cli", version = "<VERSION>", default-features = false }
```

Replace `<VERSION>` with the current crates.io release.

If you need RPM SQLite parsing, opt back into the `rpm-sqlite` feature explicitly:

```toml
[dependencies]
provenant = { package = "provenant-cli", version = "<VERSION>", default-features = false, features = ["rpm-sqlite"] }
```

For most embedders, the relevant Cargo features are:

- `rpm-sqlite` - enables RPM SQLite database parsing and pulls in `rusqlite`
- `golden-tests` - repository and CI-oriented test coverage, not usually needed for downstream embedding

## Start here: scan a path from Rust

The easiest supported entry point today is the high-level workflow API.

```rust
use provenant::workflow::{LicenseSource, ScanOptions, scan_path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = ScanOptions {
        detect_license: LicenseSource::Embedded,
        detect_packages: true,
        collect_info: true,
        ..ScanOptions::default()
    };

    let output = scan_path("/path/to/project", &options)?;

    println!("scanned {} resources", output.files.len());
    println!("assembled {} packages", output.packages.len());
    Ok(())
}
```

This gives you the same internal `provenant::Output` model that the CLI eventually turns into ScanCode-compatible output.

## Common options

`ScanOptions` is the main configuration surface for library use.

Useful fields include:

- `detect_license` - `Disabled`, `Embedded`, or `Directory(...)`
- `detect_packages` - enable package and dependency detection
- `detect_system_packages` - enable installed package database detection
- `detect_copyrights`, `detect_emails`, `detect_urls`
- `collect_info` - include extra file metadata
- `include` / `exclude` - path filtering
- `incremental` and cache-related fields

The default is intentionally conservative. Turn on the scan dimensions you actually want.

Two defaults are especially worth knowing:

- `include_input_header` is `false`, so library-driven outputs do not include raw input paths unless you opt in.
- the workflow facade does not honor `PROVENANT_CACHE` unless you explicitly set `cache_dir`, which keeps library behavior less dependent on ambient process environment.

## Writing standard output formats

If you want to serialize the workflow result using Provenant's output writers, convert the internal `Output` model to the public output schema first:

For explanations of public output fields and presence rules on that schema, see the [Output Field Reference](OUTPUT_FIELD_REFERENCE.md).

```rust
use provenant::output_schema;
use provenant::{OutputFormat, OutputWriteConfig, write_output_file};
use provenant::workflow::{ScanOptions, scan_path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = scan_path("/path/to/project", &ScanOptions::default())?;
    let schema_output = output_schema::Output::from(&output);

    write_output_file(
        "scan.json",
        &schema_output,
        &OutputWriteConfig {
            format: OutputFormat::JsonPretty,
            custom_template: None,
            scanned_path: Some("/path/to/project".to_string()),
        },
    )?;

    Ok(())
}
```

## When to use the lower-level APIs

The workflow module is the best default for embedding.

Drop down to lower-level APIs such as `collect_paths`, `process_collected`, `assembly::assemble`, or `LicenseDetectionEngine` when you need tighter control over one specific phase of the scan pipeline.

## Current boundary

The library is now a real first-class embedding surface, but the most stable path is still:

1. use `workflow::scan_path(...)` or `workflow::scan_paths(...)`
2. inspect the returned `provenant::Output`
3. optionally convert to `output_schema::Output` for serialization

That is the path this guide recommends unless you have a specific reason to assemble the pipeline manually.

## Related docs

- [README](../README.md) for installation and top-level usage
- [CLI Guide](CLI_GUIDE.md) for command-line workflows
- [Architecture](ARCHITECTURE.md) for the broader system design
- [License Detection Architecture](LICENSE_DETECTION_ARCHITECTURE.md) for the license engine and dataset behavior
