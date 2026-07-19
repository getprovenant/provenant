# Evaluating Provenant with ScanCode Workflows

This guide is for people who already know ScanCode Toolkit and want to understand what, if anything, changes when they try Provenant on a similar workflow.

For many common scan-and-consume workflows, the answer is: **not much**.

Provenant targets strong CLI and output compatibility with ScanCode where practical. If you mostly run scans and consume the usual output formats, you can often try Provenant with the same broad habits and adjust only a few power-user workflows.

## Who should read this guide?

You will probably care about this document if you:

- edited ScanCode's license and rule data directly in a cloned checkout
- compare raw JSON output fields very closely between tools
- rely on historical quirks or typos in emitted values
- want to understand where Provenant intentionally differs from ScanCode

If you mostly want a ScanCode-compatible scan from a single binary, start with the [CLI Guide](CLI_GUIDE.md) instead.

## What mostly stays the same

- Provenant keeps the ScanCode-compatible scan model and output formats as its primary compatibility target.
- `spdx_license_list_version` stays in the existing ScanCode-style header location.
- `--from-json` continues to target ScanCode-style JSON inputs rather than a Provenant-only format.
- Scanning now has an explicit `provenant scan ...` command form, while bare `provenant ...` scan invocations continue to work as a compatibility alias.

For broader project overview and trust-model context, see the [README](../README.md).

## The main workflow differences to review

### 1. Custom license data is now an export/edit/reuse workflow

With ScanCode, power users often edited the license and rule data directly in a cloned source tree.

With Provenant, the equivalent workflow is:

1. export the effective embedded dataset
2. edit the exported `.RULE` and `.LICENSE` files
3. scan with the exported dataset root

```sh
provenant export-license-dataset /tmp/provenant-license-dataset
provenant --json-pp licenses.json --license \
  --license-dataset-path /tmp/provenant-license-dataset \
  /path/to/project
```

The dataset root uses this shape:

```text
<dataset-root>/
  manifest.json
  rules/
  licenses/
```

When `--license-dataset-path` is set, Provenant uses that dataset as authoritative input instead of the embedded dataset shipped in the binary.

See also:

- [CLI Guide](CLI_GUIDE.md)
- [License Detection Architecture](LICENSE_DETECTION_ARCHITECTURE.md)

### 2. Some historical typos are fixed in canonical output

Provenant emits corrected canonical values in a few places where ScanCode historically carried typos.

Current documented examples:

- Provenant emits `nuget_nuspec`
- ScanCode historically emitted `nuget_nupsec`
- Provenant emits `rpm_specfile`
- ScanCode historically emitted `rpm_spefile`

Important: Provenant still accepts some legacy spellings on input for compatibility, especially under `--from-json`.

So if you compare raw output, you may see corrected values even though old ScanCode JSON still loads.

### 3. Unicode names are preserved more faithfully

Provenant preserves source text and author/copyright names more faithfully in some cases.

Example:

- `François` stays `François`
- not `Francois`

This is an intentional data-quality improvement, not an incompatibility bug.

### 4. Some dependency booleans are left unset unless actually proven

ScanCode's formal schema allows nullable or omitted values for booleans like:

- `is_runtime`
- `is_optional`
- `is_pinned`
- `is_direct`

Provenant keeps these fields unset when the datasource does not actually prove them, rather than coercing output to common ScanCode defaults.

If you diff raw JSON semantically, this is one of the most important intentional differences to know.

### 5. File-level copyright text is raw by default

When you enable `--copyright`, Provenant preserves file-level copyright text more faithfully in the existing `files[].copyrights[].copyright` field, including wording and punctuation that ScanCode commonly strips from its emitted value.

This is intentional. Provenant treats source-faithful copyright text as the better default for compliance review and auditability, while still keeping normalized copyright semantics internally for grouping and tallies.

In practice, the most visible differences are usually:

- `Copyright 2020 The Go Authors. All rights reserved.` staying as-is instead of becoming `Copyright 2020 The Go Authors`
- `Copyright 2017 The Kubernetes Authors.` staying with the trailing period instead of becoming `Copyright 2017 The Kubernetes Authors`

If your downstream workflow needs the historic ScanCode-style rendered value in the same field, use:

```sh
provenant scan --json-pp scan.json --copyright --compat-mode scancode /path/to/project
```

### 6. Monorepo and workspace inventories are topology-aware

On multi-package trees, ScanCode often stops at per-directory package rows: a Maven reactor’s module sources, a Gradle multiproject’s nested files, or a uv/Cargo/npm/Mix/Dart workspace’s shared lock may not be attributed to the declaring packages in a way that matches how the project is structured.

Provenant’s assembly uses declared project topology (workspace members, reactor `<modules>`, Gradle `include`, Mix umbrellas, and similar) so that:

- nested sources under a member belong to that member’s package when ownership is proven
- a shared root lockfile (for example `uv.lock`, `pubspec.lock`, or Mix lock state) is attributed to members that actually declare the locked dependency, and otherwise stays honestly hoisted
- incomplete selections are called out when you combine `--paths-file` with an SBOM export — including named missing Cargo/npm/Mix workspace roots when the selected files carry an unambiguous member marker

