// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Further copyright/holder clause strippers and header junk predicates.
//! Split from `copyright` to keep file sizes bounded; same responsibility.

use std::sync::LazyLock;

use regex::Regex;

use super::*;

pub(super) fn strip_leading_author_label_in_copyright(s: &str) -> String {
    if let Some(rest) = extract_copyright_assignment_value_after_author_assignment(s) {
        return synthesize_copyright_from_assignment_value(&rest);
    }

    static LEADING_AUTHOR_COPY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^(?:@?author)\s+(?P<rest>.+\(c\)\s*(?:19|20)\d{2}.*)$")
    });
    let trimmed = s.trim();
    let Some(cap) = LEADING_AUTHOR_COPY_RE.captures(trimmed) else {
        return s.to_string();
    };
    let rest = cap.name("rest").map(|m| m.as_str()).unwrap_or("").trim();
    if rest.is_empty() {
        return s.to_string();
    }
    rest.to_string()
}

pub(super) fn strip_leading_author_label_in_holder(s: &str) -> String {
    if let Some(rest) = extract_copyright_assignment_value_after_author_assignment(s) {
        return rest;
    }

    static LEADING_AUTHOR_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?:@?author)\b[:\s]+(?P<rest>.+)$"));
    let trimmed = s.trim();
    let Some(cap) = LEADING_AUTHOR_RE.captures(trimmed) else {
        return s.to_string();
    };
    let rest = cap.name("rest").map(|m| m.as_str()).unwrap_or("").trim();
    if rest.is_empty() {
        return s.to_string();
    }
    rest.to_string()
}

pub(super) fn prefix_has_holder_words(prefix: &str) -> bool {
    for raw in prefix.split_whitespace() {
        let token = raw.trim_matches(|c: char| c.is_ascii_punctuation() || matches!(c, '' | ''));
        if token.is_empty() {
            continue;
        }

        let lower = token.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "*" | "copyright" | "copr" | "(c)" | "c" | "\u{a9}"
        ) {
            continue;
        }

        // Ignore pure year-ish tokens.
        let yearish = token
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '+' | ','));
        if yearish {
            continue;
        }

        return true;
    }

    false
}

pub(super) fn strip_leading_licensed_material_of(s: &str) -> String {
    static LICENSED_MATERIAL_OF_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?:licensed\s+)?material\s+of\s+"));
    LICENSED_MATERIAL_OF_RE
        .replace(s, "")
        .trim_start()
        .to_string()
}

pub(super) fn strip_leading_version_number_before_c(s: &str) -> String {
    static VERSION_BEFORE_C_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)^\d+\.\d+(?:\.\d+)*\.?\s+(\(c\)|\bcopyright\b)")
    });
    // Use `captures` directly: a successful match always carries capture
    // group 1, so there is no separate fallible `find` + `unwrap` step.
    if let Some(cap) = VERSION_BEFORE_C_RE.captures(s) {
        let m = cap.get(0).expect("group 0 always present on a match");
        let keyword_start = m.start() + m.as_str().len() - cap[1].len();
        s[keyword_start..].trim_start().to_string()
    } else {
        s.to_string()
    }
}

/// Strip a trailing dangling pronoun left after a copyright/holder span ran one
/// token into the following sentence, e.g. `Copyright © 1994 David Burren. It`
/// or a holder `David Burren. It` (from "... David Burren. It is licensed ...",
/// where the node stopped at the pronoun). Only a subject pronoun sitting at the
/// very end after a sentence period is removed — no holder ends in a bare
/// pronoun — so determiner/proper-noun continuations that ScanCode keeps
/// (`Copyright (c) 1994-1999. The MITRE Corporation`) are untouched.
pub(super) fn strip_trailing_dangling_pronoun(s: &str) -> String {
    static DANGLING_PRONOUN_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?x)
            ^(?P<prefix>.+?)\.\s+
            (?:It|Its|This|These|Those|They|We|You|He|She|Their)
            \s*$
            ",
        )
    });
    let trimmed = s.trim();
    let Some(cap) = DANGLING_PRONOUN_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str().trim()).unwrap_or("");
    if prefix.is_empty() {
        return s.to_string();
    }
    prefix.to_string()
}

