// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Text extraction for downstream license/copyright detection: chooses a
//! strategy per input (RTF, PDF, image metadata, font metadata, decoded text,
//! or bounded binary-string scraping) and augments markdown/HTML license hints.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::Path;

use object::FileKind;

use crate::parsers::windows_executable::extract_windows_executable_metadata_text;
use crate::utils::font::extract_font_metadata_text;
use crate::utils::language::detect_language;

use super::encoding::{decode_bytes_to_string, looks_like_decoded_text, looks_like_textual_bytes};
use super::format_sniff::{
    detect_file_format, is_supported_image_container, is_textual_format, is_zip_archive,
    looks_like_bzip2, looks_like_deb, looks_like_gzip, looks_like_pdf, looks_like_rpm,
    looks_like_rtf, looks_like_squashfs, looks_like_xz, media_mime_from_content,
    supported_image_metadata_format,
};
use super::image_metadata::extract_image_metadata_text;
use super::path::{PLAIN_TEXT_EXTENSIONS, lower_extension};
use super::pdf::extract_pdf_text;

pub(super) const LARGE_OPAQUE_BINARY_SKIP_BYTES: usize = 512 * 1024;
const LARGE_MACHO_LEGAL_WINDOW_BYTES: usize = 64 * 1024;
const LARGE_MACHO_LEGAL_MAX_WINDOWS: usize = 24;
const LARGE_MACHO_LEGAL_MAX_WINDOWS_PER_MARKER: usize = 4;
const LARGE_MACHO_LEGAL_MAX_EXTRACT_BYTES: usize = 2 * 1024 * 1024;
const LARGE_MACHO_LEGAL_MARKERS: &[&[u8]] = &[
    b"Unicode, Inc.",
    b"http://www.unicode.org/copyright.html",
    b"https://www.unicode.org/copyright.html",
    b"SPDX-License-Identifier:",
    b"Licensed under",
    b"licensed under",
    b"Apache License",
    b"http://www.apache.org/licenses/",
    b"https://www.apache.org/licenses/",
    b"Permission is hereby granted",
    b"permission is hereby granted",
    b"Redistribution and use in source and binary forms",
    b"redistribution and use in source and binary forms",
    b"Permission to use, copy, modify, and/or distribute this software",
    b"The MIT License",
    b"GNU GENERAL PUBLIC LICENSE",
    b"GNU LESSER GENERAL PUBLIC LICENSE",
    b"Mozilla Public License",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractedTextKind {
    None,
    Decoded,
    FontMetadata,
    Pdf,
    BinaryStrings,
    ImageMetadata,
    WindowsExecutableMetadata,
}

pub fn extract_text_for_detection(path: &Path, bytes: &[u8]) -> (String, ExtractedTextKind) {
    let (text, kind, _) = extract_text_for_detection_with_diagnostics(path, bytes);
    (text, kind)
}

pub(crate) fn augment_license_detection_text<'a>(path: &Path, text: &'a str) -> Cow<'a, str> {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return Cow::Borrowed(text);
    };
    if !matches!(
        extension.to_ascii_lowercase().as_str(),
        "md" | "markdown" | "html" | "htm"
    ) {
        return Cow::Borrowed(text);
    }

    let mut hints = Vec::new();
    let has_dual_license_notice = has_dual_license_notice_text(text);
    if text.contains("CC BY 4.0") || text.contains("creativecommons.org/licenses/by/4.0") {
        hints.push("Creative Commons Attribution 4.0 International License".to_string());
    }
    if !has_dual_license_notice
        && (text.contains("Apache License (Version 2.0)")
            || text.contains("Apache License, Version 2.0"))
    {
        hints.push(
            "Licensed under the Apache License, Version 2.0. http://www.apache.org/licenses/LICENSE-2.0"
                .to_string(),
        );
    }

    if !has_dual_license_notice {
        hints.extend(extract_shields_license_badge_hints(text));
    }

    if hints.is_empty() {
        Cow::Borrowed(text)
    } else {
        let mut augmented =
            String::with_capacity(text.len() + hints.iter().map(String::len).sum::<usize>() + 8);
        augmented.push_str(text);
        augmented.push_str("\n\n");
        for (index, hint) in hints.into_iter().enumerate() {
            if index > 0 {
                augmented.push('\n');
            }
            augmented.push_str(&hint);
        }
        Cow::Owned(augmented)
    }
}

