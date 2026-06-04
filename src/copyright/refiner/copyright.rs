// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Copyright-string refinement: the `refine_copyright` orchestrator plus its
//! clause strippers and copyright-specific junk predicates.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::*;

/// Refine a detected copyright string. Returns `None` if the result is empty.
pub fn refine_copyright(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    let original = normalize_whitespace(&normalize_markup_rich_text_fragment(s));
    let original_lower = original.to_ascii_lowercase();
    if is_placeholder_or_code_junk_copyright(&original, &original_lower) {
        return None;
    }
    if contains_windows_versioninfo_token(&original)
        || (contains_xml_markup_declaration_token(&original) && !has_copyright_year(&original))
    {
        return None;
    }
    let mut c = original.clone();
    c = strip_known_copyright_wrappers(&c);
    c = strip_trailing_quote_before_email(&c);
    c = normalize_b_dot_angle_emails(&c);
    c = strip_nickname_quotes(&c);
    c = strip_leading_author_label_in_copyright(&c);
    c = strip_leading_duplicate_phrase_before_embedded_copyright(&c);
    c = strip_leading_licensed_material_of(&c);
    c = strip_leading_version_number_before_c(&c);
    c = strip_trailing_parenthesized_descriptor_after_by_holder(&c);
    c = strip_contributor_parens_after_org(&c);
    c = strip_trailing_paren_email_after_c_by(&c);
    c = strip_trailing_for_clause_after_email(&c);
    c = strip_trailing_at_affiliation(&c);
    c = strip_trailing_single_letter_obfuscated_email_phrase(&c);
    c = strip_trailing_obfuscated_email_after_dash(&c);
    c = strip_url_token_between_years_and_holder(&c);
    c = strip_obfuscated_angle_emails(&c);
    c = strip_angle_bracketed_www_domains_without_by(&c);
    c = strip_leading_simple_copyright_prefixes(&c);
    c = normalize_comma_spacing(&c);
    c = normalize_angle_bracket_comma_spacing(&c);
    c = strip_trailing_secondary_angle_email_after_comma(&c);
    c = strip_trailing_short_surname_paren_list_in_copyright(&c);
    c = strip_trailing_et_al(&c);
    c = strip_trailing_authors_clause(&c);
    c = strip_trailing_document_authors_clause(&c);
    c = strip_trailing_parenthesized_descriptor_after_by_holder(&c);
    c = strip_trailing_amp_authors(&c);
    c = strip_trailing_x509_dn_fields(&c);
    c = strip_some_punct(&c);
    c = strip_solo_quotes(&c);
    // strip trailing slashes, tildes, spaces
    c = c.trim_matches(&['/', ' ', '~'][..]).to_string();
    c = strip_all_unbalanced_parens(&c);
    c = remove_some_extra_words_and_punct(&c);
    c = strip_trailing_incomplete_as_represented_by(&c);
    c = normalize_whitespace(&c);
    c = strip_leading_js_project_version(&c);
    c = remove_dupe_copyright_words(&c);
    c = strip_trailing_portions_of(&c);
    c = strip_trailing_paren_identifier(&c);
    c = strip_trailing_company_name_placeholder(&c);
    c = strip_trailing_company_co_ltd(&c);
    c = strip_trailing_heavily_based_clause(&c);
    c = strip_trailing_obfuscated_email_in_angle_brackets_after_copyright(&c);
    c = strip_trailing_linux_ag_location_in_copyright(&c);
    c = strip_trailing_locale_timestamp_before_terminal_year_in_copyright(&c);
    c = strip_trailing_by_person_clause_after_company(&c);
    c = strip_trailing_division_of_company_suffix(&c);
    c = strip_trailing_contributor_clause(&c);
    c = strip_trailing_contact_clause(&c);
    c = strip_trailing_paren_at_without_domain(&c);
    c = strip_trailing_inc_after_today_year_placeholder(&c);
    c = truncate_trailing_boilerplate(&c);
    c = strip_trailing_everyone_is_permitted_to_copy_clause(&c);
    c = strip_trailing_all_rights_reserved_clause(&c);
    c = strip_trailing_reserved_font_name_clause(&c);
    c = strip_trailing_author_label(&c);
    c = strip_trailing_credit_file_reference_clause(&c);
    c = strip_trailing_isc_after_inc(&c);
    c = strip_trailing_caps_after_company_suffix(&c);
    c = strip_trailing_javadoc_tags(&c);
    c = strip_trailing_batch_comment_marker(&c);
    c = strip_trailing_bug_reports_after_year_only_copyright(&c);
    c = strip_prefixes(&c, &HashSet::from(["by", "c"]));
    c = c.trim().to_string();
    c = c.trim_matches('+').to_string();
    c = c.trim_matches(&[',', ' '][..]).to_string();
    c = strip_balanced_edge_parens(&c).to_string();
    c = strip_suffixes(&c, &COPYRIGHTS_SUFFIXES);
    c = c.trim_end_matches(&[',', ' '][..]).to_string();
    c = strip_trailing_ampas_acronym(&c);
    c = strip_trailing_period(&c);
    c = strip_independent_jpeg_groups_software_tail(&c);
    c = strip_trailing_original_authors(&c);
    c = strip_trailing_mountain_view_ca(&c);
    c = strip_trailing_comma_after_respective_authors(&c);
    c = strip_trailing_ansi_escape_suffix(&c);
    c = c.trim_end_matches(char::is_whitespace).to_string();
    c = c.trim_matches('\'').to_string();
    c = wrap_trailing_and_urls_in_parens(&c);
    c = strip_trailing_url_slash(&c);
    c = strip_trailing_or_suffix(&c);
    c = truncate_long_words(&c);
    c = strip_trailing_single_digit_token(&c);
    c = strip_trailing_period(&c);
    let result = c.trim().to_string();

    static SOFTWARE_COPYRIGHT_C_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?ix)\bsoftware\s+copyright\s*\(c\)\s*(?:19\d{2}|20\d{2})\b")
    });
    if SOFTWARE_COPYRIGHT_C_RE.is_match(original.as_str())
        && !result.to_ascii_lowercase().contains("copyright")
    {
        let restored = strip_trailing_period(&original);
        let restored = restored.trim().to_string();
        if !restored.is_empty() {
            return Some(restored);
        }
    }

    static YEAR_RANGE_ANGLE_EMAIL_COPY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
                ^copyright\s*\(c\)\s+
                (?:19\d{2}|20\d{2}|\?\?\?\?)
                \s*[-–/]\s*(?:\d{2,4}|\?\?\?\?)
                (?:\s*,\s*(?:19\d{2}|20\d{2}|\?\?\?\?))*
                \s+.+?<[^>\s]+@[^>\s]+>\.?$
            ",
        )
    });
    if YEAR_RANGE_ANGLE_EMAIL_COPY_RE.is_match(original.as_str()) && !result.contains('@') {
        let restored = strip_trailing_period(&original);
        let restored = restored.trim().to_string();
        if !restored.is_empty() {
            return Some(restored);
        }
    }

    static YEAR_ONLY_WITH_OBF_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)^copyright\s*\(c\)\s*(?:19\d{2}|20\d{2})\s+[a-z0-9][a-z0-9._-]{0,63}\s+at\s+[a-z0-9][a-z0-9._-]{0,63}\s+dot\s+[a-z]{2,12}$",
        )
    });
    if YEAR_ONLY_WITH_OBF_EMAIL_RE.is_match(result.as_str()) {
        return None;
    }

    let result_upper = result.to_ascii_uppercase();
    if result_upper.contains("COPYRIGHT")
        && result_upper.contains("YEAR")
        && (result_upper.contains("YOUR NAME") || result_upper.contains("ORGANIZATION"))
    {
        return None;
    }
    if looks_like_document_form_reference(&result) {
        return None;
    }
    if is_post_refine_copyright_code_fragment(&result)
        || is_explicit_junk_copyright_phrase(&result)
        || is_junk_copyright_of_header(&result)
        || is_junk_copyrighted_works_header(&result)
        || is_junk_copyrighted_software_phrase(&result)
    {
        return None;
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

pub(super) fn is_explicit_junk_copyright_phrase(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "copyright exclude"
            | "copyright doctrines of fair use, fair dealing, or other equivalents"
            | "copyright doctrines of fair use, fair dealing, or other equivalents."
            | "copyright licenses specified in the"
            | "copyright in its"
            | "copyright purposes"
            | "copyright sections were added"
            | "copyright c- core core"
            | "copyright applying to the plugin. if"
    ) || lower.starts_with("copyright purposes.")
        || is_placeholder_or_code_junk_copyright(s, &lower)
}

