// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputTypeDoc {
    pub type_name: &'static str,
    pub json_paths: &'static [&'static str],
    pub summary: &'static str,
    pub fields: &'static [OutputFieldDoc],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFieldDoc {
    pub json_name: &'static str,
    pub rust_field: &'static str,
    pub value_shape: &'static str,
    pub presence: &'static str,
    pub meaning: &'static str,
}

const OUTPUT_FIELDS: &[OutputFieldDoc] = &[
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

const HEADER_FIELDS: &[OutputFieldDoc] = &[
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

const EXTRA_DATA_FIELDS: &[OutputFieldDoc] = &[
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

const SUMMARY_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "declared_license_expression",
        rust_field: "declared_license_expression",
        value_shape: "string",
        presence: "Emitted only when the summary can derive a declared expression.",
        meaning: "Best summary-level declared license rollup derived from key files and assembled package data.",
    },
    OutputFieldDoc {
        json_name: "license_clarity_score",
        rust_field: "license_clarity_score",
        value_shape: "object",
        presence: "Emitted only when clarity scoring is available.",
        meaning: "Structured clarity signal explaining how complete and trustworthy the summary-level licensing evidence looks.",
    },
    OutputFieldDoc {
        json_name: "other_license_expressions",
        rust_field: "other_license_expressions",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "Secondary license expressions that contributed to the summary but were not chosen as the primary declared expression.",
    },
    OutputFieldDoc {
        json_name: "other_holders",
        rust_field: "other_holders",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "Secondary holders that contributed to the summary but were not chosen as the primary holder.",
    },
    OutputFieldDoc {
        json_name: "other_languages",
        rust_field: "other_languages",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "Secondary languages that contributed to the summary but were not chosen as the primary language.",
    },
];

const FILE_INFO_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "path",
        rust_field: "path",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Scan-root-relative path for the file or directory record.",
    },
    OutputFieldDoc {
        json_name: "type",
        rust_field: "file_type",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "File-vs-directory type discriminator on the public output surface.",
    },
    OutputFieldDoc {
        json_name: "name",
        rust_field: "name",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Basename of the file or directory record.",
    },
    OutputFieldDoc {
        json_name: "base_name",
        rust_field: "base_name",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Basename without extension.",
    },
    OutputFieldDoc {
        json_name: "extension",
        rust_field: "extension",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Filename extension including the leading dot when one exists.",
    },
    OutputFieldDoc {
        json_name: "size",
        rust_field: "size",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "File size in bytes for files, or zero for synthetic/default directory rows.",
    },
    OutputFieldDoc {
        json_name: "date",
        rust_field: "date",
        value_shape: "string | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "File date metadata on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "sha1",
        rust_field: "sha1",
        value_shape: "string | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "SHA-1 checksum on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "md5",
        rust_field: "md5",
        value_shape: "string | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "MD5 checksum on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "sha256",
        rust_field: "sha256",
        value_shape: "string | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "SHA-256 checksum on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "sha1_git",
        rust_field: "sha1_git",
        value_shape: "string | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Git-style SHA-1 checksum on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "mime_type",
        rust_field: "mime_type",
        value_shape: "string | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Detected MIME type on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "file_type",
        rust_field: "file_type_label",
        value_shape: "string | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Additional file-type label on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "programming_language",
        rust_field: "programming_language",
        value_shape: "string | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Language hint derived by file classification rather than package parsing.",
    },
    OutputFieldDoc {
        json_name: "is_binary",
        rust_field: "is_binary",
        value_shape: "boolean | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Binary-file hint on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "is_text",
        rust_field: "is_text",
        value_shape: "boolean | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Text-file hint on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "is_archive",
        rust_field: "is_archive",
        value_shape: "boolean | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Archive-file hint on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "is_media",
        rust_field: "is_media",
        value_shape: "boolean | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Media-file hint on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "is_source",
        rust_field: "is_source",
        value_shape: "boolean | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Source-file hint on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "is_script",
        rust_field: "is_script",
        value_shape: "boolean | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Script-file hint on the opt-in file-info surface.",
    },
    OutputFieldDoc {
        json_name: "files_count",
        rust_field: "files_count",
        value_shape: "integer | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Nested file count on directory/info records when available.",
    },
    OutputFieldDoc {
        json_name: "dirs_count",
        rust_field: "dirs_count",
        value_shape: "integer | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Nested directory count on directory/info records when available.",
    },
    OutputFieldDoc {
        json_name: "size_count",
        rust_field: "size_count",
        value_shape: "integer | null",
        presence: "Emitted only on the file-info surface.",
        meaning: "Aggregated nested size count on directory/info records when available.",
    },
    OutputFieldDoc {
        json_name: "package_data",
        rust_field: "package_data",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Raw parser-emitted package rows attached to this file record.",
    },
    OutputFieldDoc {
        json_name: "detected_license_expression_spdx",
        rust_field: "license_expression",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Primary SPDX-oriented file-level license expression after grouping detections and applying strict combination rules.",
    },
    OutputFieldDoc {
        json_name: "license_detections",
        rust_field: "license_detections",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Grouped license detections attached to this file record.",
    },
    OutputFieldDoc {
        json_name: "license_clues",
        rust_field: "license_clues",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "Low-confidence or clue-only matches that are surfaced separately from concrete detections.",
    },
    OutputFieldDoc {
        json_name: "percentage_of_license_text",
        rust_field: "percentage_of_license_text",
        value_shape: "number | null",
        presence: "Emitted only when a percentage was computed.",
        meaning: "Approximate proportion of the file that participated in license text matches.",
    },
    OutputFieldDoc {
        json_name: "copyrights",
        rust_field: "copyrights",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "File-level copyright evidence records.",
    },
    OutputFieldDoc {
        json_name: "holders",
        rust_field: "holders",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "File-level holder evidence records.",
    },
    OutputFieldDoc {
        json_name: "authors",
        rust_field: "authors",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "File-level author evidence records.",
    },
    OutputFieldDoc {
        json_name: "emails",
        rust_field: "emails",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "File-level extracted email records.",
    },
    OutputFieldDoc {
        json_name: "urls",
        rust_field: "urls",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "File-level extracted URL records.",
    },
    OutputFieldDoc {
        json_name: "for_packages",
        rust_field: "for_packages",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Package UIDs that this file record is attached to after assembly or file-reference resolution.",
    },
    OutputFieldDoc {
        json_name: "scan_errors",
        rust_field: "scan_errors",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Per-file problems that did not prevent the overall scan from completing.",
    },
    OutputFieldDoc {
        json_name: "license_policy",
        rust_field: "license_policy",
        value_shape: "array<object> | null",
        presence: "Emitted only when policy output is available.",
        meaning: "Policy decoration entries attached to the file’s license findings when policy evaluation ran.",
    },
    OutputFieldDoc {
        json_name: "is_generated",
        rust_field: "is_generated",
        value_shape: "boolean | null",
        presence: "Emitted only when generated-code classification ran.",
        meaning: "Generated-code signal on the info/classification surface.",
    },
    OutputFieldDoc {
        json_name: "source_count",
        rust_field: "source_count",
        value_shape: "integer | null",
        presence: "Emitted only when source counts are available.",
        meaning: "Count of nested source files on directory/info records when available.",
    },
    OutputFieldDoc {
        json_name: "is_legal",
        rust_field: "is_legal",
        value_shape: "boolean",
        presence: "Emitted only when true.",
        meaning: "Marks a file treated as legal-material evidence for key-file and summary logic.",
    },
    OutputFieldDoc {
        json_name: "is_manifest",
        rust_field: "is_manifest",
        value_shape: "boolean",
        presence: "Emitted only when true.",
        meaning: "Marks a file treated as a manifest-like key file for package or summary logic.",
    },
    OutputFieldDoc {
        json_name: "is_readme",
        rust_field: "is_readme",
        value_shape: "boolean",
        presence: "Emitted only when true.",
        meaning: "Marks a file treated as README-style descriptive project metadata.",
    },
    OutputFieldDoc {
        json_name: "is_top_level",
        rust_field: "is_top_level",
        value_shape: "boolean",
        presence: "Emitted only when true.",
        meaning: "Marks a file treated as top-level for summary or package-root reasoning, even if filesystem depth differs.",
    },
    OutputFieldDoc {
        json_name: "is_key_file",
        rust_field: "is_key_file",
        value_shape: "boolean",
        presence: "Emitted only when true.",
        meaning: "Marks a file that participates directly in summary and license clarity scoring.",
    },
    OutputFieldDoc {
        json_name: "is_referenced",
        rust_field: "is_referenced",
        value_shape: "boolean",
        presence: "Emitted only when true.",
        meaning: "Marks a file whose content was followed as referenced evidence from another scanned file or package record.",
    },
    OutputFieldDoc {
        json_name: "is_community",
        rust_field: "is_community",
        value_shape: "boolean",
        presence: "Emitted only when true.",
        meaning: "Marks a file treated as community-material evidence on the classification surface.",
    },
    OutputFieldDoc {
        json_name: "facets",
        rust_field: "facets",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Facet labels attached by user-defined facet rules for tally grouping.",
    },
    OutputFieldDoc {
        json_name: "tallies",
        rust_field: "tallies",
        value_shape: "object | null",
        presence: "Emitted only when file-level tallies were requested.",
        meaning: "Per-file or per-directory tally block emitted by detailed tally workflows.",
    },
];