/// Strip a leading file/component descriptor that ends in a linking verb
/// immediately before the copyright marker, e.g.
/// `The ARM memcpy code (src/string/arm/memcpy.S) is Copyright (c) 2008 ...`
/// or `src/regex/tre*) is Copyright (c) 2001-2008 Ville Laurikari`.
///
/// A holder name never ends in `is`/`was`/`are`, so anchoring the statement at
/// the copyright marker mirrors ScanCode, whose grammar never folds a run of
/// common-noun prose ahead of `<COPY>`. Two guards keep it narrow: the discarded
/// lead must not itself contain a copyright marker, and it must carry a
/// path/parenthetical descriptor (`(`, `)`, `/`, `*`). The path requirement is
/// what distinguishes musl's `The <component> (<path>) is Copyright ...` from a
/// standard notice preamble such as MPL's `Portions created by the Initial
/// Developer are Copyright ...`, which ScanCode keeps verbatim and which has no
/// such descriptor.
pub(super) fn strip_leading_prose_clause_before_copyright(s: &str) -> String {
    static LEADING_PROSE_BEFORE_COPY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            ^(?P<lead>.+?\b(?:is|was|are))\s+
            (?P<copy>(?:(?:copyright|copr\.?)\b|\u{00a9}|\(c\)).*)$
            ",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = LEADING_PROSE_BEFORE_COPY_RE.captures(trimmed) else {
        return s.to_string();
    };
    let lead = cap.name("lead").map(|m| m.as_str()).unwrap_or("");
    let copy = cap.name("copy").map(|m| m.as_str()).unwrap_or("").trim();
    let lead_lower = lead.to_ascii_lowercase();
    let lead_has_path_descriptor = lead.contains(['(', ')', '/', '*']);
    if copy.is_empty()
        || !lead_has_path_descriptor
        || lead_lower.contains("copyright")
        || lead_lower.contains("copr")
        || lead.contains("(c)")
        || lead.contains('\u{00a9}')
    {
        return s.to_string();
    }
    copy.to_string()
}

pub(super) fn strip_trailing_authors_clause(s: &str) -> String {
    static AUTHORS_CLAUSE_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"^(?P<prefix>.+?)\s+Authors?\b\s+(?P<rest>.+)$"));

    let trimmed = s.trim();

    let Some(cap) = AUTHORS_CLAUSE_RE.captures(trimmed) else {
        return s.to_string();
    };

    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("");
    let rest = cap.name("rest").map(|m| m.as_str()).unwrap_or("");
    if prefix.trim().is_empty() || rest.trim().is_empty() {
        return s.to_string();
    }

    // `<Holder> Project Authors (https://...)` / `... Authors <contact@host>` is
    // the SIL OFL copyright holder plus its contact URL/email, not a trailing
    // author-list clause — ScanCode keeps it whole. When the text after `Authors`
    // is only such a contact (optionally parenthesized, no other prose), leave the
    // statement intact so the OFL holder is not truncated.
    let rest_contact = rest
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let rest_is_only_contact = !rest_contact.contains(char::is_whitespace)
        && (rest_contact.starts_with("http://")
            || rest_contact.starts_with("https://")
            || (rest_contact.contains('@') && rest_contact.contains('.')));
    if rest_is_only_contact {
        return s.to_string();
    }

    let rest_for_count = if let Some(email_idx) = rest.find('@') {
        rest[..email_idx].trim()
    } else {
        rest.trim()
    };

    let words_before_email = rest_for_count
        .split_whitespace()
        .filter(|w| w.chars().any(|c| c.is_alphabetic()) && !w.contains('<') && !w.contains('>'))
        .count();
    if words_before_email > 2 {
        return s.to_string();
    }

    let prefix_trimmed = prefix.trim();
    let prefix_last_is_year = prefix_trimmed
        .split_whitespace()
        .last()
        .is_some_and(|w| w.chars().all(|c| c.is_ascii_digit()));
    if !prefix_trimmed.contains(',') && !prefix_last_is_year {
        return s.to_string();
    }

    prefix_trimmed
        .trim_end_matches(&[',', ';', ':'][..])
        .trim()
        .to_string()
}

