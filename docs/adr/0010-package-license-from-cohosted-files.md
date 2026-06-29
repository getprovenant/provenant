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
- **Single legal file, or agreeing files**: promote when all the promoted
  detections originate from a _single_ legal file, or when _multiple_ legal files
  resolve to one shared expression. Abstain only when _multiple separate_ legal
  files resolve to _differing_ expressions (e.g. dual `LICENSE-APACHE` +
  `LICENSE-MIT`) rather than guessing an `AND`/`OR` combination. See
  [Single legal file with a compound license vs. multiple disagreeing files](#single-legal-file-with-a-compound-license-vs-multiple-disagreeing-files)
  for why a single compound file is promoted but disagreeing separate files are not.
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
- ✅ Single-file compound-license coverage: `promotes_single_file_compound_license`
  (one `LICENSE` → canonical `bsl-1.1 AND mpl-2.0`),
  `promotes_single_file_alternative_license_without_forcing_and` (an `OR`-shaped file
  keeps its `OR`), `abstains_when_multiple_files_disagree_even_with_provenance` (dual
  `LICENSE-APACHE`/`LICENSE-MIT` still abstains), and `promotes_when_multiple_files_agree`
  (separate agreeing files still promote), plus the end-to-end
  `create_output_promotes_single_file_compound_license_from_cohosted_legal_file`.

### Scope note: assembled packages only, not file-level `package_data`

This pass enriches the assembled `Package`, not the file-level `package_data`
entries on the contributing files. Two consequences, both verified and accepted:

- The delta that motivated this ADR — `Netflix/spectator`'s root `build.gradle` —
  is **not** closed, because that `build.gradle` carries no package identity, so
  Provenant assembles **no package** for it (`packages: []`); there is nothing to
  attach a declared license to.
- Where a package _does_ assemble, ScanCode also stamps the license onto each
  contributing file's `package_data` (e.g. `go.mod`/`go.sum`), whereas this pass
  updates only the assembled package. Verified on `sirupsen/logrus`: the assembled
  `pkg:golang/.../logrus` package gains `declared: mit` (matching ScanCode), while
  the `go.mod`/`go.sum` file-level `package_data` stay null.

### Decision: file-level `package_data` license stamping is **Rejected**

ScanCode also writes the declared license onto each contributing datafile's
file-level `package_data` rows, via two mechanisms in
`packagedcode/plugin_package.py` / `licensing.py`:

- **`add_license_from_file`** — stamps the **datafile's own file-content license
  detections** (`resource.license_detections`) onto that same datafile's
  `package_data` rows when they carry no license.
- **`add_license_from_sibling_file`** (and the `add_referenced_license_*` family) —
  pulls a **sibling file's** license (`LICENSE`, but also `README`, `NOTICE`) onto a
  datafile's `package_data` rows.

The earlier revision of this ADR left this "out of scope" as a deferred expansion.
This revision upgrades that to a **definitive rejection**: Provenant will **not**
stamp declared licenses onto file-level `package_data`. The assembled-`Package`
pass above already captures the entire safe, high-value subset; the file-level
extension adds only false-positive surface. This was confirmed empirically against
the two largest recorded ScanCode-better file-level deltas, `hashicorp/terraform`
and `apache/superset` (`.provenant/compare-runs/*terraform*`, `*superset*`).

**The assembled-package delta is already closed.** On both targets the assembled
`Package` already matches or exceeds ScanCode: terraform's root
`pkg:golang/.../terraform` carries `bsl-1.1 AND mpl-2.0` (matching ScanCode), and
superset's `pkg:pypi/apache-superset` carries `apache-2.0 AND ofl-1.1` — _more_
accurate than ScanCode's `apache-2.0` (the documented intentional divergence in
[ADR 0002](0002-extraction-vs-detection.md)). Only the **file-level `package_data`**
rows still differ, and every remaining difference is a false positive a
correctness-preserving pass must not reproduce.