pub(super) fn strip_known_copyright_wrappers(s: &str) -> String {
    static VALUE_LEGALCOPYRIGHT_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r#"(?ix)
            ^VALUE\s+"LegalCopyright"\s*,\s*"(?P<value>[^"]+)"
            (?:\s+"\\0")?\s*$
            "#,
        )
    });
    static ASSIGNMENT_COPYRIGHT_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r#"(?ix)
            ^(?:PRODUCT_COPYRIGHT|INFOPLIST_KEY_NSHumanReadableCopyright)
            \s*=\s*(?P<value>.+?)\s*;?\s*$
            "#,
        )
    });
    static PLAIN_COPYRIGHT_ASSIGNMENT_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r#"(?ix)^copyright\s*=\s*(?P<value>.+?)\s*;?\s*$"#));
    static APPLICATION_LEGALESE_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r#"(?ix)^applicationLegalese\s*:\s*(?P<value>.+?)\s*,?\s*$"#)
    });
    static MARKUP_TEXT_COPYRIGHT_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r#"(?ix)
            \btext\s*=\s*(?:"(?P<dq>[^"]+)"|'(?P<sq>[^']+)')
            "#,
        )
    });

    let trimmed = s.trim();
    if let Some(captures) = VALUE_LEGALCOPYRIGHT_RE.captures(trimmed) {
        let value = captures
            .name("value")
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim();
        if !value.is_empty() {
            return prepare_text_line(value).trim().to_string();
        }
    }

    for regex in [&*ASSIGNMENT_COPYRIGHT_RE, &*APPLICATION_LEGALESE_RE] {
        if let Some(captures) = regex.captures(trimmed) {
            let value = captures
                .name("value")
                .map(|m| m.as_str())
                .unwrap_or("")
                .trim()
                .trim_matches(&['\'', '"'][..]);
            if value.starts_with("Copyright") || value.starts_with('©') {
                return prepare_text_line(value).trim().to_string();
            }
        }
    }

    if let Some(captures) = PLAIN_COPYRIGHT_ASSIGNMENT_RE.captures(trimmed) {
        let value = captures
            .name("value")
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim()
            .trim_matches(&['\'', '"'][..]);
        if !value.is_empty() {
            return synthesize_copyright_from_assignment_value(value);
        }
    }

    if let Some(captures) = MARKUP_TEXT_COPYRIGHT_RE.captures(trimmed) {
        let value = captures
            .name("dq")
            .or_else(|| captures.name("sq"))
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim();
        if value.starts_with("Copyright") || value.starts_with('©') {
            return prepare_text_line(value).trim().to_string();
        }
    }

    s.to_string()
}

