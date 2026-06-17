// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use super::binary_text::{
    extract_named_author_from_binary_line, has_binary_name_like_shape, has_excessive_at_noise,
    has_sufficient_alphabetic_content, is_binary_string_author_candidate, is_company_like_suffix,
};
use crate::copyright::{
    self, AuthorDetection, CopyrightDetection, HolderDetection, looks_like_source_code,
    prepare_text_line, refine_author, refine_copyright,
};
use crate::models::{Author, Copyright, FileInfoBuilder, Holder, LineNumber};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

pub(super) fn extract_copyright_information(
    file_info_builder: &mut FileInfoBuilder,
    path: &Path,
    text_content: &str,
    timeout_seconds: f64,
    from_binary_strings: bool,
) {
    let text_content = crate::utils::sourcemap::detection_text(path, text_content);
    let text_content = text_content.as_ref();

    if copyright::is_credits_file(path) {
        let author_detections = copyright::detect_credits_authors(text_content);
        if !author_detections.is_empty() {
            file_info_builder.authors(
                author_detections
                    .into_iter()
                    .map(|a| Author {
                        author: a.author,
                        start_line: a.start_line,
                        end_line: a.end_line,
                    })
                    .collect(),
            );
            return;
        }
    }

    let max_runtime = if timeout_seconds.is_finite() && timeout_seconds > 0.0 {
        Some(Duration::from_secs_f64(timeout_seconds))
    } else {
        None
    };

    let (copyrights, holders, authors) = copyright::detect_copyrights(text_content, max_runtime);
    let (copyrights, holders, authors) = if from_binary_strings {
        prune_binary_string_detections(text_content, copyrights, holders, authors)
    } else {
        (copyrights, holders, authors)
    };

    // Universal guard against binary garbage leaking into parties. Real copyright
    // and holder values are short and printable; a multi-kilobyte blob or a string
    // dense with replacement / non-printable bytes (e.g. a raw image or font byte
    // run) is never a legitimate notice, regardless of the detection source path.
    // The rendered copyright value is checked alongside the normalized one because
    // raw-text projection can re-expand a blob the normalized value had collapsed.
    let holders: Vec<HolderDetection> = holders
        .into_iter()
        .filter(|h| !is_binary_garbage_party_value(&h.holder))
        .collect();

    file_info_builder.copyrights(
        copyrights
            .into_iter()
            .map(|c| Copyright {
                normalized_copyright: Some(c.copyright.clone()),
                copyright: render_raw_copyright_from_text(
                    text_content,
                    c.start_line,
                    c.end_line,
                    c.copyright.as_str(),
                ),
                start_line: c.start_line,
                end_line: c.end_line,
            })
            // The font-metadata-label check is applied to every copyright, not only
            // font paths: `License Description:` / `License Info URL:` are OpenType
            // name-table field labels and are never a legitimate copyright prefix in
            // real source, so rejecting them universally is safe and path-independent.
            .filter(|c| {
                let raw_span = render_raw_span(text_content, c.start_line, c.end_line);
                !is_binary_garbage_party_value(&c.copyright)
                    && !is_font_metadata_label_copyright(&c.copyright)
                    && !detection_is_source_code(&c.copyright, &raw_span)
                    && !c
                        .normalized_copyright
                        .as_deref()
                        .is_some_and(is_binary_garbage_party_value)
                    && !c
                        .normalized_copyright
                        .as_deref()
                        .is_some_and(is_font_metadata_label_copyright)
            })
            .collect::<Vec<Copyright>>(),
    );
    file_info_builder.holders(
        holders
            .into_iter()
            .filter(|h| {
                let raw_span = render_raw_span(text_content, h.start_line, h.end_line);
                !detection_is_source_code(&h.holder, &raw_span)
            })
            .map(|h| Holder {
                holder: h.holder,
                start_line: h.start_line,
                end_line: h.end_line,
            })
            .collect::<Vec<Holder>>(),
    );
    let mut authors = authors;
    authors.extend(extract_patch_header_author_supplements(text_content));
    authors.extend(extract_comment_author_supplements(text_content));
    let mut seen_authors = HashSet::new();
    authors.retain(|author| {
        let raw_span = render_raw_span(text_content, author.start_line, author.end_line);
        !is_binary_garbage_party_value(&author.author)
            && !detection_is_source_code(&author.author, &raw_span)
            && seen_authors.insert((author.author.clone(), author.start_line, author.end_line))
    });

    file_info_builder.authors(
        authors
            .into_iter()
            .map(|a| Author {
                author: a.author,
                start_line: a.start_line,
                end_line: a.end_line,
            })
            .collect::<Vec<Author>>(),
    );
}

