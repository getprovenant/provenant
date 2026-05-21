#!/bin/bash
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

ROOT_MANIFEST="Cargo.toml"
TAG_NAME="${1:-${GITHUB_REF_NAME:-}}"

if [ -z "$TAG_NAME" ]; then
    echo "Usage: ./scripts/check_release_tag_sync.sh <vX.Y.Z>"
    echo "Or set GITHUB_REF_NAME in the environment."
    exit 1
fi

python3 - "$ROOT_MANIFEST" "$TAG_NAME" <<'PY'
import pathlib
import re
import sys

root_manifest = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
tag_name = sys.argv[2]

if tag_name.startswith("refs/tags/"):
    tag_name = tag_name.removeprefix("refs/tags/")

root_version_match = re.search(r'^version = "([^"]+)"$', root_manifest, re.MULTILINE)
if root_version_match is None:
    raise SystemExit("Could not determine root crate version from Cargo.toml")

root_version = root_version_match.group(1)
expected_tag = f"v{root_version}"

if tag_name != expected_tag:
    raise SystemExit(
        "Release tag is out of sync with Cargo.toml: "
        f"expected {expected_tag}, got {tag_name}.\n"
        "Create a tag that exactly matches the crate version."
    )
PY
