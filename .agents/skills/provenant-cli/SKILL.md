---
name: provenant-cli
description: Quick reference for Provenant CLI flags, output formats, detection modes, and common scan workflows.
---

# Provenant CLI Quick Reference

## Synopsis

```text
provenant scan --json-pp <FILE> [OPTIONS] <INPUT>...
```

At least one output flag is required. Detection flags are opt-in.

Bare `provenant ...` still defaults to scan mode for backward compatibility, but the explicit subcommand form is the current primary syntax.

- `provenant --help` shows the command tree.
- `provenant scan --help` shows the full scan-flag surface.

## Output Formats (at least one required)

| Flag                     | Format              | Notes                                           |
| ------------------------ | ------------------- | ----------------------------------------------- |
| `--json <FILE>`          | Compact JSON        | Machine-readable                                |
| `--json-pp <FILE>`       | Pretty-printed JSON | Human inspection, debugging                     |
| `--json-lines <FILE>`    | JSON Lines          | Streaming pipelines                             |
| `--yaml <FILE>`          | YAML                | Human-readable structured                       |
| `--html <FILE>`          | HTML report         | Browsable                                       |
| `--spdx-tv <FILE>`       | SPDX tag/value      | Compliance exchange                             |
| `--spdx-rdf <FILE>`      | SPDX RDF/XML        | Compliance exchange                             |
| `--cyclonedx <FILE>`     | CycloneDX JSON      | SBOM pipelines                                  |
| `--cyclonedx-xml <FILE>` | CycloneDX XML       | SBOM pipelines                                  |
| `--debian <FILE>`        | Debian copyright    | Requires `--license --copyright --license-text` |
| `--custom-output <FILE>` | Custom template     | Requires `--custom-template <FILE>`             |
| `--show-attribution`     | Attribution notices | No file output                                  |

Use `-` as the file path to write to stdout. Multiple output formats can be combined in one run.

## Detection Flags (opt-in)

| Flag                    | Short | What it adds                                                   |
| ----------------------- | ----- | -------------------------------------------------------------- |
| `--license`             | `-l`  | License detections, expressions, diagnostics/text              |
| `--copyright`           | `-c`  | Copyright statements, holders, authors                         |
| `--package`             | `-p`  | Application packages and dependencies from manifests/lockfiles |
| `--system-package`      |       | Installed system package databases (RPM, dpkg, apk)            |
| `--package-in-compiled` |       | Embedded package metadata in compiled Go/Rust binaries         |
| `--package-only`        |       | Specialized package-only mode; not a synonym for `--package`   |
| `--info`                | `-i`  | File metadata: checksums, type hints, source/script flags      |
| `--email`               | `-e`  | Extracted email addresses                                      |
| `--url`                 | `-u`  | Extracted URLs                                                 |
| `--generated`           |       | Generated code detection                                       |

`--package` returns assembled top-level `packages`, top-level `dependencies`, and file-level package data from supported manifests and lockfiles.

`--package-only` is a narrower fast path: it enables application-manifest and installed-package detection, skips license/copyright work, skips the normal top-level assembly path, and does not produce the usual top-level `packages` / `dependencies` view. If you explicitly request non-license detections such as `--email`, `--url`, or `--generated`, those still run.

## License Sub-flags

| Flag                           | Requires         | What it does                                 |
| ------------------------------ | ---------------- | -------------------------------------------- |
| `--license-text`               | `--license`      | Include matched text in detection output     |
| `--license-text-diagnostics`   | `--license-text` | Diagnostics for matched text                 |
| `--license-diagnostics`        | `--license`      | License detection diagnostics                |
| `--license-references`         | `--license`      | Top-level license/rule reference blocks      |
| `--unknown-licenses`           | `--license`      | Surface unmatched license-like text          |
| `--license-score <N>`          | `--license`      | Minimum match score threshold (default: 0)   |
| `--license-url-template <T>`   | `--license`      | Customize top-level license reference URLs   |
| `--license-policy <FILE>`      | `--license`      | Evaluate against YAML policy file            |
| `--license-dataset-path <DIR>` | `--license`      | Use a custom exported/editable dataset root  |
| `--reindex`                    | `--license`      | Force rebuild license index cache            |
| `--no-license-index-cache`     | `--license`      | Disable persistent license-index cache usage |

