# Releasing Provenant

This guide documents the maintainer release flow for `provenant`.

## Overview

Releases are split into two phases:

1. local release preparation with `release.sh`, which refreshes the embedded license data, runs the release-time sync checks, writes the release commit, and pushes the release tag
2. tag-triggered GitHub Actions publication, which publishes `provenant-cli` to crates.io via trusted publishing and creates the GitHub Release assets

The published crate name is `provenant-cli`, while the installed binary and product name remain `provenant` / Provenant.

## Prerequisites

Before cutting a release, make sure you have:

- A clean working tree
- The `reference/scancode-toolkit/` submodule initialized via `./setup.sh`
- `cargo-release` installed locally
- GPG signing configured for git tags
- A green `CI` workflow run on `main` before you start release prep

For the normal release path, you do **not** need `cargo login` on your local machine. crates.io authentication is handled in GitHub Actions through trusted publishing.

The tag-triggered publish job verifies that the tagged commit is reachable from `origin/main` before it can mint the short-lived crates.io token.

Install `cargo-release` if needed:

```sh
cargo install cargo-release
```

## Preflight Checks

The primary pre-release quality gate is the GitHub `CI` workflow in `.github/workflows/check.yml`. Start from a commit where that workflow is already green.

Use targeted local checks only when you need extra confidence before tagging. For example, `npm run check:docs`, `npm run validate:urls`, or a focused Rust test command can help verify the specific area you just changed without duplicating the full CI matrix locally.

## Release Commands

Always start with a dry run:

```sh
./release.sh patch
```

When the dry run looks correct, perform the real release:

```sh
./release.sh patch --execute
```

Supported release types:

- `patch` updates `X.Y.Z` to `X.Y.(Z+1)`
- `minor` updates `X.Y.Z` to `X.(Y+1).0`
- `major` updates `X.Y.Z` to `(X+1).0.0`

## What `release.sh` Does

On every release attempt, the script:

1. verifies a clean working tree and initialized ScanCode reference submodule
2. updates the pinned ScanCode checkout from `origin/develop` and regenerates the embedded license index artifact
3. checks ScanCode output-format version sync before continuing
4. runs the release version sync check after `cargo release` updates versioned files
5. in `--execute` mode, commits any license-data refresh with `git commit -s`
6. runs the local `cargo release` flow for versioning, tagging, and pushing

The exact `cargo release` behavior comes from `[package.metadata.release]` in `Cargo.toml`, including the `CITATION.cff` version replacement, `Cargo.lock` regeneration, signed tag creation, and push behavior. The release commit written by `release.sh` stays versionless (`chore: release`) and DCO-signed.

## One-Time Trusted Publishing Setup

Before relying on the automated publish step, configure a trusted publisher for the `provenant-cli` crate on crates.io:

1. Open the `provenant-cli` crate settings on crates.io.
2. Add a GitHub Actions trusted publisher for:
   - owner: `mstykow`
   - repository: `provenant`
   - workflow file: `.github/workflows/release.yml`
3. Protect release tags so only the small maintainer set can create or update `v*` tags.

The crate already exists on crates.io, so this is a settings change, not a first-publish migration.

## GitHub Release Automation

Pushing the `vX.Y.Z` tag triggers `.github/workflows/release.yml`.

That workflow:

- verifies release invariants on the tagged commit, including version/tag alignment and crates.io dry-run packaging
- Builds release binaries for Linux, macOS (Intel and Apple Silicon), and Windows
- publishes `provenant-cli` to crates.io via trusted publishing
- Re-runs embedded license index verification as a final release-time safeguard before building artifacts
- Packages each build under the `provenant-<platform>-<arch>` naming scheme
- Generates SHA256 checksum files
- Creates a GitHub Release and uploads all generated assets

If the tag contains `-`, GitHub marks the release as a prerelease.

## After Starting the Release

Monitor the [GitHub Actions release workflow](https://github.com/mstykow/provenant/actions) and the resulting [GitHub Releases page](https://github.com/mstykow/provenant/releases).

Verify:

- The crates.io publish job in the GitHub Actions release workflow succeeded
- The tag and release commit are present on the remote
- The GitHub Release contains all expected Linux, macOS (Intel and Apple Silicon), and Windows archives and checksum files

If the GitHub Release asset step fails after crates.io publish has already succeeded, rerun only the failed downstream jobs. Do not rerun the successful crates.io publish job for the same version.

## Common Failure Points

- Missing submodule setup: run `./setup.sh`
- Missing GPG configuration: `cargo release` cannot create the signed tag
- Dirty working tree: clean up local changes before retrying
- Missing crates.io trusted publisher configuration for `provenant-cli`
- Release tag is not reachable from `main`
- Release tag does not match the crate version in `Cargo.toml`