If you evaluate Provenant on a monorepo primarily by counting packages, prefer inspecting `for_packages` / file ownership and shared-lock attribution, not only package cardinality. Details live in [Architecture](ARCHITECTURE.md) (TopologyPlan) and the relevant [improvement notes](improvements/README.md) (Maven, Gradle, uv, npm workspace, Cargo, Dart, Mix).

### 7. Parser behavior can be more capable than ScanCode on some documented surfaces

Provenant includes many documented parser fixes and intentional differences and improvements, for example in:

- NuGet
- npm/Yarn
- Gradle
- Maven
- copyright detection

These are documented improvements on specific surfaces, not random incompatibilities.

See [Intentional Differences and Improvements](improvements/README.md) for the full index.

### 8. Path selection is split more explicitly between patterns and exact rooted paths

If you previously relied on `--include` as a rough way to express “scan this subtree”, pay close attention to Provenant's newer split here.

- `--include` is for glob-style path filtering
- recursion should be explicit in the pattern (for example `src/**`)
- `--paths-file` is the explicit rooted workflow for “scan exactly these files or directories under this root”

That means Provenant now prefers:

- `--include '*.rs' --include 'src/**/*.toml'` when you mean pattern filtering
- `--paths-file changed-files.txt /path/to/repo` when you already know the exact rooted file or directory list

When the selection is incomplete relative to a workspace or reactor, SBOM exports still write, but Provenant warns that the inventory may understate the full tree — see [CLI Guide](CLI_GUIDE.md) section 10.

This is a workflow-level difference worth knowing if you are testing Provenant against existing ScanCode habits or shell wrappers.

See also:

- [CLI Guide](CLI_GUIDE.md)
- [CLI Workflows](improvements/cli-workflows.md)

### 9. Custom templates expose ScanCode variables under a `scancode` namespace

If you drive `--custom-output`/`--custom-template` with your own report templates, note two things.

- The engine is [MiniJinja](https://docs.rs/minijinja), which is Jinja2-compatible, so ScanCode Jinja2 template _syntax_ works unchanged.
- The template _context_ differs. Provenant exposes a native context at the top level (`files` is the scan file list, alongside `output`, `headers`, `packages`, `dependencies`). The variables ScanCode passes — the path-keyed `files` reshape, `license_references`, and `version` — live under the `scancode` namespace instead.

So an existing ScanCode template ports by prefixing those variables with `scancode.`, for example `files.license_copyright` becomes `scancode.files.license_copyright` and `version` becomes `scancode.version`. See [Custom Templates](CLI_GUIDE.md#custom-templates) for the full variable list.

## Practical evaluation advice

### Compare one representative target first

If you are trying to evaluate Provenant against an existing ScanCode workflow, the most useful workflow is:

1. run ScanCode on one representative codebase and save the JSON output
2. run Provenant on that same codebase and save the JSON output
3. run `provenant compare` on those two JSON files
4. review the generated artifact bundle before switching broader workflows

```sh
provenant compare \
  --scancode-json scancode.json \
  --provenant-json provenant.json
```

`provenant compare` is aware of this intentional copyright-rendering difference and normalizes it for parity review, so a raw-default Provenant scan should not produce noisy compare failures solely because it preserved `All rights reserved` or punctuation.

By default, `compare` creates a timestamped artifact directory in the current working directory so
you can inspect:

- copied raw inputs under `raw/`
- machine-readable summaries under `comparison/`
- representative diff samples under `comparison/samples/`
- `run-manifest.json` with the exact artifact locations

Use `--artifact-dir DIR` if you want the bundle written to a specific location.

This comparison only makes sense when the two JSON files came from the same target snapshot and
the same effective scan shape. Try to keep the major detection flags and output-shaping intent in
sync; otherwise the diff mostly reflects different scan scopes rather than meaningful Provenant-vs-ScanCode behavior.

If you are moving an existing ScanCode workflow to Provenant:

1. start with the same broad scan shape you already use
2. compare outputs on one representative codebase with `provenant compare`
3. for monorepos, pick a target that declares workspace/reactor/multiproject structure and review file ownership and shared-lock attribution, not only package counts
4. check this guide if you see a meaningful delta
5. use the exported dataset workflow if you previously customized license/rule data in a ScanCode checkout
6. if your old workflow used `--include` to approximate explicit path lists, consider switching that part to `--paths-file`

## Other differences worth knowing

- Provenant may resolve some explicit project-root `LICENSE` references a bit differently in nested or vendored trees because it allows a bounded ancestor lookup for clear root-directory notices.
- Provenant may add additive metadata fields of its own, such as `license_index_provenance`, so strict JSON consumers should tolerate extra non-ScanCode fields.

## Related docs

- [CLI Guide](CLI_GUIDE.md)
- [License Detection Architecture](LICENSE_DETECTION_ARCHITECTURE.md)
- [Intentional Differences and Improvements](improvements/README.md)