pub(super) fn strip_trailing_document_authors_clause(s: &str) -> String {
    static DOCUMENT_AUTHORS_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>.+?)\s+and\s+the\s+persons\s+identified\s+as\s+document\s+authors\.?$",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = DOCUMENT_AUTHORS_RE.captures(trimmed) else {
        return s.to_string();
    };

    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() || !prefix_has_holder_words(prefix) {
        return s.to_string();
    }

    prefix
        .trim_end_matches(&[',', ';', ':', ' '][..])
        .trim()
        .to_string()
}

pub(super) fn strip_trailing_et_al(s: &str) -> String {
    static ET_AL_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?)\s*,?\s*et\s+al\.?\s*$"));

    let trimmed = s.trim();
    let Some(cap) = ET_AL_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("");
    prefix.trim().trim_end_matches(',').trim().to_string()
}

pub(super) fn strip_trailing_parenthesized_descriptor_after_by_holder(s: &str) -> String {
    static DESCRIPTOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>Copyright\s*\(c\)\s*(?:19\d{2}|20\d{2})(?:\s*[-–]\s*(?:19\d{2}|20\d{2}|\d{2}))?\s+by\s+.+?)\s*\(\s*(?P<paren>[A-Za-z][A-Za-z\s-]{2,64})\s*\)\s*$",
        )
    });

    let trimmed = s.trim();
    let Some(cap) = DESCRIPTOR_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    let desc = cap.name("paren").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() || desc.is_empty() {
        return s.to_string();
    }

    if !is_trailing_component_descriptor(desc) {
        return s.to_string();
    }

    prefix.to_string()
}

pub(super) fn strip_trailing_component_descriptor_from_holder(s: &str) -> String {
    static PAREN_DESCRIPTOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"^(?P<prefix>.+?)\s*\(\s*(?P<desc>[A-Za-z][A-Za-z\s-]{2,64})\s*\)\s*$",
        )
    });
    static TRAILING_COMPONENT_DESCRIPTOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)\s+(?P<desc>(?:[A-Za-z]+\s+)?noise(?:\s+and\s+others)?)$")
    });

    let trimmed = s.trim();

    if let Some(cap) = PAREN_DESCRIPTOR_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        let desc = cap.name("desc").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() && is_trailing_component_descriptor(desc) {
            return prefix.to_string();
        }
    }

    if let Some(m) = TRAILING_COMPONENT_DESCRIPTOR_RE.find(trimmed) {
        let prefix = trimmed[..m.start()].trim();
        let desc = m.as_str().trim();
        if !prefix.is_empty() && is_trailing_component_descriptor(desc) {
            return prefix.to_string();
        }
    }

    s.to_string()
}

pub(super) fn strip_trailing_contributor_clause(s: &str) -> String {
    static CONTRIBUTOR_CLAUSE_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?)\s+Contributor:?\s+.+$"));

    let trimmed = s.trim();
    let Some(cap) = CONTRIBUTOR_CLAUSE_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }
    prefix
        .trim_end_matches(&[',', ';', ':', ' '][..])
        .trim()
        .to_string()
}

pub(super) fn strip_trailing_contact_clause(s: &str) -> String {
    static CONTACT_CLAUSE_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?)\s+Contact:?\s+.+$"));

    let trimmed = s.trim();
    let Some(cap) = CONTACT_CLAUSE_RE.captures(trimmed) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }
    prefix
        .trim_end_matches(&[',', ';', ':', ' '][..])
        .trim()
        .to_string()
}

