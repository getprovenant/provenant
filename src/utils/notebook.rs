// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Jupyter notebook (`.ipynb`) text extraction for scanner detection.
//!
//! A `.ipynb` file is a JSON document whose human-authored text (code, markdown)
//! and program output live inside JSON string arrays. Scanning the raw JSON makes
//! detection both miss real notices (e.g. a `(c) ...` line wrapped as
//! `"\t(c) Foo, 2012\n",`) and emit false positives from JSON array punctuation
//! around source lines. This module decodes the notebook into the plain cell text
//! so license/copyright detection sees the same clean text a reader would.
//!
//! Like the source-map extractor, this trades exact line-number fidelity (offsets
//! become relative to the extracted text) for correct detection content.

use std::path::Path;

use serde_json::Value;

/// Check whether a file is a Jupyter notebook based on its extension.
pub fn is_notebook(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ipynb"))
}

/// Extract the human-readable text from a Jupyter notebook's cells.
///
/// Concatenates each cell's `source` (code and markdown) together with textual
/// outputs (`stream` text and `text/plain` results). Non-text outputs such as
/// `image/png` data are intentionally skipped to avoid scanning base64 noise.
///
/// Returns `Some(combined_text)` when the notebook parses and yields any text,
/// otherwise `None` (the caller falls back to the raw content).
pub fn extract_notebook_content(json_text: &str) -> Option<String> {
    let json: Value = serde_json::from_str(json_text).ok()?;
    let cells = json.get("cells")?.as_array()?;

    let mut parts: Vec<String> = Vec::new();
    for cell in cells {
        if let Some(source) = cell.get("source").and_then(collect_text) {
            parts.push(source);
        }
        if let Some(outputs) = cell.get("outputs").and_then(Value::as_array) {
            for output in outputs {
                // `stream` outputs (e.g. stdout) carry their text under `text`.
                if let Some(text) = output.get("text").and_then(collect_text) {
                    parts.push(text);
                }
                // `execute_result` / `display_data` carry a `text/plain` rendering.
                if let Some(text) = output
                    .get("data")
                    .and_then(|data| data.get("text/plain"))
                    .and_then(collect_text)
                {
                    parts.push(text);
                }
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        // Separate cells with exactly one newline: trim each part's own trailing
        // newlines first so a part that already ends with `\n` doesn't add a blank
        // line, while a part that doesn't still stays on its own line.
        Some(
            parts
                .iter()
                .map(|part| part.trim_end_matches('\n'))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

/// nbformat stores text either as a single string or as an array of line strings
/// (each element already including its trailing newline). Reconstruct the original
/// text by concatenating array elements verbatim.
fn collect_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => (!s.is_empty()).then(|| s.clone()),
        Value::Array(items) => {
            let combined: String = items.iter().filter_map(Value::as_str).collect();
            (!combined.is_empty()).then_some(combined)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_notebook() {
        assert!(is_notebook(&PathBuf::from("Analysis.ipynb")));
        assert!(is_notebook(&PathBuf::from("a/b/NOTEBOOK.IPYNB")));
        assert!(!is_notebook(&PathBuf::from("script.py")));
        assert!(!is_notebook(&PathBuf::from("data.json")));
    }

    #[test]
    fn test_extract_source_array_and_stream_output() {
        // `source` as a line array; a stream output carrying a copyright notice.
        let json = r#"{
          "cells": [
            {"cell_type":"code","source":["@show typeof(C)\n","C[1:10,:]\n"],
             "outputs":[{"output_type":"stream","name":"stdout",
               "text":["\t(c) Brendan O'Donoghue, Stanford University, 2012\n"]}]}
          ]
        }"#;
        let text = extract_notebook_content(json).expect("should extract");
        // Cell source is reconstructed as clean code (no JSON punctuation).
        assert!(text.contains("@show typeof(C)\nC[1:10,:]"));
        // Output text is included so genuine notices are detectable.
        assert!(text.contains("(c) Brendan O'Donoghue, Stanford University, 2012"));
        // No JSON array punctuation leaks into the extracted text.
        assert!(!text.contains("\", \""));
    }

    #[test]
    fn test_extract_skips_binary_output_data() {
        let json = r#"{
          "cells": [
            {"cell_type":"code","source":"print(1)",
             "outputs":[{"output_type":"display_data",
               "data":{"image/png":"iVBORw0KGgoAAAANSU","text/plain":"<Figure>"}}]}
          ]
        }"#;
        let text = extract_notebook_content(json).expect("should extract");
        assert!(text.contains("print(1)"));
        assert!(text.contains("<Figure>"));
        assert!(!text.contains("iVBORw0KGgoAAAANSU"));
    }

    #[test]
    fn test_extract_source_string_form() {
        let json = r##"{"cells":[{"cell_type":"markdown","source":"# Title\nSome prose"}]}"##;
        let text = extract_notebook_content(json).expect("should extract");
        assert_eq!(text, "# Title\nSome prose");
    }

    #[test]
    fn test_extract_invalid_or_empty_returns_none() {
        assert!(extract_notebook_content("not json").is_none());
        assert!(extract_notebook_content(r#"{"nbformat":4}"#).is_none());
        assert!(extract_notebook_content(r#"{"cells":[]}"#).is_none());
    }

    #[test]
    fn test_extract_no_blank_line_between_parts() {
        // A stream output whose lines each end with `\n`, followed by another cell.
        let json = r#"{
          "cells": [
            {"cell_type":"code","source":"a()","outputs":[{"output_type":"stream","text":["one\n","two\n"]}]},
            {"cell_type":"code","source":"b()"}
          ]
        }"#;
        let text = extract_notebook_content(json).expect("should extract");
        assert!(
            !text.contains("\n\n"),
            "no blank lines between parts: {text:?}"
        );
        assert!(text.contains("two\nb()"));
    }
}
