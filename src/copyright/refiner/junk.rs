// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Junk-detection predicates for copyright, holder, and author strings.
//!
//! These classify a candidate string as a false positive (code fragments,
//! markup, generated resources, garbage, prose, etc.).

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::*;

// ─── Junk detection ──────────────────────────────────────────────────────────

/// Return true if `s` matches any known junk copyright pattern.
pub fn is_junk_copyright(s: &str) -> bool {
    if looks_like_structured_copyright_notice_with_year(s) {
        return false;
    }

    COPYRIGHTS_JUNK_PATTERNS.iter().any(|re| re.is_match(s))
        || is_junk_copyright_scan_phrase(s)
        || is_junk_copyright_code_fragment(s)
        || is_junk_copyright_symbol_garbage(s)
        || is_junk_c_sign_path_fragment(s)
        || is_creative_commons_license_prose(s)
}

/// Return true if `s` is a fragment of Creative Commons (or cited treaty)
/// license body text rather than a real copyright statement or holder.
///
/// Full CC public-license texts (CC-BY, CC-BY-SA, etc.) contain legal prose that
/// repeatedly uses the words "copyright", "Licensor", "Similar Rights", and
/// treaty names. The copyright detector extracts fragments of that prose as
/// spurious copyrights and holders.
///
/// "Strong" markers are phrases that appear only in CC/treaty license bodies and
/// never in a real copyright line, so they classify as prose regardless of any
/// embedded year (the WIPO/Berne paragraph cites treaty years). "Weak" markers
/// are shorter CC phrases that are only treated as prose when the candidate
/// carries no copyright year, which keeps genuine notices such as
/// `Copyright (c) 2016 Jane Doe` untouched.
pub(super) fn is_creative_commons_license_prose(s: &str) -> bool {
    const CC_STRONG_PROSE_MARKERS: &[&str] = &[
        "rights granted under",
        "effective technological measures",
        "berne convention",
        "wipo copyright treaty",
        "wipo performances and phonograms",
        "universal copyright convention",
        "rome convention",
        "convention as revised",
        "certain other rights specified in the public",
        "declarations recited in the",
        "arising from limitations or exceptions",
    ];
    const CC_WEAK_PROSE_MARKERS: &[&str] = &[
        "similar rights",
        "copyright and/or",
        "copyright and certain other rights",
        "certain other rights",
        "other rights in the material",
    ];

    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if CC_STRONG_PROSE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return true;
    }

    !has_copyright_year(trimmed)
        && CC_WEAK_PROSE_MARKERS
            .iter()
            .any(|marker| lower.contains(marker))
}

pub(super) fn looks_like_structured_copyright_notice_with_year(s: &str) -> bool {
    static STRUCTURED_NOTICE_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^copyright\s+notice\s*\(\s*(?:19\d{2}|20\d{2})\s*\)\s+.+$")
    });

    STRUCTURED_NOTICE_RE.is_match(s.trim())
}

pub(super) fn has_copyright_year(s: &str) -> bool {
    static COPYRIGHT_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)\b(?:19\d{2}|20\d{2})(?:\s*[-–/]\s*(?:19\d{2}|20\d{2}|\d{2}))?\b",
        )
    });

    COPYRIGHT_YEAR_RE.is_match(s)
}

pub(super) fn is_junk_copyright_scan_phrase(s: &str) -> bool {
    static COPYRIGHT_SCAN_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\bcopyright\s+scan(?:s|ner|ning)?\b"));

    !has_copyright_year(s) && COPYRIGHT_SCAN_RE.is_match(s)
}

pub(super) fn is_junk_c_sign_path_fragment(s: &str) -> bool {
    let Some(tail) = s.trim().strip_prefix("(c)") else {
        return false;
    };

    !has_copyright_year(s) && is_path_like_code_fragment(tail)
}

