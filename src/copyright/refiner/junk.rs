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

/// Return true if `s` is a fragment of machine subword-tokenizer data rather
/// than a real copyright, holder, or author.
///
/// Subword-tokenizer artifacts (e.g. Hugging Face `merges.txt` BPE merge tables
/// and `vocab.json` vocabularies) embed the `©` symbol and the literal word
/// `copyright` as tokens, which the detector would otherwise surface as dozens of
/// spurious notices. Two signals are decisive and never occur in genuine
/// copyright text: the BPE end-of-word marker `</w>`, and a line that is wholly a
/// sequence of JSON `"token": <id>` vocabulary entries (e.g. `"©": 102,`).
///
/// The JSON check is anchored to the whole (trimmed) line so that a genuine
/// notice which merely *contains* a `"key": 5` fragment (e.g.
/// `Copyright 2024 Foo, "version": 5`) is never mistaken for tokenizer data — a
/// real `vocab.json` line is nothing but quoted-token/integer-id pairs.
pub(crate) fn is_tokenizer_data_fragment(s: &str) -> bool {
    static JSON_TOKEN_ID_LINE_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r#"^\{?\s*("[^"]*"\s*:\s*-?\d+\s*,?\s*)+\}?$"#));
    s.contains("</w>") || JSON_TOKEN_ID_LINE_RE.is_match(s.trim())
}

/// Return true if `content` is a Byte-Pair-Encoding (BPE) merges table, such as
/// a Hugging Face / GPT-2 tokenizer `merges.txt`.
///
/// These files open with a `#version:` header and list one whitespace-separated
/// token pair per line. They embed the `©` symbol and accented/mojibake byte
/// pairs (e.g. `Â ©`, `pok Ã©`) as merge rules, which the detector would
/// otherwise mistake for copyright notices — but a merge table never contains a
/// genuine copyright statement. The `#version:` header plus the strict two-token
/// line shape make this signature specific enough that it cannot match ordinary
/// source or text files.
pub(crate) fn looks_like_bpe_merges_table(content: &str) -> bool {
    let mut lines = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());

    if !lines
        .next()
        .is_some_and(|first| first.starts_with("#version:"))
    {
        return false;
    }

    // Every merge rule is exactly two whitespace-separated tokens. Sample a
    // bounded prefix and require all sampled rules to match the pair shape.
    let mut sampled = 0;
    for line in lines.take(64) {
        if line.split_whitespace().count() != 2 {
            return false;
        }
        sampled += 1;
    }
    sampled > 0
}

/// Return true if `content` is a Hugging Face `tokenizers` `tokenizer.json`
/// model file carrying a Byte-Pair-Encoding merges table.
///
/// Unlike `merges.txt`, this format has no `#version:` header: it is a single
/// JSON document whose `"model"` embeds a `"vocab"` map and a `"merges"` array of
/// space-separated token pairs (often with `©`/mojibake bytes such as
/// `"pok Ã©"`), none of which is a genuine copyright notice. Requiring all three
/// quoted JSON markers — `"merges"`, `"vocab"`, and the `"BPE"` model type — in a
/// `{`-led document keeps the signature specific to tokenizer model files and
/// away from ordinary JSON or prose that merely mentions those words.
pub(crate) fn looks_like_hf_tokenizer_json(content: &str) -> bool {
    content.trim_start().starts_with('{')
        && content.contains("\"merges\"")
        && content.contains("\"vocab\"")
        && content.contains("\"BPE\"")
}

