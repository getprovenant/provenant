# License Detection Architecture

## Overview

The license detection system is a multi-phase, multi-strategy detection engine that identifies license information in source code and text files. It supports exact matching, fuzzy matching, and unknown license detection through a pipeline of increasingly sophisticated algorithms.

---

## Entry Points

### CLI Entry Points

| Entry point                                 | Purpose                                                |
| ------------------------------------------- | ------------------------------------------------------ |
| `provenant scan --license-dataset-path`     | Override to load a custom license dataset root         |
| `provenant export-license-dataset <DIR>`    | Export the effective embedded license dataset and exit |
| `provenant scan --license`                  | Enable license scanning                                |
| `provenant scan --license-text`             | Include matched text in output                         |
| `provenant scan --license-text-diagnostics` | Highlight unmatched words inside matched text          |
| `provenant scan --license-diagnostics`      | Include detection post-processing diagnostics          |
| `provenant scan --license-references`       | Emit top-level license and rule reference blocks       |
| `provenant scan --license-score`            | Filter returned license detections by minimum score    |
| `provenant scan --license-url-template`     | Customize top-level `licensedb_url` references         |
| `provenant scan --reindex`                  | Force rebuild of the license index cache               |
| `provenant scan --no-license-index-cache`   | Disable persistent license index cache reads/writes    |

**Default behavior**: Uses the built-in embedded license index. No external files required.

> This document describes the current public license-detection surface and the repository's current engine/module layout.

**Custom license datasets**: Use `--license-dataset-path /path/to/dataset-root` to load from a custom dataset root containing `manifest.json`, `rules/`, and `licenses/`. This is an advanced override rather than the recommended default workflow; normal scans should keep using the embedded artifact. Custom-dataset scans are cached using a content fingerprint of the loaded effective rules/licenses, so the index is rebuilt automatically when the dataset changes.

**Dataset export**: Use `provenant export-license-dataset /path/to/output-dir` to dump the effective embedded dataset into that dataset-root layout so you can inspect or edit it before reusing it with `provenant scan --license-dataset-path`.

**Index build policy**: Provenant also applies a checked-in build policy from `resources/license_detection/index_build_policy.toml` before fingerprinting and index construction. This manifest carries the small curation decisions (ignored rule/license ids plus required rationale entries for every bundled overlay file), while downstream add/replace overlays live as real ScanCode-format files under `resources/license_detection/overlay/`. This keeps local curation in the same `.RULE` / `.LICENSE` syntax as upstream without severing the broader dataset dependency, while still making every downstream overlay carry an audit trail in one central manifest. Stale ignore ids, stale overlay-reason entries, undocumented overlays, and overlays that become identical to upstream now fail the build so maintainers get an explicit prompt to remove redundant downstream curation. When a license-detection fix is really rule or license data curation—such as reclassifying one upstream rule, tightening one rule's minimum coverage, or adding a required phrase—prefer an overlay first and only reach for matcher or refinement code when the problem spans multiple rule families or exposes a true engine-level bug.

### License Index Cache

Provenant caches the built `LicenseIndex` as an rkyv-serialized file to avoid rebuilding it on every run. The cache reduces license engine startup from ~12s (cold) to ~0.8s (warm, release build).

- **Format**: Fingerprinted rkyv files under the shared cache root, for example `license-index/embedded/<fingerprint>.rkyv` or `license-index/custom/<fingerprint>.rkyv`, each with a 32-byte SHA-256 fingerprint prefix
- **Default location**: Under the shared cache root selected by `--cache-dir`, `PROVENANT_CACHE`, or the platform-native default
- **Opt-out**: `--no-license-index-cache` skips both persistent reads and persistent writes for that run
- **Invalidation**: Automatic when the source rules change (fingerprint mismatch) or when `--reindex` is passed
- **Fingerprinting**: Embedded rules use SHA-256 of the raw artifact bytes; custom license datasets use SHA-256 of the sorted loaded rules and licenses

