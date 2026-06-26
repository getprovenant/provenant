// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;
use std::path::Path;

use allsorts::binary::read::ReadScope;
use allsorts::font_data::FontData;
use allsorts::tables::{FontTableProvider, NameTable, OpenTypeData};
use ttf_parser::{Face, Permissions, fonts_in_collection, name_id};

use crate::parsers::metadata::ParserMetadata;

pub(crate) const SUPPORTED_FONT_EXTENSIONS: &[&str] =
    &["ttf", "otf", "woff", "woff2", "eot", "ttc", "otc"];
pub(crate) const SUPPORTED_FONT_FILE_GLOBS: &[&str] = &[
    "**/*.ttf",
    "**/*.otf",
    "**/*.woff",
    "**/*.woff2",
    "**/*.eot",
    "**/*.ttc",
    "**/*.otc",
];
const OFL_URL_CANONICALIZATIONS: &[(&str, &str)] = &[
    ("https://scripts.sil.org/OFL/", "http://scripts.sil.org/OFL"),
    ("https://scripts.sil.org/OFL", "http://scripts.sil.org/OFL"),
    ("https://openfontlicense.org/", "http://scripts.sil.org/OFL"),
    ("https://openfontlicense.org", "http://scripts.sil.org/OFL"),
];
const ALLSORTS_NAME_TABLE_TAG: u32 = u32::from_be_bytes(*b"name");

pub(crate) static FONT_METADATA: &[ParserMetadata] = &[ParserMetadata {
    description: "Embedded font legal metadata (native fonts, webfonts, and collections)",
    file_patterns: SUPPORTED_FONT_FILE_GLOBS,
    package_type: "",
    primary_language: "",
    documentation_url: Some("https://learn.microsoft.com/en-us/typography/opentype/spec/name"),
}];

pub(crate) fn is_supported_font_extension(extension: &str) -> bool {
    SUPPORTED_FONT_EXTENSIONS
        .iter()
        .any(|supported| supported.eq_ignore_ascii_case(extension))
}

pub(crate) fn is_supported_font_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(is_supported_font_extension)
}

pub(crate) fn extract_font_metadata_text(path: &Path, bytes: &[u8]) -> Option<String> {
    let extension = path.extension().and_then(|ext| ext.to_str())?;
    let extension = extension.to_ascii_lowercase();
    if !is_supported_font_extension(&extension) {
        return None;
    }

    match extension.as_str() {
        "ttf" | "otf" | "woff" | "woff2" | "ttc" | "otc" => extract_sfnt_font_metadata_text(
            bytes,
            matches!(extension.as_str(), "ttf" | "otf" | "ttc" | "otc"),
        ),
        "eot" => extract_eot_metadata_text(bytes),
        _ => None,
    }
}

fn extract_sfnt_font_metadata_text(bytes: &[u8], include_permissions: bool) -> Option<String> {
    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();

    for line in extract_allsorts_name_table_lines(bytes) {
        if seen.insert(line.clone()) {
            lines.push(line);
        }
    }

    if include_permissions {
        let face_count = fonts_in_collection(bytes).unwrap_or(1);
        for face_index in 0..face_count {
            let Some(permissions) = Face::parse(bytes, face_index).ok()?.permissions() else {
                continue;
            };
            let line = format!(
                "Embedding permissions: {}",
                font_permission_label(permissions)
            );
            if seen.insert(line.clone()) {
                lines.push(line);
            }
        }
    }

    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// Decode every `name` table record into its own newline-separated line.
///
/// Each record is decoded from its own `offset`/`length` slice of the name
/// table's string storage, so adjacent records never run together. This is the
/// safe alternative to scraping raw printable strings from the font binary,
/// where packed UTF-16 name-table storage glues consecutive records (e.g.
/// designer, vendor URL, description) into run-on tokens such as
/// `bulenkovhttps://www.jetbrains.comThis`, corrupting downstream URL and
/// copyright extraction.
pub(crate) fn extract_font_name_table_strings(bytes: &[u8]) -> String {
    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();
    for line in extract_allsorts_all_name_strings(bytes) {
        let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            lines.push(normalized);
        }
    }
    lines.join("\n")
}

