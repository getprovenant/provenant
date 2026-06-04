# Overlay Gotchas

Use this reference before editing `resources/license_detection/overlay/` or `resources/license_detection/index_build_policy.toml`.

## Failure Modes

- **Missing overlay reason**: every downstream overlay needs a rationale in the build policy so future maintainers know why it exists.
- **Stale overlay reason**: if an overlay file is removed or renamed, remove the corresponding policy rationale.
- **Overlay identical to upstream**: if upstream absorbs the curation, delete the downstream overlay instead of shipping dead policy.
- **Stale ignore ID**: ignored upstream rule/license IDs must still exist upstream. Remove policy entries when the upstream ID disappears.
- **Wrong fix layer**: one rule's semantics usually belongs in overlay data; broad matcher behavior belongs in Rust detection code.
- **Artifact not regenerated**: overlay and policy changes are incomplete until `resources/license_detection/license_index.zst` is regenerated and checked.

## Curation Checklist

1. Identify whether the issue is rule/license data or engine behavior.
2. Add or update the `.RULE` / `.LICENSE` overlay when data curation is the right fix.
3. Add or update the policy rationale in `index_build_policy.toml`.
4. Run `cargo run --manifest-path xtask/Cargo.toml --bin generate-index-artifact`.
5. Run the narrow license-detection test or golden that proves the change.
6. Inspect generated artifact and expected-output diffs before committing.

## Do Not

- Do not tune Rust matcher code for a one-rule semantic issue.
- Do not keep compatibility defaults that are not proven by the dataset.
- Do not make tests depend directly on `reference/scancode-toolkit/` fixture paths.