pub(super) fn strip_trailing_holder_prose_clause(s: &str) -> String {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    for marker in [
        " and it is hereby released to the",
        " it is hereby released to the",
        ", are derived from ",
        " are derived from ",
        " and is licensed under ",
        " and labeled as such",
    ] {
        if let Some(idx) = lower.find(marker) {
            let prefix = trimmed[..idx]
                .trim_end_matches(&[',', ';', ':', ' '][..])
                .trim();
            if !prefix.is_empty() && prefix_has_holder_words(prefix) {
                return prefix.to_string();
            }
        }
    }

    s.to_string()
}

pub(super) fn strip_trailing_or_suffix(s: &str) -> String {
    static TRAILING_OR_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>copyright\b.+?)\s+or\s*$"));

    let trimmed = s.trim();
    let Some(cap) = TRAILING_OR_RE.captures(trimmed) else {
        return s.to_string();
    };
    cap.name("prefix")
        .map(|m| m.as_str().trim_end().to_string())
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn strip_trailing_x509_dn_fields(s: &str) -> String {
    static X509_DN_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>Copyright\s*\(c\)\s*\d{4}(?:\s*,\s*OU\s+[^,]+|\s+[^,]+))(?:\s*,\s*(?:OU|CN|O|C|L|ST)\s+.+)$",
        )
    });
    static OU_ENDORSED_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>Copyright\s*\(c\)\s*\d{4}\s*,\s*OU\s+.+?)\s+endorsed\s*$",
        )
    });

    let Some(cap) = X509_DN_TAIL_RE.captures(s.trim()) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() {
        return s.to_string();
    }
    if let Some(cap2) = OU_ENDORSED_TAIL_RE.captures(prefix) {
        cap2.name("prefix")
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| prefix.to_string())
    } else {
        prefix.to_string()
    }
}

pub(super) fn strip_independent_jpeg_groups_software_tail(s: &str) -> String {
    static JPEG_GROUP_SOFTWARE_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?i)\b(Independent JPEG Group's)\s+software\b\.?$")
    });
    JPEG_GROUP_SOFTWARE_RE.replace(s, "$1").trim().to_string()
}

pub(super) fn strip_trailing_original_authors(s: &str) -> String {
    static ORIGINAL_AUTHORS_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(.*\bthe original)\s+authors\b\s*$"));
    if let Some(cap) = ORIGINAL_AUTHORS_RE.captures(s) {
        cap[1].trim().to_string()
    } else {
        s.to_string()
    }
}

pub(super) fn strip_trailing_bug_reports_after_year_only_copyright(s: &str) -> String {
    static BUG_REPORTS_COPY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)^.*?Copyright\s*\((?:c|C)\)\s*(?P<year>19\d{2}|20\d{2})\.?\s+Send\s+bug\s+reports\b.*$",
        )
    });

    BUG_REPORTS_COPY_RE
        .captures(s.trim())
        .and_then(|cap| {
            cap.name("year")
                .map(|m| format!("Copyright (c) {}", m.as_str().trim()))
        })
        .unwrap_or_else(|| s.to_string())
}

pub(super) fn strip_trailing_paren_email_after_c_by(s: &str) -> String {
    static C_BY_PAREN_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>(?:Copyright\s+)?\(c\)\s+by\s+[^()]+?)\s*\([^()]*@[^()]*\)\s*$",
        )
    });

    if let Some(caps) = C_BY_PAREN_EMAIL_RE.captures(s) {
        caps.name("prefix")
            .map(|m| normalize_whitespace(m.as_str().trim()))
            .unwrap_or_else(|| s.to_string())
    } else {
        s.to_string()
    }
}

