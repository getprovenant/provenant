# Hugging Face Parser: Model/Dataset Repository Metadata and `pkg:huggingface` PURLs

## Summary

Provenant now statically extracts package metadata from checked-out Hugging Face
model and dataset repositories and emits `pkg:huggingface` Package URLs. It parses
three file surfaces:

- **Model-card `README.md`** — YAML frontmatter (`license`, `tags`, `language`,
  `library_name`, `pipeline_tag`, `base_model`, `datasets`, `model_name`, …).
- **Transformers `config.json`** — model configuration (`model_type`,
  `architectures`, `transformers_version`, `_name_or_path`, and legacy
  architecture hyperparameters).
- **Diffusers `model_index.json`** — pipeline configuration (`_class_name`,
  `_diffusers_version`, `_name_or_path`).

## Reference limitation

The Python ScanCode Toolkit reference does not ship a Hugging Face parser, so
Hugging Face model/dataset provenance is currently invisible to a scan. This work
adds that coverage. There is no ScanCode parity lane for `huggingface`.

## Design

### Identity is an honest unknown

The purl-spec `huggingface` type is `pkg:huggingface/<namespace>/<name>@<revision>`,
where `<namespace>/<name>` is the repository id and `<revision>` is the git commit
hash. Neither is reliably present in tracked files:

- The repository id lives in the remote URL / `.git` config, not in a tracked
  artifact.
- The revision is git state, not checked-in content.

The only checked-in identity hint is `_name_or_path` in `config.json` /
`model_index.json` (written by `save_pretrained` as the `<namespace>/<name>` the
weights were loaded from) and, less commonly, `model_name` in the model-card
frontmatter.

Following Provenant's honest-unknown guidance, the parser emits a
`pkg:huggingface/<namespace>/<name>` PURL **only** when one of those fields has the
unambiguous `<namespace>/<name>` shape. When identity cannot be proven, the parser
omits the PURL (no top-level package is assembled) but still reports the provable
facts: declared license, `base_model`/`datasets` dependencies, keywords/tags, and
architecture metadata in `extra_data`. The `@<revision>` qualifier is always
omitted because no tracked file proves it.

### Declared license and dependencies

- `license` (or `license_name`) from the model card is normalized through the
  shared SPDX declared-license helper. The `license` field is accepted as a
  scalar or the first element of a list, since real cards use both.
- `base_model` references become `base_model`-scoped dependencies and `datasets`
  references become `dataset`-scoped dependencies, each as a bare
  `pkg:huggingface/...` PURL.

### Assembly

Each recognized file with a derivable identity produces its own package
(`OnePerPackageData`). The files are intentionally **not** sibling-merged into one
logical model package: `config.json` and `README.md` are common filenames, and
cross-file merging would be unsafe without proving the files describe the same
model. Cross-file model-package assembly is a deferred follow-up.

## Real-repository verification

Verified against public Hugging Face repositories (LFS weights skipped):

- `prajjwal1/bert-tiny` — legacy `config.json` (only architecture
  hyperparameters, no `model_type`) and a model card whose `license` is a
  single-element list. Both files are now recognized; the card yields
  `license = MIT` and tag keywords. Neither file carries `_name_or_path`, so no
  identity PURL is emitted — the honest, no-guess outcome.
- `hf-internal-testing/tiny-random-bert` — modern `config.json` with
  `model_type` and `transformers_version` captured in `extra_data`; still no
  `_name_or_path`, so no identity PURL.
- A synthetic `config.json` with `_name_or_path: "google-bert/bert-base-uncased"`
  produces a top-level `pkg:huggingface/google-bert/bert-base-uncased` package.

These confirm the dominant real-world case: most uploaded repositories strip
`_name_or_path`, so identity PURLs are rare in practice while license/tag/
architecture facts are still recovered.

## Known limitations and deferrals

- **Identity coverage is intentionally narrow.** Without a checked-in
  `_name_or_path`/`model_name`, no PURL is emitted. Recovering identity from the
  git remote belongs in assembly/scanner topology, not a file-local parser, and
  is deferred.
- **No cross-file model assembly.** The README, `config.json`, and
  `model_index.json` of one repository are not merged into a single logical
  package yet.
- **Model-card detection is heuristic.** A `README.md` with frontmatter carrying
  a Hugging Face model-card key (e.g. `license`, `tags`, `base_model`) is treated
  as a model card. This can over-claim a non-Hugging-Face README that happens to
  use those frontmatter keys, but the result is bounded: file-level package
  metadata only, never a guessed top-level identity, and file-content license and
  copyright detection still run normally.
