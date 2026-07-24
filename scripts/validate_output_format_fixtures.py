#!/usr/bin/env python3
# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0
"""Validate Provenant's SBOM/SPDX output against official external validators.

Provenant's own golden tests (tests/output_format_golden.rs) compare emitted
CycloneDX/SPDX output against checked-in fixtures byte-for-byte (after light
normalization). That proves *stability*, not *spec conformance*: a bug that
changes both the writer and the golden fixture in the same wrong way (element
order, a missing required field, a duplicate id that violates `uniqueItems`,
...) can slip through goldens alone. See https://github.com/getprovenant/provenant/issues/1288.

This script instead runs the actual reference tooling from each spec:

- CycloneDX JSON/XML fixtures are validated against the official CycloneDX
  1.7 JSON Schema / XSD, via the `cyclonedx-python-lib` validators.
- SPDX tag-value fixtures are validated with `pyspdxtools` (spdx-tools),
  which parses and runs full SPDX-2.2/2.3 document validation.

Most checked-in `testdata/output-formats/` fixtures are validated directly.
SPDX RDF has no genuine checked-in RDF/XML fixture (the `.rdf` golden is a
JSON-shaped semantic projection used for internal comparisons, see
tests/output_format_golden.rs), so RDF -- and, as a cheap extra check, the
other formats too -- are also validated end-to-end from a tiny live scan of
testdata/smoke when `--provenant-bin` is given.

Usage:
    python3 scripts/validate_output_format_fixtures.py [--provenant-bin PATH]
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES_DIR = ROOT / "testdata" / "output-formats"
SMOKE_SCAN_ROOT = ROOT / "testdata" / "smoke"

# Real, complete CycloneDX documents. `cyclonedx-expected-without-packages.json`
# is intentionally excluded: it is a partial fixture with `metadata`/
# `serialNumber` normalized away for a narrow unit-test comparison, not a
# realistic full document.
CYCLONEDX_JSON_FIXTURES = [
    "cyclonedx-expected.json",
    "cyclonedx-dependencies-expected.json",
    # Exercises component license evidence (evidence.licenses + occurrences).
    "cyclonedx-evidence-expected.json",
]
CYCLONEDX_XML_FIXTURES = [
    "cyclonedx-expected.xml",
    "cyclonedx-dependencies-expected.xml",
    "cyclonedx-evidence-expected.xml",
]

# `spdx-empty-expected.tv` is intentionally excluded: Provenant emits a
# `# No results for package '...'.` placeholder comment (not a full SPDX
# document) when a scan finds no files, so it is out of scope for spec
# validation.
SPDX_TAGVALUE_FIXTURES = [
    "spdx-simple-expected.tv",
    # Exercises promoted resolved-dependency packages (FilesAnalyzed: false):
    # regression guard for their placement relative to the file section and for
    # WITH-exception license handling. `spdx-simple` has neither.
    "spdx-dependencies-expected.tv",
]

CYCLONEDX_SPEC_VERSION = "1.7"


def report_ok(label: str) -> None:
    print(f"OK   {label}")


def report_fail(errors: list[str], label: str, detail: str) -> None:
    message = f"{label}: {detail}"
    errors.append(message)
    print(f"FAIL {message}", file=sys.stderr)


def cyclonedx_error_detail(error) -> str:
    return str(getattr(error.data, "message", error.data))


def validate_cyclonedx_json(path: Path, errors: list[str], label: str) -> None:
    from cyclonedx.schema import OutputFormat, SchemaVersion
    from cyclonedx.validation import make_schemabased_validator

    validator = make_schemabased_validator(OutputFormat.JSON, SchemaVersion.V1_7)
    error = validator.validate_str(path.read_text())
    if error is None:
        report_ok(label)
    else:
        report_fail(errors, label, cyclonedx_error_detail(error))


def validate_cyclonedx_xml(path: Path, errors: list[str], label: str) -> None:
    from cyclonedx.schema import OutputFormat, SchemaVersion
    from cyclonedx.validation import make_schemabased_validator

    validator = make_schemabased_validator(OutputFormat.XML, SchemaVersion.V1_7)
    error = validator.validate_str(path.read_text())
    if error is None:
        report_ok(label)
    else:
        report_fail(errors, label, cyclonedx_error_detail(error))


def validate_spdx_tagvalue(path: Path, errors: list[str], label: str, tmp_dir: Path) -> None:
    # pyspdxtools infers the format from the file extension; copy to a
    # recognized `.spdx` name so the real tag-value parser + validator run.
    staged = tmp_dir / f"{path.stem}.spdx"
    shutil.copy(path, staged)
    result = subprocess.run(
        ["pyspdxtools", "-i", str(staged)],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        report_ok(label)
    else:
        detail = (result.stderr or result.stdout).strip().replace("\n", " | ")
        report_fail(errors, label, detail)


def validate_spdx_rdf(path: Path, errors: list[str], label: str) -> None:
    # `.rdf` is already a recognized pyspdxtools extension.
    result = subprocess.run(
        ["pyspdxtools", "-i", str(path)],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        report_ok(label)
    else:
        detail = (result.stderr or result.stdout).strip().replace("\n", " | ")
        report_fail(errors, label, detail)


def validate_static_fixtures(errors: list[str], tmp_dir: Path) -> None:
    for name in CYCLONEDX_JSON_FIXTURES:
        validate_cyclonedx_json(FIXTURES_DIR / name, errors, f"testdata/output-formats/{name}")
    for name in CYCLONEDX_XML_FIXTURES:
        validate_cyclonedx_xml(FIXTURES_DIR / name, errors, f"testdata/output-formats/{name}")
    for name in SPDX_TAGVALUE_FIXTURES:
        validate_spdx_tagvalue(
            FIXTURES_DIR / name, errors, f"testdata/output-formats/{name}", tmp_dir
        )


def validate_generated_scan(errors: list[str], provenant_bin: Path, tmp_dir: Path) -> None:
    tv_out = tmp_dir / "generated-scan.spdx.tag"
    rdf_out = tmp_dir / "generated-scan.spdx.rdf"
    json_out = tmp_dir / "generated-scan.cdx.json"
    xml_out = tmp_dir / "generated-scan.cdx.xml"

    subprocess.run(
        [
            str(provenant_bin),
            "scan",
            str(SMOKE_SCAN_ROOT),
            "--license",
            "--copyright",
            "--package",
            "--info",
            "--spdx-tv",
            str(tv_out),
            "--spdx-rdf",
            str(rdf_out),
            "--cyclonedx",
            str(json_out),
            "--cyclonedx-xml",
            str(xml_out),
        ],
        check=True,
        capture_output=True,
        text=True,
    )

    validate_spdx_tagvalue(tv_out, errors, "generated scan: SPDX tag-value", tmp_dir)
    validate_spdx_rdf(rdf_out, errors, "generated scan: SPDX RDF/XML")
    validate_cyclonedx_json(json_out, errors, "generated scan: CycloneDX JSON")
    validate_cyclonedx_xml(xml_out, errors, "generated scan: CycloneDX XML")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--provenant-bin",
        type=Path,
        default=None,
        help=(
            "Path to a built `provenant` binary. When given, also runs a tiny live "
            f"scan of {SMOKE_SCAN_ROOT.relative_to(ROOT)} and validates its SPDX/"
            "CycloneDX output, which is the only way to exercise the SPDX RDF "
            "writer against a real validator (no genuine RDF/XML fixture is "
            "checked in)."
        ),
    )
    args = parser.parse_args()

    print(f"Validating checked-in CycloneDX {CYCLONEDX_SPEC_VERSION} / SPDX fixtures...")
    errors: list[str] = []
    with tempfile.TemporaryDirectory(prefix="provenant-output-format-validators-") as tmp:
        tmp_dir = Path(tmp)
        validate_static_fixtures(errors, tmp_dir)
        if args.provenant_bin is not None:
            validate_generated_scan(errors, args.provenant_bin, tmp_dir)

    if errors:
        print(
            f"\n{len(errors)} output-format check(s) failed external schema/spec validation.",
            file=sys.stderr,
        )
        return 1

    print("\nAll output-format fixtures passed external schema/spec validation.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