pub(super) fn strip_trailing_all_rights_reserved_clause(s: &str) -> String {
    let Some(prefix) = all_rights_reserved_prefix(s) else {
        return s.to_string();
    };
    let lower = prefix.to_ascii_lowercase();
    if prefix.is_empty()
        || !(lower.starts_with("copyright") || lower.starts_with("(c)") || lower.starts_with('©'))
    {
        return s.to_string();
    }

    prefix
        .trim_end_matches(&[' ', '.', ',', ';', ':'][..])
        .to_string()
}

pub(super) fn strip_trailing_no_rights_reserved_clause(s: &str) -> String {
    static NO_RIGHTS_RESERVED_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?ix)^(?P<prefix>.+?)\.?\s+no\s+rights\s+reserved\.?$")
    });

    let trimmed = s.trim();
    let Some(captures) = NO_RIGHTS_RESERVED_RE.captures(trimmed) else {
        return s.to_string();
    };

    captures
        .name("prefix")
        .map(|m| {
            m.as_str()
                .trim()
                .trim_end_matches(&[',', ';', ':', '.'][..])
                .to_string()
        })
        .filter(|prefix| !prefix.is_empty())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn strip_trailing_all_rights_reserved_holder_clause(s: &str) -> String {
    let Some(prefix) = all_rights_reserved_prefix(s) else {
        return s.to_string();
    };
    if prefix.is_empty() || !prefix_has_holder_words(prefix.as_str()) {
        return s.to_string();
    }

    prefix
        .trim_end_matches(&[',', ';', ':', '.', ' '][..])
        .trim()
        .to_string()
}

