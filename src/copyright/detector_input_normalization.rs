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

    let lower = content.to_ascii_lowercase();
    if !lower.contains("copyright") || !lower.contains(" at ") || !lower.contains(" dot ") {
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
}
