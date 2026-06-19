# Source

`pip-inspect.deplock` is the `pip inspect` JSON output for a virtualenv with
`flask==3.1.0` installed, archived here as a benchmark target fixture:

- Generated on 2026-06-19 with `pip` 26.1.2 (CPython 3.14) via:

  ```
  python3 -m venv venv
  ./venv/bin/pip install flask==3.1.0
  ./venv/bin/pip inspect > pip-inspect.deplock
  ```

- The resolved set is `flask 3.1.0` plus its transitive `werkzeug`, `jinja2`,
  `click`, `itsdangerous`, `markupsafe`, and `blinker` pins, alongside `pip` itself.
- The ephemeral build-time `metadata_location` paths were normalized to `/venv/...`
  so the fixture is stable and machine-independent; no other field was altered.
- sha256: `2f30046bdfae772224ab181611f5914d09fb563866ad58be59759cf516b7d89d`

This is the single-file input measured by the `Flask 3.1.0 pip-inspect.deplock` row in
[`docs/BENCHMARKS.md`](../../../docs/BENCHMARKS.md). It is committed here only as a
test/benchmark fixture. See [`../../PROVENANCE.md`](../../PROVENANCE.md).
