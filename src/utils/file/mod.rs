// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! File-level utilities split by concern:
//!
//! - [`path`]: filesystem metadata, glob exclusion, extension/name predicates
//! - [`encoding`]: byte-to-text decoding and "looks like text" heuristics
//! - [`format_sniff`]: magic-byte and container-format detection
//! - [`classify`]: mime type and file-info classification surface
//! - [`extract`]: text extraction for license/copyright detection
//! - [`image_metadata`]: EXIF/XMP image metadata text extraction
//! - [`pdf`]: bounded PDF text extraction
//!
//! The public API is re-exported here so existing `crate::utils::file::*`
//! paths continue to resolve unchanged.

mod classify;
mod encoding;
mod extract;
mod format_sniff;
mod image_metadata;
mod path;
mod pdf;

pub use classify::{FileInfoClassification, classify_file_info, detect_mime_type};
pub use encoding::decode_bytes_to_string;
pub use extract::{ExtractedTextKind, extract_printable_strings, extract_text_for_detection};
pub use path::{get_creation_date, is_path_excluded};

pub(crate) use extract::{
    augment_license_detection_text, extract_text_for_detection_with_diagnostics,
};

#[cfg(test)]
mod tests {
    use image::ImageFormat;
    use std::path::Path;

    use crate::copyright::detect_copyrights;

    use super::classify::classify_file_info;
    use super::encoding::CORRUPTED_UTF16_BOM_PREFIX;
    use super::extract::{
        ExtractedTextKind, LARGE_OPAQUE_BINARY_SKIP_BYTES, extract_printable_strings,
        extract_text_for_detection, extract_text_for_detection_with_diagnostics,
        windows_metadata_or_empty_result,
    };
    use super::image_metadata::{MAX_XMP_PACKET_BYTES, extract_raw_xmp_packet};
    use super::pdf::{MAX_PDF_TEXT_EXTRACTION_BYTES, normalize_pdf_heading_comparison_text};

    fn png_chunk(chunk_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(chunk_type);
        out.extend_from_slice(data);
        out.extend_from_slice(&0u32.to_be_bytes());
        out
    }

    fn build_png_with_xmp(xmp: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");

        let ihdr = [
            0, 0, 0, 1, // width
            0, 0, 0, 1, // height
            8, // bit depth
            2, // color type
            0, // compression
            0, // filter
            0, // interlace
        ];
        bytes.extend_from_slice(&png_chunk(b"IHDR", &ihdr));

        let mut itxt = Vec::new();
        itxt.extend_from_slice(b"XML:com.adobe.xmp");
        itxt.push(0); // keyword terminator
        itxt.push(0); // compression flag
        itxt.push(0); // compression method
        itxt.push(0); // language tag terminator
        itxt.push(0); // translated keyword terminator
        itxt.extend_from_slice(xmp.as_bytes());
        bytes.extend_from_slice(&png_chunk(b"iTXt", &itxt));

        bytes.extend_from_slice(&png_chunk(b"IEND", &[]));
        bytes
    }