pub(super) fn strip_contributor_parens_after_org(s: &str) -> String {
    static ORG_PARENS_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"^(?P<prefix>.*)\(\s*(?P<inner>[^()]+?)\s*\)\s*$"));

    let Some(cap) = ORG_PARENS_RE.captures(s.trim()) else {
        return s.to_string();
    };
    let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
    let inner = cap.name("inner").map(|m| m.as_str()).unwrap_or("").trim();
    if prefix.is_empty() || inner.is_empty() {
        return s.to_string();
    }

    let inner_lower = inner.to_ascii_lowercase();
    let looks_like_contributor_list = inner_lower.contains(" and ") || inner.contains('<');
    if !looks_like_contributor_list {
        return s.to_string();
    }

    normalize_whitespace(&format!("{prefix} {inner}"))
}

pub(super) fn strip_angle_bracketed_www_domains_without_by(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if lower.contains(" by ") {
        return s.to_string();
    }

    static WWW_IN_COMMA_CLAUSE_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i),\s*<www\.[^>]+>\s*"));
    static WWW_TRAILING_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\s*<www\.[^>]+>\s*$"));

    let s = WWW_IN_COMMA_CLAUSE_RE.replace_all(s, ", ");
    let s = WWW_TRAILING_RE.replace(&s, "");
    normalize_whitespace(s.trim())
}

pub(super) fn strip_angle_bracketed_www_domains(s: &str) -> String {
    static WWW_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\s*<www\.[^>]+>\s*"));

    let s = WWW_RE.replace_all(s, " ");
    normalize_whitespace(s.trim())
}

pub(super) fn strip_trailing_mountain_view_ca(s: &str) -> String {
    static MOUNTAIN_VIEW_CA_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\bMountain View\s*,\s*CA\.?$"));

    if MOUNTAIN_VIEW_CA_RE.is_match(s) {
        MOUNTAIN_VIEW_CA_RE
            .replace(s, "Mountain View")
            .trim()
            .to_string()
    } else {
        s.to_string()
    }
}

pub(super) fn strip_trailing_isc_after_inc(s: &str) -> String {
    static TRAILING_ISC_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?P<prefix>.+?\bInc\.?)\s+ISC\s*$"));
    if let Some(cap) = TRAILING_ISC_RE.captures(s.trim()) {
        cap.name("prefix")
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| s.to_string())
    } else {
        s.to_string()
    }
}

pub(super) fn strip_trailing_caps_after_company_suffix(s: &str) -> String {
    static TRAILING_CAPS_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"^(?P<prefix>.+?\b(?:Corp|Inc|Ltd|LLC|Co)\.)\s+[A-Z]{2,}\s*$")
    });
    if let Some(cap) = TRAILING_CAPS_RE.captures(s.trim()) {
        cap.name("prefix")
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| s.to_string())
    } else {
        s.to_string()
    }
}

pub(super) fn strip_trailing_comma_after_respective_authors(s: &str) -> String {
    let trimmed = s.trim_end_matches(char::is_whitespace);
    let lower = trimmed.to_ascii_lowercase();
    if lower.ends_with("respective authors,") {
        let mut t = trimmed.to_string();
        if t.ends_with(',') {
            t.pop();
        }
        t.trim_end_matches(char::is_whitespace).to_string()
    } else {
        s.to_string()
    }
}

pub(super) fn strip_leading_simple_copyright_prefixes(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if (lower.starts_with("program copyright") || lower.starts_with("debian copyright"))
        && let Some(idx) = lower.find("copyright")
    {
        return s[idx..].trim_start().to_string();
    }

    if lower.contains("debian copyright")
        && let Some(idx) = lower.rfind("copyright")
    {
        let tail = s[idx..].trim_start();
        if tail.to_ascii_lowercase().starts_with("copyright") {
            return tail.to_string();
        }
    }

    if lower.starts_with("the ")
        && let Some(idx) = lower.rfind(". copyright")
        && idx + 2 < s.len()
    {
        let tail = s[(idx + 2)..].trim_start();
        if tail.to_ascii_lowercase().starts_with("copyright") {
            return tail.to_string();
        }
    }

    s.to_string()
}

