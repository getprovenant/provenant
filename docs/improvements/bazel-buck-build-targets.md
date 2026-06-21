# Bazel/Buck Build Targets — One Component Per Build Directory

**Parsers**: `BazelBuildParser` (`BUILD`/`BUILD.bazel`), `BuckBuildParser` (`BUCK`)
**Assembly**: `DatasourceId::BazelBuild`, `DatasourceId::BuckFile` use `AssemblyMode::SiblingMerge`

## Why This Exists

A single Bazel `BUILD` or Buck `BUCK` file routinely declares many independent
build targets (`*_library`, `*_binary`, …). The Python ScanCode reference
(`packagedcode/build.py`) emits **one package per target**, so a build file with
N targets produces N packages.

Provenant intentionally **does not** surface internal build targets as individual
packages. The parser still reads every target at the file level, but assembly
**collapses the targets of a build directory into one component** (sibling-merge),
which carries the file-level detected license.

## Why collapsing is the better scan result

Internal build targets are build-graph nodes, not distributable packages. They
carry no package-level provenance:

- In `denoland`/Meta `buck2` (754 `BUCK` files), **zero** targets declare a
  `licenses=` attribute.
- ScanCode's 823 `pkg:buck/*` packages for that tree have **0** with a declared
  license, **0** with dependencies, and **0** with a version — they are name-only
  shells (`{purl: pkg:buck/input, name: input, version: null, license: null}`).

Emitting one empty package per target therefore floods the package inventory with
hundreds of zero-information entries, burying the real packages and the genuine
third-party dependency graph (which Provenant already extracts from
`Cargo.toml`/`yarn.lock`/Conan/etc.). That violates the goal of a bounded,
explainable result. Collapsing to one license-bearing component per build
directory keeps the meaningful signal without the noise.

## Reference limitation

The Python reference treats each build target as a package, producing many
name-only package shells with no license, dependency, or version data.

## Rust behavior

`BazelBuild` and `BuckFile` both assemble with `SiblingMerge`: the targets in a
build directory merge into a single component that carries the directory's
file-level detected license and any declared-license file references. Build files
with non-trivial structure (macros, attribute calls) were already sibling-merged;
this makes the simple-top-level case consistent with them, and Bazel consistent
with Buck.

## Impact

- Bounded, explainable package output — no per-target empty-shell flood
- Bazel and Buck behave identically (same Starlark build-file family)
- **Expected and intentional**: Provenant reports fewer `pkg:bazel/*` /
  `pkg:buck/*` packages than ScanCode on `BUILD`/`BUCK`-heavy repositories. This
  is a deliberate noise-reduction divergence, **not** a regression or an
  extraction gap — the real packages and the dependency graph are unaffected.