const PACKAGE_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "type",
        rust_field: "package_type",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package ecosystem/type identifier on the public ScanCode-compatible surface.",
    },
    OutputFieldDoc {
        json_name: "namespace",
        rust_field: "namespace",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package namespace on the public package surface.",
    },
    OutputFieldDoc {
        json_name: "name",
        rust_field: "name",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package name on the public package surface.",
    },
    OutputFieldDoc {
        json_name: "version",
        rust_field: "version",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package version on the public package surface.",
    },
    OutputFieldDoc {
        json_name: "qualifiers",
        rust_field: "qualifiers",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "PURL-style qualifier key/value pairs. Empty object when qualifiers are absent.",
    },
    OutputFieldDoc {
        json_name: "subpath",
        rust_field: "subpath",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package subpath on the public package surface.",
    },
    OutputFieldDoc {
        json_name: "primary_language",
        rust_field: "primary_language",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Primary language associated with the package.",
    },
    OutputFieldDoc {
        json_name: "description",
        rust_field: "description",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package description.",
    },
    OutputFieldDoc {
        json_name: "release_date",
        rust_field: "release_date",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package release date.",
    },
    OutputFieldDoc {
        json_name: "parties",
        rust_field: "parties",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Party records attached to the package.",
    },
    OutputFieldDoc {
        json_name: "keywords",
        rust_field: "keywords",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Keywords attached to the package.",
    },
    OutputFieldDoc {
        json_name: "homepage_url",
        rust_field: "homepage_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package homepage URL.",
    },
    OutputFieldDoc {
        json_name: "download_url",
        rust_field: "download_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package download URL.",
    },
    OutputFieldDoc {
        json_name: "size",
        rust_field: "size",
        value_shape: "integer | null",
        presence: "Always emitted.",
        meaning: "Package size when known.",
    },
    OutputFieldDoc {
        json_name: "sha1",
        rust_field: "sha1",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package SHA-1 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "md5",
        rust_field: "md5",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package MD5 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "sha256",
        rust_field: "sha256",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package SHA-256 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "sha512",
        rust_field: "sha512",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package SHA-512 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "bug_tracking_url",
        rust_field: "bug_tracking_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package bug-tracker URL.",
    },
    OutputFieldDoc {
        json_name: "code_view_url",
        rust_field: "code_view_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package code-view URL.",
    },
    OutputFieldDoc {
        json_name: "vcs_url",
        rust_field: "vcs_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package VCS URL.",
    },
    OutputFieldDoc {
        json_name: "copyright",
        rust_field: "copyright",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package copyright string.",
    },
    OutputFieldDoc {
        json_name: "holder",
        rust_field: "holder",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package holder string.",
    },
    OutputFieldDoc {
        json_name: "declared_license_expression",
        rust_field: "declared_license_expression",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Primary declared license expression on the package record.",
    },
    OutputFieldDoc {
        json_name: "declared_license_expression_spdx",
        rust_field: "declared_license_expression_spdx",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "SPDX-form primary declared license expression on the package record.",
    },
    OutputFieldDoc {
        json_name: "license_detections",
        rust_field: "license_detections",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Structured declared or extracted license detections attached to the package record.",
    },
    OutputFieldDoc {
        json_name: "other_license_expression",
        rust_field: "other_license_expression",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Non-primary declared license text normalized into an auxiliary expression lane.",
    },
    OutputFieldDoc {
        json_name: "other_license_expression_spdx",
        rust_field: "other_license_expression_spdx",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "SPDX-form auxiliary non-primary declared license expression on the package record.",
    },
    OutputFieldDoc {
        json_name: "other_license_detections",
        rust_field: "other_license_detections",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Detections associated with the auxiliary or non-primary license lane.",
    },
    OutputFieldDoc {
        json_name: "extracted_license_statement",
        rust_field: "extracted_license_statement",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Raw extracted license statement on the package record.",
    },
    OutputFieldDoc {
        json_name: "notice_text",
        rust_field: "notice_text",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package notice text.",
    },
    OutputFieldDoc {
        json_name: "source_packages",
        rust_field: "source_packages",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Referenced source-package package URLs or identifiers associated with this package.",
    },
    OutputFieldDoc {
        json_name: "is_private",
        rust_field: "is_private",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Package-level private/public signal when the parser or datasource can state it confidently.",
    },
    OutputFieldDoc {
        json_name: "is_virtual",
        rust_field: "is_virtual",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Marks package records that represent virtual or synthetic package identities rather than concrete deliverables.",
    },
    OutputFieldDoc {
        json_name: "extra_data",
        rust_field: "extra_data",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "Datasource-specific structured metadata preserved without promoting it into the core package contract. Empty object when extra data is absent.",
    },
    OutputFieldDoc {
        json_name: "repository_homepage_url",
        rust_field: "repository_homepage_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Repository homepage URL for the package.",
    },
    OutputFieldDoc {
        json_name: "repository_download_url",
        rust_field: "repository_download_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Repository download URL for the package.",
    },
    OutputFieldDoc {
        json_name: "api_data_url",
        rust_field: "api_data_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "API data URL for the package.",
    },
    OutputFieldDoc {
        json_name: "purl",
        rust_field: "purl",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package URL for the package record.",
    },
    OutputFieldDoc {
        json_name: "package_uid",
        rust_field: "package_uid",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Stable package identifier used internally and on output links such as `for_packages`.",
    },
    OutputFieldDoc {
        json_name: "datafile_paths",
        rust_field: "datafile_paths",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Manifest or metadata file paths that contributed to this assembled package record.",
    },
    OutputFieldDoc {
        json_name: "datasource_ids",
        rust_field: "datasource_ids",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Datasource identifiers that explain which parser/input surfaces contributed to this package record.",
    },
];

const PACKAGE_DATA_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "type",
        rust_field: "package_type",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package ecosystem/type identifier on a file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "namespace",
        rust_field: "namespace",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package namespace on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "name",
        rust_field: "name",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package name on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "version",
        rust_field: "version",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package version on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "qualifiers",
        rust_field: "qualifiers",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "PURL-style qualifier key/value pairs on a file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "subpath",
        rust_field: "subpath",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package subpath on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "primary_language",
        rust_field: "primary_language",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Primary language associated with the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "description",
        rust_field: "description",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package description on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "release_date",
        rust_field: "release_date",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Release date on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "parties",
        rust_field: "parties",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Party records attached to the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "keywords",
        rust_field: "keywords",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Keywords attached to the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "homepage_url",
        rust_field: "homepage_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Homepage URL on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "download_url",
        rust_field: "download_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Download URL on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "size",
        rust_field: "size",
        value_shape: "integer | null",
        presence: "Always emitted.",
        meaning: "Package size on the file-local package_data row when known.",
    },
    OutputFieldDoc {
        json_name: "sha1",
        rust_field: "sha1",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "SHA-1 checksum on the file-local package_data row when known.",
    },
    OutputFieldDoc {
        json_name: "md5",
        rust_field: "md5",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "MD5 checksum on the file-local package_data row when known.",
    },
    OutputFieldDoc {
        json_name: "sha256",
        rust_field: "sha256",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "SHA-256 checksum on the file-local package_data row when known.",
    },
    OutputFieldDoc {
        json_name: "sha512",
        rust_field: "sha512",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "SHA-512 checksum on the file-local package_data row when known.",
    },
    OutputFieldDoc {
        json_name: "bug_tracking_url",
        rust_field: "bug_tracking_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Bug-tracker URL on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "code_view_url",
        rust_field: "code_view_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Code-view URL on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "vcs_url",
        rust_field: "vcs_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "VCS URL on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "copyright",
        rust_field: "copyright",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Copyright string on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "holder",
        rust_field: "holder",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Holder string on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "declared_license_expression",
        rust_field: "declared_license_expression",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Primary declared license expression on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "declared_license_expression_spdx",
        rust_field: "declared_license_expression_spdx",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "SPDX-form primary declared license expression on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "license_detections",
        rust_field: "license_detections",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Structured license detections attached to the raw parser-emitted package_data row.",
    },
    OutputFieldDoc {
        json_name: "other_license_expression",
        rust_field: "other_license_expression",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Auxiliary non-primary license expression lane for the package_data row.",
    },
    OutputFieldDoc {
        json_name: "other_license_expression_spdx",
        rust_field: "other_license_expression_spdx",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "SPDX-form auxiliary non-primary license expression lane for the package_data row.",
    },
    OutputFieldDoc {
        json_name: "other_license_detections",
        rust_field: "other_license_detections",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Detections associated with the auxiliary or non-primary license lane.",
    },
    OutputFieldDoc {
        json_name: "extracted_license_statement",
        rust_field: "extracted_license_statement",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Raw extracted license statement on the package_data row.",
    },
    OutputFieldDoc {
        json_name: "notice_text",
        rust_field: "notice_text",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Notice text on the package_data row.",
    },
    OutputFieldDoc {
        json_name: "source_packages",
        rust_field: "source_packages",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Referenced source-package identifiers preserved on the raw package_data row.",
    },
    OutputFieldDoc {
        json_name: "file_references",
        rust_field: "file_references",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "File-reference hints emitted by parsers for later resolution or ownership assignment.",
    },
    OutputFieldDoc {
        json_name: "is_private",
        rust_field: "is_private",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Private/public package signal on the raw parser-emitted row.",
    },
    OutputFieldDoc {
        json_name: "is_virtual",
        rust_field: "is_virtual",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Virtual/synthetic package signal on the raw parser-emitted row.",
    },
    OutputFieldDoc {
        json_name: "extra_data",
        rust_field: "extra_data",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "Datasource-specific structured metadata preserved without promotion into core fields.",
    },
    OutputFieldDoc {
        json_name: "dependencies",
        rust_field: "dependencies",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Raw dependency rows emitted directly by the parser before top-level assembly.",
    },
    OutputFieldDoc {
        json_name: "repository_homepage_url",
        rust_field: "repository_homepage_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Repository homepage URL on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "repository_download_url",
        rust_field: "repository_download_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Repository download URL on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "api_data_url",
        rust_field: "api_data_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "API data URL on the file-local package_data row.",
    },
    OutputFieldDoc {
        json_name: "datasource_id",
        rust_field: "datasource_id",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Single datasource identifier for the parser surface that produced this package_data row.",
    },
    OutputFieldDoc {
        json_name: "purl",
        rust_field: "purl",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package URL on the file-local package_data row.",
    },
];

