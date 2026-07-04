#!/bin/bash
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

# Enforces that four version facts stay consistent, treating rust-toolchain.toml
# as the single human-edited source of truth for the Rust version:
#   1. Cargo.toml `rust-version` (MSRV) matches the pinned toolchain channel.
#   2. The lockfile's `provenant-cli` version matches the Cargo.toml root version.
#   3. CITATION.cff version matches the Cargo.toml root version.
#
# The no-argument invocation is a read-only gate run in CI. `--update-lockfile`
# is passed by the cargo-release pre-release hook to rewrite the synced values.
#
# Note on overlap with Renovate: the `rust`/Cargo.toml `rust-version` custom
# manager in renovate.json bumps MSRV alongside the toolchain so routine bumps
# do not fail check (1). That is best-effort prevention at PR-creation time;
# this script remains the enforcement gate and additionally owns checks (2) and
# (3), which Renovate does not touch. Keep both.

ROOT_MANIFEST="Cargo.toml"
XTASK_LOCKFILE_CANDIDATE="xtask/Cargo.lock"
WORKSPACE_LOCKFILE="Cargo.lock"
CITATION_FILE="CITATION.cff"
RUST_TOOLCHAIN_FILE="rust-toolchain.toml"

update_lockfile="false"
for arg in "$@"; do
    case "$arg" in
        --update-lockfile) update_lockfile="true" ;;
        *)
            echo "Unknown argument: $arg" >&2
            echo "Usage: $0 [--update-lockfile]" >&2
            exit 2
            ;;
    esac
done

python3 - \
    "$ROOT_MANIFEST" \
    "$XTASK_LOCKFILE_CANDIDATE" \
    "$WORKSPACE_LOCKFILE" \
    "$CITATION_FILE" \
    "$RUST_TOOLCHAIN_FILE" \
    "$update_lockfile" <<'PY'
import pathlib
import re
import sys

root_manifest_path = pathlib.Path(sys.argv[1])
lockfile_candidate_path = pathlib.Path(sys.argv[2])
workspace_lockfile_path = pathlib.Path(sys.argv[3])
citation_file_path = pathlib.Path(sys.argv[4])
rust_toolchain_path = pathlib.Path(sys.argv[5])
update_lockfile = sys.argv[6] == "true"

root_manifest = root_manifest_path.read_text(encoding="utf-8")


def replace_root_package_version(lockfile_contents: str, root_version: str) -> str:
    blocks = lockfile_contents.split("[[package]]")
    updated_blocks = [blocks[0]]
    updated = False

    for block in blocks[1:]:
        if not updated and 'name = "provenant-cli"' in block:
            new_block, count = re.subn(
                r'(^version = ")([^"]+)("$)',
                rf'\g<1>{root_version}\g<3>',
                block,
                count=1,
                flags=re.MULTILINE,
            )

            if count == 0:
                raise SystemExit(
                    "Could not update provenant-cli version in lockfile"
                )

            block = new_block
            updated = True

        updated_blocks.append("[[package]]" + block)

    if not updated:
        raise SystemExit("Could not determine provenant-cli version from lockfile")

    return "".join(updated_blocks)


def toolchain_msrv(toolchain_contents: str) -> str:
    channel_match = re.search(
        r'^channel\s*=\s*"([^"]+)"$', toolchain_contents, re.MULTILINE
    )
    if channel_match is None:
        raise SystemExit("Could not determine channel from rust-toolchain.toml")

    channel = channel_match.group(1)
    version_match = re.match(r"^(\d+)\.(\d+)(?:\.\d+)?$", channel)
    if version_match is None:
        raise SystemExit(
            f"rust-toolchain.toml channel '{channel}' is not a pinned x.y[.z] version; "
            "rust-version sync only supports a pinned numeric channel."
        )

    return f"{version_match.group(1)}.{version_match.group(2)}"


if lockfile_candidate_path.exists():
    lockfile_path = lockfile_candidate_path
    lockfile_label = str(lockfile_candidate_path)
elif workspace_lockfile_path.exists():
    lockfile_path = workspace_lockfile_path
    lockfile_label = str(workspace_lockfile_path)
else:
    raise SystemExit(
        "Could not find xtask/Cargo.lock or workspace Cargo.lock for sync check"
    )

lockfile_contents = lockfile_path.read_text(encoding="utf-8")
citation_file = citation_file_path.read_text(encoding="utf-8")
toolchain_contents = rust_toolchain_path.read_text(encoding="utf-8")

root_version_match = re.search(r'^version = "([^"]+)"$', root_manifest, re.MULTILINE)
if root_version_match is None:
    raise SystemExit("Could not determine root crate version from Cargo.toml")

root_version = root_version_match.group(1)

# Keep Cargo.toml `rust-version` derived from the pinned toolchain so
# rust-toolchain.toml stays the single human-edited source of truth.
expected_msrv = toolchain_msrv(toolchain_contents)

rust_version_match = re.search(
    r'^rust-version = "([^"]+)"$', root_manifest, re.MULTILINE
)
if rust_version_match is None:
    raise SystemExit(
        "Could not determine rust-version from Cargo.toml; expected a "
        '`rust-version = "x.y"` line in [package].'
    )

current_msrv = rust_version_match.group(1)

if update_lockfile and current_msrv != expected_msrv:
    root_manifest = (
        root_manifest[: rust_version_match.start(1)]
        + expected_msrv
        + root_manifest[rust_version_match.end(1) :]
    )
    root_manifest_path.write_text(root_manifest, encoding="utf-8")
    current_msrv = expected_msrv

if current_msrv != expected_msrv:
    raise SystemExit(
        "Cargo.toml rust-version is out of sync with rust-toolchain.toml: "
        f"rust-version is {current_msrv}, toolchain channel implies {expected_msrv}.\n"
        "Refresh it with: ./scripts/check_release_version_sync.sh --update-lockfile"
    )

if update_lockfile:
    updated_lockfile_contents = replace_root_package_version(lockfile_contents, root_version)

    if updated_lockfile_contents != lockfile_contents:
        lockfile_path.write_text(updated_lockfile_contents, encoding="utf-8")

    lockfile_contents = updated_lockfile_contents

lockfile_version = None
for block in lockfile_contents.split("[[package]]"):
    if 'name = "provenant-cli"' not in block:
        continue
    version_match = re.search(r'^version = "([^"]+)"$', block, re.MULTILINE)
    if version_match is not None:
        lockfile_version = version_match.group(1)
        break

if lockfile_version is None:
    raise SystemExit(f"Could not determine provenant-cli version from {lockfile_label}")

if root_version != lockfile_version:
    raise SystemExit(
        f"{lockfile_label} is out of sync with Cargo.toml: "
        f"root crate is {root_version}, lockfile has {lockfile_version}.\n"
        "Refresh it with: ./scripts/check_release_version_sync.sh --update-lockfile"
    )

citation_version_match = re.search(r'^version: "([^"]+)"$', citation_file, re.MULTILINE)
if citation_version_match is None:
    raise SystemExit("Could not determine CITATION.cff version")

citation_version = citation_version_match.group(1)

if root_version != citation_version:
    raise SystemExit(
        "CITATION.cff is out of sync with Cargo.toml: "
        f"root crate is {root_version}, CITATION.cff has {citation_version}.\n"
        "Refresh it with: update CITATION.cff or run the cargo-release flow that rewrites it."
    )
PY
