// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

pub(super) fn maybe_expand_copyrighted_by_href_urls<'a>(content: &'a str) -> Cow<'a, str> {
    let lower = content.to_ascii_lowercase();
    if !lower.contains("copyrighted by") || !lower.contains("href=") {
        return Cow::Borrowed(content);
    }
    if lower.contains("<html") || lower.contains("<head") {
        return Cow::Borrowed(content);
    }
    if content.lines().count() > 40 {
        return Cow::Borrowed(content);
    }

    static HREF_HTTP_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?is)\bhref\s*=\s*['\"](?P<url>http://[^'\">\s]+)['\"]\s*/?>?"#).unwrap()
    });

    Cow::Owned(HREF_HTTP_RE.replace_all(content, " ${url} ").into_owned())
}

/// Case-insensitive ASCII substring search without allocation.
fn contains_ascii_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

/// Necessary-condition prefilter for [`normalize_split_obfuscated_email_continuation`].
///
/// Returns `true` only if some line carries a `copyright` marker and the
/// immediately following line contains an obfuscated-email parenthetical shape
/// (`(`, `at`, `dot`). This is a strict superset of the regex's matches, so
/// gating on it never drops a real match; it only avoids the expensive
/// whole-file multiline regex scan when no candidate adjacency exists.
fn has_split_obfuscated_email_continuation_candidate(content: &str) -> bool {
    let mut prev_has_copyright = false;
    for line in content.lines() {
        let bytes = line.as_bytes();
        if prev_has_copyright
            && bytes.contains(&b'(')
            && contains_ascii_ci(bytes, b"at")
            && contains_ascii_ci(bytes, b"dot")
        {
            return true;
        }
        prev_has_copyright = contains_ascii_ci(bytes, b"copyright");
    }
    false
}

