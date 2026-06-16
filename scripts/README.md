# Shell Scripts Documentation

This directory now contains only real shell helpers.

Rust-based maintainer commands such as `benchmark-target`, `compare-outputs`,
`update-parser-golden`, `update-license-golden`, `update-copyright-golden`,
`validate-urls`, `generate-supported-formats`, and `generate-index-artifact`
are documented in [`../xtask/README.md`](../xtask/README.md).

The standalone SPDX header checker lives in
[`../tools/license-headers/README.md`](../tools/license-headers/README.md).

## `sync_agent_skills.sh`

Sync the canonical `.agents/skills/` tree to `.claude/skills/` for Claude Code,
which does not currently document `.agents/skills/` discovery and has unreliable
symlink discovery reports. Use `--check` to verify the mirror without changing it.

Examples:

```bash
./scripts/sync_agent_skills.sh
./scripts/sync_agent_skills.sh --check
```

## `cargo_sort_manifests.sh`

Sort Cargo manifest sections with `cargo-sort`.

Examples:

```bash
./scripts/cargo_sort_manifests.sh
./scripts/cargo_sort_manifests.sh --check
./scripts/cargo_sort_manifests.sh Cargo.toml tools/license-headers/Cargo.toml xtask/Cargo.toml
```

## `check_unused_deps.sh`

Run `cargo-machete` against the root workspace plus the standalone
`tools/license-headers/` and `xtask/` manifests.

Example:

```bash
./scripts/check_unused_deps.sh
```

## `check_dependency_policy.sh`

Run `cargo-deny` against the shipped workspace dependency graph using the
repo-root `deny.toml` policy.

Example:

```bash
./scripts/check_dependency_policy.sh
```

## `check_dco_signoff.sh`

Validate that a commit message includes a Developer Certificate of Origin (DCO)
sign-off trailer.

Examples:

```bash
./scripts/check_dco_signoff.sh --commit-msg-file .git/COMMIT_EDITMSG
```

## `check_crate_size.sh`

Package the crate locally and fail if the resulting `.crate` archive exceeds the
crates.io size limit.

Example:

```bash
./scripts/check_crate_size.sh
```

## `check_release_version_sync.sh`

Verify that the crate version in `Cargo.toml`, the packaged `provenant-cli`
entry in the lockfile, and `CITATION.cff` all stay aligned for releases.

Use `--update-lockfile` to rewrite only the root package version in the lockfile
without re-resolving dependencies.

Example:

```bash
./scripts/check_release_version_sync.sh
./scripts/check_release_version_sync.sh --update-lockfile
```

## `check_release_tag_sync.sh`

Verify that a release tag matches the root crate version in `Cargo.toml`.

Examples:

```bash
./scripts/check_release_tag_sync.sh v0.1.1
GITHUB_REF_NAME=v0.1.1 ./scripts/check_release_tag_sync.sh
```

## `check_scancode_output_format_sync.sh`

Verify that Provenant's `OUTPUT_FORMAT_VERSION` stays aligned with the pinned
`reference/scancode-toolkit/` submodule.

Example:

```bash
./scripts/check_scancode_output_format_sync.sh
```

## `generate_third_party_notices.sh`

Generate the third-party license disclosure (`THIRD-PARTY-NOTICES.md`) for the
dependencies bundled in the Provenant binary, using `cargo-about` and the
repo-root `about.toml` / `about.hbs`. The release workflow generates this file
and bundles it into the binary release archives alongside `LICENSE` and
`NOTICE`; `release.sh` validates it generates cleanly before tagging.

Requires `cargo-about` (`cargo install --locked cargo-about --features cli`). It
fails if a bundled dependency uses a license not listed in `about.toml`'s
`accepted` set, surfacing new transitive licenses for review.

Example:

```bash
./scripts/generate_third_party_notices.sh
./scripts/generate_third_party_notices.sh THIRD-PARTY-NOTICES.md
```

## `check_license_config_sync.sh`

Verify that `about.toml` (cargo-about, third-party disclosure) stays aligned
with `deny.toml` (cargo-deny, license policy). The two tools evaluate the same
shipped dependency graph but cannot share configuration, so this enforces the
invariant: `about.toml` `targets` must equal `deny.toml` `[graph].targets`, and
`about.toml` `accepted` must equal `deny.toml` `[licenses].allow` plus its
per-crate exception licenses.

Example:

```bash
./scripts/check_license_config_sync.sh
```

## `check_notice_attribution_sync.sh`

Verify that the upstream ScanCode Toolkit attribution notices Provenant retains
(Apache-2.0 section 4(d) / CC-BY-4.0) are reproduced verbatim in both the pinned
`reference/scancode-toolkit/NOTICE` and the repository-root `NOTICE`. The
canonical retained segments live in
[`notice_retained_segments.txt`](notice_retained_segments.txt); the check is
anchored on those segments only, so notices we intentionally omit (such as
upstream's "Third-party software licenses" section) do not affect it.

Pass `--require-submodule` to fail (instead of soft-skip) when the submodule
NOTICE is missing; CI and `release.sh` use this so a submodule bump that changes
a retained notice is caught before a release is tagged. If upstream _adds_ a new
notice that pertains to the code or data Provenant distributes, add a matching
segment and reproduce it in `NOTICE` — that case is a human judgment, not
auto-detected.

Example:

```bash
./scripts/check_notice_attribution_sync.sh
./scripts/check_notice_attribution_sync.sh --require-submodule
```
