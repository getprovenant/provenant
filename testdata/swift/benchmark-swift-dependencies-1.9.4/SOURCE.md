# Source

`swift-show-dependencies.deplock` is the `swift package show-dependencies --format json`
output for `pointfreeco/swift-dependencies` at tag `1.9.4`, archived here as a
benchmark target fixture:

- Upstream: `https://github.com/pointfreeco/swift-dependencies` at tag `1.9.4`
  (commit `a501eebe552fd23691c560adf474fca2169ad8aa`)
- Generated on 2026-06-19 with Swift 6.3.2 via:

  ```
  git clone --branch 1.9.4 https://github.com/pointfreeco/swift-dependencies.git
  cd swift-dependencies
  swift package show-dependencies --format json > swift-show-dependencies.deplock
  ```

- The graph resolves to 17 dependency nodes across `combine-schedulers`,
  `swift-clocks`, `swift-concurrency-extras`, `swift-syntax`, and sibling Point-Free
  packages.
- The ephemeral build-time root `url`/`path` (`.build/checkouts/...` absolute paths)
  were normalized to `/swift-dependencies/...` so the fixture is stable and
  machine-independent; no other field was altered.
- sha256: `c03e7c749eac41f54a40d922c42e0e2d02275479e2c1d567db582a3aeee76c9d`

This is the single-file input measured by the
`swift-dependencies 1.9.4 swift-show-dependencies.deplock` row in
[`docs/BENCHMARKS.md`](../../../docs/BENCHMARKS.md). It is committed here only as a
test/benchmark fixture. See [`../../PROVENANCE.md`](../../PROVENANCE.md).
