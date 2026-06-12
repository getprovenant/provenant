// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Byte-to-text decoding and "does this look like text" heuristics shared by
//! classification and text-extraction logic.

const BINARY_CONTROL_CHAR_THRESHOLD_DIVISOR: usize = 10;

pub(super) const CORRUPTED_UTF16_BOM_PREFIX: &[u8] = &[0xEF, 0xBF, 0xBD, 0xEF, 0xBF, 0xBD];

/// Diagnostic message emitted when invalid-UTF-8 input is dropped from
/// detection because it exceeds the binary-control-char threshold. Surfaced via
/// the scanner's structured `scan_diagnostics` channel so the skip is
/// observable rather than silent.
pub(super) const NEAR_BINARY_SKIP_DIAGNOSTIC: &str = "Text skipped from license/copyright detection: invalid UTF-8 with too many control bytes (likely binary)";

/// Decode a byte buffer to a String, trying UTF-16 first when the byte shape
/// strongly suggests it, then UTF-8, then Latin-1.
///
/// Latin-1 (ISO-8859-1) maps bytes 0x00-0xFF directly to Unicode U+0000-U+00FF,
/// so it can decode any byte sequence. This matches Python ScanCode's use of
/// `UnicodeDammit` which auto-detects encoding with Latin-1 as fallback.
pub fn decode_bytes_to_string(bytes: &[u8]) -> String {
    decode_bytes_to_string_with_diagnostic(bytes).0
}

/// Like [`decode_bytes_to_string`], but also returns a structured diagnostic
/// when the result is the empty string because the input looked like binary
/// (invalid UTF-8 with a high control-byte ratio). The decoded result is
/// unchanged; only the optional diagnostic is added so the silent skip becomes
/// observable in scan output.
pub(super) fn decode_bytes_to_string_with_diagnostic(bytes: &[u8]) -> (String, Option<String>) {
    if let Some(decoded) = decode_utf16_text(bytes) {
        return (decoded, None);
    }

    match String::from_utf8(bytes.to_vec()) {
        Ok(s) => (s, None),
        Err(e) => {
            let bytes = e.into_bytes();
            if has_binary_control_chars(&bytes) {
                return (String::new(), Some(NEAR_BINARY_SKIP_DIAGNOSTIC.to_string()));
            }
            (bytes.iter().map(|&b| b as char).collect(), None)
        }
    }
}

pub(super) fn is_utf8_text(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
}

fn strip_corrupted_utf16_bom_prefix(bytes: &[u8]) -> &[u8] {
    bytes
        .strip_prefix(CORRUPTED_UTF16_BOM_PREFIX)
        .unwrap_or(bytes)
}

fn decode_utf16_units(bytes: &[u8], is_le: bool, require_text_shape: bool) -> Option<String> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(2) {
        return None;
    }

    let code_units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if is_le {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect();

    let decoded = std::char::decode_utf16(code_units)
        .collect::<Result<String, _>>()
        .ok()?;

    if !require_text_shape {
        return (!decoded.contains('\0')).then_some(decoded);
    }

    if !looks_like_decoded_text(&decoded) {
        return None;
    }

    Some(decoded)
}

pub(super) fn looks_like_decoded_text(decoded: &str) -> bool {
    if decoded
        .chars()
        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return false;
    }

    let visible = decoded
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
        .count();
    if visible < 3 || decoded.contains('\0') {
        return false;
    }

    let alpha = decoded.chars().filter(|ch| ch.is_alphabetic()).count();
    let punctuation = decoded
        .chars()
        .filter(|ch| {
            matches!(
                ch,
                '{' | '}'
                    | '['
                    | ']'
                    | '<'
                    | '>'
                    | '('
                    | ')'
                    | ':'
                    | ';'
                    | ','
                    | '"'
                    | '\''
                    | '/'
                    | '='
                    | '-'
                    | '_'
                    | '#'
                    | '!'
            )
        })
        .count();
    let whitespace = decoded.chars().filter(|ch| ch.is_whitespace()).count();

    let textish = alpha + punctuation + whitespace;
    textish + (visible / 5) >= visible && (alpha > 0 || punctuation >= 2)
}

