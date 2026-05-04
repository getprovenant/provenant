# Releasing Provenant

This guide documents the maintainer release flow for `provenant`.

## Overview

Releases are driven locally with `release.sh`, which wraps `cargo release`, refreshes the embedded license data, and checks for ScanCode output-format drift before publishing.

The published crate name is `provenant-cli`, while the installed binary and product name remain `provenant` / Provenant.

## Prerequisites

Before cutting a release, make sure you have:

- A clean working tree
- The `reference/scancode-toolkit/` submodule initialized via `./setup.sh`
- `cargo-release` installed locally
- A valid crates.io login in your Cargo credentials
- GPG signing configured for git tags
- A green `CI` workflow run for the exact commit you plan to release

Install `cargo-release` if needed:

```sh
cargo install cargo-release
```

Authenticate with crates.io if needed:

```sh
cargo login
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
4. in `--execute` mode, commits any license-data refresh with `git commit -s`
5. runs the `cargo release` flow for versioning, publishing, tagging, and pushing

The exact `cargo release` behavior comes from `[package.metadata.release]` in `Cargo.toml`, including the `CITATION.cff` version replacement, `Cargo.lock` regeneration, signed tag creation, and publish/push behavior. The release commit written by `release.sh` stays versionless (`chore: release`) and DCO-signed.

## GitHub Release Automation

Pushing the `vX.Y.Z` tag triggers `.github/workflows/release.yml`.

That workflow:

- Builds release binaries for Linux, macOS (Intel and Apple Silicon), and Windows
- Re-runs embedded license index verification as a final release-time safeguard before building artifacts
- Packages each build under the `provenant-<platform>-<arch>` naming scheme
- Generates SHA256 checksum files
- Creates a GitHub Release and uploads all generated assets

If the tag contains `-`, GitHub marks the release as a prerelease.

## After Starting the Release

Monitor the [GitHub Actions release workflow](https://github.com/mstykow/provenant/actions) and the resulting [GitHub Releases page](https://github.com/mstykow/provenant/releases).

Verify:

- The crates.io publish step succeeded
- The tag and release commit are present on the remote
- The GitHub Release contains all expected Linux, macOS (Intel and Apple Silicon), and Windows archives and checksum files

## Common Failure Points

- Missing submodule setup: run `./setup.sh`
- Missing crates.io credentials: run `cargo login`
- Missing GPG configuration: `cargo release` cannot create the signed tag
- Dirty working tree: clean up local changes before retrying
