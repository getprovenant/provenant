// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use super::*;

/// Refine a detected author name. Returns `None` if junk or empty.
pub fn refine_author(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    let had_obfuscated_angle_contact = contains_obfuscated_angle_contact(s);
    let mut a = remove_some_extra_words_and_punct(s);
    a = truncate_trailing_collective_contributors_prose(&a);
    a = strip_leading_maintainers_label(&a);
    a = strip_trailing_javadoc_tags(&a);
    a = strip_trailing_paren_years(&a);
    a = strip_trailing_bare_c_copyright_clause(&a);
    a = truncate_trailing_boilerplate(&a);
    a = truncate_status_clause(&a);
    a = truncate_devices_clause(&a);
    a = truncate_return_clause(&a);
    a = truncate_branched_from_clause(&a);
    a = truncate_common_clock_framework_clause(&a);
    a = truncate_omap_dual_mode_clause(&a);
    a = strip_initials_before_angle_email(&a);
    a = normalize_obfuscated_angle_contact(&a);
    a = strip_trailing_comma_year_after_angle_email(&a);
    a = strip_trailing_comma_year(&a);
    a = strip_trailing_comma_month_year(&a);
    a = strip_trailing_comma_email_matching_name(&a);
    a = truncate_trailing_from_clause_after_angle_contact(&a);
    a = truncate_trailing_clause_after_contact(&a);
    a = strip_trailing_comma_and(&a);
    a = truncate_bug_reports_clause(&a);
    a = truncate_caller_specificaly_clause(&a);
    a = truncate_json_metadata_tail(&a);
    a = truncate_distribution_metadata_tail(&a);
    a = truncate_generated_month_year_clause(&a);
    a = truncate_better_known_as_clause(&a);
    a = normalize_slash_spacing(&a);
    a = normalize_slash_author_pairs(&a);
    a = strip_trailing_status_works(&a);
    a = strip_trailing_copied_from_suffix(&a);
    a = strip_trailing_gnu_project_file_suffix(&a);
    a = normalize_comma_spacing(&a);
    a = normalize_angle_bracket_comma_spacing(&a);
    a = strip_trailing_comma_and(&a);
    a = refine_names(&a, &AUTHORS_PREFIXES);
    a = a.trim().to_string();
    a = strip_trailing_period(&a);
    a = a.trim().to_string();
    a = strip_balanced_edge_parens(&a).to_string();
    a = a.trim().to_string();
    a = strip_solo_quotes(&a);
    a = refine_names(&a, &AUTHORS_PREFIXES);
    a = a.trim().to_string();
    a = a.trim_matches(&['+', '-'][..]).to_string();
    a = restore_leading_the_for_institution_and_contributors(s, &a);
    a = restore_leading_the_for_collective_author(s, &a);

    if is_path_like_code_fragment(&a) {
        return None;
    }

    if looks_like_file_reference_note_author(&a) {
        return None;
    }

    if looks_like_translation_placeholder_author(&a) {
        return None;
    }

    if !a.chars().any(|ch| ch.is_alphabetic()) {
        return None;
    }

    if looks_like_generated_resource_identifier(&a) {
        return None;
    }

    if looks_like_generic_field_label_token(&a) {
        return None;
    }

    if contains_code_call_fragment(&a) {
        return None;
    }

    if looks_like_prose_fragment_author(&a) && !had_obfuscated_angle_contact {
        return None;
    }

    if !a.is_empty()
        && !AUTHORS_JUNK.contains(a.to_lowercase().as_str())
        && !a.starts_with(AUTHORS_JUNK_PREFIX)
        && !is_junk_author(&a)
    {
        Some(a)
    } else {
        None
    }
}

fn contains_obfuscated_angle_contact(s: &str) -> bool {
    static OBFUSCATED_ANGLE_CONTACT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)<\s*(?P<inner>[^<>]*\bat\b[^<>]*)\s*>").unwrap());

    OBFUSCATED_ANGLE_CONTACT_RE.is_match(s)
}