### Current Public Output Surface

When license scanning is enabled, the current ScanCode-style public surface is:

- file-level `license_detections`
- file-level `license_clues`
- file/package `detection_log` when `--license-diagnostics` is enabled
- match-level `matched_text` under `--license-text`
- match-level `matched_text_diagnostics` under `--license-text-diagnostics`
- file-level `percentage_of_license_text`
- top-level unique `license_detections`
- top-level `license_references` and `license_rule_references`

`--from-json` preserves preexisting top-level license reference blocks and can
recompute the top-level license outputs when the loaded scan is reshaped or when
license-reference generation is explicitly requested.

This document is the evergreen maintainer reference for the current public license-detection surface. It documents the live contract and module layout; it is not a claim that every downstream parity gap is closed.

### `LicenseRef-*` namespace convention

Provenant uses the shared `LicenseRef-scancode-*` namespace for SPDX-side `LicenseRef` identifiers.

If a license key is backed by the loaded ScanCode-compatible license dataset or by the public
output contract built on that dataset, reuse the dataset-owned `LicenseRef-scancode-*` identifier
instead of minting a tool-specific variant. This applies across detection output, SPDX-LID
fallbacks such as `unknown-spdx`, and parser-side declared-license normalization for keys such as
`public-domain`, `proprietary-license`, and `unknown-license-reference`.

### Initialization Flow

```text
main.rs::init_license_engine()
    │
    ├── No --license-dataset-path specified (default)
    │       ↓
    │   Compute SHA-256 fingerprint of embedded artifact bytes
    │       ↓
    │   Cache hit (fingerprint matches)?
    │       ├── Yes → Load CachedLicenseIndex from rkyv cache
    │       │         → Convert to LicenseIndex
    │       └── No  → Decompress embedded artifact (zstd)
    │                 → Deserialize LoadedRule/LoadedLicense (MessagePack)
    │                 → Build LicenseIndex
    │                 → Save rkyv cache with fingerprint prefix
    │
    └── --license-dataset-path specified
            ↓
        Validate dataset root (manifest.json + rules/ + licenses/)
            ↓
        Load .LICENSE and .RULE files from dataset directories
            ↓
        Compute SHA-256 fingerprint of sorted effective rules + licenses
            ↓
        Cache hit (fingerprint matches)?
            ├── Yes → Load CachedLicenseIndex from rkyv cache
            │         → Convert to LicenseIndex
            └── No  → Build LicenseIndex
                      → Save rkyv cache with fingerprint prefix
            ↓
    Arc<LicenseDetectionEngine> shared across scanner threads
```

---

## Embedded License Index

The binary includes a pre-built license index embedded at compile time:

- **Location**: `resources/license_detection/license_index.zst`
- **Format**: MessagePack serialization, zstd compression
- **Contents**: sorted `LoadedRule` and `LoadedLicense` values derived from the ScanCode rules dataset

### Loader/Build Stage Separation

The loading process is split into two distinct stages:

**Artifact Generation Stage** (when producing `license_index.zst`):

- Parse `.RULE` and `.LICENSE` files
- Normalize rule and license data for embedding
- Apply the checked-in license-index build policy
- Apply any checked-in downstream overlay files from `resources/license_detection/overlay/`
- Fail fast if an ignore id no longer exists upstream, an overlay-reason entry is stale or missing, or an overlay file becomes identical to upstream data
- Sort embedded rules and licenses deterministically
- Serialize the embedded loader snapshot with MessagePack
- Compress the serialized bytes with zstd

**Build Stage** (runtime):

- Validate the embedded artifact payload and schema version
- Deserialize the embedded loader snapshot
- Convert embedded rules/licenses into the runtime `LicenseIndex`
- Apply deprecated filtering policy
- Synthesize license-derived rules
- Build token dictionary and automatons
- Create `LicenseIndex` and `SpdxMapping`

