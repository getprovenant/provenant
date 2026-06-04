#!/usr/bin/env bash
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

mode="${1:-sync}"
canonical=".agents/skills"
claude=".claude/skills"

if [[ "${mode}" != "sync" && "${mode}" != "--check" ]]; then
    printf 'usage: %s [sync|--check]\n' "$0" >&2
    exit 2
fi

if [[ ! -d "${canonical}" ]]; then
    printf 'canonical skills directory is missing: %s\n' "${canonical}" >&2
    exit 1
fi

if [[ "${mode}" == "--check" ]]; then
    if [[ ! -d "${claude}" ]]; then
        printf 'Claude skills mirror is missing: %s\n' "${claude}" >&2
        exit 1
    fi
    diff -qr "${canonical}" "${claude}"
    printf 'Claude skills mirror is up to date\n'
    exit 0
fi

mkdir -p "$(dirname "${claude}")"
rm -rf "${claude}"
mkdir -p "${claude}"
cp -R "${canonical}/." "${claude}/"
printf 'Synced %s -> %s\n' "${canonical}" "${claude}"