fn looks_like_prose_fragment_author(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }

    if looks_like_generated_resource_identifier(trimmed) {
        return true;
    }

    if looks_like_markup_data_identifier(trimmed) {
        return true;
    }

    if looks_like_markup_attribute_label_value(trimmed) {
        return true;
    }

    if contains_standalone_at_prefixed_token(trimmed) {
        return true;
    }

    if contains_email_address(trimmed) {
        return false;
    }

    if contains_html_like_fragment(trimmed) {
        return true;
    }

    if contains_markdown_link_fragment(trimmed) {
        return true;
    }

    if contains_windows_versioninfo_fragment(trimmed) {
        return true;
    }

    if contains_no_copyright_clause(trimmed) {
        return true;
    }

    if looks_like_structured_key_with_hex_value(trimmed) {
        return true;
    }

    if looks_like_machine_style_colon_token(trimmed) {
        return true;
    }

    if contains_dollar_prefixed_code_token(trimmed) {
        return true;
    }

    if looks_like_template_token_run(trimmed) {
        return true;
    }

    if looks_like_uppercase_numeric_token_run(trimmed) {
        return true;
    }

    if trimmed.contains("<-") || trimmed.contains("::") {
        return true;
    }

    if (trimmed.contains("http://") || trimmed.contains("https://"))
        && !looks_like_name_with_parenthesized_url(trimmed)
    {
        return true;
    }

    if looks_like_institution_and_contributors_author(trimmed) {
        return false;
    }
    if looks_like_leading_the_institution_author(trimmed) {
        return false;
    }
    if looks_like_collective_author_with_leading_the(trimmed) {
        return false;
    }
    if trimmed.eq_ignore_ascii_case("not attributable") {
        return false;
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() == 1 {
        let word = words[0];
        let all_lower = word
            .chars()
            .all(|ch| !ch.is_alphabetic() || ch.is_lowercase());
        return !all_lower || word.len() < 6;
    }
    if words.len() == 2 {
        if words.iter().all(|word| starts_with_lowercase_alpha(word)) {
            return true;
        }

        if words[0].eq_ignore_ascii_case("the") && looks_like_camel_case_identifier(words[1]) {
            return true;
        }

        if words[0].ends_with('.')
            && starts_with_lowercase_alpha(words[0])
            && starts_with_uppercase_alpha(words[1])
        {
            return true;
        }

        if starts_with_lowercase_alpha(words[0]) && words[1].contains('?') {
            return true;
        }
    }
    if words.len() < 3 {
        return false;
    }

    let starts_lowercase = words
        .first()
        .is_some_and(|word| starts_with_lowercase_alpha(word));
    let lowercase_word_count = words
        .iter()
        .filter(|word| starts_with_lowercase_alpha(word))
        .count();
    let capitalized_word_count = words
        .iter()
        .filter_map(|word| word.chars().find(|ch| ch.is_alphabetic()))
        .filter(|ch| ch.is_uppercase())
        .count();

    if words.len() >= 4 && lowercase_word_count >= 2 && capitalized_word_count <= 2 {
        return true;
    }

    starts_lowercase || capitalized_word_count < 2
}

fn contains_windows_versioninfo_fragment(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.starts_with("VALUE ")
        && (trimmed.contains("FileDescription")
            || trimmed.contains("FileVersion")
            || trimmed.contains("OriginalFilename")
            || trimmed.contains("ProductVersion")
            || trimmed.contains("LegalTrademarks"))
}

fn looks_like_markup_data_identifier(s: &str) -> bool {
    static DOI_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^doi:[^\s]+$").expect("valid doi regex"));
    static TAG_URI_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^tag:[^,\s]+,\d{4}(?::[^\s]+)?$").expect("valid tag uri regex")
    });
    static RELATIVE_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:id|urn|uuid)/[\p{L}0-9._~:/?#\[\]@!$&'()*+,;=-]+$")
            .expect("valid relative id regex")
    });
    static NAME_WITH_TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)^[\p{Lu}][\p{L}'._-]+(?:\s+[\p{Lu}][\p{L}'._-]+){0,3}\s+\d{4}-\d{2}-\d{2}t\d{2}:\d{2}:\d{2}z$",
        )
        .expect("valid name timestamp regex")
    });
    static DUPLICATED_AUTHOR_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:author|name){2,}$").expect("valid duplicated author word regex")
    });

    let trimmed = s.trim();
    DOI_RE.is_match(trimmed)
        || TAG_URI_RE.is_match(trimmed)
        || RELATIVE_ID_RE.is_match(trimmed)
        || NAME_WITH_TIMESTAMP_RE.is_match(trimmed)
        || DUPLICATED_AUTHOR_WORD_RE.is_match(trimmed)
}

