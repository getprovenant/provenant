// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Field metadata for file-level evidence surfaces (authors, copyrights, emails, holders, URLs).

use super::OutputFieldDoc;

pub(super) const AUTHOR_FIELDS: &[OutputFieldDoc] = &[
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

pub(super) const COPYRIGHT_FIELDS: &[OutputFieldDoc] = &[
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

pub(super) const EMAIL_FIELDS: &[OutputFieldDoc] = &[
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

pub(super) const HOLDER_FIELDS: &[OutputFieldDoc] = &[
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

pub(super) const URL_FIELDS: &[OutputFieldDoc] = &[
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
