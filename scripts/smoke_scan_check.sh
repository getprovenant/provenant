#!/usr/bin/env bash
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0

#
# Cross-platform behavioral smoke check.
#
# Scans a tiny, EOL-pinned fixture and diffs a normalized projection of the scan
# output against a committed golden. This runs in the Windows, macOS, and static
# musl CI smoke jobs so platform-specific behavior differences — output path
# separators, file_type/mime detection, and copyright/license line spans — surface
# in CI instead of only when a release ships. The build is already paid for by the
# surrounding smoke job, so this adds only a few seconds.
#
# The projection drops volatile fields (headers with tool version/timings, and the
# per-file mtime `date`) and keeps the deterministic, platform-sensitive ones. The
# fixture is pinned to LF via .gitattributes so sizes/hashes/line spans are identical
# on every OS.
#
# jq and diff are preinstalled on all GitHub-hosted runners.
#
# Usage:
#   scripts/smoke_scan_check.sh [TARGET_TRIPLE]   # verify against the golden
#   UPDATE_GOLDEN=1 scripts/smoke_scan_check.sh   # regenerate the golden
set -euo pipefail

target_arg=()
if [ "${1:-}" != "" ]; then
    target_arg=(--target "$1")
fi

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

fixture="testdata/smoke"
golden="testdata/smoke-expected.json"
# Workspace-relative paths (under gitignored target/): a leading-slash absolute
# path would be mangled by Git Bash's MSYS path translation before a native
# Windows provenant.exe sees it.
raw="target/smoke-raw.json"
got="target/smoke-got.json"
mkdir -p target
trap 'rm -f "$raw" "$got"' EXIT

cargo run --quiet --locked ${target_arg[@]+"${target_arg[@]}"} --bin provenant -- \
    scan "$fixture" --license --copyright --info --json-pp "$raw" >/dev/null

# Allowlist projection: deterministic, platform-sensitive fields only.
jq_prog='{files: [.files[] | {
    path, type, name, extension, size, sha1, mime_type, file_type,
    programming_language, is_binary, is_text, is_source, is_script,
    detected_license_expression, detected_license_expression_spdx,
    percentage_of_license_text,
    copyrights: [.copyrights[]? | {copyright, start_line, end_line}],
    holders: [.holders[]? | {holder, start_line, end_line}],
    scan_errors
}] | sort_by(.path)}'

jq -S "$jq_prog" "$raw" > "$got"

if [ "${UPDATE_GOLDEN:-}" = "1" ]; then
    cp "$got" "$golden"
    echo "Updated golden: $golden"
    exit 0
fi

# Highest-value invariant, checked explicitly for a clear failure message: output
# paths must use POSIX '/', never a Windows backslash.
if jq -r '.files[].path' "$got" | grep -q '\\'; then
    echo "FAIL: scan output paths contain backslashes; expected POSIX '/' separators:"
    jq -r '.files[].path' "$got"
    exit 1
fi

if ! diff -u "$golden" "$got"; then
    echo "FAIL: scan projection differs from the golden ($golden)."
    echo "If this change is intentional, regenerate with: UPDATE_GOLDEN=1 scripts/smoke_scan_check.sh"
    exit 1
fi

echo "OK: cross-platform scan smoke check passed."
