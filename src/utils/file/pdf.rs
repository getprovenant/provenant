// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Bounded, panic-isolated PDF text extraction with first-page heading
//! reconstruction and non-actionable failure suppression.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

pub(super) const MAX_PDF_TEXT_EXTRACTION_BYTES: usize = 32 * 1024 * 1024;

pub(super) fn extract_pdf_text(path: &Path, bytes: &[u8]) -> (String, Option<String>) {
    if bytes.len() < 5 || &bytes[..5] != b"%PDF-" {
        return (String::new(), None);
    }

    if bytes.len() > MAX_PDF_TEXT_EXTRACTION_BYTES {
        return (
            String::new(),
            Some(format!(
                "PDF text extraction skipped because file exceeds {} bytes",
                MAX_PDF_TEXT_EXTRACTION_BYTES
            )),
        );
    }

    let mut failures = Vec::new();
    let mut saw_success = false;

    let extracted = catch_unwind(AssertUnwindSafe(
        || -> Result<String, Box<dyn std::error::Error>> {
            let mut document = pdf_oxide::document::PdfDocument::from_bytes(bytes.to_vec())?;
            extract_first_pdf_page_text(&mut document)
        },
    ));
    match extracted {
        Ok(Ok(text)) => {
            saw_success = true;
            if let Some(normalized) = normalize_pdf_text(text) {
                return (normalized, None);
            }
        }
        Ok(Err(err)) => failures.push(format!("from-bytes first-page: {err}")),
        Err(payload) => failures.push(format!(
            "from-bytes first-page panic: {}",
            panic_payload_to_string(payload.as_ref())
        )),
    }

    let extracted = catch_unwind(AssertUnwindSafe(
        || -> Result<String, Box<dyn std::error::Error>> {
            let mut document = pdf_oxide::document::PdfDocument::open(path)?;
            extract_pdf_text_from_document(&mut document)
        },
    ));
    match extracted {
        Ok(Ok(text)) => {
            saw_success = true;
            if let Some(normalized) = normalize_pdf_text(text) {
                return (normalized, None);
            }
        }
        Ok(Err(err)) => failures.push(format!("open full-document: {err}")),
        Err(payload) => failures.push(format!(
            "open full-document panic: {}",
            panic_payload_to_string(payload.as_ref())
        )),
    }

    let extracted = catch_unwind(AssertUnwindSafe(
        || -> Result<String, Box<dyn std::error::Error>> {
            let mut document = pdf_oxide::document::PdfDocument::from_bytes(bytes.to_vec())?;
            extract_pdf_text_from_document(&mut document)
        },
    ));
    match extracted {
        Ok(Ok(text)) => {
            saw_success = true;
            if let Some(normalized) = normalize_pdf_text(text) {
                return (normalized, None);
            }
        }
        Ok(Err(err)) => failures.push(format!("from-bytes full-document: {err}")),
        Err(payload) => failures.push(format!(
            "from-bytes full-document panic: {}",
            panic_payload_to_string(payload.as_ref())
        )),
    }

    if saw_success || is_non_actionable_pdf_failure(&failures) {
        (String::new(), None)
    } else {
        (
            String::new(),
            Some(format!(
                "PDF text extraction failed after {} attempts: {}",
                failures.len(),
                failures.join("; ")
            )),
        )
    }
}

pub(super) fn is_non_actionable_pdf_failure(failures: &[String]) -> bool {
    !failures.is_empty()
        && failures.iter().all(|failure| {
            failure.contains("requires a password")
                || failure.contains("Encrypt dictionary missing /O")
                || failure.contains("Encrypt dictionary missing /U")
                || failure.contains("security handler cannot be found")
                || failure.contains("Invalid cross-reference table")
        })
}