fn detect_utf16_endianness(bytes: &[u8]) -> Option<bool> {
    let stripped = strip_corrupted_utf16_bom_prefix(bytes);
    if stripped.len() < 4 || !stripped.len().is_multiple_of(2) {
        return None;
    }

    let pair_count = stripped.len() / 2;
    let even_zero = stripped.iter().step_by(2).filter(|&&b| b == 0).count();
    let odd_zero = stripped
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|&&b| b == 0)
        .count();

    let looks_like_be = even_zero * 3 >= pair_count && odd_zero * 6 <= pair_count;
    let looks_like_le = odd_zero * 3 >= pair_count && even_zero * 6 <= pair_count;

    match (looks_like_le, looks_like_be) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        (true, true) => Some(true),
        (false, false) => None,
    }
}

pub(super) fn decode_utf16_text(bytes: &[u8]) -> Option<String> {
    if let Some(decoded) = decode_utf16_bom_text(bytes) {
        return Some(decoded);
    }

    let stripped = strip_corrupted_utf16_bom_prefix(bytes);
    match detect_utf16_endianness(bytes) {
        Some(true) => decode_utf16_units(stripped, true, true),
        Some(false) => decode_utf16_units(stripped, false, true),
        None => None,
    }
}

pub(super) fn decode_utf16_json_text(bytes: &[u8]) -> Option<String> {
    if bytes.len() >= 2 {
        let (is_le, body) = match bytes {
            [0xFF, 0xFE, rest @ ..] => (true, rest),
            [0xFE, 0xFF, rest @ ..] => (false, rest),
            _ => {
                let stripped = strip_corrupted_utf16_bom_prefix(bytes);
                return match detect_utf16_endianness(bytes) {
                    Some(true) => decode_utf16_units(stripped, true, false),
                    Some(false) => decode_utf16_units(stripped, false, false),
                    None => None,
                };
            }
        };

        if body.is_empty() || !body.len().is_multiple_of(2) {
            return None;
        }

        return decode_utf16_units(body, is_le, false);
    }

    None
}

fn decode_utf16_bom_text(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 || !bytes.len().is_multiple_of(2) {
        return None;
    }

    let (is_le, body) = match bytes {
        [0xFF, 0xFE, rest @ ..] => (true, rest),
        [0xFE, 0xFF, rest @ ..] => (false, rest),
        _ => return None,
    };

    if body.is_empty() || body.len() % 2 != 0 {
        return None;
    }

    decode_utf16_units(body, is_le, true)
}

pub(super) fn has_binary_control_chars(bytes: &[u8]) -> bool {
    let control_count = bytes
        .iter()
        .filter(|&&b| b < 0x09 || (b > 0x0D && b < 0x20))
        .count();
    control_count > bytes.len() / BINARY_CONTROL_CHAR_THRESHOLD_DIVISOR
}

pub(super) fn has_decodable_text(bytes: &[u8]) -> bool {
    bytes.is_empty()
        || is_utf8_text(bytes)
        || decode_utf16_text(bytes).is_some()
        || !has_binary_control_chars(bytes)
}

pub(super) fn looks_like_textual_bytes(bytes: &[u8]) -> bool {
    if bytes.is_empty() || is_utf8_text(bytes) {
        return true;
    }
    if let Some(decoded) = decode_utf16_text(bytes) {
        return decoded
            .chars()
            .any(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'));
    }

    let printable_count = bytes
        .iter()
        .filter(|&&b| matches!(b, b'\n' | b'\r' | b'\t') || (0x20..=0x7e).contains(&b))
        .count();
    printable_count * 2 >= bytes.len()
}
