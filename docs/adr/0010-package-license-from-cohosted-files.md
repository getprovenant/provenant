# ADR 0010: Package Declared License From Co-hosted License Files

**Status**: Proposed
**Authors**: Provenant team
**Supersedes**: None (narrows one boundary stated in [ADR 0002](0002-extraction-vs-detection.md))

> **Current contract owner**: [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §3 and [`0002-extraction-vs-detection.md`](0002-extraction-vs-detection.md) own the declared-vs-detected boundary. The candidate implementation home is the post-assembly stage in `src/post_processing/` (`reference_following.rs`, `package_metadata_promotion.rs`).

## Context

When a package manifest declares no license, but the package directory contains a
co-hosted legal file (`LICENSE`, `COPYING`, `NOTICE`), ScanCode attaches that
file's detected license to the package's `declared_license_expression`. Provenant
does not: it surfaces the legal file only as a **file-level** detection and leaves
the package's declared license `null`.

This surfaced as a benchmark delta on `Netflix/spectator` (`build.gradle` →
ScanCode `apache-2.0` from the repo's root `LICENSE`; Provenant `null`). It is
**not** a bug or an oversight — it is the current, deliberately-chosen behavior:

- [ADR 0002](0002-extraction-vs-detection.md) (Accepted) states that "a sibling
  `LICENSE`/`README` that the manifest does not reference is surfaced only as a
  file-level detection … never promoted into a package's declared license," and
  that co-located key-file promotion "promotes only copyright/holder, never the
  declared license."
- The post-assembly pass `apply_package_reference_following`
  (`src/post_processing/reference_following.rs:856-899`) **does** adopt a license
  into the package declared expression, but only the manifest's **own** file-level
  detection or a file the manifest **references** (e.g. a `license-file` / "see
  LICENSE" pointer). Its comment is explicit: it "never adopts arbitrary
  co-located files."
- `promote_package_metadata_from_key_files`
  (`src/post_processing/package_metadata_promotion.rs:28-61`) already promotes
  `copyright` and `holder` from co-located key files when the package lacks them —
  but intentionally **not** the declared license.

So the mechanism for reading co-located key files into a package exists; the
declared license is deliberately excluded from it. Adopting ScanCode's behavior
therefore **amends an accepted ADR boundary** rather than filling a gap, which is
why it deserves an explicit decision.

### Is this a one-off, or broadly useful?

It is **not** a one-off. Many package formats either have no declared-license
field at all, or routinely omit it in sub-module/internal manifests, while the
package directory almost always carries a legal file. A parser-by-parser audit of
how each format populates `declared_license_expression`:

| Ecosystem (datasource)              | License field in format?                        | Benefit                                                              | Evidence                                                 |
| ----------------------------------- | ----------------------------------------------- | -------------------------------------------------------------------- | -------------------------------------------------------- |
| Go (`go.mod`)                       | **None by design**                              | **HIGH** — near-universal LICENSE-in-repo convention                 | `src/parsers/go.rs` sets license fields to `None`        |
| Autotools (`configure[.ac]`)        | None                                            | **HIGH** — GNU convention is `COPYING`/`LICENSE` in the package root | `src/parsers/autotools.rs` (directory-name only)         |
| Swift (`Package.swift`)             | None                                            | **HIGH**                                                             | `src/parsers/swift_manifest_json.rs` sets license `None` |
| Bazel (`BUILD`/`MODULE.bazel`)      | File-ref attr, often absent on internal targets | MEDIUM–HIGH                                                          | `src/parsers/bazel.rs`                                   |
| Gradle sub-modules (`build.gradle`) | `license {}` block, usually only at root        | MEDIUM–HIGH                                                          | `src/parsers/gradle.rs`                                  |
| SBT sub-modules (`build.sbt`)       | `licenses :=`, often omitted                    | MEDIUM                                                               | `src/parsers/sbt.rs`                                     |
| Helm (`Chart.yaml`)                 | Field exists, parser omits it                   | MEDIUM                                                               | `src/parsers/helm.rs`                                    |
| Python (`pyproject.toml`)           | Field present but many use classifiers only     | MEDIUM                                                               | `src/parsers/python/pyproject.rs`                        |
| Maven/npm/Cargo/Composer/…          | Field present, usually populated                | LOW                                                                  | parser license handling already populates declared       |

The standout is **Go**: `go.mod` carries no license metadata whatsoever, so for the
entire Go ecosystem the _only_ declared-license signal a scanner can attach to a
module is its co-hosted `LICENSE` file. The next tier (autotools, Swift, Bazel,
Gradle/SBT sub-modules) is also build-system code without a license field. This
breadth argues that — _if_ adopted — the behavior must be a single generic
post-assembly capability, not a per-parser patch.

## Decision

**Proposed — pending maintainer approval. No code lands until this ADR is
Accepted.** This document records the decision to be made and the recommended
option; it does not itself change behavior.

The choice is among three options (see Alternatives). The **recommended** option is
a **bounded, generic post-assembly pass** that promotes a co-hosted legal file's
detected license into a package's declared license, under strict guards:

- **Trigger only on genuine absence**: the assembled package has no
  `declared_license_expression` _and_ no `extracted_license_statement` (the
  manifest declared nothing and referenced nothing).
- **Source only true legal files**: files matching the existing `is_legal_file`
  classifier (`LICENSE`/`COPYING`/`NOTICE` family), co-located with one of the
  package's `datafile_paths` (same directory, or an ancestor only when no nearer
  package owns it). Never `README`/source files.
- **Single unambiguous result**: promote only when the co-hosted legal files yield
  one combined detected expression; abstain on conflicting results rather than
  guess.
- **Never smear across multiple packages**: skip when the directory hosts multiple
  packages or a multi-package datafile (e.g. a dpkg `status` database), mirroring
  the existing guard in `apply_package_reference_following`.
- **Preserve provenance**: the promoted detection must remain traceable to the
  legal file it came from (`from_file`), so output consumers can distinguish a
  manifest-declared license from a co-hosted-file-derived one.

This narrows ADR 0002's blanket prohibition to a precise one: **parsers** still
never read sibling files for declared license; only this **sanctioned
post-assembly pass** may, and only under the guards above.

Non-goals: full ScanCode parity for `README`-derived licenses; promoting when the
manifest already declared or referenced a license; reading file content inside
parsers.

## Consequences

### Benefits

- Closes a real, recurring ScanCode parity gap across many ecosystems, with the
  largest win in **Go**, where co-hosted `LICENSE` is the only possible declared
  signal.
- One generic, testable pass instead of N per-parser sibling-reading hacks (which
  ADR 0002 forbids in parsers anyway).
- Improves the package-level licensing record for build-system manifests
  (Gradle/SBT sub-modules, Bazel, autotools) that legitimately carry no license
  field.

### Trade-offs

- **Blurs the declared-vs-detected boundary** that ADR 0002 established. Mitigated
  by retained `from_file` provenance and by scoping the promotion to a dedicated
  post-assembly pass rather than parsers.
- **False-attribution risk in monorepos**: a root `LICENSE` could be promoted onto
  a sub-package that is actually licensed differently. Mitigated by the
  nearest-owner / same-directory and single-result guards, but this is the main
  correctness risk and needs golden coverage on nested layouts.
- **Golden churn** across many ecosystems on first rollout; expected and one-time,
  but must be reviewed rather than blindly regenerated.
- **Determinism**: ancestor-walk and multi-file combination rules must be fully
  deterministic to keep goldens stable.

## Alternatives Considered

1. **Keep the separation (status quo).** Reject the change; document the spectator
   delta as a justified divergence (Provenant reports declared = what the manifest
   claims; the Apache-2.0 `LICENSE` is still present as a file-level detection).
   - Pro: preserves ADR 0002 cleanly; zero false-attribution risk.
   - Con: leaves a broad, recurring parity gap, worst for Go where no other signal
     exists.
2. **Bounded generic post-assembly pass (recommended).** As described in Decision.
3. **Full ScanCode parity** (`add_license_from_file` + `add_license_from_sibling_file`,
   including `README`-derived licenses and promotion even when the package already
   has detections).
   - Rejected for now: highest false-positive surface (README prose), and it
     overrides manifest-declared data, which conflicts most strongly with ADR 0002.

## Related ADRs

- [ADR 0002: Extraction vs Detection Separation](0002-extraction-vs-detection.md) —
  this ADR narrows the "never promote co-located declared license" boundary stated
  there.
- [ADR 0006: DatasourceId-Driven Multi-Pass Package Assembly](0006-datasourceid-driven-package-assembly.md) —
  the post-assembly pass framework this capability would register with.

## References

- `src/post_processing/reference_following.rs:856-899` — existing manifest-referenced
  license adoption ("never adopts arbitrary co-located files").
- `src/post_processing/package_metadata_promotion.rs:28-61` — co-located key-file
  promotion of copyright/holder (excludes declared license).
- `src/parsers/go.rs`, `src/parsers/autotools.rs`, `src/parsers/swift_manifest_json.rs`,
  `src/parsers/bazel.rs` — formats with no declared-license field.
- ScanCode reference: `reference/scancode-toolkit/src/packagedcode/plugin_package.py`
  (`add_license_from_file`) and `licensing.py` (`add_license_from_sibling_file`).
- Origin: benchmark verification of `Netflix/spectator` (`build.gradle` declared-license delta).
