// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Field metadata for the top-level output document, header, and run-environment surfaces.

use super::OutputFieldDoc;

pub(super) const OUTPUT_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "summary",
        rust_field: "summary",
        value_shape: "object",
        presence: "Emitted only when available.",
        meaning: "Codebase-level rollup derived during post-processing.",
    },
    OutputFieldDoc {
        json_name: "tallies",
        rust_field: "tallies",
        value_shape: "object",
        presence: "Emitted only when tally generation is enabled.",
        meaning: "Count-oriented rollup across the full scan result.",
    },
    OutputFieldDoc {
        json_name: "tallies_of_key_files",
        rust_field: "tallies_of_key_files",
        value_shape: "object",
        presence: "Emitted only when key-file tallies are available.",
        meaning: "Tally rollup restricted to files treated as key files for summary scoring.",
    },
    OutputFieldDoc {
        json_name: "tallies_by_facet",
        rust_field: "tallies_by_facet",
        value_shape: "array<object>",
        presence: "Emitted only when facet tallies were requested.",
        meaning: "Facet-scoped tally output for user-defined facet groupings.",
    },
    OutputFieldDoc {
        json_name: "headers",
        rust_field: "headers",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Per-run metadata blocks describing tool/version/options and scan environment.",
    },
    OutputFieldDoc {
        json_name: "packages",
        rust_field: "packages",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Assembled top-level package records visible on the public output contract.",
    },
    OutputFieldDoc {
        json_name: "dependencies",
        rust_field: "dependencies",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Top-level dependency records emitted after assembly and hoisting.",
    },
    OutputFieldDoc {
        json_name: "license_detections",
        rust_field: "license_detections",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Top-level grouped license detections across the scanned codebase.",
    },
    OutputFieldDoc {
        json_name: "files",
        rust_field: "files",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "File and directory records with the main per-resource findings surface.",
    },
    OutputFieldDoc {
        json_name: "license_references",
        rust_field: "license_references",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Top-level unique license reference blocks for emitted detections.",
    },
    OutputFieldDoc {
        json_name: "license_rule_references",
        rust_field: "license_rule_references",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Top-level unique rule reference blocks for emitted detections.",
    },
];

pub(super) const HEADER_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "tool_name",
        rust_field: "tool_name",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Scanner tool name recorded for the run.",
    },
    OutputFieldDoc {
        json_name: "tool_version",
        rust_field: "tool_version",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Scanner version recorded for the run.",
    },
    OutputFieldDoc {
        json_name: "options",
        rust_field: "options",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "Serialized scan options so downstream readers can interpret how the run was performed.",
    },
    OutputFieldDoc {
        json_name: "notice",
        rust_field: "notice",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Run-level legal or attribution notice emitted by the scanner.",
    },
    OutputFieldDoc {
        json_name: "start_timestamp",
        rust_field: "start_timestamp",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Run start timestamp.",
    },
    OutputFieldDoc {
        json_name: "end_timestamp",
        rust_field: "end_timestamp",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Run end timestamp.",
    },
    OutputFieldDoc {
        json_name: "output_format_version",
        rust_field: "output_format_version",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Version of the public output format contract used by this run.",
    },
    OutputFieldDoc {
        json_name: "duration",
        rust_field: "duration",
        value_shape: "number",
        presence: "Always emitted.",
        meaning: "Wall-clock scan duration recorded for the run.",
    },
    OutputFieldDoc {
        json_name: "errors",
        rust_field: "errors",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Run-level errors recorded in the header rather than on a specific file.",
    },
    OutputFieldDoc {
        json_name: "warnings",
        rust_field: "warnings",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Run-level warnings recorded in the header rather than on a specific file.",
    },
    OutputFieldDoc {
        json_name: "extra_data",
        rust_field: "extra_data",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "Scanner-owned counts and provenance metadata that augment the header contract.",
    },
];

pub(super) const EXTRA_DATA_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "spdx_license_list_version",
        rust_field: "spdx_license_list_version",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "The SPDX license list version associated with the effective license data used for the run.",
    },
    OutputFieldDoc {
        json_name: "files_count",
        rust_field: "files_count",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Count of file records seen by the scan pipeline.",
    },
    OutputFieldDoc {
        json_name: "directories_count",
        rust_field: "directories_count",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Count of directory records seen by the scan pipeline.",
    },
    OutputFieldDoc {
        json_name: "excluded_count",
        rust_field: "excluded_count",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Count of paths excluded before file processing completed.",
    },
    OutputFieldDoc {
        json_name: "license_index_provenance",
        rust_field: "license_index_provenance",
        value_shape: "object",
        presence: "Emitted only when index provenance is available.",
        meaning: "Provenance metadata for the effective embedded or custom license index used during detection.",
    },
];

pub(super) const SYSTEM_ENVIRONMENT_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "operating_system",
        rust_field: "operating_system",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Operating-system name recorded for the scan environment.",
    },
    OutputFieldDoc {
        json_name: "cpu_architecture",
        rust_field: "cpu_architecture",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "CPU architecture recorded for the scan environment.",
    },
    OutputFieldDoc {
        json_name: "platform",
        rust_field: "platform",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Platform family recorded for the scan environment.",
    },
    OutputFieldDoc {
        json_name: "platform_version",
        rust_field: "platform_version",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Platform version recorded for the scan environment.",
    },
    OutputFieldDoc {
        json_name: "rust_version",
        rust_field: "rust_version",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Rust toolchain version used by the scanner binary.",
    },
];

pub(super) const LICENSE_INDEX_PROVENANCE_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "source",
        rust_field: "source",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Source lane for the effective license index, such as embedded or custom dataset.",
    },
    OutputFieldDoc {
        json_name: "dataset_fingerprint",
        rust_field: "dataset_fingerprint",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Stable fingerprint for the effective license dataset contents.",
    },
    OutputFieldDoc {
        json_name: "ignored_rules",
        rust_field: "ignored_rules",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Rule identifiers excluded while building the effective index.",
    },
    OutputFieldDoc {
        json_name: "ignored_licenses",
        rust_field: "ignored_licenses",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "License keys excluded while building the effective index.",
    },
    OutputFieldDoc {
        json_name: "ignored_rules_due_to_licenses",
        rust_field: "ignored_rules_due_to_licenses",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Rules excluded indirectly because their owning licenses were excluded.",
    },
    OutputFieldDoc {
        json_name: "added_rules",
        rust_field: "added_rules",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Rule identifiers added by local overlays or custom dataset input.",
    },
    OutputFieldDoc {
        json_name: "replaced_rules",
        rust_field: "replaced_rules",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Rule identifiers replaced by local overlays or custom dataset input.",
    },
    OutputFieldDoc {
        json_name: "added_licenses",
        rust_field: "added_licenses",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "License keys added by local overlays or custom dataset input.",
    },
    OutputFieldDoc {
        json_name: "replaced_licenses",
        rust_field: "replaced_licenses",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "License keys replaced by local overlays or custom dataset input.",
    },
];