pub(super) fn all_rights_reserved_prefix(s: &str) -> Option<String> {
    static ALL_RIGHTS_RESERVED_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?ix)^(?P<prefix>.+?)\.?\s+all\s+rights\s+reserved\.?$")
    });

    let trimmed = s.trim();
    let captures = ALL_RIGHTS_RESERVED_RE.captures(trimmed)?;

    Some(
        captures
            .name("prefix")
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
    )
}

pub(super) fn strip_trailing_obfuscated_email_after_dash(s: &str) -> String {
    static TRAILING_DASH_OBF_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)^(?P<prefix>.+?)\s*(?:--+|-)\s*(?P<user>[a-z0-9][a-z0-9._-]{0,63})\s*(?:\[\s*at\s*\]|at)\s*(?P<host>[a-z0-9][a-z0-9._-]{0,63})\s*(?:\[\s*dot\s*\]|dot)\s*(?P<tld>[a-z]{2,12})\s*$",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = TRAILING_DASH_OBF_EMAIL_RE.captures(trimmed) else {
        return s.to_string();
    };

    let prefix = cap
        .name("prefix")
        .map(|m| m.as_str().trim_end_matches(&[' ', '-', '–', '—'][..]))
        .unwrap_or(trimmed)
        .trim();
    let user = cap.name("user").map(|m| m.as_str()).unwrap_or("").trim();
    let host = cap.name("host").map(|m| m.as_str()).unwrap_or("").trim();
    let tld = cap.name("tld").map(|m| m.as_str()).unwrap_or("").trim();

    let prefix_lower = prefix.to_ascii_lowercase();
    let holderish_word_count = prefix
        .split_whitespace()
        .filter(|word| word.chars().any(|ch| ch.is_alphabetic()))
        .count();
    if prefix_lower.starts_with("copyright")
        && !user.is_empty()
        && !host.is_empty()
        && !tld.is_empty()
        && holderish_word_count >= 3
    {
        return format!("{prefix} - {user} at {host} dot {tld}");
    }

    prefix.to_string()
}

pub(super) fn strip_trailing_single_letter_obfuscated_email_phrase(s: &str) -> String {
    static SINGLE_LETTER_OBF_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^(?P<prefix>.+?)
            \s+
            (?P<user>[a-z0-9][a-z0-9._-]{0,63})
            \s+a\s+
            (?P<domain>[a-z0-9][a-z0-9._-]{0,63})
            \s+
            (?P<tld>com|org|net|edu|gov|mil|io|co|us|uk|de|fr|jp|cn|in|info|biz|me|tv|ca|au)
            \s*$",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = SINGLE_LETTER_OBF_RE.captures(trimmed) else {
        return s.to_string();
    };

    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    let user = cap.name("user").map(|m| m.as_str()).unwrap_or("").trim();
    let domain = cap.name("domain").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() || user.is_empty() || domain.is_empty() {
        return s.to_string();
    }

    let prefix_tokens: HashSet<String> = prefix
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                .to_ascii_lowercase()
        })
        .filter(|token| token.len() >= 2)
        .collect();

    if prefix_tokens.contains(&user.to_ascii_lowercase())
        && prefix_tokens.contains(&domain.to_ascii_lowercase())
    {
        return prefix.to_string();
    }

    s.to_string()
}