To inspect or customize the built-in license dataset:

```sh
provenant export-license-dataset /tmp/provenant-license-dataset
provenant scan --json-pp licenses.json --license --license-dataset-path /tmp/provenant-license-dataset /path/to/project
```

## Post-processing Flags

| Flag                      | Requires                  | What it does                           |
| ------------------------- | ------------------------- | -------------------------------------- |
| `--classify`              |                           | Enable classification output           |
| `--summary`               | `--classify`              | Codebase-level summary                 |
| `--tallies`               |                           | Count-oriented tallies                 |
| `--tallies-key-files`     | `--tallies`, `--classify` | Key-file-focused tallies               |
| `--tallies-with-details`  |                           | File/directory-level tallies           |
| `--facet <K>=<P>`         |                           | Define facet rule (e.g. `core=src/**`) |
| `--tallies-by-facet`      | `--facet`, `--tallies`    | Split tallies by facet                 |
| `--license-clarity-score` | `--classify`              | Project-level clarity scoring          |
| `--filter-clues`          |                           | Remove redundant clue output           |
| `--only-findings`         |                           | Only findings in output                |
| `--mark-source`           | `--info`                  | Mark source files                      |
| `--no-assemble`           |                           | Disable package assembly               |

## Filtering, Selection & Control

| Flag                                         | What it does                                                               |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| `--exclude <PATTERN>` / `--ignore <PATTERN>` | Exclude paths matching glob pattern                                        |
| `--include <PATTERN>`                        | Include only matching paths within one scan root                           |
| `--paths-file <FILE>`                        | Scan an explicit rooted file/directory list                                |
| `--max-depth <N>`                            | Recursion depth limit (0 = unlimited, default: 0)                          |
| `--timeout <SECS>`                           | Timeout per file (default: 120)                                            |
| `-n, --processes <N>`                        | Parallel processes (default: 11)                                           |
| `--max-in-memory <N>`                        | Max file details in memory (default: 10000, 0 = unlimited, -1 = disk-only) |
| `--strip-root`                               | Strip root prefix from paths                                               |
| `--full-root`                                | Keep full root prefix                                                      |
| `--compat-mode scancode`                     | Emit ScanCode-style compatibility shaping where supported                  |
| `-q, --quiet`                                | Suppress progress output                                                   |
| `-v, --verbose`                              | Verbose output                                                             |

Current selection behavior:

- Repeated `--include` flags are additive.
- Use `**` when you want recursion across directory boundaries.
- If you already know the exact files or directories you want, prefer `--paths-file` over encoding that selection indirectly through globs.
- `--paths-file` requires exactly one native scan root, interprets entries relative to that root, accepts one path per line, lets directory entries select subtrees, supports `--paths-file -` for stdin, and cannot be combined with `--from-json`.
- Use the default native mode for more source-faithful output; use `--compat-mode scancode` when a downstream system expects ScanCode-style file-level copyright strings specifically.

## Incremental & Cache

| Flag                 | What it does                                            |
| -------------------- | ------------------------------------------------------- |
| `--incremental`      | Reuse previous scan results for unchanged files         |
| `--cache-dir <PATH>` | Override the shared cache root (also `PROVENANT_CACHE`) |
| `--cache-clear`      | Clear cache before running                              |
| `--from-json`        | Reshape one or more existing ScanCode-style JSON inputs |

`--incremental`, `--cache-dir`, and `--cache-clear` apply only to native scans, not `--from-json`. The shared cache root is also where persistent license-index cache files live unless `--no-license-index-cache` is set. In `--from-json` mode, fresh scan flags such as `--package`, `--copyright`, `--email`, `--url`, `--generated`, and package scan variants are intentionally not allowed.