const DEPENDENCY_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "purl",
        rust_field: "purl",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package URL for the dependency row.",
    },
    OutputFieldDoc {
        json_name: "extracted_requirement",
        rust_field: "extracted_requirement",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Raw requirement/version constraint text extracted from the manifest or lockfile.",
    },
    OutputFieldDoc {
        json_name: "scope",
        rust_field: "scope",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Datasource-specific dependency scope such as runtime/dev/test/build.",
    },
    OutputFieldDoc {
        json_name: "is_runtime",
        rust_field: "is_runtime",
        value_shape: "boolean | null",
        presence: "Always emitted.",
        meaning: "Normalized runtime intent signal when it can be derived honestly.",
    },
    OutputFieldDoc {
        json_name: "is_optional",
        rust_field: "is_optional",
        value_shape: "boolean | null",
        presence: "Always emitted.",
        meaning: "Normalized optional-dependency signal when it can be derived honestly.",
    },
    OutputFieldDoc {
        json_name: "is_pinned",
        rust_field: "is_pinned",
        value_shape: "boolean | null",
        presence: "Always emitted.",
        meaning: "Normalized version-pinning signal when it can be derived honestly.",
    },
    OutputFieldDoc {
        json_name: "is_direct",
        rust_field: "is_direct",
        value_shape: "boolean | null",
        presence: "Always emitted.",
        meaning: "Normalized direct-vs-transitive signal when it can be derived honestly.",
    },
    OutputFieldDoc {
        json_name: "resolved_package",
        rust_field: "resolved_package",
        value_shape: "object | null",
        presence: "Always emitted.",
        meaning: "Resolved package identity/details nested directly under the dependency row.",
    },
    OutputFieldDoc {
        json_name: "extra_data",
        rust_field: "extra_data",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "Datasource-specific dependency metadata preserved without promotion into core fields.",
    },
];

const TOP_LEVEL_DEPENDENCY_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "purl",
        rust_field: "purl",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package URL for the top-level dependency row.",
    },
    OutputFieldDoc {
        json_name: "extracted_requirement",
        rust_field: "extracted_requirement",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Raw requirement/version constraint text for the top-level dependency row.",
    },
    OutputFieldDoc {
        json_name: "scope",
        rust_field: "scope",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Datasource-specific dependency scope such as runtime/dev/test/build.",
    },
    OutputFieldDoc {
        json_name: "is_runtime",
        rust_field: "is_runtime",
        value_shape: "boolean | null",
        presence: "Always emitted.",
        meaning: "Normalized runtime intent signal when it can be derived honestly.",
    },
    OutputFieldDoc {
        json_name: "is_optional",
        rust_field: "is_optional",
        value_shape: "boolean | null",
        presence: "Always emitted.",
        meaning: "Normalized optional-dependency signal when it can be derived honestly.",
    },
    OutputFieldDoc {
        json_name: "is_pinned",
        rust_field: "is_pinned",
        value_shape: "boolean | null",
        presence: "Always emitted.",
        meaning: "Normalized version-pinning signal when it can be derived honestly.",
    },
    OutputFieldDoc {
        json_name: "is_direct",
        rust_field: "is_direct",
        value_shape: "boolean | null",
        presence: "Always emitted.",
        meaning: "Normalized direct-vs-transitive signal when it can be derived honestly.",
    },
    OutputFieldDoc {
        json_name: "resolved_package",
        rust_field: "resolved_package",
        value_shape: "object | null",
        presence: "Always emitted.",
        meaning: "Resolved package payload nested under the top-level dependency row.",
    },
    OutputFieldDoc {
        json_name: "extra_data",
        rust_field: "extra_data",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "Datasource-specific structured metadata preserved on the top-level dependency row.",
    },
    OutputFieldDoc {
        json_name: "dependency_uid",
        rust_field: "dependency_uid",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Stable identifier for the top-level dependency row on the public contract.",
    },
    OutputFieldDoc {
        json_name: "for_package_uid",
        rust_field: "for_package_uid",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package UID that owns this hoisted dependency row after assembly.",
    },
    OutputFieldDoc {
        json_name: "datafile_path",
        rust_field: "datafile_path",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Manifest or metadata file path that contributed this top-level dependency row.",
    },
    OutputFieldDoc {
        json_name: "datasource_id",
        rust_field: "datasource_id",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Datasource identifier for the parser/input surface that contributed this row.",
    },
    OutputFieldDoc {
        json_name: "namespace",
        rust_field: "namespace",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Owning namespace lane preserved on the hoisted dependency row for output compatibility.",
    },
];

const TOP_LEVEL_LICENSE_DETECTION_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "identifier",
        rust_field: "identifier",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Stable grouped-detection identifier for this top-level license detection block.",
    },
    OutputFieldDoc {
        json_name: "license_expression",
        rust_field: "license_expression",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Grouped license expression for the top-level detection block.",
    },
    OutputFieldDoc {
        json_name: "license_expression_spdx",
        rust_field: "license_expression_spdx",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "SPDX-form grouped license expression for the top-level detection block.",
    },
    OutputFieldDoc {
        json_name: "detection_count",
        rust_field: "detection_count",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Number of file-level detections that contributed to this grouped top-level block.",
    },
    OutputFieldDoc {
        json_name: "detection_log",
        rust_field: "detection_log",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Grouping-time notes that explain why this top-level detection looks the way it does.",
    },
    OutputFieldDoc {
        json_name: "reference_matches",
        rust_field: "reference_matches",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Representative match/reference records retained on the top-level grouped detection block.",
    },
];

const TALLIES_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "detected_license_expression",
        rust_field: "detected_license_expression",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "Tally entries for file-level detected license expressions.",
    },
    OutputFieldDoc {
        json_name: "copyrights",
        rust_field: "copyrights",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "Tally entries for copyright strings.",
    },
    OutputFieldDoc {
        json_name: "holders",
        rust_field: "holders",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "Tally entries for copyright holders.",
    },
    OutputFieldDoc {
        json_name: "authors",
        rust_field: "authors",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "Tally entries for author strings.",
    },
    OutputFieldDoc {
        json_name: "programming_language",
        rust_field: "programming_language",
        value_shape: "array<object>",
        presence: "Emitted only when non-empty.",
        meaning: "Tally entries for detected programming-language hints.",
    },
];

const FACET_TALLIES_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "facet",
        rust_field: "facet",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Facet label for this grouped tally block.",
    },
    OutputFieldDoc {
        json_name: "tallies",
        rust_field: "tallies",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "Tally payload for this single facet.",
    },
];

const TALLY_ENTRY_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "value",
        rust_field: "value",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Bucket value represented by the tally row.",
    },
    OutputFieldDoc {
        json_name: "count",
        rust_field: "count",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Number of occurrences counted into this tally bucket.",
    },
];

const LICENSE_CLARITY_SCORE_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "score",
        rust_field: "score",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Overall clarity score for the summary-level licensing evidence.",
    },
    OutputFieldDoc {
        json_name: "declared_license",
        rust_field: "declared_license",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether clear declared-license evidence was found.",
    },
    OutputFieldDoc {
        json_name: "identification_precision",
        rust_field: "identification_precision",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the detected licensing evidence is precise rather than vague or generic.",
    },
    OutputFieldDoc {
        json_name: "has_license_text",
        rust_field: "has_license_text",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether substantive license-text evidence was found.",
    },
    OutputFieldDoc {
        json_name: "declared_copyrights",
        rust_field: "declared_copyrights",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether declared copyright evidence was found in the key-file set.",
    },
    OutputFieldDoc {
        json_name: "conflicting_license_categories",
        rust_field: "conflicting_license_categories",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the evidence contains conflicting license-category signals.",
    },
    OutputFieldDoc {
        json_name: "ambiguous_compound_licensing",
        rust_field: "ambiguous_compound_licensing",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the evidence suggests a compound license situation that remains ambiguous.",
    },
];

