// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Holder-name refinement: `refine_holder` / `refine_holder_in_copyright_context`,
//! the shared `refine_holder_impl`, and holder-specific clause strippers.

use std::sync::LazyLock;

use regex::Regex;

use super::*;

/// Refine a detected holder name. Returns `None` if junk or empty.
pub fn refine_holder(s: &str) -> Option<String> {
    refine_holder_impl(s, false)
}

pub fn refine_holder_in_copyright_context(s: &str) -> Option<String> {
    refine_holder_impl(s, true)
}

pub(super) fn strip_parenthesized_emails(s: &str) -> String {
    static PAREN_EMAIL_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\s*\([^()]*@[^()]*\)\s*"));
    normalize_whitespace(&PAREN_EMAIL_RE.replace_all(s, " "))
}

pub(super) fn strip_trailing_parenthesized_obfuscated_email_in_holder(s: &str) -> String {
    static TRAILING_PAREN_OBFUSCATED_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^(?P<prefix>.+?)
            \s*
            \(
                \s*
                (?P<local>[a-z0-9][a-z0-9._-]{0,63}(?:\s+dot\s+[a-z0-9][a-z0-9._-]{0,63})*)
                \s+at\s+
                (?P<domain>[a-z0-9][a-z0-9._-]{0,63})
                \s+dot\s+
                (?P<tld>[a-z]{2,12})
                \s*
            \)
            \s*$",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = TRAILING_PAREN_OBFUSCATED_RE.captures(trimmed) else {
        return s.to_string();
    };

    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.split_whitespace().count() < 2 {
        return s.to_string();
    }

    prefix.to_string()
}

pub(super) fn strip_leading_and_onwards_holder_prefix(s: &str) -> String {
    static AND_ONWARDS_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?:and\s+)?onwards\b[\s,;:.-]*"));
    normalize_whitespace(&AND_ONWARDS_RE.replace(s, " "))
}

