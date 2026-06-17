# testdata/ provenance and attribution

The fixtures in this directory are **test inputs only**. They are not part of any
Provenant distribution: `testdata/` is excluded from the published crate and the
release binaries, and from the SPDX license-header scope. It exists solely to
exercise Provenant's parsers, detectors, and golden comparisons.

## Composition

The corpus is a mix of:

1. **Provenant-authored fixtures** — synthetic inputs (hand-written manifests,
   minimal parser probes, intentionally malformed negatives, and expected golden
   outputs) created by Provenant contributors and licensed under Apache-2.0 with
   the rest of the project.

2. **Fixtures copied or adapted from the ScanCode Toolkit test suite** — mirrored
   locally per the contributor workflow rather than referenced from the
   submodule. These remain under their upstream ScanCode licensing (Apache-2.0
   for code, CC-BY-4.0 for data); see the root [`NOTICE`](../NOTICE).

3. **Small inputs derived from public open-source repositories** encountered while
   developing and benchmarking Provenant (see [`docs/BENCHMARKS.md`](../docs/BENCHMARKS.md)
   for the repositories that have been scanned). These are small, representative
   inputs used to reproduce real-world parsing and detection behavior.

## Attribution policy

Third-party fixture content remains under its original license; Provenant claims
no ownership of it and includes it solely as test data. Per-fixture provenance is
**not** individually tracked: the corpus accreted over time and exact origins
cannot be reliably reconstructed, consistent with common practice for large test
corpora.

License and copyright **texts** present as fixtures are reproductions of those
licenses, included to exercise license/copyright detection; reproducing a
license's own text is not a third-party-code concern.

When a **substantial, wholesale** copy of an identifiable third-party file or
package is added — especially under a copyleft or notice-required license — its
source and license are recorded in a **co-located note** next to the fixture (a
short `README`/`SOURCE` in the containing directory), rather than in a central
index that would drift as fixtures move. Prefer synthetic or truncated fixtures
over wholesale copies where they exercise the same behavior.
