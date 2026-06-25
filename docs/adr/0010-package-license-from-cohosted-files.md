# ADR 0010: Package Declared License From Co-hosted License Files

**Status**: Accepted
**Authors**: Provenant team
**Supersedes**: None (narrows one boundary stated in [ADR 0002](0002-extraction-vs-detection.md))

> **Current contract owner**: [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §3 and [`0002-extraction-vs-detection.md`](0002-extraction-vs-detection.md) own the declared-vs-detected boundary. The accepted behavior is implemented by the post-assembly pass `promote_package_declared_license_from_legal_files` in `src/post_processing/package_metadata_promotion.rs`.

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
| Python (`pyproject.toml`)           | Field present but many use classifiers only     | MEDIUM                                                               | `src/parsers/python/pyproject.rs`                        |
| Helm (`Chart.yaml`)                 | **Field exists; parser omits it** (parser gap)  | LOW (contingent)                                                     | `src/parsers/helm.rs`                                    |
| Maven/npm/Cargo/Composer/…          | Field present, usually populated                | LOW                                                                  | parser license handling already populates declared       |

The standout is **Go**: `go.mod` carries no license metadata whatsoever, so for the
entire Go ecosystem the _only_ declared-license signal a scanner can attach to a
module is its co-hosted `LICENSE` file. The next tier (autotools, Swift, Bazel,
Gradle/SBT sub-modules) is also build-system code without a license field. This
breadth argues that — _if_ adopted — the behavior must be a single generic
post-assembly capability, not a per-parser patch.

**Helm is a different case** and is listed separately: `Chart.yaml` _has_ a
`license`-bearing surface the parser simply does not read yet, so the correct
remedy is closing that parser gap, not co-hosted-file attribution. Its apparent
benefit here is contingent on the parser gap remaining unfixed; once the parser
reads the field, Helm belongs in the LOW row beside Maven/npm/Cargo. It is kept in
the table only so reviewers can weigh the two remedies independently — it is **not**
a justification for this pass.

## Decision

**Accepted and implemented.** Of the three options (see Alternatives), the chosen
approach is a **bounded, generic post-assembly pass**
(`promote_package_declared_license_from_legal_files`) that promotes a co-hosted
legal file's detected license into a package's declared license, under strict
guards:

- **Trigger only on genuine absence**: the assembled package has no
  `declared_license_expression` _and_ no `extracted_license_statement` (the
  manifest declared nothing and referenced nothing).
- **Source only true legal files**: files matching the existing `is_legal_file`
  classifier (`LICENSE`/`COPYING`/`NOTICE` family). Never `README`/source files.
- **Same-directory, sole-package scope** (as implemented): a legal file is adopted
  only when it sits in a directory the package is anchored in (the parent of one of
  its `datafile_paths`) _and_ that directory anchors no other package. This bounds
  attribution to the package's own directory, so a root `LICENSE` is never smeared
  across sibling packages or down into nested sub-packages — the conservative
  starting point for the "main correctness risk" below. An ancestor-walk
  generalization (inheriting a parent `LICENSE` when no nearer package boundary
  intervenes) is intentionally deferred; it widens attribution and should be a
  separate change with its own nested-layout coverage. Note this does **not** use
  the `for_packages` association that copyright/holder promotion relies on, because
  that link is only populated by ecosystem-specific resource-assign passes and is
  empty for exactly the formats this pass targets (Go, autotools, Swift, Bazel).
- **Single unambiguous result**: promote only when the co-located legal files
  resolve to exactly one distinct declared expression; abstain on conflicting
  results (e.g. dual `LICENSE-APACHE` + `LICENSE-MIT`) rather than guessing an
  `AND`/`OR` combination.
- **Preserve provenance**: the promoted `license_detections` retain each match's
  `from_file`, so output consumers can distinguish a co-hosted-file-derived license
  from a manifest-declared one.

This narrows ADR 0002's blanket prohibition to a precise one: **parsers** still
never read sibling files for declared license; only this **sanctioned
post-assembly pass** may, and only under the guards above.

Non-goals: full ScanCode parity for `README`-derived licenses; promoting when the
manifest already declared or referenced a license; reading file content inside
parsers.

### Acceptance criteria (satisfied)

- ✅ Narrowly scoped maintenance notes in [ADR 0002](0002-extraction-vs-detection.md)
  and [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §3 forward-reference ADR 0010, so
  their "never promoted into a package's declared license" statements are no longer
  silently inaccurate.
- ✅ Nested/monorepo coverage proving no smear: unit tests in
  `package_metadata_promotion.rs` (`does_not_smear_root_license_into_nested_subpackage`,
  `each_package_gets_its_own_colocated_license`, `skips_directory_hosting_multiple_packages`,
  `abstains_when_colocated_legal_files_disagree`) plus an end-to-end `create_output`
  wiring test.

### Scope note: the origin case is not closed by this pass

The delta that motivated this ADR — `Netflix/spectator`'s root `build.gradle` — is
**not** affected, because that `build.gradle` carries no package identity, so
Provenant assembles **no package** for it (`packages: []`); there is nothing to
attach a declared license to. ScanCode reports the license on the file's
`package_data` instead. Promoting onto identity-less file-level `package_data` is a
larger, higher-false-positive expansion and remains out of scope. This pass targets
the common, higher-value case: a package that **is** assembled (Go modules,
autotools, Swift, Bazel) but declares no license.

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
  a sub-package that is actually licensed differently. Addressed by the
  same-directory + sole-package guards: a sub-package never inherits an ancestor's
  `LICENSE`, and a directory shared by multiple packages is skipped entirely. The
  deferred ancestor-walk generalization would reopen this risk and must carry its
  own nested-layout coverage.
- **No golden churn observed**: the genuine-absence guard means packages that
  already declare a license are untouched; the assembly, post-processing,
  output-format, scanner-integration, and license-detection golden suites all pass
  unchanged.
- **Conservative on dual-license dirs**: a directory with `LICENSE-APACHE` +
  `LICENSE-MIT` is intentionally left unset rather than guessing `OR`/`AND`;
  recovering those is future work.

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