pub(super) fn is_junk_copyright_code_fragment(s: &str) -> bool {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    let has_windows_versioninfo_markers = contains_windows_versioninfo_token(trimmed);
    let has_code_markers = lower.contains("string?")
        || lower.contains("bool")
        || lower.contains("final ")
        || lower.contains("this.")
        || lower.contains("absl::")
        || lower.contains("strcat(")
        || lower.contains(" main.cc")
        || lower.contains("regexp")
        || lower.contains("match ")
        || lower.contains("replaceallmapped")
        || lower.contains(".startswith")
        || lower.contains("startswith(")
        || lower.contains("formatoutput")
        || lower.contains("$template")
        || lower.contains("icondata")
        || lower.contains("static const")
        || lower.contains("public void")
        || lower.contains("get set")
        || lower.contains("assert.equal")
        || lower.contains("console.writeline")
        || lower.contains("regexoptions.")
        || lower.contains("pass. group")
        || lower.contains("group 0 (")
        || lower.contains("encoding.ascii")
        || lower.contains("clonetextcontent")
        || lower.contains("getobjectforiunknown")
        || contains_embedded_file_reference_prose(trimmed)
        || lower.contains("classifiers")
        || lower.contains("authors.append")
        || lower == "copyright void"
        || trimmed.contains("??")
        || contains_member_access_code_token(trimmed)
        || contains_code_string_literal_fragment(trimmed)
        || contains_unicode_escape_token_run(trimmed)
        || contains_html_entity_decoder_artifact(trimmed)
        || contains_markup_tag_fragment(trimmed)
        || contains_xml_markup_declaration_token(trimmed)
        || contains_regex_or_template_marker(trimmed)
        || has_windows_versioninfo_markers
        || contains_generated_resource_token(trimmed)
        || contains_malformed_spaced_year(trimmed);
    let has_prose_markers = is_obvious_prose_fragment(trimmed);

    if has_windows_versioninfo_markers {
        return true;
    }

    if !lower.starts_with("copyright") {
        if lower.starts_with("(c)") && (has_code_markers || has_prose_markers) {
            return !has_copyright_year(trimmed);
        }
        return (lower.starts_with("not copyrighted") && !has_copyright_year(trimmed))
            || (lower.contains("copyright") && (has_code_markers || has_prose_markers));
    }

    (has_code_markers || has_prose_markers) && !has_copyright_year(trimmed)
}

/// Return true if `s` matches any known junk author pattern.
pub(super) fn is_junk_author(s: &str) -> bool {
    AUTHORS_JUNK_PATTERNS.iter().any(|re| re.is_match(s))
}

/// Return true if `s` matches any known junk holder pattern.
pub(crate) fn is_junk_holder(s: &str) -> bool {
    HOLDERS_JUNK_PATTERNS.iter().any(|re| re.is_match(s))
        || is_junk_holder_code_fragment(s)
        || is_junk_holder_symbol_garbage(s)
        || is_creative_commons_license_prose(s)
        || s.eq_ignore_ascii_case("MIT")
}

pub(super) fn is_junk_holder_code_fragment(s: &str) -> bool {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    let has_windows_versioninfo_markers = contains_windows_versioninfo_token(trimmed);
    let has_code_markers = lower.contains("string?")
        || lower.contains("bool")
        || lower.contains("final ")
        || lower.contains("this.")
        || lower.contains("regexp")
        || lower.contains("match ")
        || lower.contains("replaceallmapped")
        || lower.contains(".startswith")
        || lower.contains("startswith(")
        || lower.contains("$template")
        || lower.contains("::")
        || lower.contains("static const")
        || lower.contains("public void")
        || lower.contains("get set")
        || lower.contains("assert.equal")
        || lower.contains("console.writeline")
        || lower.contains("regexoptions.")
        || lower.contains("pass. group")
        || lower.contains("group 0 (")
        || lower.contains("encoding.ascii")
        || lower.contains("clonetextcontent")
        || lower.contains("getobjectforiunknown")
        || contains_embedded_file_reference_prose(trimmed)
        || lower.contains("icondata")
        || lower.contains("authors.append")
        || lower == "void"
        || looks_like_parenthesized_ui_descriptor(trimmed)
        || contains_member_access_code_token(trimmed)
        || contains_code_call_fragment(trimmed)
        || contains_code_string_literal_fragment(trimmed)
        || contains_unicode_escape_token_run(trimmed)
        || contains_html_entity_decoder_artifact(trimmed)
        || contains_markup_tag_fragment(trimmed)
        || contains_xml_markup_declaration_token(trimmed)
        || contains_regex_or_template_marker(trimmed)
        || has_windows_versioninfo_markers
        || contains_generated_resource_token(trimmed);
    let has_prose_markers = is_obvious_prose_fragment(trimmed);

    has_windows_versioninfo_markers
        || ((has_code_markers || has_prose_markers) && !has_copyright_year(trimmed))
}

