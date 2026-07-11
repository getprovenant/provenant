# CLI Guide

This guide is for anyone using `provenant`, especially when choosing among common scan workflows or coming back to them later.

Use it to answer practical questions such as:

- "What should my first scan command look like?"
- "How do I scan for licenses?"
- "How do I scan for packages and dependencies?"
- "When should I use JSON, HTML, SPDX, or CycloneDX?"
- "How do I re-use an existing scan instead of rescanning?"
- "How do I compare existing ScanCode and Provenant JSON outputs?"

For the complete scan-flag reference, use:

```sh
provenant scan --help
```

Bare `provenant ...` still defaults to scan mode for backward compatibility, but the explicit subcommand form keeps the command tree easier to navigate in help and docs.

This guide does **not** try to repeat every scan flag from `scan --help`. Instead, it focuses on the workflows most users actually need.

## `provenant serve`

Provenant also ships a long-lived HTTP service shell:

```sh
provenant serve --help
```

`provenant serve` starts a long-lived HTTP service with `/livez`, `/readyz`, `/version`, synchronous `POST /v1/scans`, asynchronous `POST /v1/scans:async`, and async job polling via `/v1/jobs/{id}` plus `/v1/jobs/{id}/result`. It supports operator-mode local paths plus remote-ingestion inputs (`input.type=repository`, `url`, and bounded `upload`). For request/response examples, see the [Serve API Guide](SERVE_API_GUIDE.md).

## Start Here: A Strong Default Scan

If you are starting a new scan and want a strong default, start with pretty JSON and explicitly ask for the scan types you care about:

```sh
provenant scan --json-pp scan.json --license --package /path/to/project
```

Why this is a good first command:

- `--json-pp scan.json` writes a readable JSON file you can inspect, diff, and feed into other tools later.
- `--license` turns on license detection. This is **opt-in**.
- `--package` turns on package and dependency detection from manifests and lockfiles. This is also **opt-in**.

What you get back:

- file-level license findings
- top-level license detections
- assembled top-level packages
- extracted dependencies from supported manifests and lockfiles

If you also want copyright, holder, and author detection, add `--copyright`:

```sh
provenant scan --json-pp scan.json --license --copyright --package /path/to/project
```

By default, Provenant preserves file-level copyright text more faithfully in `files[].copyrights[].copyright` instead of silently normalizing it to ScanCode's historic emitted value.

If you need the ScanCode-style rendered value in that same field for a parity-sensitive pipeline, add:

```sh
provenant scan --json-pp scan.json --license --copyright --compat-mode scancode /path/to/project
```

Use the default native mode for compliance review and auditability. Use `--compat-mode scancode` when a downstream system expects ScanCode-like file-level copyright strings specifically.

## Important Mental Model: Detections Are Opt-In

Like modern ScanCode, Provenant does not assume every scan should collect every kind of data.

That means you usually choose the scan dimensions you want:

| If you want to learn about...                  | Use                     | What it adds                                                   |
| ---------------------------------------------- | ----------------------- | -------------------------------------------------------------- |
| Licenses in files                              | `--license`             | license detections, expressions, and optional diagnostics/text |
| Package manifests and lockfiles                | `--package`             | top-level packages and dependencies                            |
| Installed system package databases             | `--system-package`      | package data from RPM, dpkg, apk, and similar sources          |
| Embedded package metadata in compiled binaries | `--package-in-compiled` | package data from supported Go and Rust binaries               |
| Copyrights, holders, and authors               | `--copyright`           | copyright statements, holders, and authors                     |
| File metadata such as checksums and type hints | `--info`                | extra file metadata and source/script hints                    |
| Emails or URLs                                 | `--email`, `--url`      | extracted email addresses or URLs                              |

This is the main reason the workflow guide matters: the right command depends on what question you are trying to answer.

## Choose an Output Format First

Every run needs at least one output flag, and you can request more than one in the same run.

For most users, the best default is still pretty JSON:

```sh
provenant scan --json-pp scan.json --license --package /path/to/project
```

Use other outputs when you need a specific consumer or review format:

