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
unambiguous `<namespace>/<name>` shape. When identity cannot be proven, the PURL is
omitted but the model is still assembled into a package (see Assembly) reporting the
provable facts: declared license, `base_model`/`datasets` dependencies,
keywords/tags, and architecture metadata in `extra_data`. The `@<revision>`
qualifier is always omitted because no tracked file proves it.

### Declared license and dependencies

- `license` (or `license_name`) from the model card is normalized through the
  shared SPDX declared-license helper. The `license` field is accepted as a
  scalar or the first element of a list, since real cards use both.
- `base_model` references become `base_model`-scoped dependencies and `datasets`
  references become `dataset`-scoped dependencies, each as a bare
  `pkg:huggingface/...` PURL.

### Assembly

The model-card `README.md`, Transformers `config.json`, and Diffusers
`model_index.json` in one repository directory describe a single logical model,
so a dedicated directory merger (`src/assembly/huggingface_merge.rs`) combines
them into **one** package. License, tags, language, architecture metadata, and
`base_model`/`dataset` dependencies are unified onto that package, and both
datafiles are attributed to it.

Merging is safe because only `PackageData` the Hugging Face parsers actually
claim participates — a generic `README.md`/`config.json` that the parsers
decline (because it lacks the required model-card / config signals) never
triggers a merge. When more than one source proves an identity, the merger
anchors on the `config.json`/`model_index.json` `_name_or_path` (the more
reliable repository-id hint) over the model-card `model_name`.

When **no** source proves an identity, the files still merge into one
identity-less package (`purl` omitted) so the model's provable facts are
reported together rather than lost — the honest no-guess outcome.

## Real-repository verification

Verified by scanning cloned public Hugging Face repositories with
`provenant scan <dir> --package` (LFS weights skipped, `.git` present):

- `prajjwal1/bert-tiny` — legacy `config.json` (only architecture
  hyperparameters, no `model_type`/`_name_or_path`) and a model card whose
  `license` is a single-element list (`language`, `license`, `tags`). The two
  files **merge into one package** (its `datasource_ids` list both
  `huggingface_config_json` and `huggingface_model_card`, with both datafiles
  attributed) reporting `license = MIT`, the tag keywords, and `language`. Its
  `purl` is `None`: neither file carries `_name_or_path`, and the true id
  `prajjwal1/bert-tiny` in `.git/config` is intentionally not read (see Inherent
  limitations). The honest, no-guess outcome.
- `hf-internal-testing/tiny-random-bert` and
  `hf-internal-testing/tiny-stable-diffusion-torch` — confirmed that uploaded
  repositories typically strip `_name_or_path`, so identity PURLs are rare in
  practice while license/tag/architecture facts are still recovered.
- A synthetic repository with `config.json` carrying
  `_name_or_path: "acme-ai/sentiment-demo"` plus a model card — scanning the
  directory produces exactly **one** `pkg:huggingface/acme-ai/sentiment-demo`
  package with the card's `MIT` license, the config's
  `model_type`/`architectures`/`transformers_version` in `extra_data`, and the
  `base_model`/`dataset` dependencies hoisted to the top level. This exercises
  the identity + cross-file merge path end to end and is locked by the Layer-3
  scanner/assembly test.

### Model-card detection heuristic

A `README.md` is claimed as a model card only when its YAML frontmatter carries
either:

- one **strong**, Hugging Face-distinctive key (`library_name`, `pipeline_tag`,
  `base_model`, `datasets`, `model-index`, `license_name`, `license_link`,
  `model_name`, `widget`, `co2_eq_emissions`), or
- at least **two weak** keys that also appear in generic front matter
  (`license`, `tags`, `language`, `metrics`, `inference`, `thumbnail`).

A single weak key alone (e.g. only `license`, or only `tags`) is not enough, so
an ordinary docs/blog post is not over-claimed, while a minimal real card
(e.g. `license` + `tags`) is still recognized.

## Inherent limitations

These are not deferrals; they are facts that cannot be statically proven from a
checked-out repository, so the parser does not guess them.

- **`@<revision>` is never emitted, and git-remote identity is out of scope.**
  The purl-spec revision is the git commit hash, and the canonical repository id
  lives in the git remote URL. Both live under `.git/` (remote URL in
  `.git/config`; commit in `.git/HEAD` + `packed-refs`), which Provenant
  **excludes from scanning by repo-wide policy** (`.git`, `.hg`, `.svn` are
  default collection excludes, alongside `.gitignore`). VCS internal state is
  deliberately not treated as package content anywhere in the scanner, so the
  Hugging Face assembler does not reach into `.git` to recover identity. A bare
  checkout that ships neither a checked-in identity field
  (`_name_or_path`/`model_name`) nor in-scope identity metadata therefore yields
  a package without a PURL — the honest unknown. (Should this policy ever change,
  the natural home for git-derived identity is the assembler, which is already
  the topology-aware, cross-file layer; it is intentionally not a file-local
  parser concern.)
- **`_name_or_path` is a hint, not a guarantee.** When present with a
  `<namespace>/<name>` shape it is used as the identity, because it is the only
  in-scope checked-in identity signal. It records the weights' origin, which is
  usually but not always the repository's own id; Provenant prefers it over no
  identity rather than guessing from the directory name.