fn render_raw_copyright_from_text(
    text_content: &str,
    start_line: LineNumber,
    end_line: LineNumber,
    fallback: &str,
) -> String {
    let raw_lines: Vec<&str> = text_content.lines().collect();
    let start_index = start_line.get().saturating_sub(1);
    let end_index = end_line.get();
    let Some(span) = raw_lines.get(start_index..end_index) else {
        return fallback.to_string();
    };

    let rendered = span
        .iter()
        .map(|line| strip_common_comment_wrappers(line.trim()))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let rendered = rendered.split_whitespace().collect::<Vec<_>>().join(" ");

    if rendered.is_empty() {
        fallback.to_string()
    } else if let Some(projected) = project_wrapped_copyright_value(&rendered, fallback) {
        projected
    } else if let Some(projected) = project_suspicious_native_copyright_value(&rendered) {
        projected
    } else {
        project_native_copyright_value(&rendered, fallback)
    }
}

/// Render the raw source span backing a detection so it can be tested for
/// source-code shape. Holder and author detections only carry a refined value
/// (`Region`, `Author`), which can look like an ordinary name; the underlying
/// source line (`vk::CmdCopyImage(..., &copyRegion);`,
/// `Author.objects.create(...)`) is what reveals it as code.
fn render_raw_span(text_content: &str, start_line: LineNumber, end_line: LineNumber) -> String {
    let raw_lines: Vec<&str> = text_content.lines().collect();
    let start_index = start_line.get().saturating_sub(1);
    let end_index = end_line.get();
    let Some(span) = raw_lines.get(start_index..end_index) else {
        return String::new();
    };

    span.iter()
        .map(|line| strip_common_comment_wrappers(line.trim()))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Return true when a detection's refined value or its backing source span is
/// obviously source code, so it should be dropped from output.
fn detection_is_source_code(value: &str, raw_span: &str) -> bool {
    looks_like_source_code(value) || looks_like_source_code(raw_span)
}

fn project_wrapped_copyright_value(rendered: &str, _fallback: &str) -> Option<String> {
    static VALUE_LEGALCOPYRIGHT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?ix)
            ^VALUE\s+"LegalCopyright"\s*,\s*"(?P<value>[^"]+)"
            (?:\s+"\\0")?\s*$
            "#,
        )
        .expect("valid LegalCopyright wrapper regex")
    });
    static ASSIGNMENT_COPYRIGHT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?ix)
            ^(?:PRODUCT_COPYRIGHT|INFOPLIST_KEY_NSHumanReadableCopyright)
            \s*=\s*(?P<value>.+?)\s*;?\s*$
            "#,
        )
        .expect("valid assignment copyright wrapper regex")
    });
    static APPLICATION_LEGALESE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?ix)^applicationLegalese\s*:\s*(?P<value>.+?)\s*,?\s*$"#)
            .expect("valid applicationLegalese wrapper regex")
    });
    static MARKUP_TEXT_COPYRIGHT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?ix)
            \btext\s*=\s*(?:"(?P<dq>[^"]+)"|'(?P<sq>[^']+)')
            "#,
        )
        .expect("valid markup text copyright wrapper regex")
    });

    let extracted = if let Some(captures) = VALUE_LEGALCOPYRIGHT_RE.captures(rendered) {
        captures
            .name("value")
            .map(|m| m.as_str().trim().to_string())
    } else if let Some(captures) = ASSIGNMENT_COPYRIGHT_RE.captures(rendered) {
        captures
            .name("value")
            .map(|m| m.as_str().trim().trim_matches(&['\'', '"'][..]).to_string())
    } else if let Some(captures) = APPLICATION_LEGALESE_RE.captures(rendered) {
        captures
            .name("value")
            .map(|m| m.as_str().trim().trim_matches(&['\'', '"'][..]).to_string())
    } else if let Some(captures) = MARKUP_TEXT_COPYRIGHT_RE.captures(rendered) {
        captures
            .name("dq")
            .or_else(|| captures.name("sq"))
            .map(|m| m.as_str().trim().to_string())
    } else {
        None
    }?;

    let projected = prepare_text_line(&extracted)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Some(projected)
}