#### Concrete FP / smear risk cases (from the benchmark data)

1. **Multi-entry database/lockfile own-content smear.** ScanCode's
   `add_license_from_file` reads every license detected _inside_ a lockfile's own
   text and collapses it into one declared expression for the file. superset's
   `superset-websocket/.../package-lock.json` has **31** file-level license
   detections (one per vendored dependency's license string) and is stamped
   `mit AND isc`. terraform's root `go.sum` carries **619** dependency entries and
   `go.mod` **307**; stamping a single declared license onto a 619-entry resolved
   database misrepresents it as one declared license. These files are inventories of
   _other_ packages' licenses, not a declaration of the datafile's own license.

2. **`README`/`NOTICE` sibling-derived stamping.** superset's `docs/yarn.lock`
   (`apache-2.0`, sourced from `docs/README.md` + `NOTICE`) and
   `superset-websocket/.../client-ws-app/package.json` (`apache-2.0`, sourced from a
   sibling `README.md`) get their license from non-legal prose siblings — there is
   **no** `LICENSE` file in either directory. This is exactly the high-false-positive
   `README`-derived source this ADR's parent decision already rejected (see
   Alternatives §3).

3. **Reference-following own-text + multi-source compound garbage.** superset's
   `pyproject.toml` is stamped
   `apache-2.0 AND (apache-2.0 AND ofl-1.1) AND (afl-2.1 AND … AND unlicense)` — an
   11-license expression with **duplicated `apache-2.0` operands** and parenthesized
   sub-expressions, assembled from the file's own classifier/dependency text plus
   `LICENSE.txt` and `NOTICE`. This is not a declared license; it is detection noise.

4. **A datafile contributing to / co-located with multiple datafiles.** Even the
   cleanest target fails a sole-datafile guard: terraform's root directory hosts
   **two** package datafiles (`go.mod` _and_ `go.sum`), so the directory is not the
   single-datafile, sole-package scope ADR 0010's assembled-package guards rely on.
   A file-level pass would have to stamp both, including the 619-entry `go.sum`.

5. **A `package_data` row that already declares its own license** must never be
   overridden — ScanCode itself only fills empty rows, but at the file level the
   "empty" rows include multi-entry inventory files (case 1), so "genuine absence"
   is not a sufficient guard the way it is for an assembled package.

#### Why guards cannot rescue a safe subset

The assembled-`Package` guards (genuine absence, `is_legal_file` source,
same-directory + sole-package, single/agreeing expression, retained `from_file`)
work because an assembled package is a single identity anchored in one directory.
At the file level those guards do **not** map cleanly:

- "Sole package in the directory" does not bound a _datafile_: terraform's root has
  two datafiles for one package, and lockfiles are multi-entry by nature.
- Restricting to same-directory `LICENSE`-only would **not reproduce any** of the
  recorded deltas — superset's deltas are all `README`/`NOTICE`/own-text sourced
  (no co-located `LICENSE`), and terraform's only same-dir `LICENSE`
  (`mpl-2.0 AND bsl-1.1`) lands on multi-entry `go.mod`/`go.sum` databases.