pub(super) fn is_junk_copyright_of_header(s: &str) -> bool {
    let lower = s.to_lowercase();
    let prefix = "copyright of";
    if !lower.starts_with(prefix) {
        return false;
    }

    let mut tail = s[prefix.len()..].trim();
    tail = tail.trim_matches(&[':', '-', ' ', '\t'][..]);
    if tail.is_empty() {
        return true;
    }

    let tail_lower = tail.to_lowercase();
    if tail_lower.starts_with("qt has been transferred") {
        return true;
    }
    if tail_lower.starts_with("version of nameif") {
        return true;
    }
    if tail_lower.contains("full text of") {
        return true;
    }

    if tail.contains('/') {
        return true;
    }

    !tail.chars().any(|c| c.is_ascii_uppercase())
}

pub(super) fn strip_leading_js_project_version(s: &str) -> String {
    static JS_PROJECT_VERSION_PREFIX_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^[a-z0-9_.-]+\.js\s+\d+\.\d+(?:\.\d+)?\s+"));

    JS_PROJECT_VERSION_PREFIX_RE
        .replace(s, "")
        .trim()
        .to_string()
}

pub(super) fn truncate_trailing_boilerplate(s: &str) -> String {
    static TRAILING_BOILERPLATE_RE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        let patterns = [
            r"(?i)\bDistributed in the hope\b",
            r"(?i)\bMay be used\b",
            r"(?i)\bLicense-Alias\b",
            r"(?i)\bFull text of\b",
            r"(?i)\s+-\s*icon support\b",
            r"(?i)\s+-\s*maintainer\b",
            r"(?i)\s+-\s*software\b",
            r"(?i)\.\s*Software\.?$",
            r"(?i),+\s*Software\b",
            r"(?i)\bwrite\s+to\s+the\s+Free\s+Software\s+Foundation\b",
            r"(?i)\b51\s+Franklin\s+(?:Street|St)\b",
            r"(?i)\b675\s+Mass\s+Ave\b",
            r"(?i)\b901\s+San\s+Antonio\s+Road\b",
            r"(?i)\b2601\s+Elliott\s+Avenue\b",
            r"(?i)\bKoll\s+Center\s+Parkway\b",
            r"(?i)\bGNU\s+GENERAL\s+PUBLIC\s+LICENSE\b",
            r"(?i)\s+GNU\s*$",
            r"(?i)\.\s*print\s*$",
            r"(?i)\bTheir\s+notice\s+is\s+reproduced\s+below\b",
            r"(?i)\bTheir\s+notice\s+reproduced\s+below\b",
            r"(?i)\bTheir\s+notice\s+reproduced\s+below\s+in\s+its\s+entirety\b",
            r"(?i)\band/or\s+its\s+suppliers?\b",
            r"(?i)\bNOTE\s+Sort\b",
            r"(?i)\bdocumentation\s+generated\s+by\b",
            r"(?i)\(\s*The full list is in\b",
            r#"(?i)\(\s*the\s+['"]?original\s+author['"]?\s*\)\s+and\s+additional\s+contributors\b"#,
            r"(?i)\bthe\s+original\s+author\b\s+and\s+additional\s+contributors\b",
            r"\becho\s+",
            r"(?i)\bv\d+\.\d+\s*$",
            r"(?i)\bassigned\s+to\s+the\s+",
            r"(?i)\bHP\s+IS\s+AGREEING\b",
            r"(?i)\bCA\.\s*ansi2knr\b",
            r"(?i)\bDirect\s+questions\b",
            r"(?i)\bkbd\s+driver\b",
            r"(?i)\bMIDI\s+driver\b",
            r"(?i)\bLZO\s+version\b",
            r"(?i)\bpersistent\s+bitmap\b",
            r"(?i)\bLIBERATION\b",
            r"(?i)\bAHCI\s+SATA\b",
            r"(?i)\bDTMF\s+code\b",
            r"\bOPTIONS\s*$",
            r"(?i)\bindexing\s+(?:porting|code)\b",
            r"(?i)\bvortex\b",
            r"(?i)\bLinuxTV\b",
            r"(?i)-\s*OMAP\d",
            r"\bGDB\b",
            r"(?i)\band\s+software/linux\b",
            r"(?i),\s+by\s+Paul\s+Dale\b",
            r"(?i),?\s+and\s+other\s+parties\b",
            r"(?i)\b\d+\s+Parnell\s+St\b",
            r"(?i)\b\d+\s+Main\s+(?:street|st)\b",
            r"(?i)\b\d+\s+Koll\s+Center\s+Parkway\b",
            r"(?i)\bBeverly\s+Hills\b",
            r"(?i)\bBerverly\s+Hills\b",
            r"(?i)\bDublin\s+\d\b",
            r"(?i)\band\s+Bob\s+Dougherty\b",
            r"(?i)\band\s+is\s+licensed\s+under\b",
            r"(?i)\band\s+it\s+is\s+hereby\s+released\b",
            r"(?i)\bBEGIN\s+LICENSE\s+BLOCK\b",
            r"(?i)^NOTICE,\s*DISCLAIMER,\s*and\s*LICENSE\b",
            r"(?i)\bIn\s+the\s+event\s+of\b",
            r"(?i),\s*ALL\s+RIGHTS\s+RESERVED\b",
            r"(?i)\s+All\s+rights\s+reserved\b",
            r"(?i)\s+All\s+rights\b",
            r"(?i),\s*THIS\s+SOFTWARE\s+IS\b",
            r"(?i),?\s+member\s+of\s+The\s+XFree86\s+Project\b",
            r"(?i)\s+Download\b",
            r"(?i)\bThis\s+code\s+is\s+GPL\b",
            r"(?i)\bGPLd\b",
            r"(?i)\bPlaced\s+under\s+the\s+GNU\s+GPL\b",
            r"(?i)\bSee\s+the\s+GNU\s+GPL\b",
            r"(?i)\bFor\s+other\s+copyrights\b",
            r"(?i)\bLast\s+modified\b",
            r"(?i)\(\s*the\s+original\s+version\s*\)\s*$",
            r"(?i)\bavalable\s+at\b",
            r"(?i)\bavailable\s+at\b",
            r"(?i),\s+and\s+are\s*$",
            r"(?i)\bNIN\s+logo\b",
            r"(?i),\s+with\s*$",
            r"(?i)\(\s*(?:written|brushed)\b[^)]*\)\s*$",
            r"(?i)\(\s*[^)]*implementation[^)]*\)\s*$",
            r"(?i)\bThis\s+file\s+is\s+licensed\s+under\b",
            r"(?i)\bLicensing\s+details\s+are\s+in\b",
            r"(?i)\bLinux\s+for\s+Hitachi\s+SuperH\b",
            r"(?i)\.\s*OProfile\s*$",
        ];
        // `patterns` is a fixed array of compile-time string literals, so each
        // compiles infallibly via the documented static-regex helper.
        patterns.into_iter().map(compile_static_regex).collect()
    });

    let mut cut: Option<usize> = None;
    for re in TRAILING_BOILERPLATE_RE.iter() {
        if let Some(m) = re.find(s) {
            cut = Some(cut.map_or(m.start(), |c| c.min(m.start())));
        }
    }

    if let Some(idx) = cut {
        s[..idx]
            .trim()
            .trim_matches(&['-', ',', ';'][..])
            .trim()
            .to_string()
    } else {
        s.trim().to_string()
    }
}

