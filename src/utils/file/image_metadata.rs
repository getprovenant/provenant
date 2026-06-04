// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! EXIF and XMP metadata text extraction from supported image containers,
//! including a hand-rolled PNG iTXt XMP packet reader.

use std::collections::BTreeSet;
use std::io::{BufReader, Cursor, Read};

use flate2::read::ZlibDecoder;
use image::{ImageDecoder, ImageFormat, ImageReader};
use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;

const MAX_IMAGE_METADATA_VALUES: usize = 64;
const MAX_IMAGE_METADATA_TEXT_BYTES: usize = 32 * 1024;
pub(super) const MAX_XMP_PACKET_BYTES: usize = 256 * 1024;

pub(super) fn extract_image_metadata_text(bytes: &[u8], format: ImageFormat) -> String {
    let mut values = Vec::new();
    values.extend(extract_exif_metadata_values(bytes));
    values.extend(extract_xmp_metadata_values(bytes, format));
    values_to_text(values)
}

fn extract_exif_metadata_values(bytes: &[u8]) -> Vec<String> {
    let mut cursor = BufReader::new(Cursor::new(bytes));
    let exif = match exif::Reader::new().read_from_container(&mut cursor) {
        Ok(exif) => exif,
        Err(_) => return Vec::new(),
    };

    let mut values = Vec::new();
    for field in exif.fields() {
        let rendered = match field.tag {
            exif::Tag::ImageDescription => Some(format_metadata_field(
                "Description",
                &field.display_value().with_unit(&exif).to_string(),
            )),
            exif::Tag::Copyright => Some(format_metadata_field(
                "Copyright",
                &field.display_value().with_unit(&exif).to_string(),
            )),
            exif::Tag::UserComment => Some(format_metadata_field(
                "Comment",
                &field.display_value().with_unit(&exif).to_string(),
            )),
            exif::Tag::Artist => Some(format_metadata_field(
                "Author",
                &field.display_value().with_unit(&exif).to_string(),
            )),
            _ => None,
        };

        if let Some(rendered) = rendered {
            values.push(rendered);
        }
    }

    values
}

fn extract_xmp_metadata_values(bytes: &[u8], format: ImageFormat) -> Vec<String> {
    let xmp = match extract_raw_xmp_packet(bytes, format) {
        Some(xmp) => xmp,
        None => return Vec::new(),
    };

    parse_xmp_values(&xmp)
}

pub(super) fn extract_raw_xmp_packet(bytes: &[u8], format: ImageFormat) -> Option<Vec<u8>> {
    let reader = ImageReader::with_format(BufReader::new(Cursor::new(bytes)), format);
    if let Ok(mut decoder) = reader.into_decoder()
        && let Ok(Some(xmp)) = decoder.xmp_metadata()
    {
        return (xmp.len() <= MAX_XMP_PACKET_BYTES).then_some(xmp);
    }

    match format {
        ImageFormat::Png => extract_png_xmp_packet(bytes),
        _ => None,
    }
}

fn extract_png_xmp_packet(bytes: &[u8]) -> Option<Vec<u8>> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

    if bytes.len() < PNG_SIGNATURE.len() || &bytes[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return None;
    }

    let mut offset = PNG_SIGNATURE.len();
    while offset + 12 <= bytes.len() {
        let length = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let chunk_start = offset + 8;
        let chunk_end = chunk_start + length;
        if chunk_end + 4 > bytes.len() {
            return None;
        }

        let chunk_type = &bytes[offset + 4..offset + 8];
        if chunk_type == b"iTXt" {
            let data = &bytes[chunk_start..chunk_end];
            if let Some(xmp) = parse_png_itxt_xmp(data) {
                return Some(xmp);
            }
        }

        offset = chunk_end + 4;
    }

    None
}

fn parse_png_itxt_xmp(data: &[u8]) -> Option<Vec<u8>> {
    const XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";

    let keyword_end = data.iter().position(|&b| b == 0)?;
    if &data[..keyword_end] != XMP_KEYWORD {
        return None;
    }

    let mut cursor = keyword_end + 1;
    let compression_flag = *data.get(cursor)?;
    cursor += 1;
    let compression_method = *data.get(cursor)?;
    cursor += 1;
    if compression_flag > 1 || (compression_flag == 1 && compression_method != 0) {
        return None;
    }

    let language_end = cursor + data[cursor..].iter().position(|&b| b == 0)?;
    cursor = language_end + 1;

    let translated_end = cursor + data[cursor..].iter().position(|&b| b == 0)?;
    cursor = translated_end + 1;

    let text_bytes = &data[cursor..];
    if compression_flag == 1 {
        let decoder = ZlibDecoder::new(text_bytes);
        let mut decoded = Vec::new();
        decoder
            .take((MAX_XMP_PACKET_BYTES + 1) as u64)
            .read_to_end(&mut decoded)
            .ok()?;
        (decoded.len() <= MAX_XMP_PACKET_BYTES).then_some(decoded)
    } else {
        (text_bytes.len() <= MAX_XMP_PACKET_BYTES).then(|| text_bytes.to_vec())
    }
}