The active dataset identity is surfaced in structured outputs at `headers[0].extra_data.license_index_provenance`, which includes the dataset source, dataset fingerprint, and, for embedded datasets, the exact ignored/added/replaced rule and license identifiers applied to build that embedded dataset.

This separation enables:

- Self-contained binaries with no external dependencies
- Self-contained startup without filesystem parsing of the ScanCode rules directory at runtime
- Consistent rule loading across all installations

### Regenerating the Embedded Artifact

When the ScanCode rules dataset is updated, regenerate the embedded artifact:

```sh
# Initialize the reference submodule (contains the rules dataset)
./setup.sh

# Regenerate the artifact
cargo run --manifest-path xtask/Cargo.toml --bin generate-index-artifact

# If the ignore policy changed, regenerate the artifact too
# (the generated snapshot is expected to reflect that policy)

# Commit the updated artifact
git add resources/license_detection/license_index.zst
git commit -m "chore: update embedded license data"
```

---

## Core Components

### LicenseDetectionEngine

**File**: `src/license_detection/mod.rs`

The orchestrator that coordinates the detection pipeline. Its durable responsibilities are to hold the prebuilt runtime index, expose the public load/construct entry points, and bridge internal license keys to SPDX-facing output.

### LicenseIndex

**File**: `src/license_detection/index/mod.rs`

Pre-computed data structures for efficient matching:

| Field                   | Purpose                                      |
| ----------------------- | -------------------------------------------- |
| `dictionary`            | Token → ID mapping                           |
| `len_legalese`          | Count of high-value legalese tokens          |
| `digit_only_tids`       | Set of digit-only token IDs                  |
| `rules_by_rid`          | Rules indexed by ID                          |
| `tids_by_rid`           | Token ID sequences per rule                  |
| `rid_by_hash`           | SHA1 hash → rule ID (exact match)            |
| `rules_automaton`       | Aho-Corasick automaton for all rules         |
| `unknown_automaton`     | Automaton for unknown license detection      |
| `sets_by_rid`           | Unique token sets per rule                   |
| `msets_by_rid`          | Token frequency maps per rule                |
| `high_postings_by_rid`  | Inverted index for candidate selection       |
| `regular_rids`          | Set of regular (non-false-positive) rule IDs |
| `false_positive_rids`   | Set of false-positive rule IDs               |
| `approx_matchable_rids` | Set of approx-matchable rule IDs             |
| `licenses_by_key`       | ScanCode key → License mapping               |
| `rid_by_spdx_key`       | SPDX license key → rule ID                   |
| `unknown_spdx_rid`      | Rule ID for unknown-spdx fallback            |

### Query

**File**: `src/license_detection/query/mod.rs`

Tokenized input text ready for matching. The stable concepts in a query are the original text, normalized/tokenized positions, matchable-token partitions, binary/text context, segmented query runs, SPDX identifier lines, and a reference to the loaded index.

Also includes `QueryRun` values created on demand from stored run ranges.

### Key Data Models

**File**: `src/license_detection/models/mod.rs`

| Struct         | Purpose                                                                 |
| -------------- | ----------------------------------------------------------------------- |
| `License`      | License metadata from .LICENSE files                                    |
| `Rule`         | Matchable pattern with flags (is_license_text, is_license_notice, etc.) |
| `LicenseMatch` | Single match result with score, position, matcher type                  |

---

## Detection Pipeline