pub(super) fn is_junk_copyrighted_works_header(s: &str) -> bool {
    let lower = s.to_lowercase();
    let prefix = "copyrighted works";
    if !lower.starts_with(prefix) {
        return false;
    }

    let mut tail = s[prefix.len()..].trim();
    tail = tail.trim_matches(&[':', '-', ' ', '\t'][..]);
    if tail.is_empty() {
        return true;
    }

    let tail_lower = tail.to_lowercase();
    let rest = if tail_lower == "of" {
        return true;
    } else if tail_lower.starts_with("of ") {
        tail[2..].trim()
    } else {
        return true;
    };

    if rest.is_empty() {
        return true;
    }

    !rest.chars().any(|c| c.is_ascii_uppercase())
}

pub(super) fn is_junk_copyrighted_software_phrase(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.eq_ignore_ascii_case("copyrighted software")
        || trimmed.eq_ignore_ascii_case("copyright holders")
        || trimmed.eq_ignore_ascii_case("the above copyright holders")
}

pub(super) fn strip_trailing_company_name_placeholder(s: &str) -> String {
    static COMPANY_NAME_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)(\bCOMPANY)\s+NAME\s*$"));
    COMPANY_NAME_RE.replace(s, "$1").trim().to_string()
}

pub(super) fn strip_leading_portions_comma(s: &str) -> String {
    static LEADING_PORTIONS_COMMA_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)^(?:portions?|parts?)\s*,\s*"));
    LEADING_PORTIONS_COMMA_RE.replace(s, "").trim().to_string()
}