const SYSTEM_ENVIRONMENT_FIELDS: &[OutputFieldDoc] = &[
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

const LICENSE_INDEX_PROVENANCE_FIELDS: &[OutputFieldDoc] = &[
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

const PARTY_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "type",
        rust_field: "r#type",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Normalized party type such as person or organization.",
    },
    OutputFieldDoc {
        json_name: "role",
        rust_field: "role",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Role of the party on the package or metadata record.",
    },
    OutputFieldDoc {
        json_name: "name",
        rust_field: "name",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Human-readable party name.",
    },
    OutputFieldDoc {
        json_name: "email",
        rust_field: "email",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Party email address.",
    },
    OutputFieldDoc {
        json_name: "url",
        rust_field: "url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Party homepage or profile URL.",
    },
    OutputFieldDoc {
        json_name: "organization",
        rust_field: "organization",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Owning organization for the party, when captured separately from the party name.",
    },
    OutputFieldDoc {
        json_name: "organization_url",
        rust_field: "organization_url",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Owning organization URL for the party.",
    },
    OutputFieldDoc {
        json_name: "timezone",
        rust_field: "timezone",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Timezone associated with the party metadata, when available.",
    },
];

const FILE_REFERENCE_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "path",
        rust_field: "path",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Referenced file path preserved on the package/file-reference surface.",
    },
    OutputFieldDoc {
        json_name: "size",
        rust_field: "size",
        value_shape: "integer | null",
        presence: "Emitted only when available.",
        meaning: "Referenced file size when known.",
    },
    OutputFieldDoc {
        json_name: "sha1",
        rust_field: "sha1",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Referenced file SHA-1 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "md5",
        rust_field: "md5",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Referenced file MD5 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "sha256",
        rust_field: "sha256",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Referenced file SHA-256 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "sha512",
        rust_field: "sha512",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Referenced file SHA-512 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "extra_data",
        rust_field: "extra_data",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "Additional metadata preserved on the referenced-file row.",
    },
];

const LICENSE_POLICY_ENTRY_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "license_key",
        rust_field: "license_key",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "License key this policy entry applies to.",
    },
    OutputFieldDoc {
        json_name: "label",
        rust_field: "label",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Human-facing policy label for the license.",
    },
    OutputFieldDoc {
        json_name: "color_code",
        rust_field: "color_code",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Color code used by policy-aware outputs or UIs.",
    },
    OutputFieldDoc {
        json_name: "icon",
        rust_field: "icon",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Icon identifier used by policy-aware outputs or UIs.",
    },
];

const AUTHOR_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "author",
        rust_field: "author",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Extracted author string.",
    },
    OutputFieldDoc {
        json_name: "start_line",
        rust_field: "start_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "First line where the author evidence appeared.",
    },
    OutputFieldDoc {
        json_name: "end_line",
        rust_field: "end_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Last line where the author evidence appeared.",
    },
];

const COPYRIGHT_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "copyright",
        rust_field: "copyright",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Rendered copyright string, using native or ScanCode compatibility mode as selected.",
    },
    OutputFieldDoc {
        json_name: "start_line",
        rust_field: "start_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "First line where the copyright evidence appeared.",
    },
    OutputFieldDoc {
        json_name: "end_line",
        rust_field: "end_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Last line where the copyright evidence appeared.",
    },
];

const EMAIL_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "email",
        rust_field: "email",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Extracted email address.",
    },
    OutputFieldDoc {
        json_name: "start_line",
        rust_field: "start_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "First line where the email evidence appeared.",
    },
    OutputFieldDoc {
        json_name: "end_line",
        rust_field: "end_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Last line where the email evidence appeared.",
    },
];

const HOLDER_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "holder",
        rust_field: "holder",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Extracted copyright holder string.",
    },
    OutputFieldDoc {
        json_name: "start_line",
        rust_field: "start_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "First line where the holder evidence appeared.",
    },
    OutputFieldDoc {
        json_name: "end_line",
        rust_field: "end_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Last line where the holder evidence appeared.",
    },
];

const URL_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "url",
        rust_field: "url",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Extracted URL string.",
    },
    OutputFieldDoc {
        json_name: "start_line",
        rust_field: "start_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "First line where the URL evidence appeared.",
    },
    OutputFieldDoc {
        json_name: "end_line",
        rust_field: "end_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Last line where the URL evidence appeared.",
    },
];

const LICENSE_DETECTION_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "license_expression",
        rust_field: "license_expression",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Grouped license expression for the detection block.",
    },
    OutputFieldDoc {
        json_name: "license_expression_spdx",
        rust_field: "license_expression_spdx",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "SPDX-form expression for the detection block.",
    },
    OutputFieldDoc {
        json_name: "matches",
        rust_field: "matches",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Match records that contributed to this grouped file- or package-level detection.",
    },
    OutputFieldDoc {
        json_name: "detection_log",
        rust_field: "detection_log",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Notes explaining post-processing or grouping decisions for this detection.",
    },
    OutputFieldDoc {
        json_name: "identifier",
        rust_field: "identifier",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Stable detection identifier when available.",
    },
];

const MATCH_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "license_expression",
        rust_field: "license_expression",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "License expression assigned to this individual match.",
    },
    OutputFieldDoc {
        json_name: "license_expression_spdx",
        rust_field: "license_expression_spdx",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "SPDX-form expression assigned to this individual match.",
    },
    OutputFieldDoc {
        json_name: "from_file",
        rust_field: "from_file",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Origin file path associated with the match record.",
    },
    OutputFieldDoc {
        json_name: "start_line",
        rust_field: "start_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "First line covered by the match.",
    },
    OutputFieldDoc {
        json_name: "end_line",
        rust_field: "end_line",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Last line covered by the match.",
    },
    OutputFieldDoc {
        json_name: "matcher",
        rust_field: "matcher",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Matcher kind that produced the match.",
    },
    OutputFieldDoc {
        json_name: "score",
        rust_field: "score",
        value_shape: "number",
        presence: "Always emitted.",
        meaning: "Match score on the public output scale.",
    },
    OutputFieldDoc {
        json_name: "matched_length",
        rust_field: "matched_length",
        value_shape: "integer | null",
        presence: "Emitted only when available.",
        meaning: "Matched token/text length when tracked.",
    },
    OutputFieldDoc {
        json_name: "match_coverage",
        rust_field: "match_coverage",
        value_shape: "number | null",
        presence: "Emitted only when available.",
        meaning: "Coverage ratio for the match when tracked.",
    },
    OutputFieldDoc {
        json_name: "rule_relevance",
        rust_field: "rule_relevance",
        value_shape: "integer | null",
        presence: "Emitted only when available.",
        meaning: "Rule relevance score when tracked.",
    },
    OutputFieldDoc {
        json_name: "rule_identifier",
        rust_field: "rule_identifier",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Identifier of the matched rule when available.",
    },
    OutputFieldDoc {
        json_name: "rule_url",
        rust_field: "rule_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Rule URL when available.",
    },
    OutputFieldDoc {
        json_name: "matched_text",
        rust_field: "matched_text",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Matched text payload when text output is enabled.",
    },
    OutputFieldDoc {
        json_name: "matched_text_diagnostics",
        rust_field: "matched_text_diagnostics",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Diagnostic rendering of the matched text when enabled.",
    },
    OutputFieldDoc {
        json_name: "referenced_filenames",
        rust_field: "referenced_filenames",
        value_shape: "array<string> | null",
        presence: "Emitted only when available.",
        meaning: "Referenced filenames captured on the matched rule when applicable.",
    },
];

