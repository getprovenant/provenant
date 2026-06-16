#!/bin/bash
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0

# Generate the third-party license disclosure for the dependencies bundled in
# the Provenant binary into THIRD-PARTY-NOTICES.md, using cargo-about and the
# repo-root about.toml / about.hbs.
#
# Requires cargo-about:
#   cargo install --locked cargo-about --features cli
#
# Usage: ./scripts/generate_third_party_notices.sh [output-file]

set -euo pipefail

OUTPUT="${1:-THIRD-PARTY-NOTICES.md}"

if ! cargo about --version >/dev/null 2>&1; then
    echo "cargo-about not found. Install with:" >&2
    echo "  cargo install --locked cargo-about --features cli" >&2
    exit 1
fi

# --fail turns license problems (e.g. a dependency whose license is not in the
# about.toml accepted list) into a non-zero exit so they surface for review.
cargo about generate --fail -c about.toml about.hbs -o "$OUTPUT"
echo "Wrote $OUTPUT"