fn project_native_copyright_value(rendered: &str, fallback: &str) -> String {
    let rendered = rendered.trim();
    let fallback = fallback.trim();
    if rendered.is_empty() || fallback.is_empty() {
        return fallback.to_string();
    }

    let rendered_lower = rendered.to_ascii_lowercase();
    let fallback_lower = fallback.to_ascii_lowercase();
    let Some(start) = rendered_lower.find(&fallback_lower) else {
        if refine_copyright(rendered).as_deref() == Some(fallback) {
            return preserve_native_suffix_for_semantic_match(rendered, fallback);
        }
        return rendered.to_string();
    };
    let end = start + fallback.len();
    let suffix = rendered[end..].trim();
    let Some(native_suffix) = normalize_native_suffix(suffix) else {
        return fallback.to_string();
    };

    if native_suffix.is_empty() {
        fallback.to_string()
    } else if native_suffix
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, '.' | ',' | ';' | ':'))
    {
        format!("{fallback}{native_suffix}")
    } else {
        format!("{fallback} {native_suffix}")
    }
}

fn preserve_native_suffix_for_semantic_match(rendered: &str, fallback: &str) -> String {
    let lower = rendered.to_ascii_lowercase();
    if lower.contains("all rights reserved") {
        let sep = if fallback.ends_with(['.', ',', ';', ':']) {
            ""
        } else {
            "."
        };
        return format!("{fallback}{sep} All rights reserved.");
    }

    fallback.to_string()
}

fn project_suspicious_native_copyright_value(rendered: &str) -> Option<String> {
    let lower = rendered.to_ascii_lowercase();
    let looks_suspicious = lower.ends_with(" or")
        || lower.contains(" and is licensed under ")
        || lower.contains(" are derived from ")
        || lower.contains(" it is hereby released to the")
        || lower.contains(", et al")
        || lower.contains(" cest ")
        || lower.contains(" ce?st ")
        || (lower.contains(':') && lower.contains(" edt "))
        || (lower.contains(':') && lower.contains(" cest"));
    if !looks_suspicious {
        return None;
    }

    let prepared = prepare_text_line(rendered);
    let refined = refine_copyright(&prepared)?;
    (refined != rendered.trim()).then_some(refined)
}

fn normalize_native_suffix(suffix: &str) -> Option<String> {
    static CONFIDENTIALITY_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ix)
            ^[.,;:]?\s*
            confidential
            (?:
                [\s,;:.\-]+information
              | [\s,;:.\-]+proprietary
              | [\s,;:.\-]+and[\s,;:.\-]+proprietary
            )
            \.?
            $
            ",
        )
        .expect("valid confidentiality suffix regex")
    });

    if suffix.is_empty() {
        return Some(String::new());
    }

    let normalized = suffix.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Some(String::new());
    }

    let lower = trimmed.to_ascii_lowercase();
    let punctuation_only = trimmed
        .chars()
        .all(|ch| matches!(ch, '.' | ',' | ';' | ':'));
    if punctuation_only {
        return Some(trimmed.to_string());
    }

    let all_rights_variants = [
        "all rights reserved",
        ". all rights reserved",
        ", all rights reserved",
        "; all rights reserved",
        ": all rights reserved",
        "all rights reserved.",
        ". all rights reserved.",
        ", all rights reserved.",
        "; all rights reserved.",
        ": all rights reserved.",
    ];
    if all_rights_variants.contains(&lower.as_str()) {
        return Some(trimmed.to_string());
    }

    if CONFIDENTIALITY_SUFFIX_RE.is_match(trimmed) {
        return Some(trimmed.to_string());
    }

    None
}

