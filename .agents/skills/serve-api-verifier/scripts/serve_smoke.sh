#!/usr/bin/env bash
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

base_url="${1:-http://127.0.0.1:8080}"

require_jq() {
    if ! command -v jq >/dev/null 2>&1; then
        printf 'serve_smoke.sh requires jq for response assertions\n' >&2
        exit 127
    fi
}

assert_json() {
    local path="$1"
    local filter="$2"

    curl -fsS "${base_url}${path}" | jq -e "$filter" >/dev/null
    printf 'ok %s\n' "$path"
}

require_jq
assert_json "/livez" '.status == "ok"'
assert_json "/readyz" '.status == "ready" and .api_version == "v1"'
assert_json "/version" '.service == "provenant-serve" and .api_version == "v1" and (.tool_version | type == "string")'