pub(super) fn strip_trailing_heavily_based_clause(s: &str) -> String {
    static HEAVILY_BASED_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?)\s+Heavily(?:\s+based\b.*)?$"));

    let trimmed = s.trim();
    let Some(cap) = HEAVILY_BASED_RE.captures(trimmed) else {
        return s.to_string();
    };

    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }

    let lower = prefix.to_ascii_lowercase();
    if lower.starts_with("copyright") || lower.starts_with("(c)") || prefix_has_holder_words(prefix)
    {
        return prefix.to_string();
    }

    s.to_string()
}

pub(super) fn strip_trailing_credit_file_reference_clause(s: &str) -> String {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("copyright") || lower.starts_with("(c)")) {
        return s.to_string();
    }

    for marker in [
        " see authors file",
        " see author file",
        " see credits file",
        " see credit file",
        " refer to authors file",
        " refer to credits file",
        " consult authors file",
        " consult credits file",
    ] {
        if let Some(index) = lower.find(marker) {
            let prefix = trimmed[..index]
                .trim_end_matches(&[',', ';', ':', ' '][..])
                .trim();
            if prefix.chars().any(|ch| ch.is_ascii_digit()) {
                return prefix.to_string();
            }
        }
    }

    s.to_string()
}

pub(super) fn looks_like_credit_file_reference_note(s: &str) -> bool {
    static CREDIT_FILE_REFERENCE_NOTE_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^
            (?:see|see\ also|refer\ to|consult)
            \s+
            (?:the\s+)?
            (?:authors?|credits?)
            \s+file
            $
            ",
        )
    });

    CREDIT_FILE_REFERENCE_NOTE_RE.is_match(s.trim())
}

pub(super) fn looks_like_document_form_reference(s: &str) -> bool {
    static DOCUMENT_FORM_REFERENCE_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^(?:copyright\s+)?office\s+[A-Z]{1,6}-\d{1,6}[A-Z]?$")
    });

    DOCUMENT_FORM_REFERENCE_RE.is_match(s.trim())
}

pub(super) fn strip_trailing_secondary_angle_email_after_comma(s: &str) -> String {
    static TRAILING_SECOND_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"^(?P<prefix>.+?<[^>\s]*@[^>\s]*>)\s*,\s*<[^>\s]*@[^>\s]*>\s*$")
    });

    let trimmed = s.trim();
    let Some(cap) = TRAILING_SECOND_EMAIL_RE.captures(trimmed) else {
        return s.to_string();
    };

    let full = cap.get(0).map(|m| m.as_str()).unwrap_or(trimmed);
    let emails: Vec<&str> = full
        .split('<')
        .skip(1)
        .filter_map(|p| p.split_once('>').map(|(e, _)| e.trim()))
        .filter(|e| e.contains('@'))
        .collect();
    if emails.len() >= 2 {
        let a = emails[0].to_ascii_lowercase();
        let b = emails[1].to_ascii_lowercase();
        if a != b {
            return s.to_string();
        }
    }

    cap.name("prefix")
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn normalize_b_dot_angle_emails(s: &str) -> String {
    static B_DOT_EMAIL_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)<\s*b\.(?P<email>[^>\s]*@[^>\s]+)\s*>"));
    B_DOT_EMAIL_RE.replace_all(s, ".${email}").into_owned()
}

pub(super) fn strip_url_token_between_years_and_holder(s: &str) -> String {
    static BETWEEN_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>Copyright\s*\(c\)\s*[-,\s0-9]{4,32})\s+https?://\S+\s+(?P<tail>\p{L}.+)$",
        )
    });
    let trimmed = s.trim();
    if let Some(cap) = BETWEEN_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        let tail = cap.name("tail").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() && !tail.is_empty() {
            return normalize_whitespace(&format!("{prefix} {tail}"));
        }
    }
    s.to_string()
}