```text
INPUT: File text content
        │
        ▼
┌───────────────────┐
│ 1. TOKENIZATION   │  text → Query (tokens, positions, matchables)
└─────────┬─────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 2. MATCHING (Priority Order)                                        │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 1a. HASH MATCH (1-hash)                                     │    │
│  │     • SHA1 of token sequence → lookup in rid_by_hash        │    │
│  │     • 100% confidence, immediate return if found            │    │
│  └─────────────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 1b. SPDX-LID MATCH (1-spdx-id)                              │    │
│  │     • Parse SPDX-License-Identifier tags                    │    │
│  │     • Handle AND, OR, WITH expressions                      │    │
│  └─────────────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 1c. AHO-CORASICK MATCH (2-aho)                              │    │
│  │     • Multi-pattern exact matching via automaton            │    │
│  │     • Find all overlapping matches                          │    │
│  └─────────────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 2. NEAR-DUPLICATE MATCH                                     │    │
│  │     • Set similarity >= 0.8 for whole query                 │    │
│  │     • Sequence matching on top candidates                   │    │
│  └─────────────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 3. SEQUENCE MATCH (3-seq)                                   │    │
│  │     • Set-based candidate selection                         │    │
│  │     • Sequence alignment for scoring                        │    │
│  └─────────────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 4. QUERY RUN MATCH                                          │    │
│  │     • Process segmented regions separately                  │    │
│  │     • Skip already-matched regions                          │    │
│  └─────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌───────────────────┐
│ 3. UNKNOWN MATCH  │  Detect license-like text in unmatched regions
└─────────┬─────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 4. REFINEMENT                                                       │
│     • Merge overlapping matches                                     │
│     • Filter false positives                                        │
│     • Filter too-short matches                                      │
│     • Validate required phrases                                     │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌───────────────────┐
│ 5. GROUPING       │  Group nearby matches (within 4 lines)
└─────────┬─────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 6. DETECTION CREATION                                               │
│     • Combine and simplify equivalent expressions from matches       │
│     • Convert to SPDX identifiers while preserving SPDX casing       │
│     • Classify detection quality                                    │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
OUTPUT: Vec<LicenseDetection>
```

---

## Matching Algorithms

### 1. Hash Match (1-hash)

**File**: `src/license_detection/hash_match.rs`

- **Purpose**: Exact match via SHA1 hash lookup
- **Complexity**: O(n) tokenization + O(1) lookup
- **Confidence**: 100%

### 2. SPDX-LID Match (1-spdx-id)

**File**: `src/license_detection/spdx_lid/mod.rs`

- **Purpose**: Detect `SPDX-License-Identifier:` tags
- **Handles**: Simple identifiers, expressions (AND, OR, WITH), WITH exceptions

### 3. Aho-Corasick Match (2-aho)

**File**: `src/license_detection/aho_match.rs`

- **Purpose**: Multi-pattern exact matching
- **Complexity**: O(n + m) where n = query length, m = matches
- **Process**: Encode tokens as bytes → run through automaton → verify positions

### 4. Sequence Match (3-seq)

**File**: `src/license_detection/seq_match/mod.rs`

- **Purpose**: Approximate/fuzzy matching for modified licenses
- **Phases**:
  1. Candidate selection via set similarity (Jaccard index)
  2. Ranking by containment, resemblance, matched length
  3. Sequence alignment for final scoring

Candidate ranking uses a compact score vector built from resemblance, containment, matched length, and a deterministic rule-ID tie-breaker.

### 5. Unknown Match (6-unknown)

**File**: `src/license_detection/unknown_match.rs`

- **Purpose**: Detect license-like text in unmatched regions
- **Process**: Find gaps → search with n-gram automaton (n=6) → count matches

---

## License Data Loading

### Source Files (for custom datasets / regeneration)

**Location**: `reference/scancode-toolkit/src/licensedcode/data/`

| Directory   | Contents                                      |
| ----------- | --------------------------------------------- |
| `licenses/` | `.LICENSE` files (full license texts)         |
| `rules/`    | `.RULE` files (patterns, notices, references) |

> **Note**: The reference submodule is optional for end users. The default embedded license index is already included in the binary.

### File Format

Each file has YAML frontmatter:

```yaml
---
key: mit
name: MIT License
spdx_license_key: MIT
category: Permissive
is_license_text: true
---
MIT License text here...
```

### Index Building

**File**: `src/license_detection/index/builder/mod.rs`

Steps:

1. Load legalese tokens (high-value words)
2. Build token dictionary (assign integer IDs)
3. Tokenize each rule text
4. Compute SHA1 hash for each rule → `rid_by_hash`
5. Build Aho-Corasick automaton
6. Build sets/msets for candidate selection
7. Compute match thresholds