pub(super) fn refine_holder_impl(s: &str, in_copyright_context: bool) -> Option<String> {
    if s.is_empty() {
        return None;
    }

    let had_paren_email =
        in_copyright_context && s.contains('@') && s.contains('(') && s.contains(')');

    // Choose prefix set based on whether "reserved" appears.
    let prefixes = if s.to_lowercase().contains("reserved") {
        &*HOLDERS_PREFIXES_WITH_ALL
    } else {
        &*HOLDERS_PREFIXES
    };

    let mut h = trim_separator_rule_runs(&s.replace("build.year", " "));
    let had_lowercase_handle_angle_email = is_lowercase_handle_angle_email(&h);
    h = strip_trailing_lowercase_handle_angle_email(&h);
    h = strip_trailing_quote_before_email(&h);
    h = strip_nickname_quotes(&h);
    if let Some(rest) = extract_copyright_assignment_value_after_author_assignment(&h) {
        h = rest;
    }
    h = strip_leading_author_label_in_holder(&h);
    h = strip_angle_bracketed_www_domains(&h);
    if in_copyright_context {
        h = strip_angle_bracketed_emails(&h);
        h = strip_trailing_email_token(&h);
        h = strip_trailing_obfuscated_email_phrase_in_holder(&h);
        h = strip_trailing_parenthesized_obfuscated_email_in_holder(&h);
    }
    h = strip_trailing_single_letter_obfuscated_email_phrase(&h);
    h = strip_parenthesized_emails(&h);
    h = strip_trailing_parenthesized_url_or_domain(&h);
    h = strip_contributor_parens_after_org(&h);
    h = normalize_comma_spacing(&h);
    h = normalize_angle_bracket_comma_spacing(&h);
    h = strip_trailing_linux_ag_location(&h);
    h = strip_trailing_locale_timestamp_in_holder(&h);
    h = strip_trailing_but_suffix(&h);
    if had_paren_email {
        h = remove_comma_between_person_and_company_suffix(&h);
    }
    h = strip_trailing_component_descriptor_from_holder(&h);
    h = strip_trailing_by_person_clause_after_company(&h);
    h = strip_trailing_division_of_company_suffix(&h);
    h = strip_leading_product_operating_system_title(&h);
    h = strip_trailing_everyone_is_permitted_to_copy_clause(&h);
    h = strip_trailing_reserved_font_name_clause(&h);
    h = strip_trailing_all_rights_reserved_holder_clause(&h);
    h = strip_trailing_no_rights_reserved_clause(&h);
    h = strip_trailing_parenthesized_url_or_domain(&h);
    h = strip_trailing_et_al(&h);
    h = strip_trailing_authors_clause(&h);
    h = strip_trailing_document_authors_clause(&h);
    h = strip_trailing_amp_authors(&h);
    h = strip_trailing_x509_dn_fields_from_holder(&h);
    h = strip_leading_js_project_version(&h);
    h = truncate_trailing_boilerplate(&h);
    h = strip_trailing_dangling_pronoun(&h);
    h = strip_trailing_isc_after_inc(&h);
    h = strip_trailing_caps_after_company_suffix(&h);
    h = strip_trailing_javadoc_tags(&h);
    h = strip_trailing_batch_comment_marker(&h);
    h = strip_leading_portions_comma(&h);
    h = strip_trailing_paren_identifier(&h);
    h = strip_trailing_company_name_placeholder(&h);
    h = strip_trailing_confidentiality_qualifier(&h);
    h = strip_trailing_heavily_based_clause(&h);

    if in_copyright_context {
        h = strip_trailing_short_surname_paren_list_in_holder(&h);
        h = strip_leading_and_onwards_holder_prefix(&h);
    }

    // Strip leading date-like prefix (digits, dashes, slashes).
    if h.contains(' ')
        && let Some((prefix, suffix)) = h.split_once(' ')
        && prefix
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == '/')
    {
        h = suffix.to_string();
    }

    h = remove_some_extra_words_and_punct(&h);
    h = strip_trailing_incomplete_as_represented_by(&h);
    h = strip_trailing_contributor_clause(&h);
    h = strip_trailing_contact_clause(&h);
    h = strip_trailing_holder_prose_clause(&h);
    h = h.trim_matches(&['/', ' ', '~'][..]).to_string();
    h = refine_names(&h, prefixes);
    h = strip_repeated_leading_holder_prefix(&h);
    h = strip_trailing_company_co_ltd(&h);
    h = strip_suffixes(&h, &HOLDERS_SUFFIXES);
    h = strip_trailing_ampas_acronym(&h);
    h = h.trim_matches(&['/', ' ', '~'][..]).to_string();
    h = strip_solo_quotes(&h);
    h = h.replace("( ", " ").replace(" )", " ");
    h = h.trim_matches(&['+', '-', ' '][..]).to_string();
    h = strip_trailing_period(&h);
    h = strip_independent_jpeg_groups_software_tail(&h);
    h = strip_trailing_original_authors(&h);
    h = h.trim_matches(&['+', '-', ' '][..]).to_string();
    h = remove_dupe_holder(&h);
    h = normalize_whitespace(&h);
    h = strip_trailing_url(&h);
    h = h
        .trim_matches(&['/', ' ', '~', '-', '–', '—'][..])
        .to_string();
    if in_copyright_context {
        h = strip_trailing_email_token(&h);
    }
    h = strip_trailing_at_sign(&h);
    h = strip_trailing_mountain_view_ca(&h);
    h = strip_trailing_ansi_escape_suffix(&h);
    h = h.trim_matches(&[',', ' '][..]).to_string();
    h = strip_trailing_period(&h);
    h = h.trim_matches(&[',', ' '][..]).to_string();
    h = normalize_whitespace(&h);
    if h.split_whitespace()
        .last()
        .is_some_and(|word| matches!(word.to_ascii_lowercase().as_str(), "to"))
    {
        return None;
    }
    h = truncate_long_words(&h);
    h = strip_trailing_single_digit_token(&h);
    h = strip_trailing_period(&h);
    h = h.trim().to_string();

    if looks_like_credit_file_reference_note(&h) || looks_like_document_form_reference(&h) {
        return None;
    }

    if (is_explicit_generic_field_label_token(&h)
        || (!in_copyright_context
            && !had_lowercase_handle_angle_email
            && looks_like_generic_field_label_shape(&h)))
        || looks_like_translation_or_ui_phrase(&h)
        || (looks_like_lowercase_enum_blob(&h)
            && !(in_copyright_context && looks_like_lowercase_company_suffix_holder(&h)))
    {
        return None;
    }

    let lower = h.to_lowercase();
    if h.trim_end_matches('.').eq_ignore_ascii_case("YOUR NAME") {
        return None;
    }
    let is_single_word_contributors = lower == "contributors";
    let is_contributors_as_noted_in_authors_file =
        in_copyright_context && lower.contains("contributors as noted in the authors file");
    if !h.is_empty()
        && (!HOLDERS_JUNK.contains(lower.as_str())
            || (in_copyright_context && is_single_word_contributors))
        && (is_contributors_as_noted_in_authors_file || !is_junk_holder(&h))
    {
        Some(h)
    } else {
        None
    }
}

