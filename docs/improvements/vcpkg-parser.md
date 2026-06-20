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

In addition, Rust links the manifest `overrides` array back to the dependencies it pins. A vcpkg `override` is an author-declared hard version pin, so when an override matches a declared dependency the dependency is marked `is_pinned: true` and the exact override version is recorded in dependency `extra_data` as `override_version`. The dependency's own declared `version>=` floor is preserved unchanged, and override entries that do not match a declared direct dependency remain available in the raw manifest-level `overrides` metadata rather than being synthesized into phantom dependencies.

### 5. Embedded and sibling configuration awareness

When configuration is embedded in `vcpkg.json`, Rust preserves it directly.

When embedded configuration is absent, Rust also opportunistically reads a sibling `vcpkg-configuration.json` and stores it under manifest `extra_data` as configuration metadata.

This preserves useful real-world repository metadata adjacent to a manifest, and is complemented by the standalone configuration parser below for configuration files that have no sibling manifest.

### 6. Standalone configuration provenance preservation

Rust now parses standalone `vcpkg-configuration.json` files as a first-class vcpkg package-data surface, so registry and overlay provenance is captured even when no sibling `vcpkg.json` is present (previously such a configuration file was invisible to the scan).

The parser preserves the registry/overlay fields that define where dependencies resolve from — `default-registry`, `registries`, `overlay-ports`, and `overlay-triplets` — under `extra_data`. The emitted package data is marked private because a configuration file has no package identity of its own. It is a file-local extractor: it always emits for a matching file and does not attempt to deduplicate against a sibling manifest's embedded configuration copy, leaving any cross-file merge to assembly.

### 7. Registry lock provenance preservation

Rust now parses standalone `vcpkg-lock.json` files as a first-class vcpkg package-data surface.

The parser preserves registry resolution metadata in `extra_data.registry_locks`, recording each registry `location` together with the locked reference-to-revision mapping. The fixture covers one Git URL location and one filesystem path location; it does not claim a builtin-registry-specific representation without a tool-generated sample.

It intentionally does not derive resolved packages or alter dependency output from the lockfile alone because the lockfile records registry fetch state rather than dependency intent. The emitted package data is marked private because a standalone lockfile has no package identity.

## Scope Boundary

This improvement intentionally covers:

- `vcpkg.json`
- embedded `configuration` / `vcpkg-configuration`
- sibling `vcpkg-configuration.json` ingestion into manifest metadata
- `overrides`-driven dependency pinning (`is_pinned` + `override_version`)
- standalone `vcpkg-configuration.json` registry/overlay provenance
- `vcpkg-lock.json` registry lock metadata

This improvement intentionally does **not** yet claim first-class support for:

- dependency or resolved-package inference from `vcpkg-lock.json`
- resolving `builtin-baseline` or registry baselines to concrete versions, which would require fetching the registry version database outside the scanned tree

Those supporting layers remain out of scope for this document.

## Primary Areas Affected

- vcpkg project manifest parsing
- vcpkg port/library manifest parsing
- direct dependency extraction for modern vcpkg manifests
- override-driven dependency pinning for modern vcpkg manifests
- manifest metadata preservation for baselines, overrides, and configuration
- standalone registry/overlay provenance preservation for `vcpkg-configuration.json`
- registry lock metadata preservation for standalone `vcpkg-lock.json`

## Coverage

Coverage includes:

- unit tests for project manifests
- unit tests for port/library manifests
- unit tests for project manifests without package identity
- unit tests for override-driven dependency pinning
- unit tests for sibling configuration ingestion
- unit tests for standalone configuration registry/overlay provenance
- unit tests for standalone registry lock metadata
- scanner contract tests for configuration and lockfile dispatch
- parser goldens for project manifests
- parser goldens for port/library manifests
- parser goldens for standalone configuration files
- parser goldens for registry lockfiles
