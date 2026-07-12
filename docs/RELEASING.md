# Releasing Provenant

This guide documents the maintainer release flow for `provenant`.

## Overview

Releases are split into two phases:

1. local release preparation with `release.sh`, which refreshes the embedded license data, runs the release-time sync checks, prepares the release commit, and pushes the release tag
2. tag-triggered GitHub Actions publication, which creates the GitHub Release assets, publishes `provenant-cli` to crates.io via trusted publishing, and pushes a container image to GHCR

The published crate name is `provenant-cli`, while the installed binary and product name remain `provenant` / Provenant.

## Prerequisites

Before cutting a release, make sure you have:

- A clean working tree
- The `reference/scancode-toolkit/` submodule initialized via `./setup.sh`
- `cargo-release` installed locally
- GPG signing configured for git tags
- A green `CI` workflow run on `main` before you start release prep

For the normal release path, you do **not** need `cargo login` on your local machine. crates.io authentication is handled by the tag-triggered GitHub Actions trusted publishing flow, which verifies that the tagged commit is reachable from `origin/main` before it can mint the short-lived crates.io token.

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
4. syncs the root package version in `Cargo.lock` without re-resolving dependencies, then verifies release version sync after `cargo release` updates versioned files
5. in `--execute` mode, commits any license-data refresh with `git commit -s`
6. runs the local `cargo release` flow for versioning, tagging, and pushing

The exact `cargo release` behavior comes from `[package.metadata.release]` in `Cargo.toml`, including the `CITATION.cff` version replacement, `Cargo.lock` root-package version sync without dependency re-resolution, signed tag creation, and push behavior. The release commit written by `release.sh` stays versionless (`chore: release`) and DCO-signed.

## GitHub Release Automation

Pushing the `vX.Y.Z` tag triggers `.github/workflows/release.yml`.

That workflow:

- verifies release invariants on the tagged commit, including version/tag alignment and crates.io dry-run packaging
- Re-runs embedded license index verification as a final release-time safeguard before building artifacts
- Builds release binaries for Linux, macOS (Intel and Apple Silicon), and Windows
- Packages each build under the `provenant-<platform>-<arch>` naming scheme
- Generates SHA256 checksum files
- Creates a GitHub Release, attaches SLSA build-provenance attestation for the release archives, and uploads all generated assets
- Publishes `provenant-cli` to crates.io via trusted publishing
- Publishes a multi-arch container image to `ghcr.io/getprovenant/provenant`, with its own SLSA build-provenance attestation
- On a stable (non-prerelease) tag, bumps the Homebrew formula in [`getprovenant/homebrew-tap`](https://github.com/getprovenant/homebrew-tap) to the new version and checksums

The GitHub Release (binaries + attestation) is produced independently of the crates.io and GHCR publishes: those jobs run in parallel and depend only on the build, so a crates.io or GHCR outage does not fail or skip the Release.

If the tag contains `-`, GitHub marks the release as a prerelease.

The Homebrew bump renders [`.github/homebrew/provenant.rb.tmpl`](../.github/homebrew/provenant.rb.tmpl) with the release version and per-arch checksums, validates it, and commits it directly to the tap. It authenticates with the `provenant-release-bot` GitHub App (via the `RELEASE_BOT_APP_ID` and `RELEASE_BOT_PRIVATE_KEY` repository secrets); the app is installed on the tap repo with only `contents: write`.

## After Starting the Release

Monitor the [GitHub Actions release workflow](https://github.com/getprovenant/provenant/actions) and the resulting [GitHub Releases page](https://github.com/getprovenant/provenant/releases).

Verify:

- The tag and release commit are present on the remote
- The GitHub Release contains all expected Linux, macOS (Intel and Apple Silicon), and Windows archives and checksum files
- The crates.io publish job succeeded and the container image was pushed to GHCR

The crates.io and GHCR publish jobs are independent of the GitHub Release: if one fails, rerun only that failed job for the same version. The Release itself does not depend on them, so a publish failure never requires re-cutting the Release.

## Common Failure Points

- Missing submodule setup: run `./setup.sh`
- Missing GPG configuration: `cargo release` cannot create the signed tag
- Dirty working tree: clean up local changes before retrying
- Release tag is not reachable from `main`
- Release tag does not match the crate version in `Cargo.toml`