pub(super) fn strip_trailing_but_suffix(s: &str) -> String {
    static TRAILING_BUT_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?),\s*but\s*$"));
    let trimmed = s.trim();
    let Some(cap) = TRAILING_BUT_RE.captures(trimmed) else {
        return s.to_string();
    };
    cap.name("prefix")
        .map(|m| m.as_str().trim_end().to_string())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn strip_trailing_confidentiality_qualifier(s: &str) -> String {
    static TRAILING_CONFIDENTIALITY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^(?P<prefix>.+?)
            (?:[\s,;:.\-–—]+)?
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
    });

    let trimmed = s.trim();
    let Some(cap) = TRAILING_CONFIDENTIALITY_RE.captures(trimmed) else {
        return s.to_string();
    };

    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() || !prefix_has_holder_words(prefix) {
        return s.to_string();
    }

    prefix
        .trim_end_matches([',', ';', ':', '.', '-', '–', '—', ' '])
        .trim()
        .to_string()
}

pub(super) fn strip_trailing_division_of_company_suffix(s: &str) -> String {
    static DIVISION_OF_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?),\s*a\s+division\s+of\s+.+$"));

    let trimmed = s.trim();
    let Some(cap) = DIVISION_OF_RE.captures(trimmed) else {
        return s.to_string();
    };

    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() || !prefix_has_holder_words(prefix) {
        return s.to_string();
    }

    prefix.trim_end_matches(&[',', ' '][..]).trim().to_string()
}

pub(super) fn strip_trailing_linux_ag_location(s: &str) -> String {
    static LINUX_AG_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"^(?P<prefix>\S+)\s+Linux\s+AG\s*,\s*[^,]{2,64}\s*,\s*[^,]{2,64}\s*$")
    });
    let trimmed = s.trim();
    if let Some(cap) = LINUX_AG_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

pub(super) fn strip_trailing_locale_timestamp_in_holder(s: &str) -> String {
    static LOCALE_TIMESTAMP_HOLDER_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^(?P<prefix>.+?),\s*
            [a-z]{3}\s+[a-z]{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}\s+[A-Z]{2,5}
            (?:\s+\d{4})?\s*$
            ",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = LOCALE_TIMESTAMP_HOLDER_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() || !prefix_has_holder_words(prefix) {
        return s.to_string();
    }
    prefix.trim_end_matches(&[',', ' '][..]).to_string()
}

pub(super) fn remove_comma_between_person_and_company_suffix(s: &str) -> String {
    static COMMA_CORP_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"^(?P<person>[\p{Lu}][^,]{2,64}(?:\s+[\p{Lu}][^,]{2,64})+)\s*,\s*(?P<corp>[^,]{2,64}\b(?:Corp\.?|Corporation|Inc\.?|Ltd\.?))\s*$",
        )
    });
    let trimmed = s.trim();
    if let Some(cap) = COMMA_CORP_RE.captures(trimmed) {
        let person = cap.name("person").map(|m| m.as_str()).unwrap_or("").trim();
        let corp = cap.name("corp").map(|m| m.as_str()).unwrap_or("").trim();
        if !person.is_empty() && !corp.is_empty() {
            return format!("{person} {corp}");
        }
    }
    s.to_string()
}