const LICENSE_REFERENCE_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "key",
        rust_field: "key",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Primary ScanCode-style license key when one is available for the reference block.",
    },
    OutputFieldDoc {
        json_name: "language",
        rust_field: "language",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Language tag for the referenced license text when the license reference is language-specific.",
    },
    OutputFieldDoc {
        json_name: "name",
        rust_field: "name",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Canonical human-facing name of the referenced license.",
    },
    OutputFieldDoc {
        json_name: "short_name",
        rust_field: "short_name",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Short display name of the referenced license.",
    },
    OutputFieldDoc {
        json_name: "owner",
        rust_field: "owner",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Owning organization or steward of the referenced license, when captured.",
    },
    OutputFieldDoc {
        json_name: "homepage_url",
        rust_field: "homepage_url",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Homepage URL for the referenced license.",
    },
    OutputFieldDoc {
        json_name: "spdx_license_key",
        rust_field: "spdx_license_key",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Primary SPDX license key associated with the referenced license.",
    },
    OutputFieldDoc {
        json_name: "other_spdx_license_keys",
        rust_field: "other_spdx_license_keys",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Additional SPDX license keys associated with the same referenced license.",
    },
    OutputFieldDoc {
        json_name: "osi_license_key",
        rust_field: "osi_license_key",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "OSI license key when the referenced license is recognized by OSI.",
    },
    OutputFieldDoc {
        json_name: "text_urls",
        rust_field: "text_urls",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "URLs to known license text sources for this referenced license.",
    },
    OutputFieldDoc {
        json_name: "osi_url",
        rust_field: "osi_url",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "OSI detail page URL for the referenced license.",
    },
    OutputFieldDoc {
        json_name: "faq_url",
        rust_field: "faq_url",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "FAQ URL for the referenced license.",
    },
    OutputFieldDoc {
        json_name: "other_urls",
        rust_field: "other_urls",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Additional URLs associated with the referenced license.",
    },
    OutputFieldDoc {
        json_name: "category",
        rust_field: "category",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "License category label, when the license data classifies it.",
    },
    OutputFieldDoc {
        json_name: "is_exception",
        rust_field: "is_exception",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the referenced license is an exception rather than a standalone license.",
    },
    OutputFieldDoc {
        json_name: "is_unknown",
        rust_field: "is_unknown",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the referenced license is treated as an unknown or placeholder license.",
    },
    OutputFieldDoc {
        json_name: "is_generic",
        rust_field: "is_generic",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the referenced license is generic rather than a specific named license.",
    },
    OutputFieldDoc {
        json_name: "notes",
        rust_field: "notes",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Additional notes carried by the referenced license record.",
    },
    OutputFieldDoc {
        json_name: "minimum_coverage",
        rust_field: "minimum_coverage",
        value_shape: "integer | null",
        presence: "Emitted only when available.",
        meaning: "Minimum coverage threshold associated with the referenced license, when specified.",
    },
    OutputFieldDoc {
        json_name: "standard_notice",
        rust_field: "standard_notice",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Standard notice text associated with the referenced license.",
    },
    OutputFieldDoc {
        json_name: "ignorable_copyrights",
        rust_field: "ignorable_copyrights",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Copyright strings considered ignorable for this referenced license.",
    },
    OutputFieldDoc {
        json_name: "ignorable_holders",
        rust_field: "ignorable_holders",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Holder strings considered ignorable for this referenced license.",
    },
    OutputFieldDoc {
        json_name: "ignorable_authors",
        rust_field: "ignorable_authors",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Author strings considered ignorable for this referenced license.",
    },
    OutputFieldDoc {
        json_name: "ignorable_urls",
        rust_field: "ignorable_urls",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "URL strings considered ignorable for this referenced license.",
    },
    OutputFieldDoc {
        json_name: "ignorable_emails",
        rust_field: "ignorable_emails",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Email strings considered ignorable for this referenced license.",
    },
    OutputFieldDoc {
        json_name: "scancode_url",
        rust_field: "scancode_url",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "ScanCode reference URL for this license, when available.",
    },
    OutputFieldDoc {
        json_name: "licensedb_url",
        rust_field: "licensedb_url",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "LicenseDB URL for this license, when available.",
    },
    OutputFieldDoc {
        json_name: "spdx_url",
        rust_field: "spdx_url",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "SPDX reference URL for this license, when available.",
    },
    OutputFieldDoc {
        json_name: "text",
        rust_field: "text",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Canonical license text payload preserved on the reference block.",
    },
];

const LICENSE_RULE_REFERENCE_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "identifier",
        rust_field: "identifier",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Stable identifier of the referenced license rule.",
    },
    OutputFieldDoc {
        json_name: "license_expression",
        rust_field: "license_expression",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "License expression associated with the referenced rule.",
    },
    OutputFieldDoc {
        json_name: "is_license_text",
        rust_field: "is_license_text",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule is classified as license-text evidence.",
    },
    OutputFieldDoc {
        json_name: "is_license_notice",
        rust_field: "is_license_notice",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule is classified as license-notice evidence.",
    },
    OutputFieldDoc {
        json_name: "is_license_reference",
        rust_field: "is_license_reference",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule is classified as license-reference evidence.",
    },
    OutputFieldDoc {
        json_name: "is_license_tag",
        rust_field: "is_license_tag",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule is classified as license-tag evidence.",
    },
    OutputFieldDoc {
        json_name: "is_license_clue",
        rust_field: "is_license_clue",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule is classified as clue-only evidence.",
    },
    OutputFieldDoc {
        json_name: "is_license_intro",
        rust_field: "is_license_intro",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule is classified as introductory license wording.",
    },
    OutputFieldDoc {
        json_name: "language",
        rust_field: "language",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Language tag for the referenced rule when the rule is language-specific.",
    },
    OutputFieldDoc {
        json_name: "rule_url",
        rust_field: "rule_url",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Reference URL for the rule, when available.",
    },
    OutputFieldDoc {
        json_name: "is_required_phrase",
        rust_field: "is_required_phrase",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule is a required-phrase rule.",
    },
    OutputFieldDoc {
        json_name: "skip_for_required_phrase_generation",
        rust_field: "skip_for_required_phrase_generation",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether this rule should be excluded when deriving required-phrase rules automatically.",
    },
    OutputFieldDoc {
        json_name: "replaced_by",
        rust_field: "replaced_by",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Rule identifiers that supersede this rule.",
    },
    OutputFieldDoc {
        json_name: "is_continuous",
        rust_field: "is_continuous",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule expects continuous text rather than discontinuous matches.",
    },
    OutputFieldDoc {
        json_name: "is_synthetic",
        rust_field: "is_synthetic",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule was synthesized rather than sourced directly from curated reference text.",
    },
    OutputFieldDoc {
        json_name: "is_from_license",
        rust_field: "is_from_license",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Whether the rule was derived directly from a license text record.",
    },
    OutputFieldDoc {
        json_name: "length",
        rust_field: "length",
        value_shape: "integer",
        presence: "Always emitted.",
        meaning: "Rule length on the public rule-reference surface.",
    },
    OutputFieldDoc {
        json_name: "relevance",
        rust_field: "relevance",
        value_shape: "integer | null",
        presence: "Emitted only when available.",
        meaning: "Rule relevance score, when present.",
    },
    OutputFieldDoc {
        json_name: "minimum_coverage",
        rust_field: "minimum_coverage",
        value_shape: "integer | null",
        presence: "Emitted only when available.",
        meaning: "Minimum coverage threshold associated with the rule, when specified.",
    },
    OutputFieldDoc {
        json_name: "referenced_filenames",
        rust_field: "referenced_filenames",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Referenced filenames attached to the rule metadata.",
    },
    OutputFieldDoc {
        json_name: "notes",
        rust_field: "notes",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Additional notes carried by the rule reference.",
    },
    OutputFieldDoc {
        json_name: "ignorable_copyrights",
        rust_field: "ignorable_copyrights",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Copyright strings considered ignorable for this rule.",
    },
    OutputFieldDoc {
        json_name: "ignorable_holders",
        rust_field: "ignorable_holders",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Holder strings considered ignorable for this rule.",
    },
    OutputFieldDoc {
        json_name: "ignorable_authors",
        rust_field: "ignorable_authors",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Author strings considered ignorable for this rule.",
    },
    OutputFieldDoc {
        json_name: "ignorable_urls",
        rust_field: "ignorable_urls",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "URL strings considered ignorable for this rule.",
    },
    OutputFieldDoc {
        json_name: "ignorable_emails",
        rust_field: "ignorable_emails",
        value_shape: "array<string>",
        presence: "Emitted only when non-empty.",
        meaning: "Email strings considered ignorable for this rule.",
    },
    OutputFieldDoc {
        json_name: "text",
        rust_field: "text",
        value_shape: "string | null",
        presence: "Emitted only when available.",
        meaning: "Canonical rule text payload when the reference includes it.",
    },
];

