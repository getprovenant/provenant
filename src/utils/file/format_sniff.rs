// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Magic-byte sniffing and container-format predicates: detecting archives,
//! compressed payloads, media containers, and supported image formats from
//! their leading bytes or extension hints.

use std::io::Cursor;
use std::path::Path;

use file_format::{FileFormat, Kind as FileFormatKind};
use image::ImageFormat;

use super::path::lower_extension;

pub(super) const ARCHIVE_EXTENSIONS: &[&str] = &[
    "zip", "jar", "war", "ear", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar", "apk", "deb", "rpm",
    "whl", "crate", "egg", "gem", "nupkg", "sqs", "squashfs",
];

pub(super) fn detect_file_format(bytes: &[u8]) -> FileFormat {
    FileFormat::from_reader(Cursor::new(bytes)).unwrap_or(FileFormat::ArbitraryBinaryData)
}

pub(super) fn is_textual_media_type(media_type: &str) -> bool {
    media_type.starts_with("text/")
        || matches!(
            media_type,
            "application/json" | "application/xml" | "text/xml"
        )
        || media_type.ends_with("+json")
        || media_type.ends_with("+xml")
}

pub(super) fn is_textual_format(detected_format: FileFormat) -> bool {
    matches!(detected_format, FileFormat::Empty | FileFormat::PlainText)
        || is_textual_media_type(detected_format.media_type())
}

pub(super) fn is_known_binary_format(detected_format: FileFormat) -> bool {
    !matches!(detected_format, FileFormat::ArbitraryBinaryData)
        && !is_textual_format(detected_format)
}

pub(super) fn media_mime_from_content(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"II\x2a\x00") || bytes.starts_with(b"MM\x00\x2a") {
        Some("image/tiff")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

pub(super) fn media_file_type_from_content(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("PNG image data")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("JPEG image data")
    } else if bytes.starts_with(b"II\x2a\x00") || bytes.starts_with(b"MM\x00\x2a") {
        Some("TIFF image data")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("WebP image data")
    } else {
        None
    }
}

pub(super) fn looks_like_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

pub(super) fn looks_like_rtf(bytes: &[u8], ext: Option<&str>) -> bool {
    ext == Some("rtf") || bytes.starts_with(b"{\\rtf")
}

pub(super) fn looks_like_gzip(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1f, 0x8b])
}

pub(super) fn looks_like_bzip2(bytes: &[u8]) -> bool {
    bytes.starts_with(b"BZh")
}

pub(super) fn looks_like_xz(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00])
}

pub(super) fn looks_like_deb(bytes: &[u8], path: &Path) -> bool {
    lower_extension(path).as_deref() == Some("deb") && bytes.starts_with(b"!<arch>\n")
}

pub(super) fn looks_like_rpm(bytes: &[u8], path: &Path) -> bool {
    lower_extension(path).as_deref() == Some("rpm") && bytes.starts_with(&[0xed, 0xab, 0xee, 0xdb])
}

pub(super) fn looks_like_squashfs(bytes: &[u8], path: &Path) -> bool {
    lower_extension(path)
        .as_deref()
        .is_some_and(|ext| matches!(ext, "sqs" | "squashfs"))
        && (bytes.starts_with(&[0x68, 0x73, 0x71, 0x73])
            || bytes.starts_with(&[0x73, 0x71, 0x73, 0x68]))
}

pub(super) fn is_zip_archive(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
}

pub(super) fn supported_image_metadata_format(
    ext: Option<&str>,
    detected_format: FileFormat,
) -> Option<ImageFormat> {
    match ext {
        Some("jpg" | "jpeg") => Some(ImageFormat::Jpeg),
        Some("png") => Some(ImageFormat::Png),
        Some("tif" | "tiff") => Some(ImageFormat::Tiff),
        Some("webp") => Some(ImageFormat::WebP),
        _ => match detected_format.media_type() {
            "image/jpeg" => Some(ImageFormat::Jpeg),
            "image/png" => Some(ImageFormat::Png),
            "image/tiff" => Some(ImageFormat::Tiff),
            "image/webp" => Some(ImageFormat::WebP),
            _ => None,
        },
    }
}

pub(super) fn is_supported_image_container(bytes: &[u8], format: ImageFormat) -> bool {
    match format {
        ImageFormat::Png => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        ImageFormat::Jpeg => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        ImageFormat::Tiff => bytes.starts_with(b"II\x2a\x00") || bytes.starts_with(b"MM\x00\x2a"),
        ImageFormat::WebP => {
            bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
        }
        _ => false,
    }
}

pub(super) fn format_based_file_type(detected_format: FileFormat) -> Option<String> {
    match detected_format {
        FileFormat::ArbitraryBinaryData | FileFormat::Empty | FileFormat::PlainText => None,
        format if format.short_name() == Some("PDF") => Some("PDF document".to_string()),
        format => Some(match format.kind() {
            FileFormatKind::Image => short_name_or_name(&format, "image data"),
            FileFormatKind::Audio => short_name_or_name(&format, "audio data"),
            FileFormatKind::Video => short_name_or_name(&format, "video data"),
            _ => format.name().to_string(),
        }),
    }
}

fn short_name_or_name(format: &FileFormat, suffix: &str) -> String {
    format
        .short_name()
        .map(|short_name| format!("{short_name} {suffix}"))
        .unwrap_or_else(|| format!("{} {suffix}", format.name()))
}
