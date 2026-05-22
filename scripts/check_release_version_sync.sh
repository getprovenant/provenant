#!/bin/bash
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

ROOT_MANIFEST="Cargo.toml"
XTASK_LOCKFILE_CANDIDATE="xtask/Cargo.lock"
WORKSPACE_LOCKFILE="Cargo.lock"
CITATION_FILE="CITATION.cff"
UPDATE_LOCKFILE="${1:-}"

python3 - "$ROOT_MANIFEST" "$XTASK_LOCKFILE_CANDIDATE" "$WORKSPACE_LOCKFILE" "$CITATION_FILE" "$UPDATE_LOCKFILE" <<'PY'
import pathlib
import re
import sys

root_manifest = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
lockfile_candidate_path = pathlib.Path(sys.argv[2])
workspace_lockfile_path = pathlib.Path(sys.argv[3])
citation_file_path = pathlib.Path(sys.argv[4])
update_lockfile = sys.argv[5] == "--update-lockfile"


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

root_version_match = re.search(r'^version = "([^"]+)"$', root_manifest, re.MULTILINE)
if root_version_match is None:
    raise SystemExit("Could not determine root crate version from Cargo.toml")

root_version = root_version_match.group(1)

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