pub(super) fn is_junk_holder_symbol_garbage(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.len() < 12 {
        return false;
    }

    let alpha_count = trimmed.chars().filter(|ch| ch.is_alphabetic()).count();
    let ascii_alpha_count = trimmed
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .count();
    let non_ascii_count = trimmed.chars().filter(|ch| !ch.is_ascii()).count();
    let symbol_count = trimmed
        .chars()
        .filter(|ch| !ch.is_alphanumeric() && !ch.is_whitespace())
        .count();
    let token_count = trimmed
        .split_whitespace()
        .filter(|token| token.chars().any(|ch| ch.is_alphanumeric()))
        .count();

    (alpha_count <= 2 && symbol_count >= 6)
        || (trimmed.len() >= 16
            && non_ascii_count >= 12
            && ascii_alpha_count <= 2
            && symbol_count >= 4
            && token_count <= 4)
}

pub(super) fn is_junk_copyright_symbol_garbage(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.len() < 8 || has_copyright_year(trimmed) {
        return false;
    }

    let tail = trimmed
        .strip_prefix("Copyright")
        .unwrap_or(trimmed)
        .trim()
        .strip_prefix("(c)")
        .unwrap_or(trimmed)
        .trim();

    let ascii_alpha_count = tail.chars().filter(|ch| ch.is_ascii_alphabetic()).count();
    let non_ascii_count = tail.chars().filter(|ch| !ch.is_ascii()).count();
    let symbol_count = tail
        .chars()
        .filter(|ch| !ch.is_alphanumeric() && !ch.is_whitespace())
        .count();

    (contains_malformed_spaced_year(trimmed) && !tail.contains('@'))
        || (tail.len() >= 10 && non_ascii_count >= 4 && ascii_alpha_count <= 2 && symbol_count >= 2)
}

pub(super) fn contains_regex_or_template_marker(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.contains("(?")
        || trimmed.contains("?:")
        || trimmed.contains(r"\d")
        || trimmed.contains(r"\s")
        || trimmed.contains(r"\w")
        || trimmed.contains("{{")
        || trimmed.contains("}}")
        || trimmed.contains("${")
        || trimmed.contains("0-9")
        || trimmed.contains("^ ")
        || trimmed.ends_with('$')
        || trimmed.contains(" d+")
        || trimmed.contains(" ?")
}

pub(super) fn contains_html_entity_decoder_artifact(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.contains("u00a0")
        || lower.contains("hellip")
        || lower.contains("x2014")
        || lower.contains("x2f")
        || lower.contains("reg 174")
        || lower.contains("copy 169")
        || lower.contains("&ndash")
        || lower.contains("&mdash")
        || lower.contains("&trade")
        || lower.contains("&copy")
        || lower.contains("&#169")
        || lower.contains("&#174")
}

pub(super) fn normalize_markup_rich_text_fragment(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return s.to_string();
    }

    let lower = trimmed.to_ascii_lowercase();
    let looks_like_markup_rich_text = lower.contains("href=")
        || lower.contains("<a")
        || lower.contains("</a")
        || lower.contains("<br")
        || lower.contains("<p")
        || lower.contains("<td")
        || lower.contains("<i>")
        || lower.contains("&copy")
        || lower.contains("mailto:");
    if !looks_like_markup_rich_text {
        return s.to_string();
    }

    let prepared = prepare_text_line(trimmed);
    if prepared.is_empty() {
        return s.to_string();
    }

    normalize_whitespace(&prepared)
}

pub(super) fn contains_generated_resource_token(s: &str) -> bool {
    static ASSET_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)(?:@(?:1x|2x|3x|4x))?\.(?:png|jpg|jpeg|gif|webp|bmp|ico|icns|svg|ttf|otf|woff2?|img|tmpl|json|xml|yaml|yml|g\.dart)\b",
        )
    });

    let trimmed = s.trim();
    if trimmed.contains(' ') && !trimmed.contains("FileDescription") {
        return false;
    }

    ASSET_RE.is_match(trimmed)
}

pub(super) fn contains_markup_tag_fragment(s: &str) -> bool {
    static MARKUP_TAG_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)</?[a-z][^>]*>|<[!?][^>]*>"));

    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.contains('@')
        || lower.contains("www.")
        || lower.contains(".com")
        || lower.contains(".org")
        || lower.contains(".net")
        || lower.contains(".edu")
        || lower.contains(".gov")
        || lower.contains(".io")
        || lower.contains(".dev")
    {
        return false;
    }

    MARKUP_TAG_RE.is_match(trimmed) || trimmed.contains("&#")
}

