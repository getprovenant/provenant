# ADR 0012: Promote Resolved Dependencies to SBOM Components

**Status**: Accepted
**Authors**: Provenant team
**Supersedes**: None
**Current Contract Owner**: `src/output/sbom.rs`, `src/output/cyclonedx.rs`, `src/output/spdx.rs`

## Context

Provenant's CycloneDX and SPDX SBOM renderers historically sourced their
component/package inventory **only** from top-level detected packages
(`output.packages`). The resolved dependencies that Provenant fully extracts
from lockfiles and manifests (`output.dependencies`) appeared **only** as
dependency-graph edges — a CycloneDX `dependsOn` reference or (previously)
nothing at all in SPDX.

A CycloneDX `dependsOn` entry is defined to reference the `bom-ref` of a
component **in the same document**. Because resolved dependencies were never
emitted as components, every such edge pointed at a `bom-ref` that no component
declared. Concretely:

- A 692-package `Cargo.lock` produced **1 component + 757 dangling
  `dependsOn` refs**.
- The `express` example produced **1 component + 44 dangling edges**.

The document named relationships to materials it never listed: an incomplete,
arguably invalid BOM. A consumer walking `components` saw one item for a
project with hundreds of dependencies.

The forces that made the fix non-obvious:

- **Licenses.** A resolved dependency's license is only sometimes knowable
  statically. Provenant must never fetch it (the static / bounded / no-network
  guarantee) and never guess it (honest-unknowns-over-guessed-defaults).
- **Dedup.** A vendored dependency can be _both_ a detected package and a
  lockfile entry; it must collapse to one component, not double-count.
- **Identity.** Promoted components need stable `bom-ref` / SPDX ids that the
  dependency graph can resolve, including unversioned declared deps.
- **Native output is a shipped 1.0 contract.** The ScanCode-compatible JSON
  (`--json`, `--json-pp`, `--yaml`, `--json-lines`) must not change.

## Decision

Promote **every resolved dependency to a component (CycloneDX) / package
(SPDX)**, in the SBOM renderers only.

### Scope boundary

- Only the SBOM renderers change. The native ScanCode-compatible output schema
  (`src/output_schema/`) and its writers are untouched: `output.packages` and
  `output.dependencies` keep their existing shape and field semantics. This
  honors ADR 0008 — promotion is an output-shaping concern, computed at render
  time, never a new domain field.
- The shared inventory and edge resolution live in `src/output/sbom.rs` so
  CycloneDX and SPDX build the identical inventory and graph endpoints from
  the identical rules. Format-specific presentation (CycloneDX `scope`, SPDX
  `FilesAnalyzed` / `DESCRIBES`) is derived at the renderer boundary.

### Dedup

The dedup key is the normalized **purl**.

- A resolved dependency whose purl equals a detected package's purl is **not**
  promoted — the detected package already represents it (and carries richer,
  file-backed evidence).
- Multiple resolved dependencies that share a purl collapse to **one** promoted
  component.

### Identity

- Detected packages keep their existing `bom-ref` scheme (purl when unique
  across the document, else `package_uid`, else a synthetic index).
- A promoted dependency's `bom-ref` (CycloneDX) is its **purl**. Because
  promotion is deduped by purl and skips any purl a package already owns, each
  promoted purl is unique across the document, so a `dependsOn`/`DEPENDS_ON`
  edge that targets that purl now resolves to a real component.
- Unversioned declared deps (e.g. a `package.json` dependency with no committed
  lockfile) are still promoted, keyed by their versionless purl.
- SPDX promoted packages get `SPDXRef-Package-Dependency-N` ids; the graph is
  rendered as `Relationship: <owner> DEPENDS_ON <dependency>`.

### Licenses — the hard constraint

A promoted dependency carries a license **only where Provenant can truthfully
determine it statically**:

- When the resolved dependency carries package metadata Provenant already
  extracted statically (`resolved_package`), its declared/detected license
  fields are copied faithfully. This is a license declared in a present
  manifest or lockfile — truthful, no network.
- A vendored dependency whose license was **detected in source that is in the
  repo** is a _detected package_; it flows through the existing package path
  (and dedup keeps it as the single component for that purl).
- Otherwise the license is left **unset** (CycloneDX: no `licenses`; SPDX:
  `NOASSERTION`). Provenant never fetches and never guesses.

### Metadata honesty

- CycloneDX component `scope` for a promoted dependency is set **only** from a
  proven `is_optional`: `false` → `required`, `true` → `optional`, unknown →
  omitted. When several deps share a purl, a single proven "required" wins.
- Detected top-level packages keep `scope: required` (they are present in the
  scanned tree). This is unchanged pre-existing behavior.
- No other dependency-intent flag (`is_runtime`, `is_direct`, `is_pinned`) is
  invented for a component when it is not proven.

## Amendment (issue #1320): versioned purls and the vendored-license join

The original decision keyed promotion and graph edges on the dependency's
_declared_ purl. For an ecosystem that resolves versions in a lockfile that is a
separate datasource from the manifest (npm, Cargo, PyPI, …), that produced two
shortcomings visible in a real scan:

- A dependency promoted from a manifest requirement carried the **unversioned**
  purl (`pkg:npm/leftpad`) even when the lockfile had already resolved it to a
  version — the versioned coordinate that makes the purl a usable join key for
  downstream license enrichment (deps.dev, ClearlyDefined, an SCA tool).