fn extract_shields_license_badge_hints(text: &str) -> Vec<String> {
    let mut hints = Vec::new();
    let mut rest = text;
    let needle = "img.shields.io/badge/license-";

    while let Some(index) = rest.find(needle) {
        let start = index + needle.len();
        let suffix = &rest[start..];
        let end = suffix
            .find([')', ']', '"', '\'', ' ', '\n'])
            .unwrap_or(suffix.len());
        let badge = &suffix[..end];
        let Some(badge) = badge.strip_suffix(".svg") else {
            rest = &suffix[end..];
            continue;
        };

        let mut segments: Vec<_> = badge
            .split('-')
            .filter(|segment| !segment.is_empty())
            .collect();
        if segments.len() < 2 {
            rest = &suffix[end..];
            continue;
        }
        segments.pop();
        let candidate = segments.join("-").replace("%20", " ").replace('_', "-");
        if !candidate.is_empty() {
            hints.push(canonical_shields_license_hint(&candidate));
        }

        rest = &suffix[end..];
    }

    hints.sort();
    hints.dedup();
    hints
}

fn has_dual_license_notice_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (lower.contains("licensed under either of") && lower.contains("at your option"))
        || lower.contains("dual-licensed under")
        || lower.contains("dual licensed under")
}

fn canonical_shields_license_hint(candidate: &str) -> String {
    match candidate.trim() {
        "MIT" => "The MIT License".to_string(),
        "Apache-2.0" | "Apache 2.0" => "Apache License 2.0".to_string(),
        other => format!("{other} License"),
    }
}

pub(crate) fn extract_text_for_detection_with_diagnostics(
    path: &Path,
    bytes: &[u8],
) -> (String, ExtractedTextKind, Option<String>) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    let detected_format = detect_file_format(bytes);

    if looks_like_rtf(bytes, ext.as_deref()) {
        let text = extract_rtf_text(bytes);
        return if text.trim().is_empty() {
            (String::new(), ExtractedTextKind::None, None)
        } else {
            (text, ExtractedTextKind::Decoded, None)
        };
    }

    if looks_like_pdf(bytes) || detected_format.short_name() == Some("PDF") {
        let (text, scan_error) = extract_pdf_text(path, bytes);
        return if text.is_empty() {
            (String::new(), ExtractedTextKind::None, scan_error)
        } else {
            (text, ExtractedTextKind::Pdf, None)
        };
    }

    if let Some(format) = supported_image_metadata_format(ext.as_deref(), detected_format) {
        let text = extract_image_metadata_text(bytes, format);
        return if text.is_empty() {
            if is_supported_image_container(bytes, format) {
                (String::new(), ExtractedTextKind::None, None)
            } else {
                let decoded = decode_bytes_to_string(bytes);
                if decoded.is_empty() {
                    (String::new(), ExtractedTextKind::None, None)
                } else {
                    (decoded, ExtractedTextKind::Decoded, None)
                }
            }
        } else {
            (text, ExtractedTextKind::ImageMetadata, None)
        };
    }

    if let Some(text) = extract_font_metadata_text(path, bytes) {
        let strings = extract_printable_strings(bytes);
        let combined = if strings.is_empty() {
            text
        } else {
            combine_extracted_text_fragments(Some(text), strings)
        };
        return (combined, ExtractedTextKind::FontMetadata, None);
    }

    let windows_executable_metadata_text = extract_windows_executable_metadata_text(bytes);
    let large_opaque_binary = windows_executable_metadata_text.is_none()
        && is_large_opaque_binary_candidate(bytes, detected_format);
    let bounded_macho_legal_text = if large_opaque_binary {
        extract_bounded_macho_legal_strings(bytes)
    } else {
        String::new()
    };
    let skip_large_opaque_binary_text =
        should_skip_large_opaque_binary_text_extraction(path, bytes, detected_format);

    if skip_large_opaque_binary_text {
        if !bounded_macho_legal_text.is_empty() {
            return (
                combine_extracted_text_fragments(
                    windows_executable_metadata_text,
                    bounded_macho_legal_text,
                ),
                ExtractedTextKind::BinaryStrings,
                None,
            );
        }
        return windows_metadata_or_empty_result(windows_executable_metadata_text);
    }

    if should_skip_binary_string_extraction(path, bytes, detected_format) {
        return (String::new(), ExtractedTextKind::None, None);
    }

    let is_svg_text = lower_extension(path).as_deref() == Some("svg")
        || detected_format.media_type() == "image/svg+xml";
    let should_try_decoded_text = looks_like_textual_bytes(bytes) || is_svg_text;
    let decoded_is_utf8 = std::str::from_utf8(bytes).is_ok();
    let path_suggests_text = ext.as_deref().is_some_and(|extension| {
        PLAIN_TEXT_EXTENSIONS.contains(&extension) || detect_language(path, bytes).is_some()
    });

    if !large_opaque_binary && should_try_decoded_text {
        let decoded = decode_bytes_to_string(bytes);
        if !decoded.is_empty()
            && (is_svg_text
                || decoded_is_utf8
                || path_suggests_text
                || looks_like_decoded_text(&decoded))
        {
            let combined =
                combine_extracted_text_fragments(windows_executable_metadata_text, decoded);
            return (combined, ExtractedTextKind::Decoded, None);
        }
    }

    let text = if large_opaque_binary {
        let sampled_text = extract_sampled_printable_strings(bytes);
        if bounded_macho_legal_text.is_empty() {
            sampled_text
        } else {
            combine_extracted_text_fragments(Some(sampled_text), bounded_macho_legal_text)
        }
    } else {
        extract_printable_strings(bytes)
    };
    if text.is_empty() {
        windows_metadata_or_empty_result(windows_executable_metadata_text)
    } else {
        (
            combine_extracted_text_fragments(windows_executable_metadata_text, text),
            ExtractedTextKind::BinaryStrings,
            None,
        )
    }
}