const RESOLVED_PACKAGE_FIELDS: &[OutputFieldDoc] = &[
    OutputFieldDoc {
        json_name: "type",
        rust_field: "package_type",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Package ecosystem/type identifier on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "namespace",
        rust_field: "namespace",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Resolved package namespace.",
    },
    OutputFieldDoc {
        json_name: "name",
        rust_field: "name",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Resolved package name.",
    },
    OutputFieldDoc {
        json_name: "version",
        rust_field: "version",
        value_shape: "string",
        presence: "Always emitted.",
        meaning: "Resolved package version.",
    },
    OutputFieldDoc {
        json_name: "qualifiers",
        rust_field: "qualifiers",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "PURL-style qualifier key/value pairs on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "subpath",
        rust_field: "subpath",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package subpath, when available.",
    },
    OutputFieldDoc {
        json_name: "primary_language",
        rust_field: "primary_language",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Primary language associated with the resolved package.",
    },
    OutputFieldDoc {
        json_name: "description",
        rust_field: "description",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package description.",
    },
    OutputFieldDoc {
        json_name: "release_date",
        rust_field: "release_date",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package release date.",
    },
    OutputFieldDoc {
        json_name: "parties",
        rust_field: "parties",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Party records attached to the resolved package.",
    },
    OutputFieldDoc {
        json_name: "keywords",
        rust_field: "keywords",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Keywords attached to the resolved package.",
    },
    OutputFieldDoc {
        json_name: "homepage_url",
        rust_field: "homepage_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package homepage URL.",
    },
    OutputFieldDoc {
        json_name: "download_url",
        rust_field: "download_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package download URL.",
    },
    OutputFieldDoc {
        json_name: "size",
        rust_field: "size",
        value_shape: "integer | null",
        presence: "Always emitted.",
        meaning: "Resolved package size when known.",
    },
    OutputFieldDoc {
        json_name: "sha1",
        rust_field: "sha1",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package SHA-1 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "md5",
        rust_field: "md5",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package MD5 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "sha256",
        rust_field: "sha256",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package SHA-256 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "sha512",
        rust_field: "sha512",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package SHA-512 checksum when known.",
    },
    OutputFieldDoc {
        json_name: "bug_tracking_url",
        rust_field: "bug_tracking_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package bug-tracker URL.",
    },
    OutputFieldDoc {
        json_name: "code_view_url",
        rust_field: "code_view_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package code-browsing URL.",
    },
    OutputFieldDoc {
        json_name: "vcs_url",
        rust_field: "vcs_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package VCS URL.",
    },
    OutputFieldDoc {
        json_name: "copyright",
        rust_field: "copyright",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package copyright string.",
    },
    OutputFieldDoc {
        json_name: "holder",
        rust_field: "holder",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package holder string.",
    },
    OutputFieldDoc {
        json_name: "declared_license_expression",
        rust_field: "declared_license_expression",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Primary declared license expression on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "declared_license_expression_spdx",
        rust_field: "declared_license_expression_spdx",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "SPDX-form primary declared license expression on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "license_detections",
        rust_field: "license_detections",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "License detections attached to the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "other_license_expression",
        rust_field: "other_license_expression",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Auxiliary non-primary license expression lane on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "other_license_expression_spdx",
        rust_field: "other_license_expression_spdx",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "SPDX-form auxiliary non-primary license expression lane on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "other_license_detections",
        rust_field: "other_license_detections",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Auxiliary license detections attached to the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "extracted_license_statement",
        rust_field: "extracted_license_statement",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Raw extracted license statement on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "notice_text",
        rust_field: "notice_text",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Resolved package notice text.",
    },
    OutputFieldDoc {
        json_name: "source_packages",
        rust_field: "source_packages",
        value_shape: "array<string>",
        presence: "Always emitted.",
        meaning: "Referenced source-package identifiers associated with the resolved package.",
    },
    OutputFieldDoc {
        json_name: "file_references",
        rust_field: "file_references",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Referenced-file rows attached to the resolved package.",
    },
    OutputFieldDoc {
        json_name: "is_private",
        rust_field: "is_private",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Private/public signal on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "is_virtual",
        rust_field: "is_virtual",
        value_shape: "boolean",
        presence: "Always emitted.",
        meaning: "Virtual/synthetic signal on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "extra_data",
        rust_field: "extra_data",
        value_shape: "object",
        presence: "Always emitted.",
        meaning: "Datasource-specific structured metadata preserved on the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "dependencies",
        rust_field: "dependencies",
        value_shape: "array<object>",
        presence: "Always emitted.",
        meaning: "Dependency rows nested under the resolved-package payload.",
    },
    OutputFieldDoc {
        json_name: "repository_homepage_url",
        rust_field: "repository_homepage_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Repository homepage URL for the resolved package.",
    },
    OutputFieldDoc {
        json_name: "repository_download_url",
        rust_field: "repository_download_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Repository download URL for the resolved package.",
    },
    OutputFieldDoc {
        json_name: "api_data_url",
        rust_field: "api_data_url",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "API data URL for the resolved package.",
    },
    OutputFieldDoc {
        json_name: "datasource_id",
        rust_field: "datasource_id",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Single datasource identifier associated with the resolved-package payload, when available.",
    },
    OutputFieldDoc {
        json_name: "purl",
        rust_field: "purl",
        value_shape: "string | null",
        presence: "Always emitted.",
        meaning: "Package URL for the resolved package.",
    },
];

const EMPTY_FIELDS: &[OutputFieldDoc] = &[];

const DOCUMENTED_TYPES: &[OutputTypeDoc] = &[
    OutputTypeDoc {
        type_name: "Output",
        json_paths: &["$"],
        summary: "Top-level ScanCode-compatible output object.",
        fields: OUTPUT_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputHeader",
        json_paths: &["$.headers[]"],
        summary: "Per-run metadata block for one scan invocation.",
        fields: HEADER_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputExtraData",
        json_paths: &["$.headers[].extra_data"],
        summary: "Scanner-owned counts and provenance metadata nested under a header block.",
        fields: EXTRA_DATA_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputSummary",
        json_paths: &["$.summary"],
        summary: "Optional codebase-level rollup emitted by summary/classification workflows.",
        fields: SUMMARY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputFileInfo",
        json_paths: &["$.files[]"],
        summary: "File or directory record on the main per-resource output surface.",
        fields: FILE_INFO_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputPackage",
        json_paths: &["$.packages[]"],
        summary: "Assembled top-level package record on the public output contract.",
        fields: PACKAGE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputPackageData",
        json_paths: &["$.files[].package_data[]"],
        summary: "Raw parser-emitted package record attached to a specific file.",
        fields: PACKAGE_DATA_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputDependency",
        json_paths: &["$.files[].package_data[].dependencies[]"],
        summary: "Raw dependency row preserved on parser-emitted package data.",
        fields: DEPENDENCY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputTopLevelDependency",
        json_paths: &["$.dependencies[]"],
        summary: "Hoisted top-level dependency record emitted after assembly.",
        fields: TOP_LEVEL_DEPENDENCY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputTopLevelLicenseDetection",
        json_paths: &["$.license_detections[]"],
        summary: "Grouped top-level license detection block across the scanned codebase.",
        fields: TOP_LEVEL_LICENSE_DETECTION_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputTallies",
        json_paths: &[
            "$.tallies",
            "$.tallies_of_key_files",
            "$.files[].tallies",
            "$.tallies_by_facet[].tallies",
        ],
        summary: "Tally block used on top-level, key-file, facet, and file-level tally surfaces.",
        fields: TALLIES_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputFacetTallies",
        json_paths: &["$.tallies_by_facet[]"],
        summary: "Facet-specific tally wrapper for one user-defined facet label.",
        fields: FACET_TALLIES_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputTallyEntry",
        json_paths: &[
            "$.summary.other_license_expressions[]",
            "$.summary.other_holders[]",
            "$.summary.other_languages[]",
            "$.tallies.*[]",
        ],
        summary: "Single tally bucket entry used throughout summary and tally outputs.",
        fields: TALLY_ENTRY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseClarityScore",
        json_paths: &["$.summary.license_clarity_score"],
        summary: "Structured license-clarity scoring payload on the summary surface.",
        fields: LICENSE_CLARITY_SCORE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputSystemEnvironment",
        json_paths: &["$.headers[].extra_data.system_environment"],
        summary: "Recorded environment metadata for the scan runtime.",
        fields: SYSTEM_ENVIRONMENT_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseIndexProvenance",
        json_paths: &["$.headers[].extra_data.license_index_provenance"],
        summary: "Provenance block for the effective license index used by the scan.",
        fields: LICENSE_INDEX_PROVENANCE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputParty",
        json_paths: &[
            "$.packages[].parties[]",
            "$.files[].package_data[].parties[]",
            "$.dependencies[].resolved_package.parties[]",
        ],
        summary: "Party record used on package and resolved-package surfaces.",
        fields: PARTY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputFileReference",
        json_paths: &[
            "$.files[].package_data[].file_references[]",
            "$.dependencies[].resolved_package.file_references[]",
        ],
        summary: "Referenced-file record used on package-related surfaces.",
        fields: FILE_REFERENCE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicensePolicyEntry",
        json_paths: &["$.files[].license_policy[]"],
        summary: "Policy decoration entry attached to file-level license-policy output.",
        fields: LICENSE_POLICY_ENTRY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputAuthor",
        json_paths: &["$.files[].authors[]"],
        summary: "File-level author evidence record.",
        fields: AUTHOR_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputCopyright",
        json_paths: &["$.files[].copyrights[]"],
        summary: "File-level copyright evidence record.",
        fields: COPYRIGHT_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputEmail",
        json_paths: &["$.files[].emails[]"],
        summary: "File-level email evidence record.",
        fields: EMAIL_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputHolder",
        json_paths: &["$.files[].holders[]"],
        summary: "File-level holder evidence record.",
        fields: HOLDER_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputURL",
        json_paths: &["$.files[].urls[]"],
        summary: "File-level URL evidence record.",
        fields: URL_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseDetection",
        json_paths: &[
            "$.files[].license_detections[]",
            "$.files[].package_data[].license_detections[]",
            "$.packages[].license_detections[]",
            "$.dependencies[].resolved_package.license_detections[]",
        ],
        summary: "Grouped license detection record used on file, package_data, package, and resolved-package surfaces.",
        fields: LICENSE_DETECTION_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputMatch",
        json_paths: &[
            "$.files[].license_clues[]",
            "$.files[].license_detections[].matches[]",
            "$.license_detections[].reference_matches[]",
        ],
        summary: "Match record used for clue output, grouped detections, and top-level representative references.",
        fields: MATCH_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseReference",
        json_paths: &["$.license_references[]"],
        summary: "Top-level license reference record describing one emitted license key and its reference metadata.",
        fields: LICENSE_REFERENCE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseRuleReference",
        json_paths: &["$.license_rule_references[]"],
        summary: "Top-level license-rule reference record describing one emitted rule and its reference metadata.",
        fields: LICENSE_RULE_REFERENCE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputResolvedPackage",
        json_paths: &[
            "$.dependencies[].resolved_package",
            "$.files[].package_data[].dependencies[].resolved_package",
        ],
        summary: "Resolved package payload nested under dependency rows.",
        fields: RESOLVED_PACKAGE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputFileType",
        json_paths: &["$.files[].type"],
        summary: "Serialized file-type enum used by file records.",
        fields: EMPTY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputPackageType",
        json_paths: &["$.packages[].type", "$.files[].package_data[].type"],
        summary: "Serialized package-type string newtype used by package-related records.",
        fields: EMPTY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputDatasourceId",
        json_paths: &[
            "$.packages[].datasource_ids[]",
            "$.dependencies[].datasource_id",
            "$.files[].package_data[].datasource_id",
        ],
        summary: "Serialized datasource-id string newtype used by package and dependency records.",
        fields: EMPTY_FIELDS,
    },
];