pub(super) fn strip_trailing_paren_identifier(s: &str) -> String {
    static TRAILING_PAREN_ID_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"\s+\([a-z][a-z0-9]{3,}\)\s*$"));
    static TRAILING_PAREN_ID_COMMA_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"\s+\([a-z][a-z0-9]{3,}\),\s*[a-z][a-z0-9]*\.?\s*$")
    });
    let s = TRAILING_PAREN_ID_COMMA_WORD_RE.replace(s, "");
    TRAILING_PAREN_ID_RE.replace(&s, "").trim().to_string()
}

pub(super) fn strip_trailing_portions_of(s: &str) -> String {
    static TRAILING_PORTIONS_OF_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"(?i)\b(?:some\s+)?(?:portions?|parts?)\s+of$"));
    TRAILING_PORTIONS_OF_RE.replace(s, "").trim().to_string()
}

pub(super) fn strip_trailing_short_surname_paren_list_in_holder(s: &str) -> String {
    static SHORT_SURNAME_PAREN_LIST_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"^(?P<first>[\p{Lu}][\p{L}'-]+)\s+(?:[\p{Lu}][\p{Ll}])\s*\([^)]*\)\s*,\s*.+$",
        )
    });

    let trimmed = s.trim();
    if let Some(cap) = SHORT_SURNAME_PAREN_LIST_RE.captures(trimmed) {
        cap.name("first")
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| s.to_string())
    } else {
        s.to_string()
    }
}

pub(super) fn strip_trailing_short_surname_paren_list_in_copyright(s: &str) -> String {
    static SHORT_SURNAME_PAREN_LIST_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?i)^(?P<prefix>Copyright\s+\((?:c|C)\)\s+\d{4}(?:-\d{4})?)\s+(?P<first>[\p{Lu}][\p{L}'-]+)\s+(?:[\p{Lu}][\p{Ll}])\s*\([^)]*\)\s*,\s*.+$",
        )
    });

    let trimmed = s.trim();
    if let Some(cap) = SHORT_SURNAME_PAREN_LIST_RE.captures(trimmed) {
        let prefix = cap.name("prefix").map(|m| m.as_str()).unwrap_or("").trim();
        let first = cap.name("first").map(|m| m.as_str()).unwrap_or("").trim();
        if !prefix.is_empty() && !first.is_empty() {
            return normalize_whitespace(&format!("{prefix} {first}"));
        }
    }
    s.to_string()
}
