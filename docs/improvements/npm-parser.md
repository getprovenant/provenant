# npm Parser: Resolutions and Overrides as Honestly-Scoped Pins

## Summary

The npm `package.json` parser now emits Yarn `resolutions` and npm `overrides`
entries as dependency rows, distinctly scoped (`"resolutions"` and
`"overrides"`) and flagged as pins (`is_pinned = true`) while leaving the
intent booleans (`is_runtime`, `is_optional`, `is_direct`) unset.

Previously these blocks were stored opaquely in `extra_data` and never surfaced
as dependencies, so projects that pin transitive versions exclusively through
`resolutions`/`overrides` under-reported their declared dependency surface.

## Behavior difference vs the Python reference

The Python reference maps Yarn `resolutions` to dependencies but forces
`is_runtime = true`, `is_optional = false`, and `is_direct = true` on them, and
it does not surface npm `overrides` as dependencies at all.

Provenant diverges deliberately on both points:

- **Resolutions/overrides are transitive-graph version overrides, not declared
  direct runtime dependencies.** They prove a _pin_ — a concrete version
  constraint the project asserts — but they do not prove runtime-vs-dev,
  optional-vs-required, or direct-vs-transitive intent. Per the project's
  "honest unknowns over guessed compatibility defaults" guardrail, Provenant
  sets `is_pinned = true` and leaves the intent booleans unset rather than
  asserting semantics the datasource does not establish.
- **`overrides` is surfaced too.** npm's `overrides` is the npm-native
  equivalent of Yarn `resolutions`; omitting it under-reports the pinned
  dependency surface for npm-managed projects.

Distinct scopes let downstream consumers separate override pins from real
declared dependency edges.

## Supported shapes

- Yarn `resolutions`:
  - plain `"pkg": "range"` entries
  - glob-scoped keys such as `**/@scope/pkg` and `parent/**/pkg` (the trailing
    package selector is used to build the purl)
  - `npm:name@range` aliased requirements (unwrapped to the real package name,
    matching the other npm scopes)
- npm `overrides`:
  - plain `"pkg": "version"` entries
  - the `name@range` keyed form (the `@range` qualifier is stripped for the
    purl while a scoped package's leading `@` is preserved)
  - nested objects with the `.` self-reference (pins the parent package itself)
    and immediate child overrides

## Scope boundary

npm `overrides` can nest arbitrarily deep to express path-specific transitive
scoping. The parser expands the top level plus the immediate children of a
nested object; deeper nesting is intentionally **not** recursively expanded, to
keep parsing bounded and avoid guessing transitive-scoping semantics that the
flat dependency model cannot faithfully represent.

## Coverage

Parser unit tests cover plain entries, glob/alias resolutions, the `name@range`
override key form, and the nested `.` self-reference. A parser golden locks the
full emitted shape, and a Layer-3 scan/assembly contract test verifies the pins
hoist to top-level dependencies with the expected scope and `is_pinned` flag
while declared dependencies retain their existing intent semantics.