pub(super) fn contains_member_access_code_token(s: &str) -> bool {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("www.")
        || lower.contains(".com")
        || lower.contains(".org")
        || lower.contains(".net")
        || lower.contains(".edu")
        || lower.contains(".gov")
        || lower.contains(".io")
        || lower.contains(".dev")
    {
        return false;
    }

    static MEMBER_ACCESS_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"\b(?:[a-z_][A-Za-z0-9_]{1,}\.){1,4}[A-Z][A-Za-z0-9_]{1,}(?:\.[A-Z][A-Za-z0-9_]{1,})*\b",
        )
    });
    static C_STYLE_MEMBER_ACCESS_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"\b[a-z_][A-Za-z0-9_]*->[A-Za-z_][A-Za-z0-9_]*\b"));

    MEMBER_ACCESS_RE.is_match(trimmed) || C_STYLE_MEMBER_ACCESS_RE.is_match(trimmed)
}

pub(super) fn contains_code_string_literal_fragment(s: &str) -> bool {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();

    lower.contains("r'")
        || lower.contains("r\"")
        || lower.contains("\"\\0\"")
        || lower.contains("'\\0'")
}

pub(super) fn looks_like_parenthesized_ui_descriptor(s: &str) -> bool {
    static UI_DESCRIPTOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^\((?:sharp|round|rounded|outline|outlined|filled)\)$")
    });

    UI_DESCRIPTOR_RE.is_match(s.trim())
}

pub(super) fn is_post_refine_copyright_code_fragment(s: &str) -> bool {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();

    contains_windows_versioninfo_token(trimmed)
        || contains_member_access_code_token(trimmed)
        || contains_code_call_fragment(trimmed)
        || contains_code_string_literal_fragment(trimmed)
        || contains_unicode_escape_token_run(trimmed)
        || contains_markup_tag_fragment(trimmed)
        || lower.contains("public void")
        || lower.contains("get set")
        || lower.contains("assert.equal")
        || is_junk_c_sign_code_expression_fragment(trimmed)
}

pub(super) fn contains_unicode_escape_token_run(s: &str) -> bool {
    static UNICODE_ESCAPE_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\bu[0-9a-f]{4}[a-z0-9_]*\b"));

    UNICODE_ESCAPE_RE.is_match(s.trim())
}

pub(super) fn contains_embedded_file_reference_prose(s: &str) -> bool {
    static FILE_REFERENCE_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)\b[A-Za-z0-9_.-]+\.(?:txt|md|rst|yml|yaml|json|xml|html|cs|c|cpp|h|rs)\b",
        )
    });

    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    FILE_REFERENCE_RE.is_match(trimmed)
        && (lower.contains("update ")
            || lower.contains("copy of the")
            || lower.contains("notice file")
            || lower.contains("license file")
            || lower.contains("provide a copy"))
}

pub(super) fn contains_windows_versioninfo_token(s: &str) -> bool {
    static VERSIONINFO_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)\b(?:VALUE\s+)?(?:OriginalFilename|FileDescription|FileVersion|ProductVersion|LegalTrademarks|ProductName|InternalName|CompanyName)\b",
        )
    });
    static VERSIONINFO_FILE_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\b[\p{L}0-9_.-]+\.(?:exe|dll|mui|ocx|sys)\b"));

    let trimmed = s.trim();
    VERSIONINFO_KEY_RE.is_match(trimmed)
        && (trimmed.contains("VALUE ")
            || VERSIONINFO_FILE_RE.is_match(trimmed)
            || trimmed.to_ascii_lowercase().contains("legaltrademarks"))
}

pub(super) fn contains_xml_markup_declaration_token(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.contains("<!element")
        || lower.contains("<!attlist")
        || lower.contains("<!doctype")
        || lower.contains("pcdata")
}

pub(super) fn is_explicit_generic_field_label_token(s: &str) -> bool {
    static GENERIC_FIELD_LABELS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        HashSet::from([
            "action",
            "assignee",
            "branch",
            "credits",
            "current_user",
            "description",
            "direction",
            "options",
            "organization",
            "owner_name",
            "params",
            "placeholder",
            "project",
            "ref",
            "reviewers",
            "schema",
            "sharp",
            "source",
            "text",
            "timeago",
            "toggle-text",
            "tooltip",
            "unique",
            "username",
            "round",
            "rounded",
            "outline",
            "outlined",
            "filled",
        ])
    });

    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.contains('@') {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    GENERIC_FIELD_LABELS.contains(lower.as_str())
}

pub(super) fn looks_like_generic_field_label_shape(s: &str) -> bool {
    static GENERIC_FIELD_LABEL_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"^[a-z][a-z0-9]*(?:[_-][a-z0-9]+){1,4}$"));

    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.contains('@') {
        return false;
    }

    GENERIC_FIELD_LABEL_RE.is_match(trimmed)
}

