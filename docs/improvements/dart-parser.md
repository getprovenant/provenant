# Dart Parser: Intentional Differences and Improvements

## Summary

The Dart parser in Provenant preserves pubspec and pubspec.lock metadata more accurately than the Python reference implementation in several concrete ways:

1. dependency scope is mapped explicitly (`dependencies`, `dev_dependencies`, `dependency_overrides`, `environment`)
2. YAML trailing newlines in descriptions are preserved
3. lockfile entries distinguish `direct main`, `direct dev`, and graph-derived transitive runtime vs dev-only dependencies
4. manifest and lockfile dependency source descriptors are preserved for hosted / git / path / sdk forms
5. `publish_to: none`, legacy top-level lockfile `sdk:`, and additional pubspec metadata fields are surfaced explicitly

## Improvements

### 1. Dependency scope correction

Rust preserves explicit dependency scopes from `pubspec.yaml` instead of leaving them null. Runtime dependencies stay under `dependencies`; development-only ones stay under `dev_dependencies`; environment constraints stay under `environment`.

### 2. YAML description fidelity

Rust preserves trailing newlines from folded/literal YAML description blocks, which keeps the original semantic formatting intact.

### 3. Lockfile direct/dev/transitive semantics

Rust now keeps pub lockfile intent more truthfully instead of flattening every entry into a direct runtime dependency:

- `direct main` → `is_direct=true`, `is_runtime=true`
- `direct dev` → `is_direct=true`, `is_runtime=false`, `is_optional=true`
- `transitive` packages are classified by graph reachability from direct main vs direct dev roots
- legacy top-level `sdk:` is accepted alongside modern `sdks:`

This means dev-only transitives are no longer overstated as runtime dependencies just because they appear in the lockfile.

### 4. Dependency source descriptor preservation

Rust now keeps richer dependency-source detail for pubspec manifests and lockfiles:

- manifest dependency maps preserve hosted / git / path / sdk descriptors in dependency `extra_data`
- lockfile package entries preserve `source`, `description`, and dependency-kind metadata in both dependency and resolved-package `extra_data`
- path lockfile entries such as `{ path: "../pkg", relative: true }` no longer disappear into a generic version-only view

### 5. Pub metadata parity

Rust now also surfaces several package-level pubspec metadata fields more truthfully:

- `publish_to: none` marks the package as private
- `archive_url` is honored as `download_url`
- `platforms`, `funding`, `topics`, `screenshots`, `false_secrets`, and `ignored_advisories` are preserved in `extra_data`
- `topics` also populate `keywords`

### 6. Pub workspace topology

- Root `pubspec.yaml` `workspace:` member paths are preserved in `extra_data.workspace_members`.
- Assembly claims those member directories for Dart workspace topology, keeps each member as its own package, and attributes a shared root `pubspec.lock` honestly to members that declare matching direct deps (otherwise hoisted).

## Coverage

Coverage includes:

- explicit dependency scope extraction
- YAML newline preservation
- lockfile direct/dev/transitive classification
- hosted / git / path / sdk descriptor preservation
- legacy `sdk:` lockfile compatibility
- `publish_to: none` and `archive_url` behavior
- pub workspace member extraction and topology assembly
- parser-only goldens for publish metadata, dict-style dependency descriptors, and path lockfiles