fn panic_payload_to_string(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

fn extract_first_pdf_page_text(
    document: &mut pdf_oxide::document::PdfDocument,
) -> Result<String, Box<dyn std::error::Error>> {
    if document.page_count()? == 0 {
        return Ok(String::new());
    }

    let extracted_text = document.extract_text(0)?;
    let markdown_text =
        document.to_markdown(0, &pdf_oxide::converters::ConversionOptions::default())?;
    if pdf_markdown_heading_lines(&markdown_text).is_empty() {
        return Ok(extracted_text);
    }

    let pipeline_text =
        document.to_plain_text(0, &pdf_oxide::converters::ConversionOptions::default())?;

    Ok(merge_pdf_first_page_text(
        &extracted_text,
        &markdown_text,
        &pipeline_text,
    ))
}

fn extract_pdf_text_from_document(
    document: &mut pdf_oxide::document::PdfDocument,
) -> Result<String, Box<dyn std::error::Error>> {
    Ok(document.to_plain_text_all(&pdf_oxide::converters::ConversionOptions::default())?)
}

fn normalize_pdf_text(text: String) -> Option<String> {
    let normalized = text.replace(['\r', '\u{0c}'], "\n");
    (!normalized.trim().is_empty()).then_some(normalized)
}

fn merge_pdf_first_page_text(
    _extracted_text: &str,
    markdown_text: &str,
    pipeline_text: &str,
) -> String {
    let pipeline = pipeline_text.trim();
    if pipeline.is_empty() {
        return String::new();
    }

    let prefix = pdf_first_page_heading_prefix(markdown_text);
    let Some(prefix) = prefix else {
        return pipeline_text.to_string();
    };

    if pdf_text_contains_heading_prefix(pipeline, &prefix) {
        pipeline_text.to_string()
    } else {
        format!("{prefix}\n\n{pipeline}")
    }
}

fn pdf_text_contains_heading_prefix(text: &str, prefix: &str) -> bool {
    normalize_pdf_heading_comparison_text(text)
        .contains(&normalize_pdf_heading_comparison_text(prefix))
}

pub(super) fn normalize_pdf_heading_comparison_text(text: &str) -> String {
    text.split_whitespace()
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

fn pdf_first_page_heading_prefix(markdown_text: &str) -> Option<String> {
    let mut lines = Vec::new();

    for line in pdf_markdown_heading_lines(markdown_text) {
        push_unique_line(&mut lines, line);
    }

    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn pdf_markdown_heading_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('#').map(str::trim_start))
        .map(|line| line.trim_matches('#').trim())
        .filter(|line| !line.is_empty())
        .filter(|line| !looks_like_numbered_section_heading(line))
        .take(4)
        .map(ToOwned::to_owned)
        .collect()
}

fn push_unique_line(lines: &mut Vec<String>, line: String) {
    if !lines.iter().any(|existing| existing == &line) {
        lines.push(line);
    }
}

fn looks_like_numbered_section_heading(line: &str) -> bool {
    let mut chars = line.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !first.is_ascii_digit() {
        return false;
    }

    matches!(chars.next(), Some('.'))
}

#[cfg(test)]
mod tests {
    use super::is_non_actionable_pdf_failure;

    #[test]
    fn test_non_actionable_pdf_failures_are_suppressed() {
        assert!(is_non_actionable_pdf_failure(&[
            "from-bytes first-page: PDF is encrypted and requires a password".to_string(),
            "open full-document: PDF is encrypted and requires a password".to_string(),
        ]));
        assert!(is_non_actionable_pdf_failure(&[
            "from-bytes first-page: Invalid cross-reference table".to_string(),
            "open full-document: Invalid cross-reference table".to_string(),
        ]));
        assert!(is_non_actionable_pdf_failure(&[
            "from-bytes first-page: Invalid PDF: Encrypt dictionary missing /O".to_string(),
            "open full-document: Invalid PDF: security handler cannot be found".to_string(),
        ]));
        assert!(!is_non_actionable_pdf_failure(&[
            "from-bytes first-page: some other parser failure".to_string(),
        ]));
    }
}