fn combine_extracted_text_fragments(prefix: Option<String>, suffix: String) -> String {
    match prefix {
        Some(prefix) if !prefix.is_empty() && !suffix.is_empty() => format!("{prefix}\n{suffix}"),
        Some(prefix) if !prefix.is_empty() => prefix,
        _ => suffix,
    }
}

pub(super) fn windows_metadata_or_empty_result(
    windows_executable_metadata_text: Option<String>,
) -> (String, ExtractedTextKind, Option<String>) {
    if let Some(metadata_text) = windows_executable_metadata_text {
        (
            metadata_text,
            ExtractedTextKind::WindowsExecutableMetadata,
            None,
        )
    } else {
        (String::new(), ExtractedTextKind::None, None)
    }
}

fn extract_rtf_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        match chars[index] {
            '{' | '}' => {
                index += 1;
            }
            '\\' => {
                index += 1;
                if index >= chars.len() {
                    break;
                }

                match chars[index] {
                    '\\' | '{' | '}' => {
                        output.push(chars[index]);
                        index += 1;
                    }
                    '\'' => {
                        if index + 2 < chars.len() {
                            let hex = [chars[index + 1], chars[index + 2]];
                            let hex: String = hex.iter().collect();
                            if let Ok(value) = u8::from_str_radix(&hex, 16) {
                                output.push(value as char);
                                index += 3;
                                continue;
                            }
                        }
                        index += 1;
                    }
                    control if control.is_ascii_alphabetic() => {
                        let start = index;
                        while index < chars.len() && chars[index].is_ascii_alphabetic() {
                            index += 1;
                        }
                        let control_word: String = chars[start..index].iter().collect();

                        let number_start = index;
                        if index < chars.len()
                            && (chars[index] == '-' || chars[index].is_ascii_digit())
                        {
                            index += 1;
                            while index < chars.len() && chars[index].is_ascii_digit() {
                                index += 1;
                            }
                        }
                        let parameter: String = chars[number_start..index].iter().collect();

                        if index < chars.len() && chars[index] == ' ' {
                            index += 1;
                        }

                        match control_word.as_str() {
                            "par" | "line" => output.push('\n'),
                            "tab" => output.push('\t'),
                            "emdash" => output.push('—'),
                            "endash" => output.push('–'),
                            "bullet" => output.push('•'),
                            "lquote" | "rquote" => output.push('\''),
                            "ldblquote" | "rdblquote" => output.push('"'),
                            "u" => {
                                if let Ok(codepoint) = parameter.parse::<i32>() {
                                    let normalized = if codepoint < 0 {
                                        codepoint + 65_536
                                    } else {
                                        codepoint
                                    };
                                    if let Ok(normalized) = u32::try_from(normalized)
                                        && let Some(ch) = char::from_u32(normalized)
                                    {
                                        output.push(ch);
                                    }
                                }

                                if index < chars.len()
                                    && !matches!(chars[index], '\\' | '{' | '}' | '\n' | '\r')
                                {
                                    index += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        index += 1;
                    }
                }
            }
            ch => {
                output.push(ch);
                index += 1;
            }
        }
    }

    output
        .replace(['\r', '\u{0c}'], "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn should_skip_binary_string_extraction(
    path: &Path,
    bytes: &[u8],
    detected_format: file_format::FileFormat,
) -> bool {
    use file_format::Kind as FileFormatKind;
    matches!(lower_extension(path).as_deref(), Some("pdf"))
        || supported_image_metadata_format(lower_extension(path).as_deref(), detected_format)
            .is_some()
        || (matches!(
            detected_format.kind(),
            FileFormatKind::Audio | FileFormatKind::Image | FileFormatKind::Video
        ) && !is_textual_format(detected_format))
        || media_mime_from_content(bytes).is_some()
        || is_zip_archive(bytes)
        || looks_like_gzip(bytes)
        || looks_like_bzip2(bytes)
        || looks_like_xz(bytes)
        || looks_like_deb(bytes, path)
        || looks_like_rpm(bytes, path)
        || looks_like_squashfs(bytes, path)
}

fn should_skip_large_opaque_binary_text_extraction(
    _path: &Path,
    bytes: &[u8],
    detected_format: file_format::FileFormat,
) -> bool {
    is_large_opaque_binary_candidate(bytes, detected_format)
        && !sample_has_promising_printable_strings(bytes)
}

fn is_large_opaque_binary_candidate(
    bytes: &[u8],
    detected_format: file_format::FileFormat,
) -> bool {
    use file_format::Kind as FileFormatKind;
    bytes.len() >= LARGE_OPAQUE_BINARY_SKIP_BYTES
        && !is_textual_format(detected_format)
        && !matches!(
            detected_format.kind(),
            FileFormatKind::Archive
                | FileFormatKind::Compressed
                | FileFormatKind::Package
                | FileFormatKind::Audio
                | FileFormatKind::Image
                | FileFormatKind::Video
        )
}

fn sampled_printable_window_ranges(len: usize) -> Vec<(usize, usize)> {
    const SAMPLE_WINDOW_BYTES: usize = 64 * 1024;

    let mut ranges = Vec::new();
    let mut push_range = |start: usize, end: usize| {
        if start < end && !ranges.contains(&(start, end)) {
            ranges.push((start, end));
        }
    };

    push_range(0, len.min(SAMPLE_WINDOW_BYTES));
    if len > SAMPLE_WINDOW_BYTES * 2 {
        let mid_start = len / 2 - SAMPLE_WINDOW_BYTES / 2;
        let mid_end = (mid_start + SAMPLE_WINDOW_BYTES).min(len);
        push_range(mid_start, mid_end);
    }
    if len > SAMPLE_WINDOW_BYTES {
        push_range(len - SAMPLE_WINDOW_BYTES, len);
    }

    ranges
}

fn extract_bounded_macho_legal_strings(bytes: &[u8]) -> String {
    if !matches!(
        FileKind::parse(bytes),
        Ok(FileKind::MachO32 | FileKind::MachO64 | FileKind::MachOFat32 | FileKind::MachOFat64)
    ) {
        return String::new();
    }

    let mut ranges = Vec::new();
    for marker in LARGE_MACHO_LEGAL_MARKERS {
        collect_marker_window_ranges(bytes, marker, &mut ranges);
        if ranges.len() >= LARGE_MACHO_LEGAL_MAX_WINDOWS {
            break;
        }
    }

    if ranges.is_empty() {
        return String::new();
    }

    let mut merged_ranges = merge_overlapping_ranges(ranges);
    let mut combined_lines = BTreeSet::new();
    let mut extracted_bytes = 0usize;

    for (start, end) in merged_ranges.drain(..) {
        if extracted_bytes >= LARGE_MACHO_LEGAL_MAX_EXTRACT_BYTES {
            break;
        }
        let remaining = LARGE_MACHO_LEGAL_MAX_EXTRACT_BYTES - extracted_bytes;
        let end = start.saturating_add((end - start).min(remaining));
        let window_text = extract_printable_strings(&bytes[start..end]);
        for line in window_text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            combined_lines.insert(line.to_string());
        }
        extracted_bytes += end - start;
    }

    combined_lines.into_iter().collect::<Vec<_>>().join("\n")
}

fn collect_marker_window_ranges(bytes: &[u8], marker: &[u8], ranges: &mut Vec<(usize, usize)>) {
    if marker.is_empty() || ranges.len() >= LARGE_MACHO_LEGAL_MAX_WINDOWS {
        return;
    }

    let mut search_start = 0usize;
    let mut hits_for_marker = 0usize;

    while search_start + marker.len() <= bytes.len()
        && ranges.len() < LARGE_MACHO_LEGAL_MAX_WINDOWS
        && hits_for_marker < LARGE_MACHO_LEGAL_MAX_WINDOWS_PER_MARKER
    {
        let Some(relative_match) = bytes[search_start..].iter().position(|&b| b == marker[0])
        else {
            break;
        };
        let match_start = search_start + relative_match;
        let match_end = match_start + marker.len();
        if match_end <= bytes.len() && &bytes[match_start..match_end] == marker {
            let half_window = LARGE_MACHO_LEGAL_WINDOW_BYTES / 2;
            let window_start = match_start.saturating_sub(half_window);
            let window_end = (match_end + half_window).min(bytes.len());
            ranges.push((window_start, window_end));
            hits_for_marker += 1;
            search_start = match_end;
        } else {
            search_start = match_start + 1;
        }
    }
}

fn merge_overlapping_ranges(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return ranges;
    }

    ranges.sort_unstable_by_key(|&(start, end)| (start, end));

    let mut merged = Vec::with_capacity(ranges.len());
    let mut current = ranges[0];
    for (start, end) in ranges.into_iter().skip(1) {
        if start <= current.1 {
            current.1 = current.1.max(end);
        } else {
            merged.push(current);
            current = (start, end);
        }
    }
    merged.push(current);

    merged
}