fn strip_common_comment_wrappers(line: &str) -> String {
    let mut trimmed = line.trim();

    loop {
        let next = trimmed
            .strip_prefix("///")
            .or_else(|| trimmed.strip_prefix("//!"))
            .or_else(|| trimmed.strip_prefix("//"))
            .or_else(|| trimmed.strip_prefix("/*"))
            .or_else(|| trimmed.strip_prefix("<!--"))
            .or_else(|| trimmed.strip_prefix('*'))
            .or_else(|| trimmed.strip_prefix('#'))
            .map(str::trim_start);
        let Some(next) = next else {
            break;
        };
        if next == trimmed {
            break;
        }
        trimmed = next;
    }

    trimmed = trimmed.trim_end();
    if let Some(stripped) = trimmed.strip_suffix("*/") {
        trimmed = stripped.trim_end();
    }
    if let Some(stripped) = trimmed.strip_suffix("-->") {
        trimmed = stripped.trim_end();
    }

    trimmed.to_string()
}

fn prune_binary_string_detections(
    text_content: &str,
    copyrights: Vec<CopyrightDetection>,
    holders: Vec<HolderDetection>,
    authors: Vec<AuthorDetection>,
) -> (
    Vec<CopyrightDetection>,
    Vec<HolderDetection>,
    Vec<AuthorDetection>,
) {
    let kept_copyrights: Vec<CopyrightDetection> = copyrights
        .into_iter()
        .filter(|c| is_binary_string_copyright_candidate(&c.copyright))
        .collect();

    let kept_holders: Vec<HolderDetection> = holders
        .into_iter()
        .filter(|holder| {
            kept_copyrights.iter().any(|copyright| {
                ranges_overlap(
                    holder.start_line,
                    holder.end_line,
                    copyright.start_line,
                    copyright.end_line,
                )
            })
        })
        .collect();

    let kept_authors = authors
        .into_iter()
        .filter(|author| is_binary_string_author_candidate(&author.author))
        .chain(extract_binary_string_author_supplements(text_content))
        .filter({
            let mut seen = HashSet::new();
            move |author| seen.insert(author.author.clone())
        })
        .collect();

    (kept_copyrights, kept_holders, kept_authors)
}

fn ranges_overlap(
    a_start: LineNumber,
    a_end: LineNumber,
    b_start: LineNumber,
    b_end: LineNumber,
) -> bool {
    a_start <= b_end && b_start <= a_end
}

/// Hard reject for party values (copyright / holder / author) that are obviously
/// scraped binary content rather than a human-authored notice. This is a last-line
/// correctness guard applied on every emit path: a legitimate notice is short and
/// printable, so a very long value or one dense with replacement / non-printable
/// bytes can be dropped outright without risking real source-file detections.
fn is_binary_garbage_party_value(text: &str) -> bool {
    // Longest realistic notices (multi-holder lines with URLs) stay well under
    // this bound; a value beyond it is a binary blob, not a notice.
    const MAX_PARTY_VALUE_BYTES: usize = 1_000;
    // Replacement chars come from lossy decoding of non-UTF-8 binary runs.
    const MAX_REPLACEMENT_RATIO: f64 = 0.02;
    // Non-printable controls (excluding ordinary whitespace) never appear in a
    // genuine notice but pepper binary scrapes.
    const MAX_CONTROL_RATIO: f64 = 0.02;

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.len() > MAX_PARTY_VALUE_BYTES {
        return true;
    }

    let total = trimmed.chars().count();
    let replacement = trimmed.chars().filter(|&ch| ch == '\u{FFFD}').count();
    if replacement as f64 / total as f64 > MAX_REPLACEMENT_RATIO {
        return true;
    }

    let control = trimmed
        .chars()
        .filter(|ch| ch.is_control() && !ch.is_whitespace())
        .count();
    if control as f64 / total as f64 > MAX_CONTROL_RATIO {
        return true;
    }

    false
}