/// The 1-based, inclusive line span of the `"merges"` array inside a Hugging
/// Face `tokenizer.json`, or `None` if `content` is not such a file or has no
/// merges array.
///
/// Detections inside this span are BPE merge-table junk (the mojibake `©`
/// byte-pairs); everything else in the document — including any real
/// `"copyright"`/`"license"` notice field — is left to normal detection, so the
/// suppression is scoped to the merge table rather than the whole file. The
/// array end is found by tracking `[`/`]` depth outside of double-quoted
/// strings, so merge tokens that themselves contain a bracket do not confuse it.
pub(crate) fn hf_tokenizer_merges_line_span(content: &str) -> Option<(usize, usize)> {
    if !looks_like_hf_tokenizer_json(content) {
        return None;
    }

    let start_line = content
        .lines()
        .position(|line| line.contains("\"merges\""))?;

    let mut depth = 0i32;
    let mut entered = false;
    for (offset, line) in content.lines().skip(start_line).enumerate() {
        let mut in_string = false;
        let mut escaped = false;
        for ch in line.chars() {
            if in_string {
                match ch {
                    _ if escaped => escaped = false,
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '[' => {
                    depth += 1;
                    entered = true;
                }
                ']' => {
                    depth -= 1;
                    if entered && depth == 0 {
                        return Some((start_line + 1, start_line + offset + 1));
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Return true if a candidate is running article prose that a bare `copyright`
/// token swept across a sentence boundary: a lowercase word closed by a period
/// and followed by a capitalized word (`... by default. That is ...`).
///
/// A genuine multi-clause notice names a proper-noun holder *before* that first
/// sentence boundary (`(c) Example Corp. and affiliates. Confidential ...`);
/// article prose that merely discusses copyright does not (`copyright by
/// default. That is ...`). Only the latter is treated as junk. The
/// leading-whitespace requirement also keeps email/URL TLDs
/// (`...ecp.fr. Copyright`) and company/initial abbreviations (`Inc.`, `A.`)
/// out of scope, since those are not whitespace-delimited all-lowercase words.
pub(super) fn spans_prose_sentence_boundary(s: &str) -> bool {
    // The word closing the sentence is either an all-lowercase prose word
    // (`... by default. That ...`) or an all-caps acronym (`... to MIT. In ...`,
    // `... MIT). Ma ...`). The following prefix-holder check is what keeps a
    // genuine multi-clause notice — which names a proper noun before this point —
    // from being classified as prose.
    static SENTENCE_BOUNDARY_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(r"(?:^|\s)(?P<word>\p{Ll}[\p{Ll}']+|\p{Lu}{2,})\)?\.\s+\p{Lu}")
    });
    let Some(caps) = SENTENCE_BOUNDARY_RE.captures(s) else {
        return false;
    };
    // Check the text up to and including the sentence-closing word, so a
    // collective-agent holder that is itself that word (`... the original
    // authors. Redistribution ...`) still counts as a named holder.
    let word_end = caps.name("word").expect("word group").end();
    !prefix_names_holder(&s[..word_end])
}

/// Whether the text before the first sentence boundary names a holder: a
/// capitalized proper noun, or a lowercase collective-agent word (`authors`,
/// `contributors`, ...). The leading copyright-marker words are excluded so a
/// bare `Copyright`/`Portions` does not masquerade as the holder. The collective
/// clause keeps a real lowercase notice — `Copyright by the original authors.
/// Redistribution is permitted.` — from being classified as prose, mirroring the
/// `allow_single_word_contributors` holder rule. `holders` is deliberately not a
/// collective agent: `copyright holders. If you're the sole ...` is prose.
fn prefix_names_holder(prefix: &str) -> bool {
    const COLLECTIVE_AGENTS: &[&str] = &[
        "authors",
        "contributors",
        "developers",
        "maintainers",
        "committers",
        "team",
        "project",
        "community",
        "foundation",
    ];
    prefix
        .split_whitespace()
        .filter_map(|w| {
            let lw = w
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_ascii_lowercase();
            (!matches!(
                lw.as_str(),
                "copyright" | "copyrighted" | "copr" | "copyrights" | "c" | "portions" | "portion"
            ))
            .then_some((w, lw))
        })
        .any(|(w, lw)| {
            w.chars().next().is_some_and(|c| c.is_uppercase())
                || COLLECTIVE_AGENTS.contains(&lw.as_str())
        })
}

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
        || spans_prose_sentence_boundary(s)
        || contains_unicode_segmentation_markers(s)
        || looks_like_source_code(s)
}

/// Return true if a candidate carries Unicode segmentation-test markers — the
/// division sign `÷` (U+00F7) or multiplication sign `×` (U+00D7). Unicode
/// Character Database break-test data (`WordBreakTest.txt`,
/// `GraphemeBreakTest.txt`) embeds the copyright codepoint `00A9`/`©` as literal
/// test input on lines dense with these break markers; the detector otherwise
/// manufactures a holder from the surrounding data. These markers never occur in
/// a real copyright notice, so their presence alone identifies the data line.
pub(super) fn contains_unicode_segmentation_markers(s: &str) -> bool {
    s.contains('\u{00f7}') || s.contains('\u{00d7}')
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

pub(crate) fn has_copyright_year(s: &str) -> bool {
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
    AUTHORS_JUNK_PATTERNS.iter().any(|re| re.is_match(s)) || looks_like_source_code(s)
}

/// Return true if `s` matches any known junk holder pattern.
pub(crate) fn is_junk_holder(s: &str) -> bool {
    HOLDERS_JUNK_PATTERNS.iter().any(|re| re.is_match(s))
        || is_junk_holder_code_fragment(s)
        || is_junk_holder_symbol_garbage(s)
        || is_creative_commons_license_prose(s)
        || looks_like_source_code(s)
        || starts_with_sentence_connective(s)
        || contains_unicode_segmentation_markers(s)
        || s.eq_ignore_ascii_case("MIT")
}

/// Whether a holder opens with a subordinating conjunction or clause pronoun
/// (`If you're the sole`, `When the ...`, `This is ...`). These begin a sentence
/// clause, never a party name, so a holder that starts with one is running prose
/// the copyright marker swept in. Determiners that legitimately open real
/// holders (`The Regents`, `A. Person`) are deliberately excluded.
fn starts_with_sentence_connective(s: &str) -> bool {
    const CONNECTIVES: &[&str] = &[
        "if",
        "when",
        "while",
        "because",
        "although",
        "though",
        "since",
        "whether",
        "unless",
        "however",
        "therefore",
        "thus",
        "hence",
        "moreover",
        "furthermore",
        "otherwise",
        "nevertheless",
        "this",
        "that",
        "these",
        "those",
    ];
    let Some(first) = s.split_whitespace().next() else {
        return false;
    };
    let word = first.trim_matches(|c: char| !c.is_alphanumeric());
    // A single capitalized connective word is not itself a holder; require a
    // following word so `This`/`That` used as a real (rare) token is still safe.
    if s.split_whitespace().count() < 2 {
        return false;
    }
    CONNECTIVES.contains(&word.to_ascii_lowercase().as_str())
        && word.chars().next().is_some_and(|c| c.is_uppercase())
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

/// Return true if `s` is obviously a fragment of source code rather than a
/// copyright notice, holder, or author name.
///
/// Detected authors/copyrights/holders are sometimes lifted out of program
/// source (Ruby `Author::` headers near code, ORM model definitions such as
/// `Author.objects.create(...)`, C++ API calls such as
/// `vk::CmdCopyImage(..., &copyRegion);`). These carry method-call, operator,
/// or namespace syntax that never appears in a real name or notice, so we can
/// reject them conservatively.
///
/// This is intentionally strict about what counts as code: real legal notices
/// and names use commas, periods, `Inc.`, `(c)`, angle-bracketed emails, and
/// even `&` (as in `R&D`, `AT&T`, `Ernst &young`), so those alone never trip
/// this predicate. We require syntax specific to code: namespace `::`, a
/// method/property call (`ident.ident(`), a C-style address-of argument closing
/// a call or statement (` &copyRegion)` / ` &result;`), a keyword/assignment
/// call argument (`create(pk=1`), or a Django-style ORM access (`.objects.`,
/// `models.ForeignKey`/`ManyToManyField`/`OneToOneField`).
///
/// The strong structural signals (namespace, method call, address-of-in-call,
/// ORM) are always honoured — including on a raw source span that happens to
/// embed an email or URL literal, e.g. `ns::f("a@b.com", &result)`. Only the
/// weaker assignment-argument heuristic defers when a URL scheme is present,
/// because a parenthesized URL query string (`(...?y=z)`) can otherwise look
/// like a `name=value` call argument.
pub(crate) fn looks_like_source_code(s: &str) -> bool {
    // Structural code signals that never appear in a real name or notice.
    static STRONG_SOURCE_CODE_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?x)
            # namespace resolution: ident::ident
            \b[A-Za-z_][A-Za-z0-9_]*::[A-Za-z_~]
            # method or property call: foo.bar(...). The `(` must follow the
            # member name with no space, so a filename or domain followed by a
            # parenthesized URL/name (`AUTHORS.txt (http://...)`,
            # `google.com (Name)`) is not mistaken for a call.
          | \b[A-Za-z_][A-Za-z0-9_]*\.[A-Za-z_][A-Za-z0-9_]*\(
            # C-style address-of a variable that closes a call: ` &copyRegion)`,
            # `, &result)`. Only the `)`-closing form is matched: HTML entities
            # (`&copy;`, `&euro;`, `&eacute;`, …) always close with `;`, never
            # `)`, so a copyright line carrying an entity is never mistaken for
            # code. The trailing `)` also excludes name fragments such as
            # `Ernst &young` that are not followed by call punctuation.
          | (?:^|[\s(,])&[a-z][A-Za-z0-9_]*\s*\)
            ",
        )
    });
    static ORM_RE: LazyLock<Regex> = LazyLock::new(|| {
        compile_static_regex(
            r"(?ix)
            \.objects\.
          | \bmodels\.(?:ForeignKey|ManyToManyField|OneToOneField)\b
            ",
        )
    });
    // Weaker heuristic: a `name = value` argument inside a call, e.g. `create(pk=1`.
    static CALL_ASSIGN_ARG_RE: LazyLock<Regex> =
        LazyLock::new(|| compile_static_regex(r"\([^()]*[A-Za-z_][A-Za-z0-9_]*\s*="));

    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }

    if STRONG_SOURCE_CODE_RE.is_match(trimmed) || ORM_RE.is_match(trimmed) {
        return true;
    }

    // A parenthesized URL query string can mimic a `name=value` call argument,
    // so only apply the assignment-argument heuristic when no URL scheme is
    // present. Strong signals above are unaffected by this guard.
    let lower = trimmed.to_ascii_lowercase();
    let has_url = lower.contains("http://") || lower.contains("https://") || lower.contains("www.");
    !has_url && CALL_ASSIGN_ARG_RE.is_match(trimmed)
}