pub(super) fn strip_trailing_by_person_clause_after_company(s: &str) -> String {
    static BY_PERSON_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"^(?P<prefix>.+?\b(?:Corp\.?|Corporation|Inc\.?|Ltd\.?))\s+by\s+[\p{Lu}][\p{L}'\-\.]+(?:\s+[\p{Lu}][\p{L}'\-\.]+){1,4}\s*(?:<[^>]*>)?\s*$",
        )
    });
    let trimmed = s.trim();
    if let Some(cap) = BY_PERSON_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

pub(super) fn strip_trailing_amp_authors(s: &str) -> String {
    static AMP_AUTHORS_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?)\s*(?:&|and)\s+authors?\s*$"));
    let trimmed = s.trim();
    if let Some(cap) = AMP_AUTHORS_RE.captures(trimmed)
        && let Some(prefix) = cap.name("prefix").map(|m| m.as_str().trim())
        && !prefix.is_empty()
    {
        return prefix.to_string();
    }
    s.to_string()
}

pub(super) fn strip_trailing_parenthesized_url_or_domain(s: &str) -> String {
    static TRAILING_PAREN_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^(?P<prefix>.+?)\s*\(\s*(?:https?|ftp)://[^)\s]+\s*\)\s*$")
    });
    static TRAILING_PAREN_DOMAIN_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^(?P<prefix>.+?)\s*\(\s*[a-z0-9._-]+\.[a-z]{2,12}\s*\)\s*$")
    });
    static TRAILING_SINGLE_WORD_PARENS_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"^(?P<prefix>.+?)\s*\(\s*(?P<inner>[A-Za-z0-9._-]{2,32})\s*\)\s*$")
    });

    let trimmed = s.trim();
    if let Some(cap) = TRAILING_PAREN_URL_RE.captures(trimmed) {
        return cap
            .name("prefix")
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| s.to_string());
    }
    if let Some(cap) = TRAILING_PAREN_DOMAIN_RE.captures(trimmed) {
        return cap
            .name("prefix")
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| s.to_string());
    }
    if let Some(cap) = TRAILING_SINGLE_WORD_PARENS_RE.captures(trimmed)
        && let Some(inner) = cap.name("inner").map(|m| m.as_str().trim())
        && !inner.is_empty()
    {
        let inner_has_upper = inner.chars().any(|c| c.is_ascii_uppercase());
        let inner_all_lowerish = inner
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-'));

        if !inner_has_upper && inner_all_lowerish && inner.len() >= 4 && !inner.starts_with('-') {
            return cap
                .name("prefix")
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| s.to_string());
        }
    }

    s.to_string()
}

pub(super) fn strip_angle_bracketed_emails(s: &str) -> String {
    static ANGLE_EMAIL_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"\s*<[^>\s]*@[^>\s]*>\s*"));
    ANGLE_EMAIL_RE.replace_all(s, " ").trim().to_string()
}

pub(super) fn strip_trailing_email_token(s: &str) -> String {
    static TRAILING_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"^(?P<prefix>.+?)\s+(?P<email>[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+)\s*$")
    });
    let trimmed = s.trim();
    let Some(cap) = TRAILING_EMAIL_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.split_whitespace().count() < 2 {
        return s.to_string();
    }
    prefix.to_string()
}

pub(super) fn strip_trailing_obfuscated_email_phrase_in_holder(s: &str) -> String {
    static OBFUSCATED_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>.+?)\s+(?P<user>[a-z0-9][a-z0-9._-]{0,63})\s+at\s+(?P<domain>[a-z0-9][a-z0-9._-]{0,63})\s+dot\s+(?P<tld>[a-z]{2,12})(?:\s+.*)?$",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = OBFUSCATED_RE.captures(trimmed) else {
        return s.to_string();
    };
    let user = cap.name("user").map(|m| m.as_str()).unwrap_or("").trim();
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.split_whitespace().count() < 2 {
        return s.to_string();
    }
    if user.is_empty() {
        return prefix.to_string();
    }
    let mut words: Vec<&str> = prefix.split_whitespace().collect();
    if words.last().is_some_and(|w| w.eq_ignore_ascii_case(user)) {
        words.pop();
    }
    words.join(" ")
}

pub(super) fn strip_trailing_at_sign(s: &str) -> String {
    let trimmed = s.trim_end();
    if let Some(stripped) = trimmed.strip_suffix('@') {
        return stripped.trim_end().to_string();
    }
    s.to_string()
}