- `--json` for compact machine-readable output
- `--json-pp` for human inspection and debugging
- `--json-lines` for streaming-oriented pipelines
- `--yaml` for a human-readable structured format outside JSON
- `--html` for a browsable report
- `--spdx-tv`, `--spdx-rdf`, `--cyclonedx`, `--cyclonedx-xml` for downstream compliance or SBOM workflows
- `--debian` for a machine-readable Debian copyright file
- `--sarif` for SARIF 2.1.0 output of license-policy violations, for pull-request checks and the code-scanning UI (see [policy-aware license review](#17-i-want-policy-aware-license-review))
- `--custom-output` with `--custom-template` for custom report generation

You can write more than one output format in the same run. For example:

```sh
provenant scan --json-pp scan.json --html report.html --license --package /path/to/project
```

That is useful when you want one machine-readable result for automation and one human-readable report for review.

You can also write to stdout by using `-` as the output file:

```sh
provenant scan --json-pp - --license /path/to/project
```

That is useful when you want to inspect a quick result in the terminal or pipe it to another command.

When you need to interpret JSON output fields and presence rules, see the [Output Field Reference](OUTPUT_FIELD_REFERENCE.md).

## Custom Templates

`--custom-output <FILE>` renders the scan through a template you supply with `--custom-template <FILE>`:

```sh
provenant scan --custom-output report.txt --custom-template report.j2 --license --copyright /path/to/project
```

Templates use [MiniJinja](https://docs.rs/minijinja), a Jinja2-compatible engine, so the syntax matches the templates you would write for Jinja2 tools (including ScanCode).

The template receives a **Provenant-native context** that mirrors the JSON output schema:

- `output` — the full output object (same shape as `--json`)
- `headers` — the scan headers (e.g. `headers[0].tool_version`)
- `files` — the list of scanned files
- `packages` — top-level detected packages
- `dependencies` — top-level dependencies

For example, to list each file and its detected license expression:

```jinja
{% for file in files %}
{{ file.path }}: {{ file.detected_license_expression_spdx }}
{% endfor %}
```

### ScanCode compatibility (`scancode` namespace)

For porting templates written against ScanCode Toolkit's `--custom-template`, the same variables ScanCode exposes are available under the `scancode` namespace:

- `scancode.files.license_copyright` — path-keyed map of `{start, end, what, value}` license and copyright entries; as in ScanCode, files with no license or copyright detections are omitted
- `scancode.files.infos` — path-keyed map of per-file metadata (one entry per scanned file)
- `scancode.files.package_data` — path-keyed map of package data (one entry per scanned file, `[]` when a file has no packages)
- `scancode.license_references` — the license reference list
- `scancode.version` — the tool version string

A ScanCode template that referenced top-level `files`, `license_references`, and `version` is ported by prefixing those variables with `scancode.` (for example `files.infos` becomes `scancode.files.infos`).

## Common Workflows

The examples below are organized by the question a user is trying to answer.

### 1. "I want a good first inventory of this codebase"

```sh
provenant scan --json-pp scan.json --license --copyright --package /path/to/project
```

Use this when you want a broad provenance-oriented view of a repository.

Why it is useful:

- `--license` finds detected license expressions and file-level matches.
- `--copyright` adds copyright statements, holders, and authors.
- `--package` finds manifests/lockfiles and assembles top-level packages and dependencies.

This is the best place to start if you are doing general review or compliance triage.

### 2. "I only care about licenses"

```sh
provenant scan --json-pp licenses.json --license /path/to/project
```

Use this when your main question is "what licenses were detected in this tree?"

This is especially useful for:

- quick license triage
- comparing license-detection changes between runs
- collecting top-level license results without package-focused noise

If you need to customize the license dataset Provenant uses, first export the built-in effective dataset and then point a scan at the exported dataset root:

```sh
provenant export-license-dataset /tmp/provenant-license-dataset
provenant scan --json-pp licenses.json --license --license-dataset-path /tmp/provenant-license-dataset /path/to/project
```

Use this advanced workflow when you want to inspect, edit, or replace the `.RULE` and `.LICENSE` files Provenant uses. The dataset root must contain:

```text
<dataset-root>/
  manifest.json
  rules/
  licenses/
```

When `--license-dataset-path` is set, Provenant uses that dataset as authoritative input instead of the embedded dataset shipped in the binary.

If you need the matched text that triggered a detection, add `--license-text`:

```sh
provenant scan --json-pp licenses.json --license --license-text /path/to/project
```

Add diagnostics only when you are actively investigating why something matched:

```sh
provenant scan --json-pp licenses.json --license --license-text --license-text-diagnostics --license-diagnostics /path/to/project
```

Add `--license-references` when you want top-level unique license and rule reference blocks, and add `--unknown-licenses` when you want unmatched license-like text surfaced for review.

Add `--no-sequence-matching` when you want to disable Provenant's approximate sequence matcher and keep license detection on the non-sequence paths only. This is mainly useful when you are triaging noisy partial matches or comparing results with and without the approximate matcher enabled.

If you are troubleshooting PDF extraction specifically, Provenant suppresses noisy `pdf_oxide`
dependency logs by default so normal scan output stays readable. To inspect the raw PDF parser
logs for a debugging run, rerun with `RUST_LOG=pdf_oxide=warn` (or `=error` if you only want
higher-severity dependency logs).

#### License index cache

On first use with `--license`, Provenant builds a license index from the embedded rules and saves
it under the shared cache root (`license-index/embedded/<fingerprint>.rkyv`, ~340 MB). Subsequent
runs load the cache instead of rebuilding the index, reducing startup from ~12s to ~0.8s.

The cache is automatically invalidated when:

- a new provenant binary ships with different embedded rules (detected via SHA-256 fingerprint)
- a custom license dataset loaded with `--license-dataset-path` changes between runs

Three CLI flags control cache behavior:

- `--reindex` — force a cache rebuild, ignoring any existing cache
- `--no-license-index-cache` — build the license index in memory for this run without reading or writing persistent license-cache files
- `--cache-dir <DIR>` — choose the shared cache root for both incremental manifests and license-index cache files

```sh
provenant scan --json-pp scan.json --license --cache-dir .cache/provenant --reindex /path/to/project
```

### 3. "I want file metadata such as checksums and type hints"

```sh
provenant scan --json-pp info.json --info /path/to/project
```

Use `--info` when you want file-level metadata rather than legal or package detections.

This is useful for:

- checksums and file sizes
- source/script hints
- output-shaping workflows that depend on file metadata later

You also need `--info` for some related features such as `--mark-source`.

### 4. "I want packages and dependencies"

```sh
provenant scan --json-pp packages.json --package /path/to/project
```

Use this when you want package manifests, lockfile-derived dependencies, and assembled package records.

This is a strong default for:

- ecosystem inventory
- dependency review
- preparing for SBOM-oriented output later

What to expect in the results:

- top-level `packages`
- top-level `dependencies`
- file-level package data attached to supported manifests and lockfiles

### 5. "I want both packages and licenses together"

```sh
provenant scan --json-pp scan.json --license --package /path/to/project
```

This is one of the most common real-world scans.

Use it when you want to answer both:

- "What components are here?"
- "What licenses were detected in this codebase?"

This combination is often more useful than a package-only or license-only run because it gives both codebase-level license findings and package/dependency context in one result file.

### 6. "I only want package data, and I want it fast"

```sh
provenant scan --json-pp packages.json --package-only /path/to/project
```

Use `--package-only` when you explicitly want a narrower package-focused scan and do **not** want license or copyright detection.

This is useful when:

- you are doing package inventory only
- you want a faster specialized scan
- you plan to run a deeper license scan separately

Important: `--package-only` is a special mode, not a synonym for `--package`. It enables both application-manifest and installed-package detection, intentionally skips license/copyright work, skips the normal top-level package assembly path, and does not create the usual top-level `packages` and `dependencies` view you get from `--package`.

If you explicitly ask for non-license detections such as `--email`, `--url`, or `--generated`, those still behave normally in `--package-only` mode.

If you want assembled top-level packages and dependencies, use `--package` instead.

### 7. "I need system package data"

```sh
provenant scan --json-pp system-packages.json --system-package /path/to/rootfs-or-image-extract
```

Use this when scanning extracted environments or roots that contain installed package databases rather than just source manifests.

This is the right workflow for things like:

- extracted container filesystems
- unpacked root filesystems
- operating-system package metadata trees

### 8. "I want package data from compiled binaries"

```sh
provenant scan --json-pp compiled-packages.json --package-in-compiled /path/to/project
```

Use this when you want package metadata embedded in supported compiled Go or Rust binaries.

This is useful when:

- the source manifests are missing
- you are auditing built artifacts rather than source
- you want binary-level package provenance in addition to manifest-based scans

If you also want manifest/lockfile package detection, combine it with `--package`.

### 9. "I want a browsable HTML report"

```sh
provenant scan --html report.html --license --copyright /path/to/project
```

Use this when you want to review findings in a browser rather than inspect JSON directly.

HTML is useful for:

- manual review
- sharing a quick report with someone who does not want raw JSON
- checking whether the scan is generally finding what you expected before moving into machine-readable formats

### 10. "I need SPDX or CycloneDX output"

```sh
provenant scan --cyclonedx bom.json --package /path/to/project
```

or:

```sh
provenant scan --spdx-tv sbom.spdx --package /path/to/project
```

Use these formats when another tool or downstream process expects them.

In practice:

- CycloneDX is often the better fit for BOM-oriented pipelines.
- SPDX is often the better fit for compliance-oriented exchange.
- `--package` is usually part of these workflows because package/dependency data is central to SBOM output.

### 11. "I need Debian copyright output"

```sh
provenant scan --debian debian.copyright --license --copyright --license-text /path/to/project
```

Use this when you need a machine-readable Debian copyright file.

Why the extra flags matter:

- `--license` provides the detected license expressions
- `--copyright` provides copyright holders and statements
- `--license-text` provides matched text blocks used in the Debian output

This workflow is more specialized than JSON or HTML, so it is usually something you generate after you already know you need Debian-format output.

### 12. "I want to ignore obvious noise"

```sh
provenant scan --json-pp scan.json --license --package /path/to/project --ignore "*.min.js" --ignore "node_modules/*"
```

Use ignore patterns when you want to:

- skip vendored or generated content
- reduce scan time on very large trees
- keep results focused on the code you actually care about

Use quotes around glob patterns so your shell does not expand them before Provenant sees them.

### 13. "I want to inspect results in the terminal first"

```sh
provenant scan --json-pp - --license --package /path/to/project
```

Use stdout when you are trying to validate a command quickly before saving a file or when you want to pipe the result elsewhere.

### 14. "I already have a scan and only want to reshape it"

```sh
provenant scan --json-pp reshaped.json --from-json scan.json --only-findings
```

Use `--from-json` when you want to reuse an existing ScanCode-style JSON result instead of rescanning the original inputs.

This is especially useful for:

- applying output filters after the fact
- producing a different output view from the same base scan
- merging or reshaping multiple prior JSON scans

Important: `--from-json` is for reshaping existing results. It is not a second scan pass, and scan-time options such as fresh detection flags are intentionally restricted in this mode.

Also note that `--from-json` cannot recover newer native-only evidence details that were never serialized in the original JSON. For example, replaying older ScanCode-style or compatibility-mode JSON cannot reconstruct the newer less-normalized file-level copyright text without rescanning the original files.

### 15. "I want a codebase-level summary instead of reading raw file-by-file results"

```sh
provenant scan --json-pp summary.json --license --package --classify --summary /path/to/project
```

Use this when the raw scan output is correct but too detailed for your immediate question.

Why it is useful:

- `--classify` enables higher-level classification output.
- `--summary` adds codebase-level summary information rather than leaving you with only file-by-file details.

If you want count-oriented review, add `--tallies`:

```sh
provenant scan --json-pp summary.json --license --package --classify --summary --tallies /path/to/project
```

This is a good second-step workflow after a first broad scan, especially on larger repositories.

### 16. "I run the same scan repeatedly"

```sh
provenant scan --json-pp scan.json --license --package --incremental /path/to/project
```

Use incremental reuse for repeated native directory scans.

After a completed scan, Provenant stores an incremental manifest under the cache root and uses it
on the next run to skip unchanged files. In practice, this is most useful when you are scanning
the same checkout repeatedly: local iteration, CI retries, or rerunning after a later failed or
interrupted scan.

Good use cases:

- iterative local review on the same repository
- repeated scans in a CI-like workflow
- large trees where rescanning unchanged content is expensive
- retrying a later scan without redoing unchanged work from the last completed run

Important details:

- `--incremental` enables this behavior.
- `--cache-dir PATH` and `PROVENANT_CACHE` choose the shared cache root.
- that root stores both incremental manifests and reusable license-index cache files.
- `--cache-clear` clears that shared cache state before the run.
- if the previous manifest is missing, unreadable, or incompatible, Provenant falls back to a full rescan and rewrites it.
- incremental reuse applies to native scans, not `--from-json` reshaping.

#### Faster warm re-scans with `--cache-trust-mtime`

By default, incremental reuse is paranoid: even when a file's size and nanosecond
mtime still match the cached fingerprint, Provenant re-reads the file and re-hashes
its content (SHA-256) before reusing the cached result. This makes default scans
fully reproducible and byte-identical, but it still pays full read + hash I/O for
the entire tree on every warm re-scan.

`--cache-trust-mtime` (only valid together with `--incremental`) opts into trusting
the size + mtime fingerprint: a matching fingerprint reuses the cached result
without re-reading or re-hashing the file.

Trade-off:

- Faster: warm re-scans skip the read + SHA-256 of every unchanged file.
- Slightly less safe: it can miss a file that was modified in place within the same
  mtime tick at exactly the same size (rare, and typically only seen with very fast
  programmatic edits or filesystems with coarse mtime resolution).

The default stays paranoid (full re-hash) so reproducibility is never silently
traded away. Reach for `--cache-trust-mtime` when warm-rescan speed matters more
than catching that rare same-tick, same-size edit (for example, fast local
iteration on large trees). A genuinely changed size or mtime is still detected as
changed in either mode.

The miss does not become permanent: when a trust-mtime run reuses a stale result,
the rewritten manifest keeps the hash of the bytes that produced that result rather
than re-hashing the current file. A later run without `--cache-trust-mtime`
therefore re-hashes, sees the hash no longer matches, and re-scans the file. In
other words, switching back to the default paranoid mode recovers correct results
on the next scan.

### 17. "I want policy-aware license review"

```sh
provenant scan --json-pp policy.json --license --license-references --filter-clues --license-policy policy.yml /path/to/project
```

Use this when you want a review-oriented license scan rather than raw low-level findings.

Why it is useful:

- `--license-references` adds top-level license and rule reference blocks.
- `--filter-clues` removes redundant clue output that is usually noisy in broad review workflows.
- `--license-policy policy.yml` evaluates file findings against a YAML policy after the scan.
- `--ignore-author PATTERN` and `--ignore-copyright-holder PATTERN` let you suppress entire resources when those findings match review-specific regexes.

This workflow is also useful with `--from-json` when you want to reshape an existing scan instead of rescanning the original inputs.

#### The license policy file

`--license-policy` takes a YAML file with a top-level `license_policies:` list. Each entry maps a license key to display metadata (`label`, `color_code`, `icon`, all optional) and, as a Provenant extension, an optional `compliance_alert` severity of `error` or `warning`:

```yaml
license_policies:
  - license_key: gpl-3.0
    label: Prohibited License
    compliance_alert: error
  - license_key: gpl-2.0
    label: Copyleft License
    compliance_alert: warning
  - license_key: mit
    label: Approved License
    # no compliance_alert => informational only
```

For each scanned file, Provenant collects the license keys from its detected expressions, matches them against the policy, and attaches the matching entries (including `compliance_alert`) to that file's `license_policy` output field. Entries without a `compliance_alert` are informational and never fail a build.

#### Failing CI on a policy violation

Add `--fail-on <error|warning>` to turn the policy into a build gate. The scan exits with code **3** when a file's detected license — or a top-level package's or dependency's **declared** license — matches a policy whose `compliance_alert` is at or above the given level (`warning` trips on warning and error; `error` trips only on error). The report is still written before the process exits, so the artifact is never lost. `--fail-on` requires `--license-policy`.

```sh
provenant scan --json-pp scan.json --license --license-policy policy.yml --fail-on error /path/to/project
```

#### Surfacing violations in pull requests (SARIF)

`--sarif <FILE>` writes the policy violations as SARIF 2.1.0, which GitHub can render as pull-request annotations and code-scanning alerts (upload it with `github/codeql-action/upload-sarif`). Each severity-carrying policy match becomes a result at the file's detection line; with no policy the run has zero results, so SARIF stays quiet unless you opt into a policy.

```sh
provenant scan --sarif provenant.sarif --license --license-policy policy.yml /path/to/project
```

### 18. "I want tallies, facets, or clarity scoring"

```sh
provenant scan --json-pp summary.json --license --package --classify --summary --tallies /path/to/project
```

Build on that baseline when you need more structured review output:

- add `--license-clarity-score` for project-level clarity scoring
- add `--tallies-with-details` for file- and directory-level tallies
- add `--tallies-key-files` for key-file-focused tallies
- add one or more `--facet <facet>=<pattern>` rules, then `--tallies-by-facet`, to split tallies by shipping code vs tests/docs/examples

Example:

```sh
provenant scan --json-pp summary.json --license --package --classify --summary --tallies --facet core=src/** --facet tests=test/** --tallies-by-facet --license-clarity-score /path/to/project
```

### 19. "I need to scan more than one input path"

```sh
provenant scan --json-pp scan.json --license dir-a dir-b
```

Use this when you want one result file covering more than one native input path.

This is useful for:

- scanning related repositories together
- scanning split source trees in one run
- collecting one combined report for several directories

These native multi-input paths still follow the current common-prefix behavior. They work best when you can invoke Provenant from a cwd where the relative input paths share a usable common ancestor.

You can also pass multiple JSON inputs with `--from-json`.

### 20. "I want to scan only files matching certain patterns"

```sh
provenant scan --json-pp scan.json --license /path/to/repo --include "*.rs" --include "src/**/*.toml"
```

Use `--include` when you want glob-style path filtering inside one scan root.

Current behavior:

- `--include` matches file/path patterns; repeated flags are additive
- use `**` when you want recursion across directory boundaries
- plain directory-looking tokens such as `src/foo` are treated as literal path patterns, not as an implicit “scan this whole subtree” shortcut
- if you already know the exact files or directories you want, prefer `--paths-file` instead of encoding that selection indirectly through globs

### 21. "I have an explicit list of files or directories to scan"

```sh
provenant scan --json-pp scan.json --license /path/to/repo --paths-file changed-files.txt
```

Use this when you already have a selected path list under one known root, especially for CI and pull-request workflows where cwd cannot be the repo root.

`--paths-file` is the preferred workflow when:

- `git diff --name-only` or another tool already produced the changed-file list
- Provenant must run from a fixed mount location or other non-repo cwd
- you want Provenant itself, not shell `xargs`, to own the selection semantics

Current behavior:

- pass exactly one native scan root as the positional input
- entries in the paths file are interpreted relative to that root
- one path per line, with blank lines ignored and CRLF tolerated
- directory entries select that subtree
- missing entries are skipped with a warning
- `--paths-file -` reads the list from stdin
- `--paths-file` cannot currently be combined with `--from-json`

Example with stdin:

```sh
git diff --name-only --diff-filter=d origin/main...HEAD | provenant scan --json-pp - --license /path/to/repo --paths-file -
```

## Important Flag Combinations

These are worth learning early because they change what the output means:

- `--license-text` requires `--license`
- `--license-text-diagnostics` requires `--license-text`
- `--license-diagnostics` requires `--license`
- `--license-references` requires `--license`
- `--no-sequence-matching` requires `--license` and disables the approximate sequence matcher
- `--license-clarity-score` requires `--classify`
- `--mark-source` requires `--info`
- `--custom-output <FILE>` requires `--custom-template <FILE>`
- `--tallies-key-files` requires `--tallies` and `--classify`
- `--tallies-by-facet` requires `--facet` and `--tallies`
- `--debian <FILE>` requires `--license`, `--copyright`, and `--license-text`
- `--fail-on <LEVEL>` requires `--license-policy`; a violation exits with code 3
- `--sarif <FILE>` only emits results for `--license-policy` entries that carry a `compliance_alert`
- `--paths-file <FILE>` requires exactly one native scan root and is currently native-scan only (no `--from-json`)
- `--reindex` only matters when the license engine is initialized (`--license` and some `--from-json` reference-recompute flows)
- `--no-license-index-cache` only matters when the license engine is initialized

## Memory Use and What `--max-in-memory` Bounds

`--max-in-memory <INT>` caps how many per-file scan results Provenant keeps in
memory while it is processing files. Once the cap is reached, further results
spill to a temporary on-disk store instead of staying resident. `0` disables the
cap (everything stays in memory) and `-1` spills as aggressively as possible
during the scan.

Important: this flag bounds only the working set held _during file scanning_, not
the whole process's peak memory. The later phases — assembly, summaries, and
output — need the complete result set in memory at once, and that is where peak
memory use is highest. So spilling during the scan does not lower the overall
peak, and very aggressive spilling can even raise it slightly.

Use `--max-in-memory` to keep the scan phase from growing without bound on very
large trees. Do not rely on it to cap the whole process's peak memory.

## A Simple Decision Guide

If you are not sure where to start, use this rule of thumb:

- Want a general first scan? → `--json-pp` + `--license` + `--package`
- Want copyright review too? → add `--copyright`
- Want assembled top-level packages and dependencies? → `--package`
- Want a narrower file-level package-data pass across application and installed-package inputs without normal top-level assembly? → `--package-only`
- Want SBOM-oriented output? → add `--cyclonedx` or `--spdx-*`, usually with `--package`
- Want browser-friendly review? → `--html`
- Want policy-aware license review? → add `--license-references`, `--filter-clues`, and `--license-policy` (add `--fail-on` to gate CI, `--sarif` for pull-request alerts)
- Want summary/tally/facet review? → add `--classify`, `--summary`, and optionally `--tallies*` / `--facet`
- Want glob-style file filtering inside one scan root? → add one or more `--include` patterns
- Want an explicit rooted list of files/directories? → use `--paths-file`
- Already have JSON and only want to filter or reshape it? → `--from-json`
- Migrating from ScanCode and want a structured confidence check on one representative target? → `compare`

## Compare ScanCode and Provenant During Migration

When you are migrating from ScanCode to Provenant, run both tools on one representative
codebase and then use `compare` to generate a focused artifact set showing where the JSON outputs
differ:

```sh
provenant compare \
  --scancode-json scancode.json \
  --provenant-json provenant.json
```

By default, this creates a timestamped artifact directory in your current working directory, for
example `./provenant-compare-20260428T131500Z/`. Use `--artifact-dir DIR` when you want the
artifact bundle somewhere specific.

Each compare run writes:

- copied raw JSON inputs under `raw/`
- `comparison/summary.json`
- `comparison/summary.tsv`
- detailed sample diff artifacts under `comparison/samples/`
- `run-manifest.json`

For the comparison to be meaningful, make sure the ScanCode and Provenant JSON files were produced
with the same effective scan shape: the same target snapshot, the same broad detection flags, and
the same output-shaping intent. Comparing mismatched scan modes is usually noise, not migration
signal.

Use this workflow when you want to review parity or regression deltas before trusting Provenant on
broader repositories or automation. It is most useful as a migration-confidence check, not as a
generic replacement for normal scanning.

## Where to Go Next

- Run `provenant --help` for the command tree, `provenant scan --help` for the full scan CLI surface, `provenant serve --help` for the service shell, and `provenant compare --help` for JSON comparison options
- See [README.md](../README.md) for installation and quick start
- See [SUPPORTED_FORMATS.md](SUPPORTED_FORMATS.md) for supported package and ecosystem coverage
- See [ARCHITECTURE.md](ARCHITECTURE.md) for implementation details