So the only deltas a guarded file-level pass _could_ close are precisely the unsafe
ones (README-derived, multi-entry smears), and the only safe candidate
(terraform's same-dir `LICENSE`) targets multi-entry database files. There is no
residual safe subset: the safe, high-value surface is fully covered by the
assembled-`Package` pass, and the file-level remainder is all false-positive risk.

#### Consequences of the rejection

- **Benchmark deltas stay open by design.** terraform's `go.mod`/`go.sum` and
  superset's `setup.py`/`pyproject.toml`/`docs/yarn.lock`/`client-ws-app` file-level
  `package_data` declared-license fields remain `null`. These are recorded as
  **justified Provenant advantages / accepted divergences**, not bugs: Provenant
  declines to assert a declared license on a datafile (especially a multi-entry
  database) that does not itself declare one.
- **The authoritative surface is unaffected.** Consumers that want the package's
  declared license read the assembled `Package`, which is already correct (and on
  superset, more accurate than ScanCode).
- No code change ships from this decision; the existing
  `promote_package_declared_license_from_legal_files` pass and its
  assembled-package-only scope are retained verbatim.

### Single legal file with a compound license vs. multiple disagreeing files

The "abstain on ambiguity" guard above must distinguish two cases that both surface
as _more than one distinct license expression_ among the co-located detections:

1. **One legal file carrying a compound license.** A single `LICENSE` can
   legitimately declare more than one license — e.g. `hashicorp/terraform`'s
   repo-root `LICENSE` detects both `mpl-2.0` and `bsl-1.1`, producing two
   `license_detections` from one file. This is not ambiguity: the file's own
   `detected_license_expression` (`bsl-1.1 AND mpl-2.0`) is the authoritative combined
   form, and ScanCode attaches exactly that to the package. The promotion pass
   therefore adopts that file's own combined expression for the package. It is
   **adopted as-is, not re-combined under a forced `AND`** — only normalized for
   canonical operand ordering — so a single file whose own expression is an `OR`
   (`apache-2.0 OR mit`) is promoted as that `OR`, never silently tightened into an
   `AND`.
2. **Multiple separate legal files that disagree.** A directory containing
   `LICENSE-APACHE` (`apache-2.0`) _and_ `LICENSE-MIT` (`mit`) is genuinely
   ambiguous — the package is dual-licensed and the choice (`AND` vs. `OR`) is not
   recoverable from the files alone. The pass keeps abstaining here.

The two are told apart by **detection provenance**: every detection's
`matches[].from_file` records the legal file it came from, so the pass groups the
co-located detections by their source legal file. If they all come from a single
source file, that file's own `detected_license_expression` is promoted verbatim (case
1). If they come from multiple source files, the pass promotes only when those files
resolve to one shared expression and otherwise abstains (case 2). This **does not
reopen** the dual-license-ambiguity abstention: the anti-ambiguity guard now keys on
_how many distinct legal files disagree_, not on _how many detections a single file
produced_, so the `LICENSE-APACHE` + `LICENSE-MIT` case is unchanged while a single
compound `LICENSE` is no longer wrongly abstained on.

Every other ADR 0010 guard is preserved unchanged: genuine-absence trigger,
`is_legal_file` source only, same-directory + sole-package scope, retained
`from_file` provenance, and skipping resolved-dependency records.

As with every assembled-package promotion under this ADR, the contributing
`go.mod`/`go.sum` file-level `package_data` entries stay `null` by design (see
[Scope note](#scope-note-assembled-packages-only-not-file-level-package_data)); only
the assembled `pkg:golang/.../terraform` package gains `bsl-1.1 AND mpl-2.0`.

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
- **Conservative on dual-license dirs**: a directory with _separate_ `LICENSE-APACHE`
  - `LICENSE-MIT` files is intentionally left unset rather than guessing `OR`/`AND`;
    recovering those is future work. A _single_ legal file carrying a compound license
    (e.g. `mpl-2.0 AND bsl-1.1`) is **not** this case and is promoted as its own
    `AND`-combination — see
    [Single legal file with a compound license vs. multiple disagreeing files](#single-legal-file-with-a-compound-license-vs-multiple-disagreeing-files).

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
4. **File-level `package_data` license stamping** (ScanCode's `add_license_from_file`
   / `add_license_from_sibling_file` onto each datafile's `package_data` rows).
   - **Rejected** (see [Decision: file-level `package_data` license stamping is
     Rejected](#decision-file-level-package_data-license-stamping-is-rejected)).
     Empirically every recorded ScanCode-better file-level delta on `terraform` and
     `apache/superset` is a false positive (multi-entry lockfile own-content smear,
     `README`/`NOTICE` sibling derivation, or a multi-source compound expression with
     duplicated operands), and the assembled-`Package` pass already closes the entire
     safe, high-value subset.

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