/// Read each face's `name` table from an SFNT/WOFF/collection and hand it to
/// `visit`. Centralizes the `ReadScope` → `FontData` → face loop → provider →
/// `NameTable` setup shared by the name-table extractors so the reading logic
/// (tag constant, error handling, allsorts API) lives in one place.
fn for_each_name_table(bytes: &[u8], mut visit: impl FnMut(&NameTable<'_>)) {
    let Some(font_data) = ReadScope::new(bytes).read::<FontData<'_>>().ok() else {
        return;
    };

    for face_index in 0..allsorts_face_count(&font_data) {
        let Ok(provider) = font_data.table_provider(face_index) else {
            continue;
        };
        let Ok(name_table_data) = provider.read_table_data(ALLSORTS_NAME_TABLE_TAG) else {
            continue;
        };
        let Ok(name_table) = ReadScope::new(name_table_data.as_ref()).read::<NameTable<'_>>()
        else {
            continue;
        };
        visit(&name_table);
    }
}

fn extract_allsorts_all_name_strings(bytes: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    for_each_name_table(bytes, |name_table| {
        // Decode each distinct name id via `string_for_id`, which reads each
        // record from its own offset/length rather than concatenating storage.
        let mut name_ids = BTreeSet::new();
        for record in name_table.name_records.iter() {
            name_ids.insert(record.name_id);
        }
        for name_id in name_ids {
            if let Some(value) = name_table.string_for_id(name_id) {
                strings.push(value);
            }
        }
    });
    strings
}

fn extract_allsorts_name_table_lines(bytes: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();
    for_each_name_table(bytes, |name_table| {
        for (source_name_id, target_name_id) in [
            (NameTable::COPYRIGHT_NOTICE, name_id::COPYRIGHT_NOTICE),
            (NameTable::LICENSE_DESCRIPTION, name_id::LICENSE),
            (NameTable::LICENSE_INFO_URL, name_id::LICENSE_URL),
        ] {
            let Some(value) = name_table.string_for_id(source_name_id) else {
                continue;
            };
            let Some(line) = build_font_metadata_line(target_name_id, value) else {
                continue;
            };
            if seen.insert(line.clone()) {
                lines.push(line);
            }
        }
    });
    lines
}

fn allsorts_face_count(font_data: &FontData<'_>) -> usize {
    match font_data {
        FontData::OpenType(font) => match &font.data {
            OpenTypeData::Single(_) => 1,
            OpenTypeData::Collection(ttc) => ttc.offset_tables.len(),
        },
        FontData::Woff(_) => 1,
        FontData::Woff2(font) => font
            .collection_directory
            .as_ref()
            .map(|directory| directory.fonts().count())
            .unwrap_or(1),
    }
}

fn extract_eot_metadata_text(bytes: &[u8]) -> Option<String> {
    let text = extract_eot_utf16le_marker_text(bytes).join("\n");
    if text.is_empty() {
        return None;
    }

    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();
    for segment in split_eot_legal_metadata_segments(&text) {
        let normalized = normalize_eot_metadata_segment(&segment);
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            lines.push(normalized);
        }
    }

    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn extract_eot_utf16le_marker_text(bytes: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();
    for marker in [
        "Copyright",
        "This Font Software is licensed under",
        "http://",
        "https://",
    ] {
        let encoded = marker.encode_utf16().collect::<Vec<_>>();
        let marker_bytes = encoded
            .iter()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<_>>();
        let mut search_start = 0;
        while let Some(relative_start) = bytes[search_start..]
            .windows(marker_bytes.len())
            .position(|window| window == marker_bytes.as_slice())
        {
            let start = search_start + relative_start;
            let decoded = decode_utf16le_ascii_from_offset(bytes, start);
            if !decoded.is_empty() && seen.insert(decoded.clone()) {
                lines.push(decoded);
            }
            search_start = start + marker_bytes.len();
        }
    }
    lines
}