/// Reject copyright values that are actually font name-table metadata lines wrapped
/// with their field label (e.g. `License Description: ...`). The license/URL text
/// of a font may embed a copyright notice, but the labeled wrapper line is metadata,
/// not a copyright statement, and license detection already owns that text.
fn is_font_metadata_label_copyright(text: &str) -> bool {
    const FONT_METADATA_LABELS: &[&str] = &["License Description:", "License Info URL:"];
    let trimmed = text.trim_start();
    // Compare on bytes so a multi-byte UTF-8 char at the slice boundary cannot panic;
    // the labels are pure ASCII, so a byte-level case-insensitive prefix is exact.
    FONT_METADATA_LABELS.iter().any(|label| {
        trimmed
            .as_bytes()
            .get(..label.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(label.as_bytes()))
    })
}

fn is_binary_string_copyright_candidate(text: &str) -> bool {
    if text
        .chars()
        .any(|ch| ch.is_control() && !ch.is_whitespace())
    {
        return false;
    }

    if contains_year(text) {
        return true;
    }

    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    let tail = if let Some(tail) = lower.strip_prefix("copyright") {
        tail.trim()
    } else {
        lower.trim()
    };
    let original_tail = if lower.starts_with("copyright") {
        trimmed["copyright".len()..].trim()
    } else {
        trimmed
    };

    if tail.is_empty() || !has_sufficient_alphabetic_content(tail) || has_excessive_at_noise(tail) {
        return false;
    }

    if !contains_year(text) && has_suspicious_binary_digit_noise(original_tail) {
        return false;
    }

    let alpha_tokens: Vec<&str> = tail
        .split_whitespace()
        .filter(|token| token.chars().any(|c| c.is_alphabetic()))
        .collect();

    if alpha_tokens.len() <= 1 {
        return has_explicit_copyright_marker(text)
            && alpha_tokens.iter().any(|token| {
                is_company_like_suffix(token.trim_matches(|c: char| !c.is_alphanumeric()))
            });
    }

    if !has_explicit_copyright_marker(text) {
        return false;
    }

    if original_tail.chars().any(|ch| ch.is_ascii_digit()) {
        return has_plausible_digit_bearing_holder(original_tail);
    }

    has_binary_name_like_shape(original_tail)
}

fn extract_binary_string_author_supplements(text_content: &str) -> Vec<AuthorDetection> {
    let mut authors = Vec::new();

    for (line_index, line) in text_content.lines().enumerate() {
        if let Some(author) = extract_named_author_from_binary_line(line) {
            authors.push(AuthorDetection {
                author,
                start_line: LineNumber::from_0_indexed(line_index),
                end_line: LineNumber::from_0_indexed(line_index),
            });
        }
    }

    authors
}

fn has_suspicious_binary_digit_noise(text: &str) -> bool {
    let tokens: Vec<&str> = text
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return false;
    }

    let digit_tokens: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|token| token.chars().any(|ch| ch.is_ascii_digit()))
        .collect();
    if digit_tokens.is_empty() {
        return false;
    }

    let weak_digit_tokens = digit_tokens
        .iter()
        .filter(|token| token.chars().filter(|ch| ch.is_ascii_alphabetic()).count() < 2)
        .count();
    let has_symbolic_token = tokens.iter().any(|token| {
        token.chars().any(|ch| {
            !ch.is_ascii_alphanumeric()
                && !matches!(ch, '.' | '\'' | '&' | '-' | '_' | ',' | '(' | ')')
        })
    });

    (has_symbolic_token && weak_digit_tokens > 0)
        || (digit_tokens.len() >= 2 && weak_digit_tokens == digit_tokens.len())
}