pub(super) fn strip_leading_product_operating_system_title(s: &str) -> String {
    static PRODUCT_OPERATING_SYSTEM_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^the\s+(?:[\p{L}0-9._-]+\s+){1,5}operating\s+system(?:[.,]|\s|$)",
        )
    });

    if !PRODUCT_OPERATING_SYSTEM_RE.is_match(s.trim()) {
        return s.to_string();
    }

    if let Some((_, suffix)) = s.split_once(',') {
        return suffix.trim().to_string();
    }

    s.to_string()
}

pub(super) fn strip_trailing_x509_dn_fields_from_holder(s: &str) -> String {
    static X509_DN_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^(?P<prefix>.+?)(?:\s*,\s*(?:OU|CN|O|C|L|ST)\s+.+)$")
    });
    static TRAILING_ENDORSED_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?)\s+endorsed\s*$"));

    let trimmed = s.trim();
    if !trimmed.contains(", OU ")
        && !trimmed.contains(", CN ")
        && !trimmed.contains(", O ")
        && !trimmed.contains(", C ")
        && !trimmed.contains(", L ")
        && !trimmed.contains(", ST ")
    {
        return s.to_string();
    }

    let Some(cap) = X509_DN_TAIL_RE.captures(trimmed) else {
        return s.to_string();
    };
    let mut prefix = cap
        .name("prefix")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if prefix.is_empty() {
        return s.to_string();
    }
    if let Some(cap2) = TRAILING_ENDORSED_RE.captures(&prefix) {
        prefix = cap2
            .name("prefix")
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or(prefix);
    }
    prefix
}

pub(super) fn strip_trailing_ampas_acronym(s: &str) -> String {
    static AMPAS_SUFFIX_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\s+\(?A\.M\.P\.A\.S\.?\)?\s*$"));
    AMPAS_SUFFIX_RE.replace(s, "").trim().to_string()
}

pub(super) fn strip_trailing_javadoc_tags(s: &str) -> String {
    static JAVADOC_TAGS_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)\s+@(?:generated|version|since|param|return|see)\b.*$")
    });
    JAVADOC_TAGS_RE.replace(s, "").trim().to_string()
}

pub(super) fn strip_trailing_batch_comment_marker(s: &str) -> String {
    static BATCH_COMMENT_TAIL_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?)\.?\s+@?rem\b.*$"));
    let trimmed = s.trim();
    let Some(cap) = BATCH_COMMENT_TAIL_RE.captures(trimmed) else {
        return s.to_string();
    };
    cap.name("prefix")
        .map(|m| m.as_str().trim_end_matches(&[' ', '.'][..]).to_string())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn strip_trailing_paren_years(s: &str) -> String {
    static PAREN_YEARS_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"^(?P<prefix>.+?)\s*\(\s*(?:19\d{2}|20\d{2})(?:\s*[-–]\s*(?:19\d{2}|20\d{2}|\d{2}))?(?:\s*,\s*(?:19\d{2}|20\d{2}))*\s*\)\s*$",
        )
    });
    let trimmed = s.trim();
    let Some(cap) = PAREN_YEARS_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }
    if prefix.split_whitespace().count() < 2 {
        return s.to_string();
    }
    prefix.to_string()
}

pub(super) fn strip_trailing_bare_c_copyright_clause(s: &str) -> String {
    static BARE_C_CLAUSE_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^(?P<prefix>.+?)\s*\(c\)\s*(?:19\d{2}|20\d{2})\b.*$")
    });
    let trimmed = s.trim();
    let Some(cap) = BARE_C_CLAUSE_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }
    prefix.to_string()
}

pub(super) fn strip_trailing_single_digit_token(s: &str) -> String {
    static TRAILING_SINGLE_DIGIT_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"^(?P<prefix>.+?)\s+[1-9]\s*$"));
    let trimmed = s.trim();
    let Some(cap) = TRAILING_SINGLE_DIGIT_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }
    if prefix.split_whitespace().count() < 2 {
        return s.to_string();
    }
    if !prefix.chars().any(|c| c.is_alphabetic()) {
        return s.to_string();
    }
    prefix.to_string()
}