fn sample_has_promising_printable_strings(bytes: &[u8]) -> bool {
    let mut structured_signal_seen = false;
    let promising_license_windows = sampled_printable_window_ranges(bytes.len())
        .into_iter()
        .filter(|&(start, end)| {
            let window = &bytes[start..end];
            if has_strong_structured_text_signal(window) {
                structured_signal_seen = true;
            }
            has_license_or_notice_signal(window)
        })
        .count();

    structured_signal_seen || promising_license_windows >= 2
}

fn extract_sampled_printable_strings(bytes: &[u8]) -> String {
    let mut combined_lines = BTreeSet::new();

    for (start, end) in sampled_printable_window_ranges(bytes.len()) {
        let window_text = extract_printable_strings(&bytes[start..end]);
        for line in window_text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            combined_lines.insert(line.to_string());
        }
    }

    combined_lines.into_iter().collect::<Vec<_>>().join("\n")
}

fn has_license_or_notice_signal(bytes: &[u8]) -> bool {
    let strings = extract_printable_strings(bytes);
    if strings.is_empty() {
        return false;
    }

    let lower = strings.to_ascii_lowercase();
    [
        "copyright",
        "license",
        "licensed under",
        "all rights reserved",
        "permission is hereby granted",
        "redistribution and use",
        "spdx-license-identifier",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn has_strong_structured_text_signal(bytes: &[u8]) -> bool {
    let strings = extract_printable_strings(bytes);
    if strings.is_empty() {
        return false;
    }

    let email_markers = strings.matches('@').count();
    let url_markers = strings.matches("http://").count() + strings.matches("https://").count();

    email_markers + url_markers >= 3
}

pub fn extract_printable_strings(bytes: &[u8]) -> String {
    const MIN_LEN: usize = 4;
    const MIN_OUTPUT_BYTES: usize = 2_000_000;
    const MAX_OUTPUT_BYTES_CAP: usize = 16_000_000;

    let max_output_bytes = bytes.len().clamp(MIN_OUTPUT_BYTES, MAX_OUTPUT_BYTES_CAP);

    fn is_printable_ascii(b: u8) -> bool {
        matches!(b, 0x20..=0x7E)
    }

    let mut out = String::new();
    let mut run: Vec<u8> = Vec::new();

    let flush_run = |out: &mut String, run: &mut Vec<u8>| {
        if run.len() >= MIN_LEN {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&String::from_utf8_lossy(run));
        }
        run.clear();
    };

    for &b in bytes {
        if is_printable_ascii(b) {
            run.push(b);
        } else {
            flush_run(&mut out, &mut run);
            if out.len() >= max_output_bytes {
                return out;
            }
        }
    }
    flush_run(&mut out, &mut run);
    if out.len() >= max_output_bytes {
        return out;
    }

    for start in 0..=1 {
        run.clear();
        let mut i = start;
        while i + 1 < bytes.len() {
            let b0 = bytes[i];
            let b1 = bytes[i + 1];
            let (ch, zero) = if start == 0 { (b0, b1) } else { (b1, b0) };
            if is_printable_ascii(ch) && zero == 0 {
                run.push(ch);
            } else {
                flush_run(&mut out, &mut run);
                if out.len() >= max_output_bytes {
                    return out;
                }
            }
            i += 2;
        }
        flush_run(&mut out, &mut run);
        if out.len() >= max_output_bytes {
            return out;
        }
    }

    out
}