    #[test]
    fn test_extract_text_for_detection_skips_jar_archives() {
        let path = Path::new(
            "testdata/license-golden/datadriven/lic1/do-not_detect-licenses-in-archive.jar",
        );
        let bytes = std::fs::read(path).expect("failed to read jar fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert!(text.is_empty());
        assert_eq!(kind, ExtractedTextKind::None);
    }

    #[test]
    fn test_extract_text_for_detection_reads_pdf_fixture_text() {
        let path = Path::new("testdata/license-golden/datadriven/lic2/bsd-new_156.pdf");
        let bytes = std::fs::read(path).expect("failed to read pdf fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_eq!(kind, ExtractedTextKind::Pdf);
        assert!(text.contains("Redistribution and use in source and binary forms"));
    }

    #[test]
    fn test_extract_text_for_detection_prefers_first_pdf_page_before_full_document() {
        let path =
            Path::new("testdata/license-golden/datadriven/lic4/should_detect_something_5.pdf");
        let bytes = std::fs::read(path).expect("failed to read pdf fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_eq!(kind, ExtractedTextKind::Pdf);
        assert!(text.contains("SUN INDUSTRY STANDARDS SOURCE LICENSE"));
        assert!(!text.contains("DISCLAIMER OF WARRANTY"));
    }

    #[test]
    fn test_extract_text_for_detection_does_not_duplicate_pdf_heading_prefix() {
        let path =
            Path::new("testdata/license-golden/datadriven/lic4/should_detect_something_5.pdf");
        let bytes = std::fs::read(path).expect("failed to read pdf fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_eq!(kind, ExtractedTextKind::Pdf);

        let normalized = normalize_pdf_heading_comparison_text(&text);
        let heading =
            normalize_pdf_heading_comparison_text("SUN INDUSTRY STANDARDS SOURCE LICENSE");
        assert_eq!(normalized.matches(&heading).count(), 1);
    }

    #[test]
    fn test_extract_text_for_detection_reads_pdf_fixture_without_pdf_extension() {
        let path = Path::new("testdata/license-golden/datadriven/lic2/bsd-new_156.pdf");
        let bytes = std::fs::read(path).expect("failed to read pdf fixture");

        let (text, kind) = extract_text_for_detection(Path::new("renamed.bin"), &bytes);

        assert_eq!(kind, ExtractedTextKind::Pdf);
        assert!(text.contains("Redistribution and use in source and binary forms"));
    }

    #[test]
    fn test_extract_text_for_detection_skips_oversized_pdf_payload() {
        let mut bytes = b"%PDF-1.7\n".to_vec();
        bytes.resize(MAX_PDF_TEXT_EXTRACTION_BYTES + 1, b'0');

        let (text, kind, scan_error) =
            extract_text_for_detection_with_diagnostics(Path::new("oversized.pdf"), &bytes);

        assert!(text.is_empty());
        assert_eq!(kind, ExtractedTextKind::None);
        assert!(
            scan_error
                .as_deref()
                .is_some_and(|message| message.contains("PDF text extraction skipped"))
        );
    }

    #[test]
    fn test_extract_text_for_detection_reports_terminal_pdf_failure() {
        let malformed = b"%PDF-1.7\nthis is not a valid pdf object graph\n";

        let (text, kind, scan_error) =
            extract_text_for_detection_with_diagnostics(Path::new("broken.pdf"), malformed);

        assert!(text.is_empty());
        assert_eq!(kind, ExtractedTextKind::None);
        let scan_error = scan_error.expect("terminal pdf failure should be surfaced");
        assert!(scan_error.contains("PDF text extraction failed after"));
    }

    #[test]
    fn test_extract_text_for_detection_skips_large_opaque_binary_blobs() {
        let bytes = vec![0_u8; LARGE_OPAQUE_BINARY_SKIP_BYTES + 8];

        let (text, kind) = extract_text_for_detection(Path::new("model.bin"), &bytes);

        assert!(text.is_empty());
        assert_eq!(kind, ExtractedTextKind::None);
    }

    #[test]
    fn test_extract_text_for_detection_keeps_large_binaries_with_promising_strings() {
        let mut bytes = vec![0_u8; LARGE_OPAQUE_BINARY_SKIP_BYTES + 8];
        let text = b"Copyright 2026 Example Project!!!";
        bytes[..text.len()].copy_from_slice(text);
        let second_offset = LARGE_OPAQUE_BINARY_SKIP_BYTES / 2;
        bytes[second_offset..second_offset + text.len()].copy_from_slice(text);

        let (text, kind) = extract_text_for_detection(Path::new("weights.bin"), &bytes);

        assert_ne!(kind, ExtractedTextKind::None);
        assert!(text.contains("Copyright 2026 Example Project"));
    }

    #[test]
    fn test_extract_text_for_detection_skips_large_binary_with_unstructured_runs() {
        let mut bytes = vec![0_u8; LARGE_OPAQUE_BINARY_SKIP_BYTES + 8];
        let noise = b"(c) $1234567890ABCDEF[]{}--==++";
        bytes[..noise.len()].copy_from_slice(noise);
        let second_offset = LARGE_OPAQUE_BINARY_SKIP_BYTES / 2;
        bytes[second_offset..second_offset + noise.len()].copy_from_slice(noise);

        let (text, kind) = extract_text_for_detection(Path::new("tensor.bin"), &bytes);

        assert!(text.is_empty());
        assert_eq!(kind, ExtractedTextKind::None);
    }

    #[test]
    fn test_extract_text_for_detection_uses_windows_executable_metadata() {
        let path = Path::new("testdata/compiled-binary-golden/win_pe/libiconv2.dll");
        let bytes = std::fs::read(path).expect("read PE fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_eq!(kind, ExtractedTextKind::BinaryStrings);
        assert!(text.contains("License: This program is free software"));
        assert!(text.contains("LegalCopyright:"));
    }

    #[test]
    fn test_extract_text_for_detection_keeps_windows_metadata_for_large_pe_without_sampled_signal()
    {
        let path = Path::new("testdata/compiled-binary-golden/win_pe/libiconv2.dll");
        let mut bytes = std::fs::read(path).expect("read PE fixture");
        bytes.resize(LARGE_OPAQUE_BINARY_SKIP_BYTES + 8, 0);

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_ne!(kind, ExtractedTextKind::None);
        assert!(!text.trim().is_empty());
    }

    #[test]
    fn test_windows_metadata_or_empty_result_preserves_metadata() {
        let (text, kind, scan_error) =
            windows_metadata_or_empty_result(Some("LegalCopyright: Example Corp".to_string()));

        assert_eq!(kind, ExtractedTextKind::WindowsExecutableMetadata);
        assert_eq!(text, "LegalCopyright: Example Corp");
        assert!(scan_error.is_none());
    }

    #[test]
    fn test_extract_text_for_detection_keeps_image_author_separate_from_title_and_description() {
        let xmp = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Chinmay Garde</dc:creator><dc:title>Bay Bridge At Night</dc:title><dc:description>Embarcadero in the evening on Delta 3200</dc:description></rdf:Description></rdf:RDF></x:xmpmeta>"#;
        let bytes = build_png_with_xmp(xmp);

        let (text, kind) = extract_text_for_detection(Path::new("fixture.png"), &bytes);

        assert_eq!(kind, ExtractedTextKind::ImageMetadata);
        assert!(text.contains("Author: Chinmay Garde"), "text: {text:?}");
        assert!(
            text.contains("Title: Bay Bridge At Night"),
            "text: {text:?}"
        );
        assert!(
            text.contains("Description: Embarcadero in the evening on Delta 3200"),
            "text: {text:?}"
        );

        let (_copyrights, _holders, authors) = detect_copyrights(&text, None);
        assert_eq!(
            authors
                .iter()
                .map(|a| a.author.as_str())
                .collect::<Vec<_>>(),
            vec!["Chinmay Garde"],
            "authors: {authors:?}; text: {text:?}"
        );
    }

    #[test]
    fn test_extract_text_for_detection_skips_large_binary_with_single_isolated_string_run() {
        let mut bytes = vec![0_u8; LARGE_OPAQUE_BINARY_SKIP_BYTES + 8];
        let text = b"Copyright 2026 Example Project!!!";
        bytes[..text.len()].copy_from_slice(text);

        let (text, kind) = extract_text_for_detection(Path::new("opaque.bin"), &bytes);

        assert!(text.is_empty());
        assert_eq!(kind, ExtractedTextKind::None);
    }

    #[test]
    fn test_extract_text_for_detection_keeps_large_binary_with_single_contact_rich_window() {
        let mut bytes = vec![0_u8; LARGE_OPAQUE_BINARY_SKIP_BYTES + 8];
        let text = b"Andreas Schneider <asn@redhat.com> Rob Crittenden (rcritten@redhat.com) Mr. Sam <sam@email-scan.com> https://publicsuffix.org/ http://tukaani.org/xz/";
        bytes[..text.len()].copy_from_slice(text);

        let (text, kind) = extract_text_for_detection(Path::new("rootfs.bin"), &bytes);

        assert_ne!(kind, ExtractedTextKind::None);
        assert!(text.contains("asn@redhat.com"));
        assert!(text.contains("https://publicsuffix.org/"));
    }

    #[test]
    fn test_extract_text_for_detection_keeps_large_macho_with_off_window_legal_markers() {
        let mut bytes = vec![0_u8; LARGE_OPAQUE_BINARY_SKIP_BYTES * 2];
        bytes[..4].copy_from_slice(&[0xCF, 0xFA, 0xED, 0xFE]);
        let apache_notice = b"// Licensed under the Apache License, Version 2.0 (the \"License\");\n// http://www.apache.org/licenses/LICENSE-2.0\n// SPDX-License-Identifier: Apache-2.0\n";
        let insert_offset = 200 * 1024;
        bytes[insert_offset..insert_offset + apache_notice.len()].copy_from_slice(apache_notice);

        let (text, kind) = extract_text_for_detection(Path::new("node"), &bytes);

        assert_eq!(kind, ExtractedTextKind::BinaryStrings);
        assert!(text.contains("Apache License, Version 2.0"), "{text}");
        assert!(
            text.contains("SPDX-License-Identifier: Apache-2.0"),
            "{text}"
        );
    }

    #[test]
    fn test_extract_text_for_detection_keeps_large_macho_with_unicode_notice_markers() {
        let mut bytes = vec![0_u8; LARGE_OPAQUE_BINARY_SKIP_BYTES * 2];
        bytes[..4].copy_from_slice(&[0xCF, 0xFA, 0xED, 0xFE]);
        let unicode_notice = b"Copyright (c) 1991-2024 Unicode, Inc.\nFor terms of use, see http://www.unicode.org/copyright.html\n";
        let insert_offset = 700 * 1024;
        bytes[insert_offset..insert_offset + unicode_notice.len()].copy_from_slice(unicode_notice);

        let (text, kind) = extract_text_for_detection(Path::new("node"), &bytes);

        assert_eq!(kind, ExtractedTextKind::BinaryStrings);
        assert!(text.contains("Unicode, Inc."), "{text}");
        assert!(text.contains("unicode.org/copyright.html"), "{text}");
    }

    #[test]
    fn test_extract_text_for_detection_does_not_reopen_single_window_legal_noise_for_non_macho() {
        let mut bytes = vec![0_u8; LARGE_OPAQUE_BINARY_SKIP_BYTES * 2];
        let apache_notice = b"// Licensed under the Apache License, Version 2.0 (the \"License\");\n// http://www.apache.org/licenses/LICENSE-2.0\n// SPDX-License-Identifier: Apache-2.0\n";
        let insert_offset = 200 * 1024;
        bytes[insert_offset..insert_offset + apache_notice.len()].copy_from_slice(apache_notice);

        let (text, kind) = extract_text_for_detection(Path::new("model.bin"), &bytes);

        assert!(text.is_empty());
        assert_eq!(kind, ExtractedTextKind::None);
    }

    #[test]
    fn test_extract_text_for_detection_avoids_latin1_decode_for_binary_blob_noise() {
        let bytes = vec![
            0x28, 0x63, 0x29, 0x20, 0x4b, 0x30, 0x0e, 0x71, 0x86, 0x20, 0x62, 0x24, 0x4c,
        ];

        let (text, kind) = extract_text_for_detection(Path::new("fixture.blb"), &bytes);

        assert_eq!(kind, ExtractedTextKind::BinaryStrings);
        assert_eq!(text, "(c) K0\n b$L");
    }

    #[test]
    fn test_extract_raw_xmp_packet_rejects_oversized_png_itxt_payload() {
        let xmp = "A".repeat(MAX_XMP_PACKET_BYTES + 1);
        let bytes = build_png_with_xmp(&xmp);

        assert!(extract_raw_xmp_packet(&bytes, ImageFormat::Png).is_none());
    }

    #[test]
    fn test_extract_text_for_detection_skips_zip_like_archives() {
        let zip_bytes = b"PK\x03\x04\x14\x00\x00\x00\x08\x00artifact";

        let (whl_text, whl_kind) = extract_text_for_detection(Path::new("demo.whl"), zip_bytes);
        let (crate_text, crate_kind) =
            extract_text_for_detection(Path::new("demo.crate"), zip_bytes);

        assert!(whl_text.is_empty());
        assert_eq!(whl_kind, ExtractedTextKind::None);
        assert!(crate_text.is_empty());
        assert_eq!(crate_kind, ExtractedTextKind::None);
    }

    #[test]
    fn test_extract_text_for_detection_keeps_binary_strings_for_lib_fixtures() {
        let path =
            Path::new("testdata/copyright-golden/copyrights/copyright_php_lib-php_embed_lib.lib");
        let bytes = std::fs::read(path).expect("failed to read lib fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_ne!(kind, ExtractedTextKind::None);
        assert!(text.contains("Copyright nexB and others (c) 2012"));
    }

    #[test]
    fn test_extract_text_for_detection_reads_font_metadata() {
        let path = Path::new("testdata/font-fixtures/Lato-Bold.ttf");
        let bytes = std::fs::read(path).expect("failed to read font fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_eq!(kind, ExtractedTextKind::FontMetadata);
        assert!(text.contains("License Description:"), "{text}");
        assert!(
            text.contains("Open Font License") || text.contains("OFL"),
            "{text}"
        );
        assert!(text.contains("Lato"), "{text}");
    }

    #[test]
    fn test_extract_printable_strings_scales_cap_for_medium_binary_files() {
        let bytes = b"abcd\0".repeat(525_000);

        let text = extract_printable_strings(&bytes);

        assert!(
            text.len() > 2_000_000,
            "unexpected truncation at {}",
            text.len()
        );
        assert!(text.ends_with("abcd"));
    }

    #[test]
    fn test_extract_text_for_detection_decodes_svg_fixture_text() {
        let path = Path::new(
            "testdata/license-golden/datadriven/external/fossology-tests/Public-domain/biohazard.svg",
        );
        let bytes = std::fs::read(path).expect("failed to read svg fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_eq!(kind, ExtractedTextKind::Decoded);
        assert!(text.contains("creativecommons.org/licenses/publicdomain"));
    }

    #[test]
    fn test_extract_text_for_detection_preserves_blank_comment_lines_in_utf8_source() {
        let path = Path::new("testdata/plugin_email_url/files/IMarkerActionFilter.java");
        let bytes = std::fs::read(path).expect("failed to read java fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_eq!(kind, ExtractedTextKind::Decoded);
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.get(2).copied(), Some(" *"));
        assert_eq!(
            lines.get(3).copied(),
            Some(" *https://github.com/rpm-software-management")
        );
        assert_eq!(lines.get(5).copied(), Some("https://gitlab.com/Conan_Kudo"));
    }

    #[test]
    fn test_extract_text_for_detection_decodes_rtf_fixture_text() {
        let path = Path::new(
            "testdata/license-golden/datadriven/external/fossology-tests/LGPL/License.rtf",
        );
        let bytes = std::fs::read(path).expect("failed to read rtf fixture");

        let (text, kind) = extract_text_for_detection(path, &bytes);

        assert_eq!(kind, ExtractedTextKind::Decoded);
        assert!(text.contains("GNU Lesser General Public"));
        assert!(text.contains("version"));
        assert!(text.contains("2.1 of the License"));
    }

    #[test]
    fn test_classify_file_info_marks_empty_files_as_text_not_source() {
        let classification = classify_file_info(Path::new("test.txt"), b"");

        assert_eq!(classification.mime_type, "inode/x-empty");
        assert_eq!(classification.file_type, "empty");
        assert!(!classification.is_binary);
        assert!(classification.is_text);
        assert!(!classification.is_source);
        assert_eq!(classification.programming_language, None);
    }

    #[test]
    fn test_classify_file_info_keeps_json_out_of_programming_language() {
        let classification = classify_file_info(Path::new("package.json"), br#"{"name":"demo"}"#);

        assert_eq!(classification.mime_type, "application/json");
        assert_eq!(classification.file_type, "JSON text data");
        assert!(classification.is_text);
        assert!(!classification.is_source);
        assert_eq!(classification.programming_language, None);
    }

    #[test]
    fn test_classify_file_info_does_not_label_invalid_json_text_as_json() {
        let classification =
            classify_file_info(Path::new("broken.json"), b"{ definitely not json\n");

        assert_eq!(classification.mime_type, "text/plain");
        assert_eq!(classification.file_type, "UTF-8 Unicode text");
        assert!(classification.is_text);
        assert!(!classification.is_binary);
    }

    #[test]
    fn test_classify_file_info_does_not_label_binary_json_garbage_as_json() {
        let classification =
            classify_file_info(Path::new("broken.json"), &[0xff, 0x00, 0x01, 0x02]);

        assert_eq!(classification.mime_type, "application/octet-stream");
        assert_eq!(classification.file_type, "data");
        assert!(classification.is_binary);
        assert!(!classification.is_text);
    }

    #[test]
    fn test_classify_file_info_treats_valid_utf16_json_with_bom_as_text() {
        let classification = classify_file_info(
            Path::new("utf16.json"),
            &[
                0xFF, 0xFE, 0x5B, 0x00, 0x22, 0x00, 0xE9, 0x00, 0x22, 0x00, 0x5D, 0x00,
            ],
        );

        assert!(!classification.is_binary);
        assert!(classification.is_text);
        assert_eq!(classification.mime_type, "application/json");
        assert_eq!(classification.file_type, "JSON text data");
    }

    #[test]
    fn test_classify_file_info_treats_valid_utf16be_json_without_bom_as_text() {
        let classification = classify_file_info(
            Path::new("utf16be.json"),
            &[0x00, 0x5B, 0x00, 0x22, 0x00, 0xE9, 0x00, 0x22, 0x00, 0x5D],
        );

        assert!(!classification.is_binary);
        assert!(classification.is_text);
        assert_eq!(classification.mime_type, "application/json");
        assert_eq!(classification.file_type, "JSON text data");
    }

    #[test]
    fn test_classify_file_info_treats_small_valid_utf16be_json_literal_as_text() {
        let classification =
            classify_file_info(Path::new("utf16be-literal.json"), &[0x00, 0x5B, 0x00, 0x5D]);

        assert!(!classification.is_binary);
        assert!(classification.is_text);
        assert_eq!(classification.mime_type, "application/json");
        assert_eq!(classification.file_type, "JSON text data");
    }

    #[test]
    fn test_extract_text_for_detection_decodes_utf16be_text_with_corrupted_bom_prefix() {
        let mut bytes = CORRUPTED_UTF16_BOM_PREFIX.to_vec();
        for code_unit in
            "Licensed to the Apache Software Foundation\nApache License, Version 2.0".encode_utf16()
        {
            bytes.extend_from_slice(&code_unit.to_be_bytes());
        }

        let (text, kind) = extract_text_for_detection(Path::new("notice.ftl"), &bytes);

        assert_eq!(kind, ExtractedTextKind::Decoded);
        assert!(text.contains("Apache Software Foundation"), "{text}");
        assert!(text.contains("Apache License, Version 2.0"), "{text}");
    }

    #[test]
    fn test_classify_file_info_treats_small_valid_json_literals_as_text() {
        let classification = classify_file_info(Path::new("true.json"), b"true");

        assert!(!classification.is_binary);
        assert!(classification.is_text);
        assert_eq!(classification.mime_type, "application/json");
        assert_eq!(classification.file_type, "JSON text data");
    }

    #[test]
    fn test_classify_file_info_treats_json_wrapped_invalid_utf8_sequences_as_text() {
        let classification = classify_file_info(
            Path::new("wrapped.json"),
            &[0x5B, 0x22, 0xE6, 0x97, 0xA5, 0xD1, 0x88, 0xFA, 0x22, 0x5D],
        );

        assert!(!classification.is_binary);
        assert!(classification.is_text);
        assert_eq!(classification.mime_type, "text/plain");
        assert_eq!(classification.file_type, "text, with no line terminators");
    }

    #[test]
    fn test_classify_file_info_keeps_lone_ff_json_byte_binary() {
        let classification =
            classify_file_info(Path::new("lone-ff.json"), &[0x5B, 0x22, 0xFF, 0x22, 0x5D]);

        assert!(classification.is_binary);
        assert!(!classification.is_text);
        assert_eq!(classification.mime_type, "application/octet-stream");
        assert_eq!(classification.file_type, "data");
    }

    #[test]
    fn test_classify_file_info_keeps_nul_heavy_crash_json_binary() {
        let classification = classify_file_info(
            Path::new("crash.json"),
            &[
                0xFE, 0x90, 0x00, 0x00, 0x00, 0x93, 0x5B, 0x5B, 0x32, 0x38, 0x36,
            ],
        );

        assert!(classification.is_binary);
        assert!(!classification.is_text);
        assert_eq!(classification.mime_type, "application/octet-stream");
    }

    #[test]
    fn test_classify_file_info_treats_dockerfile_as_source() {
        let classification = classify_file_info(Path::new("Dockerfile"), b"FROM scratch\n");

        assert_eq!(
            classification.programming_language.as_deref(),
            Some("Dockerfile")
        );
        assert!(classification.is_source);
        assert!(!classification.is_script);
        assert_eq!(
            classification.file_type,
            "Dockerfile source, UTF-8 Unicode text"
        );
    }

    #[test]
    fn test_classify_file_info_treats_makefile_as_text_not_source() {
        let classification = classify_file_info(Path::new("Makefile"), b"all:\n\techo hi\n");

        assert_eq!(classification.programming_language, None);
        assert!(classification.is_text);
        assert!(!classification.is_source);
        assert!(!classification.is_script);
        assert_eq!(classification.file_type, "UTF-8 Unicode text");
    }

    #[test]
    fn test_classify_file_info_marks_supported_package_archives() {
        let zip_bytes = b"PK\x03\x04\x14\x00\x00\x00";

        let egg = classify_file_info(Path::new("demo.egg"), zip_bytes);
        let nupkg = classify_file_info(Path::new("demo.nupkg"), zip_bytes);

        assert!(egg.is_archive);
        assert_eq!(egg.mime_type, "application/zip");
        assert_eq!(egg.file_type, "Zip archive data");
        assert!(nupkg.is_archive);
        assert_eq!(nupkg.mime_type, "application/zip");
        assert_eq!(nupkg.file_type, "Zip archive data");
    }

    #[test]
    fn test_classify_file_info_marks_png_as_binary_media() {
        let png_bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR";

        let classification = classify_file_info(Path::new("logo.png"), png_bytes);

        assert_eq!(classification.mime_type, "image/png");
        assert_eq!(classification.file_type, "PNG image data");
        assert!(classification.is_binary);
        assert!(!classification.is_text);
        assert!(classification.is_media);
        assert!(!classification.is_archive);
        assert!(!classification.is_source);
    }

    #[test]
    fn test_classify_file_info_marks_pdf_as_binary_document() {
        let pdf_bytes = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\n";

        let classification = classify_file_info(Path::new("report.pdf"), pdf_bytes);

        assert_eq!(classification.mime_type, "application/pdf");
        assert_eq!(classification.file_type, "PDF document");
        assert!(classification.is_binary);
        assert!(!classification.is_text);
        assert!(!classification.is_archive);
        assert!(!classification.is_media);
    }

    #[test]
    fn test_classify_file_info_marks_binary_blobs_as_binary() {
        let classification =
            classify_file_info(Path::new("blob.bin"), &[0, 159, 146, 150, 0, 1, 2, 3, 4, 5]);

        assert!(classification.is_binary);
        assert!(!classification.is_text);
        assert!(!classification.is_source);
        assert_eq!(classification.programming_language, None);
    }

    #[test]
    fn test_classify_file_info_treats_yaml_as_text_not_source() {
        let classification = classify_file_info(Path::new("config.yaml"), b"key: value\n");

        assert_eq!(classification.programming_language, None);
        assert!(classification.is_text);
        assert!(!classification.is_source);
        assert_eq!(classification.file_type, "YAML text data");
    }

    #[test]
    fn test_classify_file_info_classifies_common_build_manifests() {
        let gradle = classify_file_info(Path::new("build.gradle"), b"plugins { id 'java' }\n");
        let flake = classify_file_info(Path::new("flake.nix"), b"{ inputs, ... }: {}\n");
        let cmake = classify_file_info(
            Path::new("toolchain.cmake"),
            b"set(CMAKE_CXX_STANDARD 20)\n",
        );
        let gitmodules = classify_file_info(
            Path::new(".gitmodules"),
            b"[submodule \"demo\"]\n\tpath = vendor/demo\n",
        );

        assert_eq!(gradle.programming_language.as_deref(), Some("Groovy"));
        assert!(gradle.is_source);
        assert_eq!(gradle.mime_type, "text/plain");
        assert_eq!(gradle.file_type, "Groovy source, UTF-8 Unicode text");

        assert_eq!(flake.programming_language.as_deref(), Some("Nix"));
        assert!(flake.is_source);
        assert_eq!(flake.mime_type, "text/plain");
        assert_eq!(flake.file_type, "Nix source, UTF-8 Unicode text");

        assert_eq!(cmake.programming_language.as_deref(), Some("CMake"));
        assert!(cmake.is_source);
        assert_eq!(cmake.file_type, "CMake source, UTF-8 Unicode text");

        assert_eq!(gitmodules.programming_language, None);
        assert!(gitmodules.is_text);
        assert!(!gitmodules.is_source);
        assert_eq!(gitmodules.file_type, "Git configuration text");
    }

    #[test]
    fn test_classify_file_info_labels_cpp_headers_and_ipp_separately() {
        let header = classify_file_info(
            Path::new("include/demo.hpp"),
            b"#pragma once\nclass Demo {};\n",
        );
        let ipp = classify_file_info(
            Path::new("include/detail/demo.ipp"),
            b"template <class T> void parse() {}\n",
        );

        assert_eq!(header.programming_language.as_deref(), Some("C++"));
        assert!(header.is_source);
        assert!(!header.is_script);
        assert_eq!(header.file_type, "C++ source, UTF-8 Unicode text");

        assert_eq!(ipp.programming_language, None);
        assert!(!ipp.is_source);
        assert!(!ipp.is_script);
        assert_eq!(ipp.file_type, "UTF-8 Unicode text");
    }

    #[test]
    fn test_classify_file_info_preserves_specific_shell_family_labels() {
        let bash = classify_file_info(Path::new("bin/run"), b"#!/usr/bin/env bash\necho hi\n");

        assert_eq!(bash.programming_language.as_deref(), Some("Bash"));
        assert!(bash.is_script);
        assert_eq!(bash.file_type, "bash script, UTF-8 Unicode text executable");
    }

    #[test]
    fn test_classify_file_info_marks_jamfile_as_source() {
        let jamfile = classify_file_info(Path::new("Jamfile"), b"lib boost_json ;\n");

        assert_eq!(jamfile.programming_language.as_deref(), Some("Jamfile"));
        assert!(jamfile.is_source);
        assert!(!jamfile.is_script);
        assert_eq!(jamfile.file_type, "Jamfile source, UTF-8 Unicode text");
    }

    #[test]
    fn test_classify_file_info_labels_javascript_shebang_scripts() {
        let classification = classify_file_info(
            Path::new("bin/run"),
            b"#!/usr/bin/env node\nconsole.log('hello');\n",
        );

        assert_eq!(
            classification.programming_language.as_deref(),
            Some("JavaScript")
        );
        assert!(classification.is_script);
        assert_eq!(
            classification.file_type,
            "javascript script, UTF-8 Unicode text executable"
        );
    }

    #[test]
    fn test_classify_file_info_uses_non_utf8_text_labels_for_latin1_scripts() {
        let classification = classify_file_info(
            Path::new("script.py"),
            b"# coding: latin-1\nprint(\"caf\xe9\")\n",
        );

        assert_eq!(
            classification.programming_language.as_deref(),
            Some("Python")
        );
        assert!(classification.is_script);
        assert_eq!(classification.file_type, "python script, text executable");
    }

    #[test]
    fn test_classify_file_info_treats_textual_tga_as_media() {
        let classification = classify_file_info(Path::new("texture.tga"), b"not really a tga\n");

        assert!(classification.is_media);
        assert!(classification.is_text);
        assert!(!classification.is_binary);
    }

    #[test]
    fn test_classify_file_info_keeps_binaryish_source_extension_out_of_text_path() {
        let classification =
            classify_file_info(Path::new("main.ts"), &[0x80, 0x81, 0x82, 0x83, 0x84, 0x85]);

        assert!(classification.is_binary);
        assert!(!classification.is_text);
        assert!(!classification.is_source);
        assert_eq!(classification.programming_language, None);
    }

    #[test]
    fn test_extract_text_for_detection_skips_unsupported_image_formats() {
        let gif_bytes = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

        let (text, kind) = extract_text_for_detection(Path::new("tiny.gif"), gif_bytes);

        assert!(text.is_empty());
        assert_eq!(kind, ExtractedTextKind::None);
    }

    #[test]
    fn test_classify_file_info_preserves_language_detection_precedence_matrix() {
        let cases = [
            (
                Path::new("bin/run"),
                b"#!/usr/bin/env node\nconsole.log('hello');\n".as_slice(),
                Some("JavaScript"),
                true,
                true,
            ),
            (
                Path::new("Dockerfile"),
                b"FROM scratch\n".as_slice(),
                Some("Dockerfile"),
                true,
                false,
            ),
            (
                Path::new("package.json"),
                br#"{"name":"demo"}"#.as_slice(),
                None,
                false,
                false,
            ),
            (
                Path::new("config.yaml"),
                b"key: value\n".as_slice(),
                None,
                false,
                false,
            ),
            (
                Path::new("Makefile"),
                b"all:\n\techo hi\n".as_slice(),
                None,
                false,
                false,
            ),
        ];

        for (path, bytes, expected_language, expected_is_source, expected_is_script) in cases {
            let classification = classify_file_info(path, bytes);

            assert_eq!(
                classification.programming_language.as_deref(),
                expected_language,
                "unexpected language for {}",
                path.display()
            );
            assert_eq!(
                classification.is_source,
                expected_is_source,
                "unexpected is_source for {}",
                path.display()
            );
            assert_eq!(
                classification.is_script,
                expected_is_script,
                "unexpected is_script for {}",
                path.display()
            );
        }
    }
}