pub(super) fn wrap_trailing_and_urls_in_parens(s: &str) -> String {
    static TRAILING_URLS_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>.+?)\s+(?P<urls>https?://\S+\s+and\s+https?://\S+)\s*$",
        )
    });
    let trimmed = s.trim();
    let Some(cap) = TRAILING_URLS_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap
        .name("prefix")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim_end();
    let urls = cap.name("urls").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() || urls.is_empty() {
        return s.to_string();
    }
    if urls.starts_with('(') {
        return s.to_string();
    }
    format!("{prefix} ({urls})")
}

pub(super) fn strip_obfuscated_angle_emails(s: &str) -> String {
    static OBF_ANGLE_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\s*<[^>]*(?:\[at\]|\bat\b)[^>]*>\s*"));
    let trimmed = s.trim();
    if !(trimmed.contains("<") && trimmed.contains(">")) {
        return s.to_string();
    }
    let out = OBF_ANGLE_RE.replace_all(trimmed, " ").into_owned();
    normalize_whitespace(&out)
}

pub(super) fn strip_trailing_linux_ag_location_in_copyright(s: &str) -> String {
    static LINUX_AG_COPY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>Copyright\b.*?\s)(?P<name>\S+)\s+Linux\s+AG\s*,\s*[^,]{2,64}\s*,\s*[^,]{2,64}\s*$",
        )
    });
    let trimmed = s.trim();
    if let Some(cap) = LINUX_AG_COPY_RE.captures(trimmed) {
        let prefix = cap
            .name("prefix")
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim_end();
        let name = cap.name("name").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() && !name.is_empty() {
            return format!("{prefix} {name}");
        }
    }
    s.to_string()
}

pub(super) fn strip_trailing_locale_timestamp_before_terminal_year_in_copyright(s: &str) -> String {
    static LOCALE_TIMESTAMP_COPY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^(?P<prefix>.+?),\s*
            [a-z]{3}\s+[a-z]{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}\s+[A-Z]{2,5}
            (?:\s+(?P<year>\d{4}))?\s*$
            ",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = LOCALE_TIMESTAMP_COPY_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }
    let lower = prefix.to_ascii_lowercase();
    if !(lower.starts_with("copyright") || lower.starts_with("(c)") || lower.starts_with('©')) {
        return s.to_string();
    }
    if let Some(year) = cap
        .name("year")
        .map(|m| m.as_str())
        .filter(|y| !y.is_empty())
    {
        return format!("{} {}", prefix.trim_end_matches(&[',', ' '][..]), year);
    }
    prefix.trim_end_matches(&[',', ' '][..]).to_string()
}

pub(super) fn strip_trailing_ansi_escape_suffix(s: &str) -> String {
    static ANSI_ESCAPE_SUFFIX_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?ix)\s+x1b(?:\s+\d+(?:;\d+)*[a-z])+\s*$"));

    ANSI_ESCAPE_SUFFIX_RE.replace(s, "").trim().to_string()
}

pub(super) fn strip_trailing_quote_before_email(s: &str) -> String {
    static TRAILING_QUOTE_BEFORE_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<head>.*?\b[\p{L}])'\s+(?P<email><[^>\s]*@[^>\s]+>|[^\s<>]*@[^\s<>]+)(?P<tail>.*)$",
        )
    });

    let trimmed = s.trim();
    if !trimmed.contains('@') {
        return s.to_string();
    }
    let Some(cap) = TRAILING_QUOTE_BEFORE_EMAIL_RE.captures(trimmed) else {
        return s.to_string();
    };
    let head = cap.name("head").map(|m| m.as_str()).unwrap_or("");
    let email = cap.name("email").map(|m| m.as_str()).unwrap_or("");
    let tail = cap.name("tail").map(|m| m.as_str()).unwrap_or("");
    normalize_whitespace(&format!("{head} {email}{tail}"))
}

pub(super) fn strip_nickname_quotes(s: &str) -> String {
    static NICK_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?P<first>\b[\p{Lu}][\p{L}'-]+)\s+'(?P<nick>[A-Za-z]{2,20})'\s+(?P<last>\b[\p{Lu}][\p{L}'-]+)",
        )
    });
    NICK_RE
        .replace_all(s, "${first} ${nick} ${last}")
        .into_owned()
}