/// Join a copyright/name line with a following comment line whose only payload
/// is an obfuscated-email parenthetical such as `(chris at kohlhoff dot com)`.
///
/// Multi-line C-style or `//` headers sometimes wrap the holder's contact email
/// onto the next comment line. Because each line is prepared independently, the
/// per-line obfuscated-email collapse never sees the full `(name at host dot
/// tld)` span and the parser leaks the first token (e.g. `chris`) into the
/// holder. Folding the parenthetical back onto the preceding line restores the
/// single-line behavior, which already recovers the clean holder.
fn normalize_split_obfuscated_email_continuation<'a>(content: &'a str) -> Cow<'a, str> {
    // Conservative: the previous line carries a copyright marker, and the
    // continuation line is solely a comment-prefixed `(... at ... dot ...)`.
    static SPLIT_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?im)(?P<head>^.*\bcopyright\b.*[A-Za-z])[ \t]*\r?\n[ \t]*(?:[/*#;]+[ \t]*)?(?P<email>\([^()\n]*\bat\b[^()\n]*\bdot\b[^()\n]*\))[ \t]*$",
        )
        .unwrap()
    });

    // Cheap, allocation-free necessary-condition gate before the multiline
    // PikeVM scan. The regex can only match when a line carrying a `copyright`
    // marker is immediately followed by a comment-style obfuscated-email
    // parenthetical such as `(name at host dot tld)`. Confirming that adjacency
    // with byte scans first avoids lowercasing and PikeVM-scanning entire
    // multi-megabyte files (e.g. gettext catalogs) that merely happen to mention
    // "copyright", "at" and "dot" in unrelated places. This is a strict superset
    // of the regex's matches, so the detector output is unchanged.
    if !has_split_obfuscated_email_continuation_candidate(content) {
        return Cow::Borrowed(content);
    }
    if !SPLIT_EMAIL_RE.is_match(content) {
        return Cow::Borrowed(content);
    }

    Cow::Owned(
        SPLIT_EMAIL_RE
            .replace_all(content, "${head} ${email}")
            .into_owned(),
    )
}

/// Apply every pre-line-split normalization in sequence, returning one `Cow`.
///
/// This is the single seam the detector uses to fold multi-line constructs back
/// onto one logical line before the per-line preparation runs. Adding another
/// split-normalizer means chaining it here, not at each detector call site.
pub(super) fn normalize_split_input<'a>(content: &'a str) -> Cow<'a, str> {
    let normalized = normalize_split_angle_bracket_urls(content);
    match normalize_split_obfuscated_email_continuation(normalized.as_ref()) {
        Cow::Borrowed(_) => normalized,
        Cow::Owned(joined) => Cow::Owned(joined),
    }
}

fn normalize_split_angle_bracket_urls<'a>(content: &'a str) -> Cow<'a, str> {
    static SPLIT_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)<\s*(?P<url>https?://[^\s>]+)\s*\r?\n\s*(?P<tail>[^\s>]+)\s*>").unwrap()
    });

    if !content.contains('<') || !content.contains('>') {
        return Cow::Borrowed(content);
    }
    if !SPLIT_URL_RE.is_match(content) {
        return Cow::Borrowed(content);
    }

    Cow::Owned(
        SPLIT_URL_RE
            .replace_all(content, "${url} ${tail}")
            .into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_slash_comment_email_continuation() {
        let input =
            "// Copyright (c) 2003-2008 Christopher M. Kohlhoff\n// (chris at kohlhoff dot com)\n";
        let out = normalize_split_obfuscated_email_continuation(input);
        assert_eq!(
            out.as_ref(),
            "// Copyright (c) 2003-2008 Christopher M. Kohlhoff (chris at kohlhoff dot com)\n"
        );
    }

    #[test]
    fn folds_star_comment_email_continuation() {
        let input =
            " * Copyright (c) 2003-2008 Christopher M. Kohlhoff\n * (chris at kohlhoff dot com)\n";
        let out = normalize_split_obfuscated_email_continuation(input);
        assert_eq!(
            out.as_ref(),
            " * Copyright (c) 2003-2008 Christopher M. Kohlhoff (chris at kohlhoff dot com)\n"
        );
    }

    #[test]
    fn leaves_non_email_parenthetical_alone() {
        let input = "// Copyright (c) 2020 Acme Inc\n// (see LICENSE for details)\n";
        assert!(matches!(
            normalize_split_obfuscated_email_continuation(input),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn requires_a_copyright_marker_on_the_preceding_line() {
        let input = "// Some heading\n// (chris at kohlhoff dot com)\n";
        assert!(matches!(
            normalize_split_obfuscated_email_continuation(input),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn prefilter_accepts_real_continuation_shapes() {
        // The cheap prefilter must not reject inputs the regex would match.
        for input in [
            "// Copyright (c) 2003-2008 Christopher M. Kohlhoff\n// (chris at kohlhoff dot com)\n",
            " * Copyright (c) 2003-2008 Christopher M. Kohlhoff\n * (chris at kohlhoff dot com)\n",
            "Copyright 2020 ACME\n(jane AT example DOT org)\n",
        ] {
            assert!(
                has_split_obfuscated_email_continuation_candidate(input),
                "prefilter rejected a real candidate: {input:?}"
            );
            assert!(matches!(
                normalize_split_obfuscated_email_continuation(input),
                Cow::Owned(_)
            ));
        }
    }

    #[test]
    fn prefilter_rejects_non_adjacent_or_missing_markers() {
        // No copyright marker on the preceding line.
        assert!(!has_split_obfuscated_email_continuation_candidate(
            "// Some heading\n// (chris at kohlhoff dot com)\n"
        ));
        // Marker and parenthetical present but not on adjacent lines.
        assert!(!has_split_obfuscated_email_continuation_candidate(
            "Copyright 2020 ACME\nfiller line\n(chris at kohlhoff dot com)\n"
        ));
        // Bulk text that merely mentions the trigger words in scattered places.
        assert!(!has_split_obfuscated_email_continuation_candidate(
            "look at the cat\nthe dog ran\nCopyright notice text\nplain tail line\n"
        ));
    }

    #[test]
    fn prefilter_is_a_superset_of_the_regex() {
        // For every input the regex matches, the prefilter must also accept it.
        let inputs = [
            "// Copyright (c) 1999 Foo Bar\n// (foo at bar dot com)\n",
            "/* Copyright 2010 Baz */\n/* (baz at baz dot net) */\n",
            "# Copyright Qux\n# (qux at qux dot io)\n",
            "no marker here\n(a at b dot c)\n",
            "Copyright but no email next\nplain text\n",
        ];
        for input in inputs {
            let regex_matches = matches!(
                normalize_split_obfuscated_email_continuation(input),
                Cow::Owned(_)
            );
            if regex_matches {
                assert!(
                    has_split_obfuscated_email_continuation_candidate(input),
                    "prefilter must accept every regex match: {input:?}"
                );
            }
        }
    }
}
