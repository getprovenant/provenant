#!/bin/bash
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0

# Verify that the upstream ScanCode Toolkit attribution notices Provenant
# retains are reproduced verbatim in BOTH the pinned submodule NOTICE and the
# repository-root NOTICE. Anchored on the retained segments only, so notices we
# intentionally omit (e.g. upstream's "Third-party software licenses" section)
# never affect the result.
#
# Usage: ./scripts/check_notice_attribution_sync.sh [--require-submodule]
#   --require-submodule: fail (instead of soft-skip) when the submodule NOTICE
#                        is missing. Used by CI and release.sh.

set -euo pipefail

REQUIRE_SUBMODULE=""
if [ "${1:-}" = "--require-submodule" ]; then
    REQUIRE_SUBMODULE="1"
fi

UPSTREAM_NOTICE="reference/scancode-toolkit/NOTICE"
LOCAL_NOTICE="NOTICE"
SEGMENTS="scripts/notice_retained_segments.txt"

if [ ! -f "$UPSTREAM_NOTICE" ]; then
    if [ -n "$REQUIRE_SUBMODULE" ]; then
        echo "Could not find upstream NOTICE at $UPSTREAM_NOTICE." >&2
        echo "Initialize the reference/scancode-toolkit submodule first (npm run setup)." >&2
        exit 1
    fi
    echo "ℹ️  Skipping NOTICE attribution check: $UPSTREAM_NOTICE not present (submodule not initialized)."
    exit 0
fi

for required in "$LOCAL_NOTICE" "$SEGMENTS"; do
    if [ ! -f "$required" ]; then
        echo "Could not find required file: $required" >&2
        exit 1
    fi
done

python3 - "$UPSTREAM_NOTICE" "$LOCAL_NOTICE" "$SEGMENTS" <<'PY'
import pathlib
import sys

upstream_path = pathlib.Path(sys.argv[1])
local_path = pathlib.Path(sys.argv[2])
segments_path = pathlib.Path(sys.argv[3])

BEGIN = "@@BEGIN@@"
END = "@@END@@"


def normalize(text):
    # Strip trailing whitespace per line (upstream NOTICE carries a few trailing
    # spaces; our formatting tooling may not). Leading indentation is preserved
    # because it is part of the reproduced notice.
    return "\n".join(line.rstrip() for line in text.splitlines())


def parse_segments(text):
    segments = []
    current = None
    for line in text.splitlines():
        stripped = line.strip()
        if stripped == BEGIN:
            if current is not None:
                raise SystemExit(f"Nested {BEGIN} marker in {segments_path}")
            current = []
        elif stripped == END:
            if current is None:
                raise SystemExit(f"Unmatched {END} marker in {segments_path}")
            # Drop blank lines (never content lines) immediately inside the
            # markers, so the fragment can be authored with readable spacing.
            # Blank-line context around a block is intentionally not asserted:
            # the check guarantees the verbatim notice text, not surrounding
            # whitespace, which carries no attribution meaning.
            while current and not current[0].strip():
                current.pop(0)
            while current and not current[-1].strip():
                current.pop()
            segments.append("\n".join(current))
            current = None
        elif current is not None:
            current.append(line)
    if current is not None:
        raise SystemExit(f"Unterminated {BEGIN} segment in {segments_path}")
    return segments


segments = parse_segments(segments_path.read_text(encoding="utf-8"))
if not segments:
    raise SystemExit(
        f"No retained segments found in {segments_path}; expected at least one "
        f"{BEGIN}/{END} block."
    )

upstream = normalize(upstream_path.read_text(encoding="utf-8"))
local = normalize(local_path.read_text(encoding="utf-8"))

problems = []
for index, segment in enumerate(segments, start=1):
    norm = normalize(segment)
    preview = norm.splitlines()[0] if norm.splitlines() else "(empty)"
    if norm not in upstream:
        problems.append(
            f"Segment {index} (\"{preview} ...\") is NOT a verbatim match in "
            f"{upstream_path}.\n"
            "  The pinned upstream NOTICE changed this retained notice. Re-sync "
            f"both {segments_path} and {local_path} with the new upstream text."
        )
    if norm not in local:
        problems.append(
            f"Segment {index} (\"{preview} ...\") is NOT reproduced verbatim in "
            f"{local_path}.\n"
            f"  The root NOTICE drifted from {segments_path}. Restore the exact "
            "upstream notice text in NOTICE."
        )

if problems:
    raise SystemExit(
        "NOTICE attribution sync check failed:\n\n"
        + "\n\n".join(problems)
        + "\n\nApache-2.0 section 4(d) requires a faithful copy of retained "
        "attribution notices. See scripts/notice_retained_segments.txt for the "
        "canonical segments."
    )

print(
    f"✅ NOTICE attribution in sync: {len(segments)} retained upstream "
    "segment(s) match both the submodule NOTICE and the root NOTICE."
)
PY