## Ignore/Filter by Content

| Flag                                  | What it does                                      |
| ------------------------------------- | ------------------------------------------------- |
| `--ignore-author <PATTERN>`           | Ignore files where author matches regex           |
| `--ignore-copyright-holder <PATTERN>` | Ignore files where copyright holder matches regex |
| `--max-email <N>`                     | Max emails per file (default: 50, 0 = unlimited)  |
| `--max-url <N>`                       | Max URLs per file (default: 50, 0 = unlimited)    |

## Common Workflows

### Strong default scan

```sh
provenant scan --json-pp scan.json --license --package /path/to/project
```

### Full inventory (licenses + copyright + packages)

```sh
provenant scan --json-pp scan.json --license --copyright --package /path/to/project
```

### License-only scan

```sh
provenant scan --json-pp licenses.json --license /path/to/project
```

### Assembled packages and dependencies

```sh
provenant scan --json-pp packages.json --package /path/to/project
```

### File-level package data only (no normal top-level assembly)

```sh
provenant scan --json-pp packages.json --package-only /path/to/project
```

### System packages (container/rootfs)

```sh
provenant scan --json-pp syspkg.json --system-package /path/to/rootfs
```

### Compiled binary packages

```sh
provenant scan --json-pp compiled.json --package-in-compiled /path/to/project
```

### HTML report

```sh
provenant scan --html report.html --license --copyright /path/to/project
```

### SBOM (CycloneDX)

```sh
provenant scan --cyclonedx bom.json --package /path/to/project
```

### Debian copyright

```sh
provenant scan --debian debian.copyright --license --copyright --license-text /path/to/project
```

### Summary with tallies

```sh
provenant scan --json-pp summary.json --license --package --classify --summary --tallies /path/to/project
```

### Incremental reuse

```sh
provenant scan --json-pp scan.json --license --package --incremental /path/to/project
```

### Reshape existing scan

```sh
provenant scan --json-pp reshaped.json --from-json scan.json --only-findings
```

### Policy-aware license review

```sh
provenant scan --json-pp policy.json --license --license-references --filter-clues --license-policy policy.yml /path/to/project
```

### Ignore noise

```sh
provenant scan --json-pp scan.json --license --package --ignore "*.min.js" --ignore "node_modules/*" /path/to/project
```

### Multiple input paths

```sh
provenant scan --json-pp scan.json --license dir-a dir-b
```

### Explicit rooted file list

```sh
provenant scan --json-pp scan.json --license /path/to/repo --paths-file changed-files.txt
```

Or from stdin:

```sh
git diff --name-only --diff-filter=d origin/main...HEAD | provenant scan --json-pp - --license /path/to/repo --paths-file -
```

### ScanCode-style compatibility rendering

```sh
provenant scan --json-pp scan.json --license --copyright --compat-mode scancode /path/to/project
```

## xtask profile shorthands

These are used by xtask commands (`benchmark-target`, `compare-outputs`), not directly by `provenant`:

| Profile                | Expands to                                                   |
| ---------------------- | ------------------------------------------------------------ |
| `common`               | `-clupe --system-package --strip-root`                       |
| `common-with-compiled` | `-clupe --system-package --package-in-compiled --strip-root` |
| `licenses`             | `-l --strip-root`                                            |
| `packages`             | `-p --strip-root`                                            |

## Reference

- `docs/CLI_GUIDE.md` — full workflow guide with explanations
- `provenant --help` — command tree
- `provenant scan --help` — full scan-flag reference

## Other top-level commands

- `provenant serve --help` — long-lived HTTP service mode
- `provenant compare --help` — compare existing ScanCode-style JSON outputs
- `provenant export-license-dataset <DIR>` — export the effective built-in license dataset for inspection or customization