fn has_plausible_digit_bearing_holder(text: &str) -> bool {
    let tokens: Vec<&str> = text
        .split_whitespace()
        .filter(|token| token.chars().any(|ch| ch.is_ascii_alphabetic()))
        .collect();
    if tokens.is_empty() {
        return false;
    }

    let has_company_suffix = tokens
        .iter()
        .any(|token| is_company_like_suffix(token.trim_matches(|ch: char| !ch.is_alphanumeric())));
    let uppercase_like = tokens
        .iter()
        .filter(|token| {
            token
                .chars()
                .find(|ch| ch.is_ascii_alphabetic())
                .is_some_and(|ch| ch.is_ascii_uppercase())
        })
        .count();
    let strong_digit_token = tokens.iter().any(|token| {
        token.chars().any(|ch| ch.is_ascii_digit())
            && token.chars().filter(|ch| ch.is_ascii_alphabetic()).count() >= 2
    });

    strong_digit_token && (has_company_suffix || uppercase_like >= 1)
}

fn extract_patch_header_author_supplements(text_content: &str) -> Vec<AuthorDetection> {
    static PATCH_AUTHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)^(?:from:|patch by|signed-off-by:|co-developed-by:|authored-by:)\s+(?P<author>[^<\n]+<[^>]+>)\s*$",
        )
        .expect("valid patch header author regex")
    });

    text_content
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let captures = PATCH_AUTHOR_RE.captures(line.trim())?;
            let author = captures.name("author")?.as_str().trim();
            let author = refine_author(author)?;
            Some(AuthorDetection {
                author,
                start_line: LineNumber::from_0_indexed(line_index),
                end_line: LineNumber::from_0_indexed(line_index),
            })
        })
        .collect()
}

fn extract_comment_author_supplements(text_content: &str) -> Vec<AuthorDetection> {
    static COMMENT_AUTHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\b(?:written|edited|modified|updated|originally)\s+by\s+(?P<author>[^<\n]+<\s*(?:[^>\s]+@[^>\s]+|https?://[^>\s]+|[^>\n]*\bat\b[^>\n]*)\s*>)\s*\.?$|^(?:[#;/*!\-\s]+)?(?:[^<\n]*?\bby\s+(?P<author2>[^<\n]+<\s*(?:[^>\s]+@[^>\s]+|https?://[^>\s]+|[^>\n]*\bat\b[^>\n]*)\s*>))\s*\.?$|^(?:[#;/*!\-\s]+)?author:\s*(?P<author3>[^<\n]+<\s*(?:[^>\s]+@[^>\s]+|https?://[^>\s]+|[^>\n]*\bat\b[^>\n]*)\s*>)\s*\.?$",
        )
        .expect("valid comment author regex")
    });
    static COMMENT_PAREN_CONTACT_AUTHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\b(?:written|edited|modified|updated|originally)\s+by\s+(?P<name>[^()\n]+?)\s*\(\s*(?P<contact>(?:[^)\s]+@[^)\s]+|https?://[^)\s]+))\s*\)\s*\.?$|^(?:[#;/*!\-\s]+)?(?:[^()\n]*?\bby\s+(?P<name2>[^()\n]+?)\s*\(\s*(?P<contact2>(?:[^)\s]+@[^)\s]+|https?://[^)\s]+))\s*\))\s*\.?$",
        )
        .expect("valid parenthesized contact author regex")
    });
    static DOCKER_MAINTAINER_LABEL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)^label\s+maintainer\s*=\s*[\"']?(?P<author>[^\"'\n]+<[^>]+>)[\"']?\s*$"#)
            .expect("valid docker maintainer label regex")
    });
    static EMAIL_PAREN_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(?P<email>[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,63})\s*\((?P<name>[^)]+)\)")
            .expect("valid email paren name regex")
    });

    let mut authors = Vec::new();

    for (line_index, line) in text_content.lines().enumerate() {
        let trimmed = line.trim();
        let normalized = normalize_comment_author_line(trimmed);
        let line_number = LineNumber::from_0_indexed(line_index);
        let is_comment_like = looks_like_comment_author_source_line(trimmed);

        if is_comment_like
            && let Some(captures) = COMMENT_AUTHOR_RE.captures(&normalized)
            && let Some(author) = captures
                .name("author")
                .or_else(|| captures.name("author2"))
                .or_else(|| captures.name("author3"))
                .map(|m| m.as_str().trim())
            && let Some(author) = refine_author(&normalize_comment_author_candidate(author))
        {
            authors.push(AuthorDetection {
                author,
                start_line: line_number,
                end_line: line_number,
            });
        }

        if is_comment_like
            && let Some(captures) = COMMENT_PAREN_CONTACT_AUTHOR_RE.captures(&normalized)
        {
            let name = captures
                .name("name")
                .or_else(|| captures.name("name2"))
                .map(|m| m.as_str().trim());
            let contact = captures
                .name("contact")
                .or_else(|| captures.name("contact2"))
                .map(|m| m.as_str().trim());

            if let (Some(name), Some(contact)) = (name, contact) {
                let author = normalize_parenthesized_contact_author(name, contact);
                let Some(author) = refine_author(&author) else {
                    continue;
                };
                authors.push(AuthorDetection {
                    author,
                    start_line: line_number,
                    end_line: line_number,
                });
            }
        }

        if let Some(captures) = DOCKER_MAINTAINER_LABEL_RE.captures(trimmed)
            && let Some(author) = captures.name("author").map(|m| m.as_str().trim())
            && let Some(author) = refine_author(author)
        {
            authors.push(AuthorDetection {
                author,
                start_line: line_number,
                end_line: line_number,
            });
        }

        if !is_comment_like {
            continue;
        }

        for captures in EMAIL_PAREN_NAME_RE.captures_iter(trimmed) {
            let Some(email) = captures.name("email").map(|m| m.as_str().trim()) else {
                continue;
            };
            let Some(name) = captures.name("name").map(|m| m.as_str().trim()) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let author = format!("{name} <{email}>");
            let Some(author) = refine_author(&author) else {
                continue;
            };
            authors.push(AuthorDetection {
                author,
                start_line: line_number,
                end_line: line_number,
            });
        }
    }

    authors
}