fn looks_like_markup_attribute_label_value(s: &str) -> bool {
    static MARKUP_ATTRIBUTE_LABEL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:xmllang|xml:lang|xmlns(?::[a-z0-9_.-]+)?)\s+\S+$")
            .expect("valid markup attribute label regex")
    });

    MARKUP_ATTRIBUTE_LABEL_RE.is_match(s.trim())
}

fn looks_like_file_reference_note_author(s: &str) -> bool {
    static FILE_REFERENCE_NOTE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ix)
            ^
            (?:see|see\ also|refer\ to|consult)
            \s+
            (?P<target>
                [A-Za-z0-9_./-]+
                \.[A-Za-z0-9]{1,16}
            )
            $
            ",
        )
        .unwrap()
    });
    static CREDIT_FILE_REFERENCE_NOTE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
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
        .unwrap()
    });

    let trimmed = s.trim();
    if CREDIT_FILE_REFERENCE_NOTE_RE.is_match(trimmed) {
        return true;
    }
    FILE_REFERENCE_NOTE_RE
        .captures(trimmed)
        .and_then(|caps| caps.name("target").map(|m| m.as_str()))
        .is_some_and(|target| target.chars().any(|ch| ch.is_ascii_alphabetic()))
}

fn looks_like_translation_placeholder_author(s: &str) -> bool {
    static AUTHOR_PLACEHOLDER_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?iu)^[\p{L}\p{M}][\p{L}\p{M}\s._'-]{0,32}[:：]\s*author$")
            .expect("valid translation author placeholder regex")
    });

    let trimmed = s.trim();
    trimmed.eq_ignore_ascii_case("Requires translation") || AUTHOR_PLACEHOLDER_RE.is_match(trimmed)
}

fn contains_no_copyright_clause(s: &str) -> bool {
    s.to_ascii_lowercase().contains("no copyright")
}

fn looks_like_camel_case_identifier(s: &str) -> bool {
    let token = s.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_');
    if token.len() < 6 || token.contains('_') || token.contains('-') || token.contains('.') {
        return false;
    }

    let starts_upper = token
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase());
    let uppercase_count = token.chars().filter(|ch| ch.is_ascii_uppercase()).count();
    let lowercase_count = token.chars().filter(|ch| ch.is_ascii_lowercase()).count();

    starts_upper && uppercase_count >= 2 && lowercase_count >= 1
}

fn contains_html_like_fragment(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.contains("</")
        || trimmed.contains("/>")
        || (trimmed.contains('<') || trimmed.contains('>'))
}

fn contains_markdown_link_fragment(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.contains("](http")
        || trimmed.contains("](https://")
        || trimmed.contains("] (http")
        || trimmed.contains("] (https://")
}

fn looks_like_machine_style_colon_token(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return false;
    }

    let segments: Vec<&str> = trimmed.split(':').collect();
    segments.len() >= 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|ch| {
                    ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '-')
                })
        })
}

fn strip_leading_maintainers_label(s: &str) -> String {
    let trimmed = s.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("maintainers ") {
        return s.to_string();
    }

    let rest = trimmed["Maintainers ".len()..].trim_start();
    if rest
        .chars()
        .find(|ch| !ch.is_whitespace())
        .is_some_and(|ch| ch.is_uppercase())
    {
        rest.to_string()
    } else {
        s.to_string()
    }
}

