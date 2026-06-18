# vcpkg Parser: Modern Manifest Support

## Summary

Rust now parses the primary modern vcpkg manifest surface, `vcpkg.json`, for both project manifests and port/library manifests.

This delivers the core vcpkg manifest-mode behavior that matters most to scans: direct dependency extraction, package identity for named manifests, and preservation of versioning/configuration metadata that affects dependency resolution.

## Upstream / Reference Context

The Python reference has no modern `vcpkg.json` manifest parser and does not preserve manifest-mode dependency/configuration metadata.

`vcpkg.json` is the required manifest and direct dependency surface, while configuration and registry lock metadata are supporting layers.

## Rust Improvements

### 1. Strict-JSON `vcpkg.json` parsing

Rust now parses `vcpkg.json` as strict JSON, matching Microsoft’s documented format rules.

This covers both important manifest roles:

- top-level project manifests, where `name` and version can be omitted
- port/library manifests, where `name` and a version field are present

### 2. Version-field normalization for port manifests

Rust supports the documented manifest version fields and folds `port-version` into the final version string when present.

This means a vcpkg port manifest can produce a stable package version even when the packaging revision is tracked separately from the upstream version.

### 3. Direct dependency extraction from string and object forms

Rust now extracts dependencies from both supported dependency syntaxes:

- simple string entries such as `"fmt"`
- object entries with additional manifest metadata

For object dependencies, Rust preserves the most important vcpkg dependency metadata in dependency `extra_data`, including:

- `version>=`
- `features`
- `default-features`
- `host`
- `platform`

This gives the scan result the core modern vcpkg dependency graph while keeping lock-state support as a separate provenance surface.

### 4. Preserve manifest-level resolution metadata

Rust now keeps top-level vcpkg manifest metadata that meaningfully affects dependency resolution or policy, including:

- `builtin-baseline`
- `overrides`
- `supports`
- `default-features`
- `features`

These are stored in `extra_data` so the scan preserves the context needed to understand how the manifest constrains dependency selection.

### 5. Embedded and sibling configuration awareness

When configuration is embedded in `vcpkg.json`, Rust preserves it directly.

When embedded configuration is absent, Rust also opportunistically reads a sibling `vcpkg-configuration.json` and stores it under manifest `extra_data` as configuration metadata.

This preserves useful real-world repository metadata without claiming standalone `vcpkg-configuration.json` provenance as its own parser surface.

### 6. Registry lock provenance preservation

Rust now parses standalone `vcpkg-lock.json` files as a first-class vcpkg package-data surface.

The parser preserves registry resolution metadata in `extra_data.registry_locks`, recording each registry repository/path together with the locked reference-to-revision mapping.

It intentionally does not derive resolved packages or alter dependency output from the lockfile alone because the lockfile records registry fetch state rather than dependency intent.

## Scope Boundary

This improvement intentionally covers:

- `vcpkg.json`
- embedded `configuration` / `vcpkg-configuration`
- sibling `vcpkg-configuration.json` ingestion into manifest metadata
- `vcpkg-lock.json` registry lock metadata

This improvement intentionally does **not** yet claim first-class support for:

- standalone `vcpkg-configuration.json` provenance as its own parser/datasource
- dependency or resolved-package inference from `vcpkg-lock.json`

Those supporting layers remain out of scope for this document.

## Primary Areas Affected

- vcpkg project manifest parsing
- vcpkg port/library manifest parsing
- direct dependency extraction for modern vcpkg manifests
- manifest metadata preservation for baselines, overrides, and configuration
- registry lock metadata preservation for standalone `vcpkg-lock.json`

## Coverage

Coverage includes:

- unit tests for project manifests
- unit tests for port/library manifests
- unit tests for project manifests without package identity
- unit tests for sibling configuration ingestion
- unit tests for standalone registry lock metadata
- scanner contract tests for lockfile dispatch
- parser goldens for project manifests
- parser goldens for port/library manifests
- parser goldens for registry lockfiles
