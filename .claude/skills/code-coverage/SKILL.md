---
name: code-coverage
description: Measure and improve Provenant's Rust test coverage locally with cargo-llvm-cov. Use for code coverage, llvm-cov, finding untested code, test gaps, or "what isn't tested".
---

# Code Coverage (local signal)

Coverage in Provenant is a **local tool for finding untested code**, not a CI gate
or a number to maximize. See [`docs/TESTING_STRATEGY.md`](../../../docs/TESTING_STRATEGY.md)
for the "confidence, not coverage" philosophy. Use this to locate genuinely
untested paths, then write meaningful behavior tests for them — do not add tests
just to move the number.

## Prerequisites

`cargo-llvm-cov` is installed by `npm run setup`. Otherwise:

```bash
cargo install --locked cargo-llvm-cov
```

## Commands

```bash
# Per-file summary table for the whole library (fast triage)
npm run coverage            # = cargo llvm-cov --lib

# Annotated line-by-line HTML report, opened in a browser
npm run coverage:html       # = cargo llvm-cov --lib --open

# Scope to one module while iterating (much faster)
# The test-name filter must follow `--` so it is forwarded to the test binary.
cargo llvm-cov --lib -- copyright::refiner

# Show uncovered regions for a path as text
cargo llvm-cov --lib --show-missing-lines -- <filter>
```

## How to read it

- `--lib` covers **unit tests only**. Integration suites (`tests/*.rs`) and
  golden suites (`--features golden-tests`) exercise far more code than this
  credits, so treat the number as a **lower bound**. A file showing 0% may still
  be well-covered by golden/integration tests.
- Low coverage on pure data tables (e.g. `copyright/grammar/rules.rs`) or generated
  code is expected and not worth "fixing".
- Prioritize gaps in branching logic: error paths, parser fallbacks, edge cases
  in detection/assembly — places where a wrong branch produces wrong output.

## Workflow to close a real gap

1. Run `npm run coverage` and pick a module with thin coverage on real logic.
2. Re-run scoped (`cargo llvm-cov --lib --show-missing-lines -- <module>`) to see the
   exact uncovered lines.
3. Confirm the lines aren't already covered by a golden/integration suite before
   assuming they're untested (grep the relevant `tests/` and `*_scan_test.rs`).
4. Add behavior-focused tests per [`docs/TESTING_STRATEGY.md`](../../../docs/TESTING_STRATEGY.md)
   (right layer: unit vs scan/assembly contract vs golden).
5. Re-run scoped coverage to confirm the gap is closed.
