#!/bin/bash
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0

# Verify that about.toml (cargo-about, third-party disclosure generation) stays
# aligned with deny.toml (cargo-deny, license policy). The two tools evaluate
# the same shipped dependency graph but cannot share configuration, so this
# check enforces the invariant instead:
#   - about.toml `targets`  == deny.toml [graph].targets
#   - about.toml `accepted` == deny.toml [licenses].allow + per-crate exception licenses

set -euo pipefail

python3 - <<'PY'
import tomllib

with open("about.toml", "rb") as f:
    about = tomllib.load(f)
with open("deny.toml", "rb") as f:
    deny = tomllib.load(f)

problems = []

about_targets = set(about.get("targets", []))
deny_targets = set(deny.get("graph", {}).get("targets", []))
if about_targets != deny_targets:
    problems.append(
        "targets differ:\n"
        f"  only in about.toml: {sorted(about_targets - deny_targets)}\n"
        f"  only in deny.toml:  {sorted(deny_targets - about_targets)}"
    )

about_accepted = set(about.get("accepted", []))
deny_licenses = deny.get("licenses", {})
expected = set(deny_licenses.get("allow", []))
for exc in deny_licenses.get("exceptions", []):
    expected.update(exc.get("allow", []))
if about_accepted != expected:
    problems.append(
        "accepted vs (deny allow + exceptions) differ:\n"
        f"  only in about.toml accepted:        {sorted(about_accepted - expected)}\n"
        f"  only in deny.toml allow/exceptions: {sorted(expected - about_accepted)}"
    )

if problems:
    raise SystemExit(
        "License config out of sync between about.toml and deny.toml:\n\n"
        + "\n\n".join(problems)
        + "\n\nKeep cargo-about (disclosure) aligned with cargo-deny (policy):\n"
        "  - about.toml `targets`  must equal deny.toml [graph].targets\n"
        "  - about.toml `accepted` must equal deny.toml [licenses].allow + exception licenses"
    )

print(
    "✅ license config in sync: about.toml matches deny.toml "
    "(targets + accepted/allow)."
)
PY
