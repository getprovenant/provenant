# Maven Parser: Intentional Differences and Improvements

## Summary

The Maven POM parser in Provenant now improves on the Python reference in ten important areas:

1. **🔍 Enhanced Extraction**: description handling now matches Maven `name` + `description` semantics without duplicating identical values
2. **✨ New Feature**: `dependencyManagement` entries are surfaced as dependency records instead of being preserved only as opaque metadata
3. **✨ New Feature**: package qualifiers, source package PURLs, and packaging-aware download URLs are emitted for Maven packages
4. **✨ New Feature**: relocation metadata from `distributionManagement.relocation` is extracted and preserved
5. **🐛 Bug Fix**: extracted license statements are rendered as structured Maven license records and can include top-level XML license comments
6. **🐛 Bug Fix**: organization parties use the correct `owner` role and developer parties keep the expected `developer` spelling
7. **✨ New Feature**: nested `META-INF/maven/**` extracted-JAR cases now follow the intended single-POM versus multi-POM safety behavior
8. **✨ New Feature**: Maven 4.1.0 POMs are accepted
9. **🔍 Enhanced Extraction**: packaging aliases and property-resolved dependency scope/optional values are normalized after property resolution
10. **✨ New Feature**: declared `<modules>` reactor lists now drive reactor-aware assembly, attributing source files nested under each module to the correct module package instead of leaving multi-module source trees as per-directory packages only

## Improvement 1: Description De-duplication

### Python Behavior

Python combines `<name>` and `<description>` when both exist, but the existing Rust implementation previously dropped top-level POM descriptions entirely.

### Rust Behavior

Rust now emits `description` using Maven-aware rules:

- if only `<name>` exists, use it
- if only `<description>` exists, use it
- if both exist and are identical, keep one value
- if both exist and differ, join them with a newline

This matches the useful ScanCode behavior while fixing the missing-description gap and the duplicated-name issue.

## Improvement 2: dependencyManagement Dependencies

### Python Behavior

Python surfaces `dependencyManagement` entries as dependency records with synthetic scopes such as `dependencymanagement` and `import`.

### Rust Behavior

Rust now does the same. Managed entries are no longer hidden only in `extra_data.dependency_management`; they are also emitted as first-class dependencies, while the raw management metadata is still preserved in `extra_data`.

For import-scoped BOMs, Rust keeps both normalized views in the dependency list: an `import` entry that records BOM provenance and a `dependencymanagement` entry that keeps the same coordinate visible as a managed-constraint source. This lets downstream JSON/template/SBOM-adjacent consumers preserve both facts even when they do not inspect Maven-specific `extra_data`.

## Improvement 3: Maven Qualifiers, Source Packages, and Packaging-aware Downloads

### Python Behavior

Python emits package qualifiers for Maven `classifier` and non-default package `type`, and it also generates a `?classifier=sources` source package PURL.

### Rust Behavior

Rust now emits:

- package qualifiers for `classifier` and normalized `type`
- source package PURLs such as `pkg:maven/foo/bar@1.2.3?classifier=sources`
- packaging-aware repository download URLs

Rust also normalizes Maven packaging aliases like `maven-plugin` to the correct jar-style artifact extension instead of using raw packaging values verbatim.

## Improvement 4: Relocation Metadata Support

### Python Behavior

The current Python reference does not preserve Maven `distributionManagement.relocation` as structured output.

### Rust Behavior

Rust extracts relocation coordinates and messages, preserves them in `extra_data.relocation`, and emits a relocation dependency when coordinates are present. Message-only relocation notices are also retained so no relocation warning text is silently lost.

## Improvement 5: Structured License Statement Rendering

### Python Behavior

Python keeps Maven license records as structured normalized data before license detection, including name and URL, instead of reducing them to plain display text.

### Rust Behavior

Rust now renders `extracted_license_statement` as structured Maven license records, preserving `name`, `url`, and `comments` when present. It also promotes top-level licenselike XML comments into the extracted license statement so package-level Maven license output can preserve comment-only declarations that Python still misses.

## Improvement 6: Correct Owner Party Role