### Token Dictionary

**File**: `src/license_detection/index/dictionary.rs`

Token ID assignment order:

1. **Legalese tokens** (IDs 0..N-1): High-value words like "license", "copyright"
2. **Regular tokens** (IDs N..): Other words from rules

---

## Subsystem Layout

The subsystem is easier to maintain when described by responsibility rather than exact module paths:

| Subsystem               | Responsibility                                                                                        |
| ----------------------- | ----------------------------------------------------------------------------------------------------- |
| Engine orchestration    | load the embedded/custom rule set and expose the public detection surface                             |
| Query preparation       | tokenize text, preserve position data, segment query runs, and surface SPDX lines                     |
| Runtime index           | hold token dictionaries, rule metadata, automata, and candidate-selection structures                  |
| Matchers                | exact hash/SPDX matching, Aho-Corasick matching, approximate sequence matching, and unknown detection |
| Refinement and grouping | merge/filter matches and build grouped detections                                                     |
| Output mapping          | convert internal detections into public ScanCode-style and SPDX-facing structures                     |

Use the `src/license_detection/` tree for the current file/module layout.

---

## Constants and Thresholds

The durable thresholds to understand are:

- **matcher identifiers** remain stable enough to appear in public diagnostics (`1-hash`, `1-spdx-id`, `2-aho`, `3-seq`, `5-undetected`, `6-unknown`)
- **grouping proximity** controls when nearby matches collapse into a single detection
- **near-duplicate thresholds** control resemblance-based candidate admission
- **small/tiny rule thresholds** adjust how aggressively very short rules are treated

Exact numeric values are owned by the detector code and tests rather than this architecture doc.

---

## Output Structure

The engine still carries richer internal detection metadata than the current
public ScanCode-style JSON output. `detection_log`, clue-only serialization, and
matched-text diagnostics are now preserved publicly, and internal detections now
carry real file-region metadata for unique aggregation. File/resource
reference-following now consumes that metadata internally, but some downstream
package/reference consumers are still not fully represented in the current
serialized surfaces.

The current public-output structure and file-region-aware aggregation behavior
are described here.

### Internal Detection Structure

Internally, a detection carries the normalized license expressions, the underlying matches, optional diagnostic/classification logs, a stable identifier when grouping requires one, and internal file-region aggregation metadata.

### JSON Output Example

The current public JSON output still omits `file_region`, but it does preserve
`detection_log` on public detections. `file_regions` remain internal aggregation metadata used by detector and downstream post-processing flows.

```json
{
  "license_expression": "mit",
  "license_expression_spdx": "MIT",
  "matches": [
    {
      "license_expression": "mit",
      "matcher": "2-aho",
      "score": 1.0,
      "match_coverage": 100.0,
      "start_line": 1,
      "end_line": 20
    }
  ]
}
```

### Local File Reference Following

After raw license detection, `src/post_processing/reference_following.rs` may merge detections from
referenced local files such as `LICENSE`, `COPYING`, or manifest-adjacent sidecars.

The default lookup stays conservative: current directory, package-manifest directories, then the
current scan root / repo root.

For notices that explicitly say the license file lives in the **root directory of this source
tree** or equivalent project-root language, Provenant also allows a bounded ancestor lookup within
the current scan root. Plain `See LICENSE` notices stay on the conservative path, and incompatible
ancestor targets are rejected.

This is an intentional product-quality difference from current ScanCode behavior, not a schema
change.

## Related Documentation

- [CLI_GUIDE.md](CLI_GUIDE.md) - user-facing workflows for the public license flags documented here
- [TESTING_STRATEGY.md](TESTING_STRATEGY.md) - verification layers and fixture ownership for license-related golden coverage
- [ARCHITECTURE.md](ARCHITECTURE.md) - broader pipeline placement and cross-subsystem boundaries