pub(super) fn looks_like_generic_field_label_token(s: &str) -> bool {
    is_explicit_generic_field_label_token(s) || looks_like_generic_field_label_shape(s)
}

pub(super) fn contains_code_call_fragment(s: &str) -> bool {
    static NATURAL_PAREN_VARIANT_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"\b[a-z][a-z-]{5,}\((?:-)?[a-z-]{1,8}\)(?:$|[^A-Za-z0-9_])")
    });
    static CODE_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?x)
            \b[a-z_][A-Za-z0-9_]*(?:[&.]\w+)*\([^)]*\)
            |\b[a-z_][A-Za-z0-9_]*::[A-Za-z_][A-Za-z0-9_]*
            |\b[A-Za-z_][A-Za-z0-9_]*\s+\.\.\.[A-Za-z_][A-Za-z0-9_]*
            |\b[a-z_][A-Za-z0-9_]*&\.[A-Za-z_][A-Za-z0-9_]*
            |\b[a-z_][A-Za-z0-9_]*:[a-z_][A-Za-z0-9_]*
            ",
        )
    });

    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("http://") || lower.contains("https://") || lower.contains("www.") {
        return false;
    }

    if NATURAL_PAREN_VARIANT_RE.is_match(trimmed)
        && !trimmed.contains("::")
        && !trimmed.contains(':')
        && !trimmed.contains('&')
        && !trimmed.contains('_')
    {
        return false;
    }

    CODE_CALL_RE.is_match(trimmed)
}

pub(super) fn looks_like_translation_or_ui_phrase(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty()
        || has_copyright_year(trimmed)
        || trimmed.contains('@')
        || trimmed.contains("http://")
        || trimmed.contains("https://")
    {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("msgid") || lower.contains("msgstr") {
        return true;
    }

    let has_ui_legal_noun = lower.contains("copyright")
        || lower.contains("trademark")
        || lower.contains("placeholder")
        || lower.contains("schema")
        || lower.contains("project")
        || lower.contains("credits");
    if !has_ui_legal_noun {
        return false;
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    words.len() <= 8
        && words.iter().all(|word| {
            word.chars().all(|ch| {
                ch.is_ascii_lowercase()
                    || ch.is_ascii_digit()
                    || matches!(ch, '_' | '-' | ',' | '.' | ':' | ';' | '/' | '\'' | '’')
            })
        })
}

// Shared matcher for a lowercase `handle <email>` string, used by both the
// strip and predicate helpers below. Defined once so the pattern is compiled a
// single time rather than once per function.
static HANDLE_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    compile_static_regex(
        r"(?i)^(?P<name>[a-z0-9][a-z0-9._-]{1,63})\s*<\s*(?P<email>[^>\s]+@[^>\s]+)\s*>\s*$",
    )
});

pub(super) fn strip_trailing_lowercase_handle_angle_email(s: &str) -> String {
    let trimmed = s.trim();
    let Some(cap) = HANDLE_EMAIL_RE.captures(trimmed) else {
        return s.to_string();
    };

    cap.name("name")
        .map(|m| m.as_str().trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn is_lowercase_handle_angle_email(s: &str) -> bool {
    HANDLE_EMAIL_RE.is_match(s.trim())
}

pub(super) fn strip_trailing_everyone_is_permitted_to_copy_clause(s: &str) -> String {
    static EVERYONE_PERMITTED_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^(?P<prefix>.+?)\.\s+Everyone\s+is\s+permitted\s+to\s+copy\b.*$")
    });

    let trimmed = s.trim();
    let Some(cap) = EVERYONE_PERMITTED_RE.captures(trimmed) else {
        return s.to_string();
    };

    cap.name("prefix")
        .map(|m| m.as_str().trim().to_string())
        .filter(|prefix| !prefix.is_empty())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn strip_trailing_reserved_font_name_clause(s: &str) -> String {
    static RESERVED_FONT_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^(?P<prefix>.+?)
            (?:
                \s*,\s*
              | \s+\(\s*
              | \s+\(\s*,\s*
              | \s+
            )
            with\s+(?:no\s+)?reserved\s+font\s+name\b.*$
            ",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = RESERVED_FONT_NAME_RE.captures(trimmed) else {
        return s.to_string();
    };

    cap.name("prefix")
        .map(|m| {
            m.as_str()
                .trim_end_matches(&[',', ';', ':', ' ', '('][..])
                .trim()
                .to_string()
        })
        .filter(|prefix| !prefix.is_empty())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn looks_like_lowercase_enum_blob(s: &str) -> bool {
    static LOWERCASE_ENUM_BLOB_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"^[a-z][a-z0-9_-]*(?:\s+\d+)?(?:,\s*[a-z][a-z0-9_-]*(?:\s+\d+)?){1,6}$",
        )
    });

    LOWERCASE_ENUM_BLOB_RE.is_match(s.trim())
}

