# Gradle Parser Improvements

## Summary

Rust now intentionally differs from or improves on current Python ScanCode Gradle handling in several concrete ways:

1. classifies `compileOnly`-style Gradle scopes as non-runtime instead of treating everything except `test*` as runtime
2. extracts Gradle POM license metadata into package license fields so CycloneDX output can carry component license expressions
3. resolves TOML-backed `libs.versions.toml` version-catalog aliases such as `libs.androidx.appcompat` to real Maven package identifiers
4. preserves parent path segments for local project dependencies like `project(":libs:download")`
5. merges all discovered `dependencies {}` blocks in a build file instead of only parsing the first one
6. parses Kotlin DSL quoted configuration names such as `"implementation"("io.ktor:ktor-client-core:3.2.3")` instead of dropping those central dependency declarations

## Python Status

- Current Python Gradle handling is token-based and safe, but still shallow in the areas tracked by upstream issues.
- Upstream explicitly tracks:
  - incorrect runtime classification for `compileOnly`
  - missing Gradle SBOM component licenses
  - incorrect package identifiers for Android/version-catalog dependency aliases
- Repeated `dependencies {}` blocks are semantically additive in Gradle, so stopping after the first block loses declared dependencies from real-world Groovy and Kotlin builds.
- The misbucketed template-POM issue grouped with this batch is upstream-confirmed, but it is actually a Maven placeholder-detection problem rather than a Gradle parser problem.

## Rust Improvements

### Runtime scope classification

- `compileOnly`, `compileOnlyApi`, `annotationProcessor`, `kapt`, and `ksp` are now treated as non-runtime dependencies.
- `test*` scopes remain non-runtime and optional.
- This fixes the upstream `compileOnly` misclassification bug instead of reproducing it.

### License propagation for SBOM output

- Rust now extracts Gradle `pom { licenses { ... } }` metadata from both Groovy and Kotlin DSL fixtures.
- Recognizable SPDX-like Gradle license declarations are promoted into:
  - `declared_license_expression`
  - `declared_license_expression_spdx`
  - `extracted_license_statement`
- CycloneDX output already consumes `declared_license_expression_spdx`, so this closes the local package-to-SBOM license gap for Gradle metadata that is present in the build file.

### Version catalog alias resolution

- Rust now resolves TOML-backed `libs.versions.toml` aliases from nearby version catalogs.
- Example: `implementation libs.androidx.appcompat` now resolves to `pkg:maven/androidx.appcompat/appcompat@1.7.0` instead of a truncated identifier.
- This is intentionally limited to static TOML-backed catalogs and does not attempt full semantic evaluation of arbitrary Gradle settings/build logic.

### Local project identifiers

- Local project references such as `project(":libs:download")` now preserve their parent path segments as namespace data (`pkg:maven/libs/download`) instead of collapsing to only the last segment.

### All dependencies blocks in one build file

- Rust now parses every discovered `dependencies {}` block in a single `build.gradle` / `build.gradle.kts` file instead of stopping after the first block.
- This includes repeated top-level blocks and later nested blocks that the current token parser already recognizes lexically.

### Kotlin DSL quoted configuration names

- Kotlin DSL also allows configuration names to be called as quoted strings, for example `"implementation"("group:artifact:version")` and `"implementation"(project(":core"))`.
- Rust now treats these quoted configuration names the same way it treats unquoted identifiers, including ordinary Maven coordinates, nested `project(":...")` references, and trailing exclusion closures.
- This keeps centralized multi-project Gradle builds scannable when they route module dependencies through root-level `project(":module") { dependencies { ... } }` blocks using quoted Kotlin DSL configuration calls.

### Template POM guardrail

- Rust also skips placeholder-only Maven coordinates like `${groupId}` / `${artifactId}` / `${version}` instead of emitting junk package identifiers.

### Multiproject topology from `settings.gradle(.kts)`

- Rust now parses literal `include` / `includeFlat` project paths from `settings.gradle` / `settings.gradle.kts` into `extra_data.projects` (datasource `gradle_settings`).
- Assembly builds a Gradle multiproject topology domain from those declarations, synthesizes package identity for each resolved `build.gradle(.kts)` using settings root name / project path plus build `group`/`version`, and attributes nested sources to the deepest enclosing project while skipping each module's immediate `build/` output directory.
- Non-literal / dynamic includes and custom `project(...).projectDir` remaps are intentionally skipped rather than guessed.

## Coverage

Coverage spans scope classification, all discovered dependency-block parsing, Kotlin DSL quoted configuration names, version-catalog alias resolution, Gradle POM license extraction, placeholder-coordinate guardrails, and settings-driven multiproject topology.