pub fn documented_output_types() -> &'static [OutputTypeDoc] {
    DOCUMENTED_TYPES
}

#[cfg(test)]
mod tests {
    use super::documented_output_types;
    use std::collections::BTreeSet;

    use crate::output_schema::{
        OutputAuthor, OutputCopyright, OutputDatasourceId, OutputDependency, OutputEmail,
        OutputExtraData, OutputFileInfo, OutputFileReference, OutputFileType, OutputHeader,
        OutputHolder, OutputLicenseDetection, OutputLicenseIndexProvenance,
        OutputLicensePolicyEntry, OutputMatch, OutputPackage, OutputPackageData, OutputPackageType,
        OutputParty, OutputSystemEnvironment, OutputTallies, OutputTallyEntry,
        OutputTopLevelDependency, OutputTopLevelLicenseDetection, OutputURL,
    };
    use serde::Serialize;
    use serde_json::{Map, Value};

    fn metadata_field_names(type_name: &str) -> BTreeSet<&'static str> {
        documented_output_types()
            .iter()
            .find(|ty| ty.type_name == type_name)
            .unwrap_or_else(|| panic!("{} should be documented", type_name))
            .fields
            .iter()
            .map(|field| field.json_name)
            .collect()
    }

    fn serialized_object_keys<T: Serialize>(value: &T) -> BTreeSet<String> {
        serde_json::to_value(value)
            .expect("serialize")
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect()
    }

    fn sample_match() -> OutputMatch {
        OutputMatch {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("src/lib.rs".to_string()),
            start_line: 1,
            end_line: 2,
            matcher: Some("1-hash".to_string()),
            score: 100.0,
            matched_length: Some(42),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: Some("mit_1.RULE".to_string()),
            rule_url: Some("https://example.invalid/rule".to_string()),
            matched_text: Some("Permission is hereby granted".to_string()),
            matched_text_diagnostics: Some("diagnostics".to_string()),
            referenced_filenames: Some(vec!["LICENSE".to_string()]),
        }
    }

    fn sample_license_detection() -> OutputLicenseDetection {
        OutputLicenseDetection {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            matches: vec![sample_match()],
            detection_log: vec!["normalized".to_string()],
            identifier: Some("det-1".to_string()),
        }
    }

    fn sample_tally_entry() -> OutputTallyEntry {
        OutputTallyEntry {
            value: Some("mit".to_string()),
            count: 1,
        }
    }

    fn sample_tallies() -> OutputTallies {
        OutputTallies {
            detected_license_expression: vec![sample_tally_entry()],
            copyrights: vec![sample_tally_entry()],
            holders: vec![sample_tally_entry()],
            authors: vec![sample_tally_entry()],
            programming_language: vec![sample_tally_entry()],
        }
    }

    fn sample_party() -> OutputParty {
        OutputParty {
            r#type: Some("person".to_string()),
            role: Some("author".to_string()),
            name: Some("Example Person".to_string()),
            email: Some("person@example.invalid".to_string()),
            url: Some("https://example.invalid/person".to_string()),
            organization: Some("Example Org".to_string()),
            organization_url: Some("https://example.invalid".to_string()),
            timezone: Some("UTC".to_string()),
        }
    }

    fn sample_file_reference() -> OutputFileReference {
        OutputFileReference {
            path: "LICENSE".to_string(),
            size: Some(123),
            sha1: Some("a".repeat(40)),
            md5: Some("b".repeat(32)),
            sha256: Some("c".repeat(64)),
            sha512: Some("d".repeat(128)),
            extra_data: Some(std::collections::HashMap::from_iter([(
                "hint".to_string(),
                Value::String("local".to_string()),
            )])),
        }
    }

    fn sample_license_policy_entry() -> OutputLicensePolicyEntry {
        OutputLicensePolicyEntry {
            license_key: "mit".to_string(),
            label: "Allowed".to_string(),
            color_code: "#00ff00".to_string(),
            icon: "check".to_string(),
        }
    }

    fn sample_author() -> OutputAuthor {
        OutputAuthor {
            author: "Example Author".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_copyright() -> OutputCopyright {
        OutputCopyright {
            copyright: "Copyright 2026 Example".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_email() -> OutputEmail {
        OutputEmail {
            email: "example@example.invalid".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_holder() -> OutputHolder {
        OutputHolder {
            holder: "Example Holder".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_url() -> OutputURL {
        OutputURL {
            url: "https://example.invalid".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_package_data() -> OutputPackageData {
        OutputPackageData {
            package_type: Some(OutputPackageType::from(crate::models::PackageType::Cargo)),
            namespace: Some("example".to_string()),
            name: Some("crate-name".to_string()),
            version: Some("1.2.3".to_string()),
            qualifiers: Some(std::collections::HashMap::from_iter([(
                "arch".to_string(),
                "x86_64".to_string(),
            )])),
            subpath: Some("sub".to_string()),
            primary_language: Some("Rust".to_string()),
            description: Some("Example package data".to_string()),
            release_date: Some("2026-05-31".to_string()),
            parties: vec![sample_party()],
            keywords: vec!["example".to_string()],
            homepage_url: Some("https://example.invalid/home".to_string()),
            download_url: Some("https://example.invalid/download".to_string()),
            size: Some(42),
            sha1: Some("a".repeat(40)),
            md5: Some("b".repeat(32)),
            sha256: Some("c".repeat(64)),
            sha512: Some("d".repeat(128)),
            bug_tracking_url: Some("https://example.invalid/issues".to_string()),
            code_view_url: Some("https://example.invalid/code".to_string()),
            vcs_url: Some("git+https://example.invalid/repo.git".to_string()),
            copyright: Some("Copyright 2026 Example".to_string()),
            holder: Some("Example Holder".to_string()),
            declared_license_expression: Some("mit".to_string()),
            declared_license_expression_spdx: Some("MIT".to_string()),
            license_detections: vec![sample_license_detection()],
            other_license_expression: Some("apache-2.0".to_string()),
            other_license_expression_spdx: Some("Apache-2.0".to_string()),
            other_license_detections: vec![sample_license_detection()],
            extracted_license_statement: Some("MIT".to_string()),
            notice_text: Some("notice".to_string()),
            source_packages: vec!["pkg:cargo/source@1.0.0".to_string()],
            file_references: vec![sample_file_reference()],
            is_private: true,
            is_virtual: true,
            extra_data: Some(std::collections::HashMap::from_iter([(
                "custom".to_string(),
                Value::String("value".to_string()),
            )])),
            dependencies: vec![sample_dependency()],
            repository_homepage_url: Some("https://example.invalid/repo-home".to_string()),
            repository_download_url: Some("https://example.invalid/repo-download".to_string()),
            api_data_url: Some("https://example.invalid/api".to_string()),
            datasource_id: Some(OutputDatasourceId::from(
                crate::models::DatasourceId::CargoToml,
            )),
            purl: Some("pkg:cargo/example/crate-name@1.2.3".to_string()),
        }
    }

    fn sample_dependency() -> OutputDependency {
        OutputDependency {
            purl: Some("pkg:cargo/example/dep@1.0.0".to_string()),
            extracted_requirement: Some("^1.0".to_string()),
            scope: Some("runtime".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(true),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: Some(std::collections::HashMap::from_iter([(
                "kind".to_string(),
                Value::String("normal".to_string()),
            )])),
        }
    }

    fn sample_top_level_dependency() -> OutputTopLevelDependency {
        OutputTopLevelDependency {
            purl: Some("pkg:cargo/example/dep@1.0.0".to_string()),
            extracted_requirement: Some("^1.0".to_string()),
            scope: Some("runtime".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(true),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: Some(std::collections::HashMap::from_iter([(
                "kind".to_string(),
                Value::String("normal".to_string()),
            )])),
            dependency_uid: "dep-uid".to_string(),
            for_package_uid: Some("pkg-uid".to_string()),
            datafile_path: "Cargo.toml".to_string(),
            datasource_id: OutputDatasourceId::from(crate::models::DatasourceId::CargoToml),
            namespace: Some("example".to_string()),
        }
    }

    fn sample_top_level_license_detection() -> OutputTopLevelLicenseDetection {
        OutputTopLevelLicenseDetection {
            identifier: "top-1".to_string(),
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            detection_count: 1,
            detection_log: vec!["grouped".to_string()],
            reference_matches: vec![sample_match()],
        }
    }

    fn sample_header() -> OutputHeader {
        OutputHeader {
            tool_name: "provenant".to_string(),
            tool_version: "0.1.7".to_string(),
            options: Map::from_iter([("--license".to_string(), Value::Bool(true))]),
            notice: "Generated with Provenant".to_string(),
            start_timestamp: "2026-05-31T00:00:00Z".to_string(),
            end_timestamp: "2026-05-31T00:00:10Z".to_string(),
            output_format_version: "3.0.0".to_string(),
            duration: 10.0,
            errors: vec!["none".to_string()],
            warnings: vec!["warning".to_string()],
            extra_data: OutputExtraData {
                system_environment: OutputSystemEnvironment {
                    operating_system: "Linux".to_string(),
                    cpu_architecture: "x86_64".to_string(),
                    platform: "linux".to_string(),
                    platform_version: "6.0".to_string(),
                    rust_version: "1.88.0".to_string(),
                },
                spdx_license_list_version: "3.26".to_string(),
                files_count: 1,
                directories_count: 1,
                excluded_count: 0,
                license_index_provenance: Some(OutputLicenseIndexProvenance {
                    source: "embedded".to_string(),
                    dataset_fingerprint: "fingerprint".to_string(),
                    ignored_rules: vec!["rule-a".to_string()],
                    ignored_licenses: vec!["lic-a".to_string()],
                    ignored_rules_due_to_licenses: vec!["rule-b".to_string()],
                    added_rules: vec!["rule-c".to_string()],
                    replaced_rules: vec!["rule-d".to_string()],
                    added_licenses: vec!["lic-b".to_string()],
                    replaced_licenses: vec!["lic-c".to_string()],
                }),
            },
        }
    }

    fn sample_file_info() -> OutputFileInfo {
        OutputFileInfo {
            name: "mod.rs".to_string(),
            base_name: "mod".to_string(),
            extension: ".rs".to_string(),
            path: "src/mod.rs".to_string(),
            file_type: OutputFileType::File,
            mime_type: Some("text/rust".to_string()),
            file_type_label: Some("source".to_string()),
            size: 123,
            date: Some("2026-05-31".to_string()),
            sha1: Some("a".repeat(40)),
            md5: Some("b".repeat(32)),
            sha256: Some("c".repeat(64)),
            sha1_git: Some("d".repeat(40)),
            programming_language: Some("Rust".to_string()),
            package_data: vec![sample_package_data()],
            license_expression: Some("MIT".to_string()),
            license_detections: vec![sample_license_detection()],
            license_clues: vec![sample_match()],
            percentage_of_license_text: Some(50.0),
            copyrights: vec![sample_copyright()],
            holders: vec![sample_holder()],
            authors: vec![sample_author()],
            emails: vec![sample_email()],
            urls: vec![sample_url()],
            for_packages: vec!["pkg-uid".to_string()],
            scan_errors: vec!["parse warning".to_string()],
            license_policy: Some(vec![sample_license_policy_entry()]),
            is_generated: Some(true),
            is_binary: Some(false),
            is_text: Some(true),
            is_archive: Some(false),
            is_media: Some(false),
            is_source: Some(true),
            is_script: Some(false),
            files_count: Some(1),
            dirs_count: Some(0),
            size_count: Some(123),
            source_count: Some(1),
            is_legal: true,
            is_manifest: true,
            is_readme: true,
            is_top_level: true,
            is_key_file: true,
            is_referenced: true,
            is_community: true,
            facets: vec!["core".to_string()],
            tallies: Some(sample_tallies()),
        }
    }

    fn sample_package() -> OutputPackage {
        OutputPackage {
            package_type: Some(OutputPackageType::from(crate::models::PackageType::Cargo)),
            namespace: Some("example".to_string()),
            name: Some("crate-name".to_string()),
            version: Some("1.2.3".to_string()),
            qualifiers: Some(std::collections::HashMap::from_iter([(
                "arch".to_string(),
                "x86_64".to_string(),
            )])),
            subpath: Some("sub".to_string()),
            primary_language: Some("Rust".to_string()),
            description: Some("Example package".to_string()),
            release_date: Some("2026-05-31".to_string()),
            parties: vec![sample_party()],
            keywords: vec!["example".to_string()],
            homepage_url: Some("https://example.invalid/home".to_string()),
            download_url: Some("https://example.invalid/download".to_string()),
            size: Some(42),
            sha1: Some("a".repeat(40)),
            md5: Some("b".repeat(32)),
            sha256: Some("c".repeat(64)),
            sha512: Some("d".repeat(128)),
            bug_tracking_url: Some("https://example.invalid/issues".to_string()),
            code_view_url: Some("https://example.invalid/code".to_string()),
            vcs_url: Some("git+https://example.invalid/repo.git".to_string()),
            copyright: Some("Copyright 2026 Example".to_string()),
            holder: Some("Example Holder".to_string()),
            declared_license_expression: Some("mit".to_string()),
            declared_license_expression_spdx: Some("MIT".to_string()),
            license_detections: vec![sample_license_detection()],
            other_license_expression: Some("apache-2.0".to_string()),
            other_license_expression_spdx: Some("Apache-2.0".to_string()),
            other_license_detections: vec![sample_license_detection()],
            extracted_license_statement: Some("MIT".to_string()),
            notice_text: Some("notice".to_string()),
            source_packages: vec!["pkg:cargo/source@1.0.0".to_string()],
            is_private: true,
            is_virtual: true,
            extra_data: Some(std::collections::HashMap::from_iter([(
                "custom".to_string(),
                Value::String("value".to_string()),
            )])),
            repository_homepage_url: Some("https://example.invalid/repo-home".to_string()),
            repository_download_url: Some("https://example.invalid/repo-download".to_string()),
            api_data_url: Some("https://example.invalid/api".to_string()),
            purl: Some("pkg:cargo/example/crate-name@1.2.3".to_string()),
            package_uid: "pkg-uid".to_string(),
            datafile_paths: vec!["Cargo.toml".to_string()],
            datasource_ids: vec![OutputDatasourceId::from(
                crate::models::DatasourceId::CargoToml,
            )],
        }
    }

    fn assert_metadata_matches_serialized_keys<T: Serialize>(type_name: &str, value: &T) {
        let documented = metadata_field_names(type_name);
        let serialized = serialized_object_keys(value)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let documented_owned = documented
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            documented_owned, serialized,
            "metadata mismatch for {}",
            type_name
        );
    }

    #[test]
    fn documented_type_names_are_unique() {
        let mut seen = BTreeSet::new();
        for ty in documented_output_types() {
            assert!(
                seen.insert(ty.type_name),
                "duplicate type doc: {}",
                ty.type_name
            );
        }
    }

    #[test]
    fn documented_json_paths_are_unique() {
        let mut seen = BTreeSet::new();
        for ty in documented_output_types() {
            for path in ty.json_paths {
                assert!(seen.insert(*path), "duplicate json path doc: {}", path);
            }
        }
    }

    #[test]
    fn documented_field_names_are_unique_per_type() {
        for ty in documented_output_types() {
            let mut seen = BTreeSet::new();
            for field in ty.fields {
                assert!(
                    seen.insert(field.json_name),
                    "duplicate field doc in {}: {}",
                    ty.type_name,
                    field.json_name
                );
            }
        }
    }

    #[test]
    fn output_file_info_doc_starts_with_public_serialization_order() {
        let file_info = documented_output_types()
            .iter()
            .find(|ty| ty.type_name == "OutputFileInfo")
            .expect("OutputFileInfo should be documented");
        let fields = file_info
            .fields
            .iter()
            .map(|field| field.json_name)
            .collect::<Vec<_>>();

        assert_eq!(
            &fields[..5],
            &["path", "type", "name", "base_name", "extension",]
        );
    }

    #[test]
    fn metadata_matches_serialized_keys_for_core_documented_types() {
        assert_metadata_matches_serialized_keys("OutputHeader", &sample_header());
        assert_metadata_matches_serialized_keys("OutputFileInfo", &sample_file_info());
        assert_metadata_matches_serialized_keys("OutputPackage", &sample_package());
        assert_metadata_matches_serialized_keys("OutputPackageData", &sample_package_data());
        assert_metadata_matches_serialized_keys("OutputDependency", &sample_dependency());
        assert_metadata_matches_serialized_keys(
            "OutputTopLevelDependency",
            &sample_top_level_dependency(),
        );
        assert_metadata_matches_serialized_keys(
            "OutputTopLevelLicenseDetection",
            &sample_top_level_license_detection(),
        );
    }
}