pub(super) fn looks_like_lowercase_company_suffix_holder(s: &str) -> bool {
    static LOWERCASE_COMPANY_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^[a-z][a-z0-9._-]*
            (?:\s+[a-z0-9._-]+)*
            ,\s*
            (?:inc|corp|co|ltd|llc|gmbh|sarl|bv|b\.v|ag)
            \.?$
            ",
        )
    });

    LOWERCASE_COMPANY_SUFFIX_RE.is_match(s.trim())
}

pub(super) fn is_junk_c_sign_code_expression_fragment(s: &str) -> bool {
    static LEADING_LOWERCASE_MEMBER_ACCESS_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"^[a-z_]{1,2}[.:,]"));

    let trimmed = s.trim();
    let Some(tail) = trimmed.strip_prefix("(c)") else {
        return false;
    };

    if has_copyright_year(trimmed) {
        return false;
    }

    let tail = tail.trim();
    let first_word = tail
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_');
    let looks_like_lowercase_member_access = LEADING_LOWERCASE_MEMBER_ACCESS_RE.is_match(tail);

    matches!(first_word, "and" | "const" | "let" | "puts" | "var")
        || looks_like_lowercase_member_access
        || contains_code_call_fragment(tail)
        || looks_like_lowercase_enum_blob(tail)
}

pub(super) fn looks_like_embedded_c_sign_code_fragment(s: &str) -> bool {
    static EMBEDDED_C_SIGN_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)\b[A-Za-z_][A-Za-z0-9_]*\(\s*c\s*\)\s*(?:;|=|->)")
    });

    let trimmed = s.trim();
    !has_copyright_year(trimmed) && EMBEDDED_C_SIGN_CALL_RE.is_match(trimmed)
}

pub(super) fn is_copyright_edit_note(s: &str) -> bool {
    static COPYRIGHT_EDIT_NOTE_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^copyright\s+sections?\s+were\s+added$"));

    COPYRIGHT_EDIT_NOTE_RE.is_match(s.trim())
}

pub(super) fn contains_malformed_spaced_year(s: &str) -> bool {
    static SPACED_YEAR_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"\b(?:19|20)\s+\d{2}\b|\b\d{3}\s+\d{1,2}\b"));

    SPACED_YEAR_RE.is_match(s)
}

pub(super) fn is_obvious_prose_fragment(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty()
        || has_copyright_year(trimmed)
        || trimmed.contains('@')
        || trimmed.contains("http://")
        || trimmed.contains("https://")
    {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("not by ") {
        return false;
    }
    if lower.contains("code sample for")
        || lower.contains("tests using the examples provided by")
        || lower.ends_with("row header")
        || lower.ends_with("column header")
    {
        return true;
    }

    let phrase_markers = [
        "directing the reader",
        "for use as part of",
        "original works produced specifically for use as",
        "confusion over",
        "original source",
    ];
    if phrase_markers.iter().any(|marker| lower.contains(marker)) {
        return true;
    }

    let words: Vec<&str> = lower.split_whitespace().collect();
    if words.len() < 3 {
        return false;
    }

    matches!(
        words.first().copied(),
        Some("comment" | "comments" | "referencing" | "resulting" | "not")
    )
}

pub(super) fn is_trailing_component_descriptor(desc: &str) -> bool {
    let desc_lower = desc.to_ascii_lowercase();
    desc_lower.contains("noise") || desc_lower.ends_with("and others")
}

pub(crate) fn is_path_like_code_fragment(s: &str) -> bool {
    static PATH_LIKE_CODE_FRAGMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?x)
            ^
            [A-Za-z_$][A-Za-z0-9_$]*
            (?:
                /[A-Za-z_$][A-Za-z0-9_$]*
              | \.[A-Za-z_$][A-Za-z0-9_$]*
              | \$[A-Za-z_$][A-Za-z0-9_$]*
            )+
            $
            ",
        )
    });

    PATH_LIKE_CODE_FRAGMENT_RE.is_match(s.trim())
}