- When the dependency's source or metadata was on disk (`node_modules/<dep>/`,
  `*.dist-info/METADATA`, vendored source), Provenant detected and licensed it
  as a package at its **versioned** purl, but the manifest requirement was
  promoted as a **separate, unversioned, unlicensed** component. The same
  dependency appeared twice, and the licensed copy was not the purl-keyed one.

### Decision

Resolve each dependency edge purl to its **versioned coordinate** before
promotion and edge resolution, using only static evidence already in the
document. An already-versioned edge keeps its purl. An unversioned edge is
upgraded only from proof about that same edge:

1. the edge's own `resolved_package` purl (its actual resolution), else
2. a **single unambiguous** versioned sibling recorded under the **same owner**
   (`for_package_uid`) and the same identity.

`SbomInventory` builds that evidence as a per-owner version index keyed by
`(owner, version-less identity)` from dependency edges only — never from
detected packages, which are not owner-scoped. Two correctness rules make the
join safe:

- **Owner-scoped.** A version is never borrowed across owners. Two packages can
  resolve the same requirement to different versions; each edge takes its
  version only from its own owner's resolution. An absent or empty owner is not
  a shared bucket either: ownerless edges never contribute to or consult the
  sibling index, so a bare hoisted requirement can only take a version from its
  own `resolved_package`, never from an unrelated ownerless dependency.
- **Identity strips only the version.** Type, namespace, name, qualifiers, and
  subpath are all preserved, so variants that differ by a qualifier (`?arch=`,
  `?classifier=`, …) or subpath are distinct coordinates and never conflated.

When the owner has zero or multiple candidate versions for an identity, the edge
keeps its unversioned purl (honest-unknowns — Provenant does not guess which
version a range resolved to; e.g. a workspace member whose lockfile edges are
owned elsewhere, or a diamond that resolves two versions). Both promotion dedup
and `dependsOn`/`DEPENDS_ON` edge resolution route through the same resolver, so
a component and the edges that target it always agree and the graph stays
closed.

This closes both gaps with the existing purl-dedup: once the unversioned
requirement is resolved to the coordinate, it dedups against the detected
package (vendored case → one licensed, versioned component) or against the
versioned lockfile edge (source-absent case → one versioned component, license
carried from `resolved_package` when present, otherwise unset). Nothing is
fetched; the license and version come only from what was scanned.

Both formats render the versioned coordinate. CycloneDX components carry the
versioned purl as `bom-ref` and `purl`. SPDX packages (subject and promoted
dependency alike) now emit `PackageVersion` and a package-manager purl
`ExternalRef` — tag-value `ExternalRef: PACKAGE-MANAGER purl <purl>` and RDF
`spdx:versionInfo` + `spdx:externalRef` (referenceCategory `packageManager`,
referenceType `purl`) — so the join key is usable from SPDX too.

Still static and no-network: the change is purely a render-time join over data
already extracted. When neither a detected package nor a same-owner resolved
version exists for an identity, the component keeps its unversioned purl and
unset license.

## Consequences

### Benefits

- The `dependsOn` / `DEPENDS_ON` graph resolves to real components — no dangling
  refs. The BOM is a complete inventory plus a valid graph.
- CycloneDX and SPDX share one inventory + edge implementation, so the two
  formats can never drift on which dependencies they list or which graph
  endpoints resolve.
- Honest licenses and metadata: consumers can trust that a populated license was
  statically determined, and that an absent one is genuinely unknown.

### Trade-offs

- SBOM documents grow from O(detected packages) to O(all resolved dependencies)
  — the point of the change, but larger output for big lockfiles.
- Many promoted components will have an unset license. That is the honest state;
  an optional, opt-in online license-enrichment path is tracked separately.
- Promotion clones/synthesizes component records at render time. This is output
  shaping, not a hot path.

## Alternatives Considered

- **Drop dangling `dependsOn` edges instead of promoting.** Produces a valid but
  far less useful BOM — the dependency inventory users actually want disappears.
- **Add a new field to the domain/output schema for promoted components.** Would
  change the shipped native JSON contract and violate ADR 0008's separation.
- **Fetch missing licenses from registries.** Rejected: breaks the static /
  bounded / no-network guarantee.
- **Emit a synthesized version (`unknown`) or guessed scope.** Rejected under
  honest-unknowns-over-guessed-defaults.

## Related ADRs

- [ADR 0008: Output Schema Type Separation](0008-output-schema-separation.md) —
  promotion is an output-shaping concern computed at the render boundary, not a
  new internal/output-schema field.
- [ADR 0006: DatasourceId-Driven Package Assembly](0006-datasourceid-driven-package-assembly.md)
  — assembly produces the `packages` and `dependencies` the renderers consume.

## References

- Issue #1319
- Issue #1320 (versioned purls + vendored-license join amendment)
- Promotion/dedup: `src/output/sbom.rs`
- CycloneDX renderer: `src/output/cyclonedx.rs`
- SPDX renderer: `src/output/spdx.rs`
- Golden fixtures: `testdata/output-formats/cyclonedx-dependencies-expected.{json,xml}`,
  `testdata/output-formats/spdx-*-expected.tv`
