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
use std::time::Instant;

/// Per-detection durations so the caller can attribute them to the matching
/// `scan:*` progress phases; `None` means the detection did not run.
#[derive(Default)]
pub(super) struct ContactDetectionTimings {
    pub emails_seconds: Option<f64>,
    pub urls_seconds: Option<f64>,
}

pub(super) fn extract_email_url_information(
    file_info_builder: &mut FileInfoBuilder,
    path: &Path,
    text_content: &str,
    text_options: &TextDetectionOptions,
    from_binary_strings: bool,
) -> ContactDetectionTimings {
    let mut timings = ContactDetectionTimings::default();
    if !text_options.detect_emails && !text_options.detect_urls {
        return timings;
    }

    let applies_gettext_exception = is_gettext_mo_path(path);
    let is_font_metadata_path = !from_binary_strings && is_font_metadata_contact_path(path);
    let apply_binary_contact_filters =
        (from_binary_strings || is_font_metadata_path) && !applies_gettext_exception;
    let font_metadata_lines =
        is_font_metadata_path.then(|| text_content.lines().collect::<Vec<_>>());

    if text_options.detect_emails {
        let started = Instant::now();
        let config = DetectionConfig {
            max_emails: text_options.max_emails,
            max_urls: text_options.max_urls,
            unique: from_binary_strings,
        };
        let mut seen_email_spans = HashSet::new();
        let emails = finder::find_emails(text_content, &config)
            .into_iter()
            .filter(|d| {
                !apply_binary_contact_filters
                    || is_binary_string_email_candidate(&d.email)
                    || is_short_font_metadata_email_alias(
                        font_metadata_lines.as_deref(),
                        &d.email,
                        d.start_line,
                    )
            })
            .filter(|d| seen_email_spans.insert((d.email.clone(), d.start_line, d.end_line)))
            .map(|d| OutputEmail {
                email: d.email,
                start_line: d.start_line,
                end_line: d.end_line,
            })
            .collect::<Vec<_>>();
        file_info_builder.emails(emails);
        timings.emails_seconds = Some(started.elapsed().as_secs_f64());
    }

    if text_options.detect_urls {
        let started = Instant::now();
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
        timings.urls_seconds = Some(started.elapsed().as_secs_f64());
    }

    timings
}

fn is_gettext_mo_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mo"))
}

fn is_font_metadata_contact_path(path: &Path) -> bool {
    is_supported_font_path(path)
}

fn is_short_font_metadata_email_alias(
    font_metadata_lines: Option<&[&str]>,
    email: &str,
    line_number: LineNumber,
) -> bool {
    let raw_line = font_metadata_lines
        .and_then(|lines| lines.get(line_number.get() - 1).copied())
        .map(|line| {
            line.trim().trim_matches(|c: char| {
                matches!(
                    c,
                    '<' | '>' | '(' | ')' | '[' | ']' | '"' | '\'' | '`' | ',' | ';'
                )
            })
        })
        .unwrap_or("");
    let normalized_email = email.to_ascii_lowercase();
    let raw_line_lower = raw_line.to_ascii_lowercase();
    let Some(email_start) = raw_line_lower.find(&normalized_email) else {
        return false;
    };
    let email_end = email_start + normalized_email.len();
    let prefix = &raw_line[email_start.saturating_sub(email_start)..email_start];
    let suffix = &raw_line[email_end..];
    let extra = [prefix, suffix].concat();
    if !extra.is_empty()
        && (extra.contains(char::is_whitespace)
            || extra.contains('@')
            || extra.chars().count() > 12
            || extra.chars().filter(|c| c.is_ascii_alphanumeric()).count() > 2)
    {
        return false;
    }

    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    let Some((host, tld)) = domain.rsplit_once('.') else {
        return false;
    };

    !local.is_empty()
        && local.len() <= 2
        && local.chars().any(|c| c.is_ascii_alphabetic())
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-'))
        && !host.is_empty()
        && host.len() <= 2
        && host.chars().all(|c| c.is_ascii_alphanumeric())
        && (2..=3).contains(&tld.len())
        && tld.chars().all(|c| c.is_ascii_alphabetic())
        && raw_line.chars().any(|c| c.is_ascii_uppercase())
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