pub(super) fn strip_trailing_for_clause_after_email(s: &str) -> String {
    static COMPANY_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)(?:ab|ag|aps|a/s|as|inc\.?|corp\.?|corporation|company|co\.?|co\.,?|ltd\.?|limited|llc|gmbh|kg|oy|oyj|s\.?a\.?|s\.?r\.?o\.?|bv|nv)\s*$",
        )
    });

    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.contains(" for ") {
        return s.to_string();
    }
    if !lower.starts_with("copyright") {
        return s.to_string();
    }
    if !trimmed.contains('@') {
        return s.to_string();
    }
    let Some((head, _tail)) = trimmed.split_once(" for ") else {
        return s.to_string();
    };

    if let Some((_, tail)) = trimmed.split_once(" for ") {
        let tail = tail.trim();
        if tail.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            let word_count = tail.split_whitespace().count();
            let lower_tail = tail.to_ascii_lowercase();
            let looks_like_affiliation = word_count >= 3
                && (lower_tail.contains("laboratory")
                    || lower_tail.contains("computer science")
                    || lower_tail.contains("facility")
                    || lower_tail.contains("institute")
                    || lower_tail.contains("university")
                    || lower_tail.contains("department")
                    || lower_tail.contains("center"));
            let looks_like_company = word_count >= 2 && COMPANY_SUFFIX_RE.is_match(tail);
            if looks_like_affiliation || looks_like_company {
                return s.to_string();
            }
        }
    }
    head.trim_end().to_string()
}

pub(super) fn is_placeholder_or_code_junk_copyright(original: &str, _original_lower: &str) -> bool {
    static COPYRIGHT_HOLDER_PLACEHOLDER_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^copyright
            (?:\s*\(c\))?
            (?:\s+(?:19\d{2}|20\d{2})(?:-(?:19\d{2}|20\d{2}))?)?
            \s+
            [\p{L}0-9._-]+(?:'s)?
            \s+copyright\s+holder
            $",
        )
    });

    looks_like_embedded_c_sign_code_fragment(original)
        || is_copyright_edit_note(original)
        || COPYRIGHT_HOLDER_PLACEHOLDER_RE.is_match(original.trim())
}

pub(super) fn strip_trailing_at_affiliation(s: &str) -> String {
    let trimmed = s.trim();
    if !trimmed.to_ascii_lowercase().starts_with("copyright") {
        return s.to_string();
    }
    let Some((head, tail)) = trimmed.split_once(" @ ") else {
        return s.to_string();
    };
    let tail = tail.trim();
    if tail.is_empty() {
        return s.to_string();
    }
    if tail.contains('@') {
        return s.to_string();
    }
    if tail.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        return head.trim_end().to_string();
    }
    s.to_string()
}

pub(super) fn strip_trailing_paren_at_without_domain(s: &str) -> String {
    static TRAILING_PAREN_AT_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^(?P<prefix>.+?)\s*\(\s*(?P<inner>[^)]*\bat\b[^)]*)\)\s*$")
    });

    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("copyright") || lower.starts_with("(c)")) {
        return s.to_string();
    }

    let Some(cap) = TRAILING_PAREN_AT_RE.captures(trimmed) else {
        return s.to_string();
    };
    let inner = cap.name("inner").map(|m| m.as_str()).unwrap_or("").trim();
    if inner.is_empty() {
        return s.to_string();
    }

    let inner_lower = inner.to_ascii_lowercase();
    if inner.contains('@') || inner.contains('.') || inner_lower.contains(" dot ") {
        return s.to_string();
    }

    cap.name("prefix")
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn strip_trailing_inc_after_today_year_placeholder(s: &str) -> String {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.contains("today.year") || lower.contains("current_year")) {
        return s.to_string();
    }
    if !(lower.ends_with(" inc.") || lower.ends_with(" inc")) {
        return s.to_string();
    }
    let prefix = trimmed
        .trim_end_matches('.')
        .trim_end_matches(|c: char| c.is_whitespace())
        .strip_suffix("Inc")
        .or_else(|| trimmed.strip_suffix("Inc."));
    let Some(prefix) = prefix else {
        return s.to_string();
    };
    prefix.trim_end().to_string()
}