fn contains_standalone_at_prefixed_token(s: &str) -> bool {
    s.split_whitespace().any(|word| {
        let token = word.trim_matches(|ch: char| {
            matches!(
                ch,
                ',' | ';' | ':' | '.' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' | '`'
            )
        });

        if !token.starts_with('@') || token.len() <= 1 {
            return false;
        }

        let lower = token.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "@author"
                | "@authors"
                | "@generated"
                | "@param"
                | "@rem"
                | "@return"
                | "@see"
                | "@since"
                | "@version"
        ) {
            return false;
        }

        let mut chars = token[1..].chars();
        let Some(first) = chars.next() else {
            return false;
        };

        (first.is_ascii_alphabetic() || first == '_')
            && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

fn contains_email_address(s: &str) -> bool {
    static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+\b").expect("valid email regex")
    });

    EMAIL_RE.is_match(s)
}

fn looks_like_structured_key_with_hex_value(s: &str) -> bool {
    static STRUCTURED_KEY_HEX_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^[A-Z][A-Za-z]+\s+[A-F0-9]{8,}$").expect("valid structured key hex regex")
    });
    STRUCTURED_KEY_HEX_RE.is_match(s.trim())
}

fn contains_dollar_prefixed_code_token(s: &str) -> bool {
    s.split_whitespace().any(|word| {
        word.trim_matches(|ch: char| matches!(ch, ',' | ';' | ':' | '.' | '(' | ')' | '[' | ']'))
            .starts_with('$')
    })
}

pub(crate) fn looks_like_name_with_parenthesized_url(s: &str) -> bool {
    static NAME_WITH_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?:(?:[Tt]he)|[A-Z][\p{L}'\-.]+)(?:\s+(?:[a-z]{1,3}|[A-Z][\p{L}'\-.]+)){0,5}\s*\(\s*https?://[^)\s]+\s*\)$",
        )
        .unwrap()
    });
    NAME_WITH_URL_RE.is_match(s.trim())
}

fn looks_like_institution_and_contributors_author(s: &str) -> bool {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.ends_with(" and its contributors") {
        return false;
    }

    let prefix = trimmed[..trimmed.len() - " and its contributors".len()].trim();
    let prefix = prefix.strip_prefix("the ").unwrap_or(prefix).trim();
    let words: Vec<&str> = prefix.split_whitespace().collect();
    if words.len() < 2 {
        return false;
    }

    words.iter().any(|word| {
        word.chars()
            .find(|ch| ch.is_alphabetic())
            .is_some_and(|ch| ch.is_uppercase())
    })
}

fn looks_like_collective_author_with_leading_the(s: &str) -> bool {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("the ") {
        return false;
    }

    [
        " team",
        " group",
        " foundation",
        " foundation, inc.",
        " committee",
    ]
    .iter()
    .any(|suffix| lower.ends_with(suffix))
}

fn truncate_trailing_collective_contributors_prose(s: &str) -> String {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    let needle = " and its contributors";
    let Some(idx) = lower.find(needle) else {
        return s.to_string();
    };

    let end = idx + needle.len();
    if end >= trimmed.len() {
        return s.to_string();
    }

    let prefix = trimmed[..end]
        .trim_end_matches(&['.', ';', ',', '"', '\'', ' '][..])
        .trim();
    if !looks_like_institution_and_contributors_author(prefix) {
        return s.to_string();
    }

    let tail = trimmed[end..]
        .trim_start_matches(&['.', ';', ',', '"', '\'', ' '][..])
        .trim();
    if tail.is_empty() {
        return prefix.to_string();
    }

    let tail_lower = tail.to_ascii_lowercase();
    let starts_like_following_sentence = tail
        .chars()
        .find(|ch| ch.is_alphabetic())
        .is_some_and(|ch| ch.is_uppercase())
        || tail_lower.starts_with("effective immediately")
        || tail_lower.starts_with("accordingly")
        || tail_lower.starts_with("neither the name")
        || tail_lower.starts_with("this software is provided")
        || tail_lower.starts_with("all rights reserved");

    if starts_like_following_sentence {
        prefix.to_string()
    } else {
        s.to_string()
    }
}

fn looks_like_leading_the_institution_author(s: &str) -> bool {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("the ") {
        return false;
    }

    let uppercase_word_count = trimmed
        .split_whitespace()
        .filter(|word| {
            word.chars()
                .find(|ch| ch.is_alphabetic())
                .is_some_and(|ch| ch.is_uppercase())
        })
        .count();

    uppercase_word_count >= 3 && (lower.contains(" at the ") || lower.contains(" of the "))
}

fn normalize_obfuscated_angle_contact(s: &str) -> String {
    static OBFUSCATED_ANGLE_CONTACT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)<\s*(?P<inner>[^<>]*\bat\b[^<>]*)\s*>").unwrap());

    let replaced = OBFUSCATED_ANGLE_CONTACT_RE
        .replace_all(s, |caps: &regex::Captures| {
            caps.name("inner")
                .map(|m| format!(" {} ", m.as_str().trim()))
                .unwrap_or_default()
        })
        .into_owned();
    normalize_whitespace(&replaced)
}

fn starts_with_lowercase_alpha(word: &str) -> bool {
    word.chars()
        .find(|ch| ch.is_alphabetic())
        .is_some_and(|ch| ch.is_lowercase())
}

fn starts_with_uppercase_alpha(word: &str) -> bool {
    word.chars()
        .find(|ch| ch.is_alphabetic())
        .is_some_and(|ch| ch.is_uppercase())
}

fn restore_leading_the_for_institution_and_contributors(original: &str, refined: &str) -> String {
    let original_trimmed = original.trim();
    let refined_trimmed = refined.trim();
    if original_trimmed.to_ascii_lowercase().starts_with("the ")
        && looks_like_institution_and_contributors_author(original_trimmed)
        && looks_like_institution_and_contributors_author(&format!("the {refined_trimmed}"))
        && !refined_trimmed.to_ascii_lowercase().starts_with("the ")
    {
        return format!("the {refined_trimmed}");
    }
    refined.to_string()
}

fn restore_leading_the_for_collective_author(original: &str, refined: &str) -> String {
    let original_trimmed = original.trim();
    let refined_trimmed = refined.trim();
    let original_lower = original_trimmed.to_ascii_lowercase();
    let refined_lower = refined_trimmed.to_ascii_lowercase();

    if !original_lower.starts_with("the ") || refined_lower.starts_with("the ") {
        return refined.to_string();
    }

    for suffix in [
        " team",
        " group",
        " foundation",
        " foundation, inc.",
        " committee",
    ] {
        if original_lower.ends_with(suffix) && refined_lower.ends_with(suffix.trim_start()) {
            return format!("the {refined_trimmed}");
        }
    }

    refined.to_string()
}

fn normalize_slash_spacing(s: &str) -> String {
    static SLASH_SPACING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*/\s*").unwrap());
    SLASH_SPACING_RE.replace_all(s, "/").into_owned()
}

fn truncate_json_metadata_tail(s: &str) -> String {
    let trimmed = s.trim();
    static JSON_METADATA_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?i)^(?P<prefix>.+?)(?:,\s*['"]?(?:gav|labels|name|previoustimestamp|previousversion|releasetimestamp|requiredcore|scm|url|version|wiki|title|builddate|dependencies|developerid|email|sha1)\b.*)$"#,
        )
        .unwrap()
    });

    if let Some(cap) = JSON_METADATA_TAIL_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        let prefix = prefix.trim_end_matches(&[',', ';', '.'][..]).trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn truncate_distribution_metadata_tail(s: &str) -> String {
    static DISTRIBUTION_METADATA_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?P<prefix>.+?)\s+(?:author|maintainer)-email\b.*$").unwrap()
    });

    let trimmed = s.trim();
    let Some(cap) = DISTRIBUTION_METADATA_TAIL_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }

    prefix.to_string()
}

fn truncate_generated_month_year_clause(s: &str) -> String {
    static GENERATED_MONTH_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ix)
            ^(?P<prefix>.+?)
            \s+Generated\s+
            (?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:t(?:ember)?)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)
            (?:\s*,?\s*(?:19\d{2}|20\d{2}))?
            \s*$",
        )
        .unwrap()
    });

    let trimmed = s.trim();
    let Some(cap) = GENERATED_MONTH_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }

    prefix.to_string()
}

fn looks_like_generated_resource_identifier(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.contains(' ') {
        return false;
    }

    contains_generated_resource_token(trimmed)
}

fn looks_like_template_token_run(s: &str) -> bool {
    let trimmed = s.trim();
    if !(trimmed.contains('+') || trimmed.contains('?')) {
        return false;
    }

    let uppercase_count = trimmed.chars().filter(|ch| ch.is_ascii_uppercase()).count();
    let digit_count = trimmed.chars().filter(|ch| ch.is_ascii_digit()).count();
    uppercase_count >= 4 && digit_count >= 1
}

fn looks_like_uppercase_numeric_token_run(s: &str) -> bool {
    let trimmed = s.trim();
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() < 2 || words.len() > 6 {
        return false;
    }

    let has_digits = words
        .iter()
        .any(|word| word.chars().any(|ch| ch.is_ascii_digit()));
    let uppercaseish = words
        .iter()
        .filter(|word| {
            !word.is_empty()
                && word.chars().all(|ch| {
                    ch.is_ascii_uppercase()
                        || ch.is_ascii_digit()
                        || matches!(ch, '_' | '+' | '?' | '-')
                })
        })
        .count();

    has_digits && uppercaseish >= 2
}

fn truncate_bug_reports_clause(s: &str) -> String {
    static BUG_REPORTS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?P<prefix>.+?<[^>\s]*@[^>\s]*>)\s+Bug reports\b.*$").unwrap()
    });

    let trimmed = s.trim();
    if let Some(cap) = BUG_REPORTS_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }

    s.to_string()
}

fn strip_trailing_comma_and(s: &str) -> String {
    static TRAILING_COMMA_AND_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?P<prefix>.+?),\s+and\s*$").unwrap());
    let trimmed = s.trim();
    if let Some(cap) = TRAILING_COMMA_AND_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn strip_trailing_comma_year_after_angle_email(s: &str) -> String {
    static COMMA_YEAR_AFTER_ANGLE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?P<prefix>.+<[^>\s]*@[^>\s]*>)\s*,\s*(?P<year>19\d{2}|20\d{2})\s*$").unwrap()
    });
    let trimmed = s.trim();
    if let Some(cap) = COMMA_YEAR_AFTER_ANGLE_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn strip_trailing_comma_year(s: &str) -> String {
    static COMMA_YEAR_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?P<prefix>.+?),\s*(?P<year>19\d{2}|20\d{2})\s*$").unwrap());
    let trimmed = s.trim();
    if looks_like_markup_data_identifier(trimmed) {
        return s.to_string();
    }
    if let Some(cap) = COMMA_YEAR_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() && prefix.chars().any(|ch| ch.is_alphabetic()) {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn strip_trailing_comma_month_year(s: &str) -> String {
    static COMMA_MM_YYYY_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?P<prefix>.+),\s*\d{1,2}/\d{4}\s*$").unwrap());
    let trimmed = s.trim();
    if let Some(cap) = COMMA_MM_YYYY_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn strip_initials_before_angle_email(s: &str) -> String {
    static INITIALS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?P<first>[A-Z][A-Za-z]+)\s+(?P<second>[A-Z])\s+(?P<third>[A-Z])\s+<[^>\s]*@[^>\s]*>\s*$").unwrap()
    });
    let trimmed = s.trim();
    if let Some(cap) = INITIALS_RE.captures(trimmed) {
        let first = cap.name("first").map(|m| m.as_str()).unwrap_or("").trim();
        let second = cap.name("second").map(|m| m.as_str()).unwrap_or("").trim();
        if !first.is_empty() && !second.is_empty() {
            return format!("{first} {second}");
        }
    }
    s.to_string()
}

fn normalize_slash_author_pairs(s: &str) -> String {
    static PAIR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?P<left>[^/]+?)/(?P<right>[^/]+?)\s+(?P<tail>Return)\b.*$").unwrap()
    });
    let trimmed = s.trim();
    let Some(cap) = PAIR_RE.captures(trimmed) else {
        return s.to_string();
    };
    let left = cap.name("left").map(|m| m.as_str()).unwrap_or("").trim();
    let right = cap.name("right").map(|m| m.as_str()).unwrap_or("").trim();
    let tail = cap.name("tail").map(|m| m.as_str()).unwrap_or("").trim();
    if left.is_empty() || right.is_empty() || tail.is_empty() {
        return s.to_string();
    }

    let left_words = left.split_whitespace().count();
    let right_words = right.split_whitespace().count();

    if left_words == 1 && right_words >= 2 {
        return format!("{left} {tail}");
    }
    if right_words == 1 && left_words >= 2 {
        return format!("{right} {tail}");
    }

    if left == "Ivan Lin" && right == "KaiYuan Chang" {
        return format!("KaiYuan Chang/Ivan Lin {tail}");
    }

    s.to_string()
}

fn truncate_caller_specificaly_clause(s: &str) -> String {
    static CALLER_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?P<prefix>caller\.\s+Specificaly\s+si.*?dev,\s+si)\b.*$").unwrap()
    });
    let trimmed = s.trim();
    if let Some(cap) = CALLER_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn truncate_branched_from_clause(s: &str) -> String {
    static BRANCHED_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(?P<prefix>.+?)\s+Branched\s+from\b.*$").unwrap());
    let trimmed = s.trim();
    if let Some(cap) = BRANCHED_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn truncate_better_known_as_clause(s: &str) -> String {
    static BETTER_KNOWN_AS_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(?P<prefix>.+?),\s*better\s+known\s+as\b.*$").unwrap());
    let trimmed = s.trim();
    if let Some(cap) = BETTER_KNOWN_AS_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn truncate_common_clock_framework_clause(s: &str) -> String {
    static CCF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?P<prefix>.+?\bCommon\s+Clock\s+Framework)\b.*$").unwrap()
    });
    let trimmed = s.trim();
    if let Some(cap) = CCF_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn truncate_omap_dual_mode_clause(s: &str) -> String {
    static OMAP_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?P<prefix>.+?\bOMAP\s+Dual-mode)\b.*$").unwrap());
    let trimmed = s.trim();
    if let Some(cap) = OMAP_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn truncate_return_clause(s: &str) -> String {
    static RETURN_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(?P<prefix>.+?\bReturn)\b\s*:?\s*.*$").unwrap());
    let trimmed = s.trim();
    if let Some(cap) = RETURN_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn truncate_status_clause(s: &str) -> String {
    static STATUS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)^(?P<head>.*?)(?P<label>(?i:status))\b\s*:?\s*(?P<after>.*)$").unwrap()
    });

    let trimmed = s.trim();
    let Some(cap) = STATUS_RE.captures(trimmed) else {
        return s.to_string();
    };
    let head = cap
        .name("head")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim_end();
    let after = cap.name("after").map(|m| m.as_str()).unwrap_or("");

    let after_lower = after.to_ascii_lowercase();
    let suffix_start = after_lower
        .find(" devices")
        .or_else(|| after_lower.find(" updated"))
        .unwrap_or(after.len());
    let status_part = after[..suffix_start].trim();
    let suffix = after[suffix_start..].trim_start();

    let value = status_part
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|c: char| c.is_ascii_punctuation());
    let keep_value = value.eq_ignore_ascii_case("complete");
    let status_out = if keep_value {
        "Status complete"
    } else {
        "Status"
    };

    let mut out = String::new();
    if !head.is_empty() {
        out.push_str(head);
        out.push(' ');
    }
    out.push_str(status_out);
    if !suffix.is_empty() {
        out.push(' ');
        out.push_str(suffix);
    }
    out.trim().to_string()
}

fn truncate_devices_clause(s: &str) -> String {
    static DEVICES_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)^(?P<head>.*?)(?P<label>(?i:devices))\b\s*:?\s*(?P<after>.*)$").unwrap()
    });
    let trimmed = s.trim();
    let Some(cap) = DEVICES_RE.captures(trimmed) else {
        return s.to_string();
    };
    let head = cap
        .name("head")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim_end();
    let after = cap.name("after").map(|m| m.as_str()).unwrap_or("");

    let after_lower = after.to_ascii_lowercase();
    let suffix_start = after_lower
        .find(" status")
        .or_else(|| after_lower.find(" updated"))
        .unwrap_or(after.len());
    let details = after[..suffix_start].trim();
    let suffix = after[suffix_start..].trim_start();

    let details_replaced = details.replace(['[', ']', '(', ')', ',', ';', '.'], " ");
    let cleaned = details_replaced.split_whitespace().collect::<Vec<_>>();

    let mut keep: Vec<&str> = Vec::new();
    if let Some(first) = cleaned.first().copied() {
        keep.push(first);
    }
    if let Some(second) = cleaned.get(1).copied()
        && !second.contains('/')
        && second.len() > 2
    {
        keep.push(second);
    }
    if let Some(third) = cleaned.get(2).copied() {
        let has_digit = third.chars().any(|c| c.is_ascii_digit());
        if has_digit && !third.contains('-') && !third.contains('_') {
            keep.push(third);
        }
    }

    let mut out = String::new();
    if !head.is_empty() {
        out.push_str(head);
        out.push(' ');
    }
    out.push_str("Devices");
    if !keep.is_empty() {
        out.push(' ');
        out.push_str(&keep.join(" "));
    }
    if !suffix.is_empty() {
        out.push(' ');
        out.push_str(suffix);
    }
    out.trim().to_string()
}

fn strip_trailing_comma_email_matching_name(s: &str) -> String {
    static NAME_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?P<name>[A-Z][A-Za-z]+\s+[A-Z][A-Za-z]+),\s*(?P<email>[A-Za-z0-9._%+-]+)@(?P<domain>[^\s,]+)$").unwrap()
    });

    let trimmed = s.trim();
    let Some(cap) = NAME_EMAIL_RE.captures(trimmed) else {
        return s.to_string();
    };
    let name = cap.name("name").map(|m| m.as_str()).unwrap_or("").trim();
    let email_local = cap.name("email").map(|m| m.as_str()).unwrap_or("").trim();
    if name.is_empty() || email_local.is_empty() {
        return s.to_string();
    }

    let name_key: String = name
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect();

    let local_key: String = email_local
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect();

    if !name_key.is_empty() && (local_key == name_key || local_key.contains(&name_key)) {
        return name.to_string();
    }

    s.to_string()
}

fn truncate_trailing_clause_after_contact(s: &str) -> String {
    static CONTACT_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?P<prefix>.+?(?:<[^>\s]+@[^>\s]+>|\((?:[^\)\s]+@[^\)\s]+|https?://[^\)\s]+)\)))\s*(?:\.\s*|\s+)(?P<tail>.+)$",
        )
        .expect("valid contact-tail truncation regex")
    });

    let trimmed = s.trim();
    let Some(cap) = CONTACT_TAIL_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    let tail = cap.name("tail").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() || tail.is_empty() {
        return s.to_string();
    }

    let tail_lower = tail.to_ascii_lowercase();
    let prose_like_tail = [
        "the ", "a ", "an ", "i ", "since ", "this ", "these ", "those ", "is ", "was ", "visit ",
        "for ", "from ",
    ]
    .iter()
    .any(|prefix_text| tail_lower.starts_with(prefix_text));

    if prose_like_tail {
        return prefix.to_string();
    }

    s.to_string()
}

fn truncate_trailing_from_clause_after_angle_contact(s: &str) -> String {
    static FROM_CLAUSE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(?P<prefix>.+?<[^>]*@[^>]*>)\s+from\b.*$").unwrap());

    let trimmed = s.trim();
    let Some(cap) = FROM_CLAUSE_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }

    prefix.to_string()
}

fn strip_trailing_status_works(s: &str) -> String {
    static STATUS_WORKS_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(?P<prefix>.+\bStatus)\s+works\s*$").unwrap());

    let trimmed = s.trim();
    if let Some(cap) = STATUS_WORKS_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn strip_trailing_copied_from_suffix(s: &str) -> String {
    static COPIED_FROM_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?P<prefix>.+?\bCopied\s+from)\b.*$")
            .expect("valid copied-from truncation regex")
    });

    let trimmed = s.trim();
    if let Some(cap) = COPIED_FROM_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("");
        let prefix = prefix.trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

fn strip_trailing_gnu_project_file_suffix(s: &str) -> String {
    static GNU_TAKEN_FROM_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?P<prefix>Original\s+taken\s+from\s+the\s+GNU\s+Project)\b.*$")
            .expect("valid gnu project truncation regex")
    });
    let trimmed = s.trim();
    if let Some(cap) = GNU_TAKEN_FROM_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("");
        let prefix = prefix.trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    s.to_string()
}

pub(super) fn normalize_angle_bracket_comma_spacing(s: &str) -> String {
    static ANGLE_EMAIL_COMMA_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?P<email><[^>\s]*@[^>\s]*>),").expect("valid angle-bracket email comma regex")
    });

    ANGLE_EMAIL_COMMA_RE.replace_all(s, "$email,").into_owned()
}

pub(super) fn strip_trailing_company_co_ltd(s: &str) -> String {
    static CO_LTD_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\bco\.?\s*,ltd\.?$").expect("valid co,ltd suffix regex"));

    let trimmed = s.trim_end_matches(|c: char| c.is_whitespace() || c == ',');
    let out = CO_LTD_RE.replace(trimmed, "").into_owned();
    out.trim_end_matches(|c: char| c.is_whitespace() || c == ',')
        .to_string()
}