fn decode_utf16le_ascii_from_offset(bytes: &[u8], start: usize) -> String {
    let mut decoded = Vec::new();
    let mut index = start;
    while index + 1 < bytes.len() {
        let lo = bytes[index];
        let hi = bytes[index + 1];
        if hi == 0 && (0x20..=0x7E).contains(&lo) {
            decoded.push(lo);
            index += 2;
            continue;
        }
        break;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn split_eot_legal_metadata_segments(text: &str) -> Vec<String> {
    let mut segments = Vec::new();

    if let Some(segment) = extract_text_between_markers(
        text,
        "Copyright",
        &["All Rights Reserved.", "All rights reserved."],
    ) {
        segments.push(segment);
    }
    if let Some(segment) = extract_text_between_markers(
        text,
        "This Font Software is licensed under",
        &[
            "governing your use of this Font Software.",
            "This Font Software.",
        ],
    ) {
        segments.push(segment);
    }
    segments.extend(extract_http_segments(text));

    segments
}

fn extract_text_between_markers(
    text: &str,
    start_marker: &str,
    end_markers: &[&str],
) -> Option<String> {
    let start = text.find(start_marker)?;
    let tail = &text[start..];
    let end = end_markers
        .iter()
        .filter_map(|marker| tail.find(marker).map(|idx| idx + marker.len()))
        .min()
        .unwrap_or(tail.len());
    Some(tail[..end].to_string())
}

fn extract_http_segments(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    for marker in ["http://", "https://"] {
        let mut search_start = 0;
        while let Some(relative_start) = text[search_start..].find(marker) {
            let start = search_start + relative_start;
            let tail = &text[start + marker.len()..];
            let mut end = text.len();
            for boundary in [
                "http://",
                "https://",
                "This Font Software",
                "Copyright",
                "Version ",
            ] {
                if let Some(relative_end) = tail.find(boundary) {
                    end = end.min(start + marker.len() + relative_end);
                }
            }
            if let Some(relative_end) = tail.find(char::is_whitespace) {
                end = end.min(start + marker.len() + relative_end);
            }

            let segment = text[start..end]
                .trim_end_matches(&['.', ',', ';', ':'][..])
                .to_string();
            if !segment.is_empty() {
                segments.push(segment);
            }
            search_start = end.max(start + marker.len());
        }
    }
    segments
}

fn normalize_eot_metadata_segment(segment: &str) -> String {
    let normalized = segment
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    if normalized.is_empty() {
        return normalized;
    }

    let lowered = normalized.to_ascii_lowercase();
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        return canonicalize_ofl_license_reference_urls(normalized);
    }

    if lowered.contains("font software") || lowered.contains("open font license") {
        return canonicalize_ofl_license_reference_urls(normalized);
    }

    normalized
}

fn build_font_metadata_line(name_id_value: u16, value: String) -> Option<String> {
    let value = normalize_font_value(name_id_value, value);
    if value.is_empty() {
        return None;
    }

    if name_id_value == name_id::COPYRIGHT_NOTICE {
        return Some(value);
    }

    let label = font_name_label(name_id_value)?;
    Some(format!("{label}: {value}"))
}

fn font_name_label(name_id_value: u16) -> Option<&'static str> {
    match name_id_value {
        name_id::LICENSE => Some("License Description"),
        name_id::LICENSE_URL => Some("License Info URL"),
        _ => None,
    }
}

fn normalize_font_value(name_id_value: u16, value: String) -> String {
    let normalized = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    match name_id_value {
        name_id::COPYRIGHT_NOTICE => strip_reserved_font_name_clause(normalized),
        name_id::LICENSE | name_id::LICENSE_URL => {
            canonicalize_ofl_license_reference_urls(normalized)
        }
        _ => normalized,
    }
}

fn strip_reserved_font_name_clause(value: String) -> String {
    let lower = value.to_ascii_lowercase();
    for marker in [
        ", with reserved font name",
        ", with no reserved font name",
        " with reserved font name",
        " with no reserved font name",
    ] {
        if let Some(index) = lower.find(marker) {
            return value[..index]
                .trim_end_matches(&[',', ';', ':', ' ', '('][..])
                .trim()
                .to_string();
        }
    }

    value
}

fn canonicalize_ofl_license_reference_urls(mut value: String) -> String {
    for (from, to) in OFL_URL_CANONICALIZATIONS {
        value = value.replace(from, to);
    }
    value
}