fn parse_xmp_values(xmp: &[u8]) -> Vec<String> {
    let mut reader = XmlReader::from_reader(xmp);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut values = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                stack.push(local_xml_name(e.name().as_ref()));
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::Text(text)) => {
                if let Some(field) = stack
                    .iter()
                    .rev()
                    .find_map(|name| allowed_xmp_field(name.as_str()))
                    && let Ok(decoded) = text.decode()
                {
                    let decoded = decoded.into_owned();
                    if !decoded.trim().is_empty() {
                        values.push(format_xmp_value(field, &decoded));
                    }
                }
            }
            Ok(Event::CData(text)) => {
                if let Some(field) = stack
                    .iter()
                    .rev()
                    .find_map(|name| allowed_xmp_field(name.as_str()))
                    && let Ok(decoded) = text.decode()
                {
                    let decoded = decoded.into_owned();
                    if !decoded.trim().is_empty() {
                        values.push(format_xmp_value(field, &decoded));
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    values
}

fn local_xml_name(name: &[u8]) -> String {
    let name = std::str::from_utf8(name).unwrap_or_default();
    name.rsplit(':').next().unwrap_or(name).to_string()
}

fn allowed_xmp_field(name: &str) -> Option<&'static str> {
    match name {
        "creator" => Some("creator"),
        "rights" => Some("rights"),
        "description" => Some("description"),
        "title" => Some("title"),
        "subject" => Some("subject"),
        "UsageTerms" => Some("usage_terms"),
        "WebStatement" => Some("web_statement"),
        _ => None,
    }
}

pub(super) fn format_xmp_value(field: &str, value: &str) -> String {
    match field {
        "creator" => format_metadata_field("Author", value),
        "rights" => format_metadata_field("Copyright", value),
        "description" => format_metadata_field("Description", value),
        "title" => format_metadata_field("Title", value),
        "subject" => format_metadata_field("Subject", value),
        "usage_terms" => format_metadata_field("UsageTerms", value),
        "web_statement" => format_metadata_field("WebStatement", value),
        _ => value.to_string(),
    }
}

pub(super) fn format_metadata_field(label: &str, value: &str) -> String {
    format!("{label}: {value}")
}

pub(super) fn values_to_text(values: Vec<String>) -> String {
    let mut seen = BTreeSet::new();
    let mut normalized_lines = Vec::new();

    for value in values {
        let normalized = normalize_metadata_value(&value);
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }

        normalized_lines.push(normalized);
    }

    let author_values: BTreeSet<String> = normalized_lines
        .iter()
        .filter_map(|line| split_metadata_field(line))
        .filter(|(label, _)| label.eq_ignore_ascii_case("Author"))
        .map(|(_, value)| value.to_string())
        .collect();

    let mut lines = Vec::new();
    let mut total_bytes = 0usize;

    for normalized in normalized_lines {
        if lines.len() >= MAX_IMAGE_METADATA_VALUES {
            break;
        }

        if should_suppress_bare_copyright_metadata_line(&normalized, &author_values) {
            continue;
        }

        let added_bytes = normalized.len() + usize::from(!lines.is_empty());
        if total_bytes + added_bytes > MAX_IMAGE_METADATA_TEXT_BYTES {
            break;
        }

        total_bytes += added_bytes;
        lines.push(normalized);
    }

    lines.join("\n")
}

fn split_metadata_field(line: &str) -> Option<(&str, &str)> {
    let (label, value) = line.split_once(':')?;
    Some((label.trim(), value.trim()))
}

fn should_suppress_bare_copyright_metadata_line(
    line: &str,
    author_values: &BTreeSet<String>,
) -> bool {
    let Some((label, value)) = split_metadata_field(line) else {
        return false;
    };
    if !label.eq_ignore_ascii_case("Copyright")
        || value.is_empty()
        || !author_values.contains(value)
    {
        return false;
    }

    let lower = value.to_ascii_lowercase();
    !lower.contains("copyright")
        && !lower.contains("(c)")
        && !lower.contains('©')
        && !lower.contains("all rights")
        && !value.chars().any(|ch| ch.is_ascii_digit())
}

fn normalize_metadata_value(value: &str) -> String {
    value
        .chars()
        .filter(|&ch| ch != '\0')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{format_metadata_field, format_xmp_value, values_to_text};

    #[test]
    fn test_format_xmp_value_labels_creator_and_title_fields() {
        assert_eq!(
            format_xmp_value("creator", "Chinmay Garde"),
            "Author: Chinmay Garde"
        );
        assert_eq!(
            format_xmp_value("title", "Bay Bridge At Night"),
            "Title: Bay Bridge At Night"
        );
        assert_eq!(
            format_xmp_value("description", "Embarcadero in the evening on Delta 3200"),
            "Description: Embarcadero in the evening on Delta 3200"
        );
    }

    #[test]
    fn test_format_metadata_field_prefixes_exif_text() {
        assert_eq!(
            format_metadata_field("Author", "Chinmay Garde"),
            "Author: Chinmay Garde"
        );
        assert_eq!(
            format_metadata_field("Description", "Bay Bridge At Night"),
            "Description: Bay Bridge At Night"
        );
    }

    #[test]
    fn test_values_to_text_suppresses_bare_copyright_duplicate_of_author() {
        let text = values_to_text(vec![
            "Author: Chinmay Garde".to_string(),
            "Copyright: Chinmay Garde".to_string(),
            "Title: Bay Bridge At Night".to_string(),
        ]);

        assert!(text.contains("Author: Chinmay Garde"), "text: {text:?}");
        assert!(
            text.contains("Title: Bay Bridge At Night"),
            "text: {text:?}"
        );
        assert!(!text.contains("Copyright: Chinmay Garde"), "text: {text:?}");
    }
}