fn looks_like_comment_author_source_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#')
        || trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with(';')
        || trimmed.starts_with("--")
        || trimmed.starts_with("<!--")
}

fn normalize_comment_author_line(line: &str) -> String {
    line.trim()
        .trim_start_matches("<!--")
        .trim_start()
        .trim_end_matches("*/")
        .trim_end_matches("-->")
        .trim()
        .to_string()
}

fn normalize_comment_author_candidate(author: &str) -> String {
    static ANGLE_URL_AUTHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?P<name>[^<>]+?)\s*<\s*(?P<url>https?://[^>\s]+)\s*>\s*$")
            .expect("valid angle url author regex")
    });

    let trimmed = author.trim().trim_end_matches('.').trim();
    if let Some(captures) = ANGLE_URL_AUTHOR_RE.captures(trimmed) {
        let name = captures
            .name("name")
            .map(|m| m.as_str().trim())
            .unwrap_or(trimmed);
        let url = captures
            .name("url")
            .map(|m| m.as_str().trim_end_matches('/'))
            .unwrap_or(trimmed);
        return format!("{name} ({url})");
    }

    trimmed.to_string()
}

fn normalize_parenthesized_contact_author(name: &str, contact: &str) -> String {
    let normalized_name = name.trim().trim_end_matches('.').trim();
    let normalized_contact = if contact.starts_with("http://") || contact.starts_with("https://") {
        contact.trim_end_matches('/')
    } else {
        contact.trim()
    };
    format!("{normalized_name} ({normalized_contact})")
}

fn has_explicit_copyright_marker(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("(c)") || lower.contains('©') || lower.contains("copr")
}

fn contains_year(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.windows(4).any(|window| {
        window.iter().all(|b| b.is_ascii_digit())
            && matches!(window[0], b'1' | b'2')
            && matches!(window[1], b'9' | b'0')
    })
}

#[cfg(test)]
#[path = "copyright_test.rs"]
mod tests;
