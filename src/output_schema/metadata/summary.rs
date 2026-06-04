// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Field metadata for summary, tally, and clarity-score surfaces.

use super::OutputFieldDoc;

pub(super) const SUMMARY_FIELDS: &[OutputFieldDoc] = &[
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

pub(super) const TALLIES_FIELDS: &[OutputFieldDoc] = &[
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

pub(super) const FACET_TALLIES_FIELDS: &[OutputFieldDoc] = &[
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

pub(super) const TALLY_ENTRY_FIELDS: &[OutputFieldDoc] = &[
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

pub(super) const LICENSE_CLARITY_SCORE_FIELDS: &[OutputFieldDoc] = &[
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
