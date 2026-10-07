# Go Parser Improvements

## Summary

Rust now intentionally differs from or improves on released Python ScanCode Go handling in several concrete ways:

1. preserves fallback datasource IDs across Go parser error paths
2. preserves `replace`, `retract`, and `toolchain` directive fidelity across real parser inputs
3. adds a dedicated `go.mod graph` parser so direct vs transitive module relationships are modeled separately from `go.sum`
4. categorizes `_test.go` and `//go:build test` files as non-production source for source-directory heuristics

## Reference limitation

The Python reference covers `go.mod` and `go.sum`, but module-graph data, directive fidelity, and test-only source categorization remain thinner than modern Go workflows need.

## Rust Improvements

### Directive and fallback correctness

- Rust preserves `replace`, `retract`, and `toolchain` directive semantics on real parser inputs such as the `opencensus-service` fixture.
- Fallback `PackageData` for `go.mod`, `go.sum`, and `Godeps.json` now keep the correct `datasource_id`, which is important for assembly/accounting consistency.

### Module graph support

- Rust adds a dedicated `go.mod.graph` / `go.modgraph` / `go-mod-graph.deplock` parser for checked-in `go mod graph` output.
- The graph parser models:
  - direct module edges from the main module
  - transitive module edges from dependency modules
  - pinned versions from the graph artifact itself
- `go@…` and `toolchain@…` nodes are skipped because they are Go language and toolchain requirements, not modules.
- `is_runtime` and `is_optional` stay unset because the graph does not prove them.
- This keeps graph semantics distinct from `go.sum`, which remains checksum-focused.

### Build-constraint-aware categorization

- Scanner-side Go categorization now treats these files as non-production source for directory `is_source` heuristics:
  - `_test.go`
  - files with `//go:build test`
  - files with `// +build test`
- This prevents ordinary Go package directories from being penalized in source-count heuristics just because they contain test-only files.

## Why this matters

- **Better module graph fidelity**: checked-in `go mod graph` outputs become usable dependency evidence
- **Safer assembly accounting**: fallback parser identity stays intact across Go metadata surfaces
- **More accurate source heuristics**: test-only Go files no longer distort production-source classification