### Python Behavior

The issue backlog documents a typo/problem around party role handling for Maven organization ownership metadata.

### Rust Behavior

Rust now emits organization ownership parties with the correct `owner` role, preserving organization name and URL as structured party data. Developer parties also retain the expected `developer` spelling.

## Improvement 7: Nested META-INF Maven Validation

### Python Behavior

Python treats nested `META-INF/maven/<group>/<artifact>/pom.xml` resources as valid Maven origins for extracted JARs, but avoids assigning the whole archive to one package when multiple nested POMs are present.

### Rust Behavior

Rust now validates both sides of that contract:

- a single nested `META-INF/maven/**/pom.xml` can assemble with sibling `pom.properties` and `META-INF/MANIFEST.MF`
- multiple nested Maven POMs under the same extracted archive root intentionally skip the nested whole-archive merge, preventing one package from claiming the entire JAR

This makes the extracted-JAR Maven behavior explicit instead of relying on incidental nested-merge behavior.

## Improvement 8: Maven 4.1.0 Support

### Python Behavior

Python-era assumptions were centered on 4.0.0 POMs.

### Rust Behavior

Rust now explicitly tests and accepts `modelVersion` 4.1.0 POMs, including qualifier-bearing packages, managed imports, and relocation metadata. This keeps the parser compatible with modern Maven metadata.

## Improvement 9: Post-resolution Dependency Normalization

### Python Behavior

The Python reference still carries TODOs around some dependency qualifier/type handling.

### Rust Behavior

Rust now resolves dependency scope, optional flags, classifier, and type after property substitution and then recomputes `scope`, `is_runtime`, `is_optional`, dependency PURLs, and pinning from the resolved values. This avoids incorrect dependency flags when Maven properties drive dependency metadata.

## Improvement 10: Reactor / Multi-module Topology

### Python Behavior

The Python reference stores a POM's `<modules>` list on the package (`self.modules`) but never uses it to drive assembly. A Maven source reactor (a parent `pom.xml` plus its module `pom.xml` files) is left as a flat set of per-directory packages, with no explicit ownership link between a module's package and the source files nested under it.

### Rust Behavior

Rust's POM parser already stashes the declared `<modules>` list under `extra_data.modules` (unchanged from prior behavior). What is new is that assembly's `TopologyPlan` now reads that list to build an explicit **Maven reactor domain**: the root POM plus every `<module>` string that resolves to a scanned, purl-bearing `pom.xml` on disk. Module strings that do not resolve (a declared-but-missing or unscanned module) are dropped rather than guessed at, matching the project-wide preference for honest unknowns over speculative scopes.

Each POM in a reactor, root or nested module, still assembles into its own top-level package exactly as before, via the normal per-directory merge — the reactor domain does not change package identity, dependency extraction, or merge which POMs become packages. What it adds is a post-assembly pass that fills in **file ownership**: every currently-unowned file nested under a declared reactor root (source files, resources, a root-level `README`, …) is attributed to the _deepest_ (most specific) enclosing module's package. A module that itself declares further `<modules>` (a nested reactor) contributes its own, more specific anchor, so files under a nested module resolve to that module rather than to any outer root, without explicit recursion. Maven's own build output directory (`target/`) is excluded from this attribution so compiled artifacts are never attached to a source package.

This is deliberately narrower than a full reactor contract: Rust does not synthesize new inter-module dependency edges beyond what the POM parser already extracts (e.g. an explicit `<dependency>` from one module on another), and a file that already belongs to a package (for example, one claimed by another assembler) is left untouched rather than reassigned.

## Why This Matters

- **Parity with Maven semantics**: dependency management, relocations, classifiers, and modern POM versions are now represented explicitly
- **Better SBOM fidelity**: richer descriptions, source packages, structured licenses, and normalized dependency flags improve downstream package analysis
- **Less lossy output**: key Maven metadata is preserved instead of being flattened or discarded

## References

- Maven POM reference: <https://maven.apache.org/pom.html>
- Maven dependency mechanism: <https://maven.apache.org/guides/introduction/introduction-to-dependency-mechanism.html>