fn font_permission_label(permission: Permissions) -> &'static str {
    match permission {
        Permissions::Installable => "Installable",
        Permissions::Restricted => "Restricted",
        Permissions::PreviewAndPrint => "Preview and Print",
        Permissions::Editable => "Editable",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::copyright::detect_copyrights;
    use crate::license_detection::LicenseDetectionEngine;
    use ttf_parser::name_id;

    use crate::finder::{DetectionConfig, find_urls};

    use super::{
        build_font_metadata_line, canonicalize_ofl_license_reference_urls,
        extract_font_metadata_text, extract_font_name_table_strings,
    };

    #[test]
    fn extracts_ofl_metadata_from_lato_font_fixture() {
        let bytes =
            fs::read("testdata/font-fixtures/Lato-Bold.ttf").expect("read lato font fixture");

        let text = extract_font_metadata_text(Path::new("Lato-Bold.ttf"), &bytes)
            .expect("font metadata text");

        assert!(text.contains("License Description:"), "{text}");
        assert!(
            text.contains("Open Font License") || text.contains("OFL"),
            "{text}"
        );
    }

    #[test]
    fn extracts_apache_metadata_from_underline_test_font_fixture() {
        let bytes = fs::read("testdata/font-fixtures/UnderlineTest-Close.ttf")
            .expect("read apache font fixture");

        let text = extract_font_metadata_text(Path::new("UnderlineTest-Close.ttf"), &bytes)
            .expect("font metadata text");

        assert!(
            text.contains("License Description:") || text.contains("Copyright"),
            "{text}"
        );
        assert!(
            text.contains("Apache") || text.contains("http://www.apache.org/licenses"),
            "{text}"
        );
    }

    #[test]
    fn canonicalizes_ofl_url_variants_in_font_license_metadata() {
        let canonical = canonicalize_ofl_license_reference_urls(
            "This license is available with a FAQ at: https://openfontlicense.org/".to_string(),
        );

        assert_eq!(
            canonical,
            "This license is available with a FAQ at: http://scripts.sil.org/OFL"
        );
    }

    #[test]
    fn font_metadata_lines_detect_noto_ofl_text_without_trademark_noise() {
        let metadata_text = [
            build_font_metadata_line(
                name_id::COPYRIGHT_NOTICE,
                "Copyright 2022 The Noto Project Authors (https://github.com/notofonts/latin-greek-cyrillic)".to_string(),
            ),
            build_font_metadata_line(
                name_id::TRADEMARK,
                "Noto is a trademark of Google LLC.".to_string(),
            ),
            build_font_metadata_line(
                name_id::LICENSE,
                "This Font Software is licensed under the SIL Open Font License, Version 1.1. This license is available with a FAQ at: https://scripts.sil.org/OFL".to_string(),
            ),
            build_font_metadata_line(
                name_id::LICENSE_URL,
                "https://scripts.sil.org/OFL".to_string(),
            ),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");

        assert!(!metadata_text.contains("Trademark:"), "{metadata_text}");
        assert!(
            metadata_text.contains("Copyright 2022 The Noto Project Authors"),
            "{metadata_text}"
        );
        assert!(
            metadata_text.contains("http://scripts.sil.org/OFL"),
            "{metadata_text}"
        );

        let engine = LicenseDetectionEngine::from_embedded().expect("initialize license engine");
        let detections = engine
            .detect_with_kind_and_source_with_score(&metadata_text, false, false, "font.ttf", 0.0)
            .expect("detect licenses from font metadata text");

        assert!(
            detections.iter().any(|detection| {
                detection
                    .license_expression_spdx
                    .as_deref()
                    .is_some_and(|expression| expression.contains("OFL-1.1"))
            }),
            "detections: {detections:#?}"
        );

        let (copyrights, holders, _authors) = detect_copyrights(&metadata_text, None);
        assert!(
            copyrights.iter().any(|detection| {
                detection.copyright
                    == "Copyright 2022 The Noto Project Authors (https://github.com/notofonts/latin-greek-cyrillic)"
            }),
            "copyrights: {copyrights:#?}"
        );
        assert!(
            holders
                .iter()
                .any(|detection| detection.holder == "The Noto Project Authors"),
            "holders: {holders:#?}"
        );
    }

    #[test]
    fn extracts_metadata_from_sourcecodepro_woff_fixture() {
        let bytes = fs::read("testdata/font-fixtures/SourceCodePro-Regular.otf.woff")
            .expect("read woff font fixture");

        let text = extract_font_metadata_text(Path::new("SourceCodePro-Regular.otf.woff"), &bytes)
            .expect("woff font metadata text");

        assert!(text.contains("Adobe"), "{text}");
        assert!(
            text.contains("Open Font License") || text.contains("OFL"),
            "{text}"
        );
        assert!(text.contains("http://scripts.sil.org/OFL"), "{text}");
    }

    #[test]
    fn extracts_metadata_from_sourcecodepro_woff2_fixture() {
        let bytes = fs::read("testdata/font-fixtures/SourceCodePro-Regular.otf.woff2")
            .expect("read woff2 font fixture");

        let text = extract_font_metadata_text(Path::new("SourceCodePro-Regular.otf.woff2"), &bytes)
            .expect("woff2 font metadata text");

        assert!(text.contains("Adobe"), "{text}");
        assert!(
            text.contains("Open Font License") || text.contains("OFL"),
            "{text}"
        );
        assert!(text.contains("http://scripts.sil.org/OFL"), "{text}");
    }

    #[test]
    fn extracts_legal_strings_from_notosans_eot_fixture() {
        let bytes =
            fs::read("testdata/font-fixtures/NotoSans-Regular.eot").expect("read eot font fixture");

        let text = extract_font_metadata_text(Path::new("NotoSans-Regular.eot"), &bytes)
            .expect("eot font metadata text");

        assert!(text.contains("Copyright 2015 Google Inc."), "{text}");
        assert!(
            text.contains("This Font Software is licensed under the SIL Open Font License"),
            "{text}"
        );
        assert!(text.contains("http://scripts.sil.org/OFL"), "{text}");
    }

    #[test]
    fn wrapped_font_metadata_detects_sourcecodepro_ofl_without_reserved_font_tail() {
        let bytes = fs::read("testdata/font-fixtures/SourceCodePro-Regular.otf.woff")
            .expect("read woff font fixture");
        let metadata_text =
            extract_font_metadata_text(Path::new("SourceCodePro-Regular.otf.woff"), &bytes)
                .expect("wrapped font metadata text");

        let engine = LicenseDetectionEngine::from_embedded().expect("initialize license engine");
        let detections = engine
            .detect_with_kind_and_source_with_score(&metadata_text, false, false, "font.woff", 0.0)
            .expect("detect licenses from wrapped font metadata text");
        assert!(
            detections.iter().any(|detection| {
                detection
                    .license_expression_spdx
                    .as_deref()
                    .is_some_and(|expression| expression.contains("OFL-1.1"))
            }),
            "detections: {detections:#?}"
        );

        let (copyrights, holders, _authors) = detect_copyrights(&metadata_text, None);
        assert!(
            copyrights.iter().any(|detection| {
                detection.copyright == "(c) 2023 Adobe (http://www.adobe.com/)"
            }),
            "copyrights: {copyrights:#?}"
        );
        assert!(
            holders.iter().any(|detection| detection.holder == "Adobe"),
            "holders: {holders:#?}"
        );
    }

    #[test]
    fn extracts_metadata_from_ttc_fixture() {
        let bytes = fs::read("testdata/font-fixtures/TTC.ttc").expect("read ttc font fixture");

        let text = extract_font_metadata_text(Path::new("TTC.ttc"), &bytes)
            .expect("ttc font metadata text");

        assert!(
            text.contains("Copyright") || text.contains("License"),
            "{text}"
        );
        assert!(text.contains("No rights reserved"), "{text}");
    }

    #[test]
    fn name_table_strings_do_not_run_records_together_into_malformed_urls() {
        // Reproduces the JetBrains variable-font name-table packing where
        // designer, vendor URL, and description records are stored contiguously
        // in UTF-16 storage with no separators. A raw whole-binary
        // printable-strings scrape glued them into run-on URLs such as
        // `bulenkovhttps://www.jetbrains.comThis`; per-record decoding keeps each
        // value on its own line.
        let bytes = fs::read("testdata/font-fixtures/SyntheticVariableNameRunon.ttf")
            .expect("read synthetic variable font fixture");

        let name_strings = extract_font_name_table_strings(&bytes);

        assert!(
            name_strings.contains("https://www.jetbrains.com"),
            "{name_strings}"
        );
        // The vendor URL must stand alone, never fused with the adjacent
        // designer or description records.
        assert!(
            !name_strings.contains("bulenkovhttps://www.jetbrains.com"),
            "{name_strings}"
        );
        assert!(
            !name_strings.contains("www.jetbrains.comThis"),
            "{name_strings}"
        );
        assert!(
            !name_strings.contains("OFLhttps://scripts.sil.org/OFL"),
            "{name_strings}"
        );

        // The name-table strings feed URL detection downstream; assert no
        // malformed run-on URL survives end-of-URL cleaning.
        let urls = find_urls(&name_strings, &DetectionConfig::default());
        let detected: Vec<&str> = urls.iter().map(|url| url.url.as_str()).collect();
        assert!(
            detected
                .iter()
                .any(|url| url.starts_with("https://www.jetbrains.com")),
            "detected URLs: {detected:?}"
        );
        for url in &detected {
            assert!(
                !url.contains("comThis") && !url.contains("OFLhttps"),
                "malformed run-on URL detected: {url:?} (all: {detected:?})"
            );
        }
    }
}