pub(super) fn strip_trailing_obfuscated_email_in_angle_brackets_after_copyright(s: &str) -> String {
    static OBFUSCATED_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>copyright\b.+?)\s*<[^>]*\bat\b[^>]*\bdot\b[^>]*>\s*$",
        )
    });

    let trimmed = s.trim();
    if !trimmed
        .get(.."Copyright".len())
        .is_some_and(|p| p.eq_ignore_ascii_case("Copyright"))
    {
        return s.to_string();
    }

    let Some(cap) = OBFUSCATED_TAIL_RE.captures(trimmed) else {
        return s.to_string();
    };
    cap.name("prefix")
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn strip_trailing_author_label(s: &str) -> String {
    static TRAILING_AUTHOR_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"\s+(?:Author|AUTHOR)\b"));
    let Some(m) = TRAILING_AUTHOR_RE.find(s) else {
        return s.to_string();
    };

    let prefix = s[..m.start()].trim_end();
    if !prefix_has_holder_words(prefix) {
        return s.to_string();
    }

    prefix.to_string()
}

pub(super) fn extract_copyright_assignment_value_after_author_assignment(
    s: &str,
) -> Option<String> {
    static AUTHOR_ASSIGNMENT_COPY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r#"(?ix)
            ^(?:@?authors?)\s*=\s*(?:"[^"]*"|'[^']*'|.+?)\s+
            copyright\s*=\s*(?P<rest>.+?)\s*$
            "#,
        )
    });

    let trimmed = s.trim();
    let captures = AUTHOR_ASSIGNMENT_COPY_RE.captures(trimmed)?;
    let rest = captures
        .name("rest")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim()
        .trim_matches(&['\'', '"'][..]);
    if rest.is_empty() {
        None
    } else {
        Some(normalize_whitespace(rest))
    }
}

pub(super) fn synthesize_copyright_from_assignment_value(value: &str) -> String {
    let prepared = prepare_text_line(value).trim().to_string();
    if prepared.is_empty() {
        String::new()
    } else if prepared.starts_with("Copyright") || prepared.starts_with('©') {
        prepared
    } else {
        format!("Copyright {prepared}")
    }
}

pub(super) fn strip_leading_duplicate_phrase_before_embedded_copyright(s: &str) -> String {
    static EMBEDDED_COPY_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?)\s+copyright\s+(?P<rest>.+)$"));
    static LEADING_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)^(?P<years>(?:19\d{2}|20\d{2}|CURRENT_YEAR|\?\?\?\?)(?:\s*[-–/]\s*(?:19\d{2}|20\d{2}|\d{2}|CURRENT_YEAR|\?\?\?\?))?(?:\s*,\s*(?:19\d{2}|20\d{2}|CURRENT_YEAR|\?\?\?\?))*(?:\s*,)?)\s+(?P<tail>.+)$",
        )
    });

    let trimmed = s.trim();
    let Some(captures) = EMBEDDED_COPY_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = normalize_whitespace(
        captures
            .name("prefix")
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim(),
    );
    if prefix.split_whitespace().count() < 2 {
        return s.to_string();
    }

    let rest = normalize_whitespace(
        captures
            .name("rest")
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim(),
    );
    let Some(year_captures) = LEADING_YEAR_RE.captures(&rest) else {
        return s.to_string();
    };
    let years = year_captures
        .name("years")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim();
    let tail = year_captures
        .name("tail")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim();
    if years.is_empty() || tail.is_empty() {
        return s.to_string();
    }

    let prefix_lower = prefix.to_ascii_lowercase();
    let tail_lower = tail.to_ascii_lowercase();
    if tail_lower != prefix_lower && !tail_lower.starts_with(&format!("{prefix_lower} ")) {
        return s.to_string();
    }

    normalize_whitespace(&format!("Copyright {years} {tail}"))
}
