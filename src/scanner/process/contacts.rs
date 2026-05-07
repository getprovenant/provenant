// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::binary_text::{is_binary_string_email_candidate, normalize_binary_string_url};
use crate::finder::{self, DetectionConfig};
use crate::models::LineNumber;
use crate::models::{FileInfoBuilder, OutputEmail, OutputURL};
use crate::scanner::TextDetectionOptions;
use crate::utils::font::is_supported_font_path;
use std::collections::HashSet;
use std::path::Path;

pub(super) fn extract_email_url_information(
    file_info_builder: &mut FileInfoBuilder,
    path: &Path,
    text_content: &str,
    text_options: &TextDetectionOptions,
    from_binary_strings: bool,
) {
    if !text_options.detect_emails && !text_options.detect_urls {
        return;
    }

    let apply_binary_contact_filters =
        (from_binary_strings || is_font_metadata_contact_path(path)) && !is_gettext_mo_path(path);

    if text_options.detect_emails {
        let config = DetectionConfig {
            max_emails: text_options.max_emails,
            max_urls: text_options.max_urls,
            unique: from_binary_strings,
        };
        let mut seen_email_spans = HashSet::new();
        let emails = finder::find_emails(text_content, &config)
            .into_iter()
            .filter(|d| !apply_binary_contact_filters || is_binary_string_email_candidate(&d.email))
            .filter(|d| seen_email_spans.insert((d.email.clone(), d.start_line, d.end_line)))
            .map(|d| OutputEmail {
                email: d.email,
                start_line: d.start_line,
                end_line: d.end_line,
            })
            .collect::<Vec<_>>();
        file_info_builder.emails(emails);
    }

    if text_options.detect_urls {
        let config = DetectionConfig {
            max_emails: text_options.max_emails,
            max_urls: if apply_binary_contact_filters {
                0
            } else {
                text_options.max_urls
            },
            unique: !apply_binary_contact_filters,
        };
        let mut urls = finder::find_urls(text_content, &config)
            .into_iter()
            .filter_map(|d| {
                let url = if apply_binary_contact_filters {
                    normalize_binary_string_url(&d.url)?
                } else {
                    d.url
                };
                Some(OutputURL {
                    url,
                    start_line: d.start_line,
                    end_line: d.end_line,
                })
            })
            .collect::<Vec<_>>();
        if apply_binary_contact_filters {
            urls.extend(collect_binary_url_salvage_detections(text_content));
            let mut seen = HashSet::new();
            urls.retain(|url| seen.insert(url.url.clone()));
            if text_options.max_urls > 0 && urls.len() > text_options.max_urls {
                urls.truncate(text_options.max_urls);
            }
        }
        file_info_builder.urls(urls);
    }
}

fn is_gettext_mo_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mo"))
}

fn is_font_metadata_contact_path(path: &Path) -> bool {
    is_supported_font_path(path)
}

fn collect_binary_url_salvage_detections(text_content: &str) -> Vec<OutputURL> {
    text_content
        .lines()
        .enumerate()
        .flat_map(|(line_index, line)| {
            let line_number = LineNumber::from_0_indexed(line_index);
            line.split_whitespace().filter_map(move |token| {
                let candidate = token.trim_matches(|c: char| {
                    matches!(
                        c,
                        '<' | '>' | '(' | ')' | '[' | ']' | '"' | '\'' | '`' | ',' | ';'
                    )
                });
                if !candidate.contains("://") {
                    return None;
                }
                let url = normalize_binary_string_url(candidate)?;
                Some(OutputURL {
                    url,
                    start_line: line_number,
                    end_line: line_number,
                })
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "contacts_test.rs"]
mod tests;
