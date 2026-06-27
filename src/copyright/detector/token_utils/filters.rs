// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::normalize_whitespace;
use crate::copyright::refiner::{is_path_like_code_fragment, is_tokenizer_data_fragment};
use crate::copyright::types::{
    AuthorDetection, CopyrightDetection, HolderDetection, ParseNode, PosTag, Token, TreeLabel,
};
use crate::models::LineNumber;

pub fn should_merge_following_copyright_clause(
    all_leaves: &[&Token],
    start: usize,
    next_copy_idx: usize,
) -> bool {
    if start >= all_leaves.len() || next_copy_idx >= all_leaves.len() || next_copy_idx == 0 {
        return false;
    }

    let first = all_leaves[start];
    let next = all_leaves[next_copy_idx];
    if first.tag != PosTag::Copy || !first.value.eq_ignore_ascii_case("copyrighted") {
        return false;
    }
    if next.tag != PosTag::Copy || !next.value.eq_ignore_ascii_case("copyright") {
        return false;
    }

    let has_comma_before_next = {
        let prev = all_leaves[next_copy_idx - 1];
        let prev2 = if next_copy_idx >= 2 {
            Some(all_leaves[next_copy_idx - 2])
        } else {
            None
        };
        prev.value.ends_with(',')
            || prev.value == ","
            || prev.tag == PosTag::Cc
            || prev2.is_some_and(|t| t.value.ends_with(','))
    };
    if !has_comma_before_next {
        return false;
    }
    if next.start_line != first.start_line {
        return false;
    }

    let look_end = std::cmp::min(next_copy_idx + 24, all_leaves.len());
    all_leaves[next_copy_idx..look_end].iter().any(|t| {
        matches!(t.tag, PosTag::Yr | PosTag::YrPlus | PosTag::BareYr)
            || t.value.chars().filter(|c| c.is_ascii_digit()).count() >= 4
    })
}

pub fn should_merge_following_c_sign_after_year(
    all_leaves: &[&Token],
    start: usize,
    next_copy_idx: usize,
) -> bool {
    if start >= all_leaves.len() || next_copy_idx >= all_leaves.len() {
        return false;
    }
    let next = all_leaves[next_copy_idx];
    if next.tag != PosTag::Copy || !next.value.eq_ignore_ascii_case("(c)") {
        return false;
    }

    let line = next.start_line;
    let mut has_copyright_word = false;
    let mut has_yearish = false;
    let mut has_any_on_line = false;
    for t in all_leaves.iter().take(next_copy_idx).skip(start) {
        if t.start_line != line {
            continue;
        }
        has_any_on_line = true;
        if t.tag == PosTag::Copy && t.value.eq_ignore_ascii_case("copyright") {
            has_copyright_word = true;
        }
        if matches!(t.tag, PosTag::Yr | PosTag::YrPlus | PosTag::BareYr)
            || t.value.chars().filter(|c| c.is_ascii_digit()).count() >= 4
        {
            has_yearish = true;
        }
    }
    if !has_any_on_line || !has_copyright_word || !has_yearish {
        return false;
    }

    let look_end = std::cmp::min(next_copy_idx + 24, all_leaves.len());
    all_leaves[next_copy_idx + 1..look_end].iter().any(|t| {
        t.start_line == line
            && matches!(
                t.tag,
                PosTag::Nnp
                    | PosTag::Nn
                    | PosTag::Caps
                    | PosTag::Pn
                    | PosTag::MixedCap
                    | PosTag::Comp
            )
    })
}

pub fn apply_written_by_for_markers(
    group: &[(usize, String)],
    copyrights: &mut [CopyrightDetection],
    holders: &mut [HolderDetection],
) {
    static WRITTEN_BY_FOR_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^\s*\(\s*written\s+by\b.*\bfor\b.*\)\s*$").unwrap());

    for cr in copyrights.iter_mut() {
        let next_line = cr.end_line.saturating_add(1);
        let next_text = group.iter().find_map(|(ln, text)| {
            (LineNumber::new(*ln) == Some(next_line)).then_some(text.as_str())
        });

        let Some(next_text) = next_text else {
            continue;
        };
        if !WRITTEN_BY_FOR_RE.is_match(next_text) {
            continue;
        }

        if !cr.copyright.ends_with("Written") {
            cr.copyright = format!("{} Written", cr.copyright.trim_end());
        }

        for h in holders
            .iter_mut()
            .filter(|h| h.end_line.get() == cr.end_line.get())
        {
            if !h.holder.ends_with("Written") {
                h.holder = format!("{} Written", h.holder.trim_end());
            }
        }
    }
}

pub fn restore_bare_holder_angle_emails(
    copyrights: &[CopyrightDetection],
    holders: &mut [HolderDetection],
) {
    static LEADING_NAME_EMAIL_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?P<name>[^<]+?)\s*(?P<email><[^>\s]*@[^>\s]*>)").unwrap());

    for h in holders.iter_mut() {
        if h.holder.contains('@') {
            continue;
        }

        let has_nearby_explicit_copyright = copyrights.iter().any(|c| {
            c.copyright.to_ascii_lowercase().contains("copyright")
                && c.start_line.abs_diff(h.start_line) <= 25
        });
        if !has_nearby_explicit_copyright {
            continue;
        }

        for cr in copyrights.iter().filter(|c| {
            h.start_line.get() >= c.start_line.get()
                && h.end_line.get() <= c.end_line.get()
                && !c.copyright.to_ascii_lowercase().contains("copyright")
        }) {
            let Some(cap) = LEADING_NAME_EMAIL_RE.captures(cr.copyright.as_str()) else {
                continue;
            };
            let name = normalize_whitespace(cap.name("name").map(|m| m.as_str()).unwrap_or(""));
            let email = cap.name("email").map(|m| m.as_str()).unwrap_or("");
            if name.is_empty() || email.is_empty() {
                continue;
            }

            if normalize_whitespace(h.holder.as_str()) == name {
                h.holder = format!("{name} {email}");
                break;
            }
        }
    }
}

pub fn looks_like_bad_generic_author_candidate(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("@ref")
        || lower.contains("developerid")
        || lower.contains("disambiguatingdescription")
        || lower.contains("releasetimestamp")
        || lower.contains("requiredcore")
        || lower.contains("previoustimestamp")
        || lower.contains("previousversion")
        || lower.contains("builddate")
        || lower.contains("dependencies")
        || lower.contains("labels")
        || lower.contains("sha1")
        || lower.contains("scm")
        || lower.contains("@type")
        || lower.contains("type'")
        || lower.contains("sponsor'")
        || lower.contains("logo")
        || lower.contains("url'")
        || lower.contains("wiki:")
        || lower.contains("gav:")
        || lower.contains("u201")
        || lower.contains("nil attrs")
        || lower.contains("ptr parameter")
        || lower.contains("satisfy the request")
        || lower.contains("with key equal")
        || lower.contains("may wish to provide")
        || lower.contains("developers can trust")
        || (lower.contains("inspired by") && lower.contains("proposal"))
    {
        return true;
    }

    if trimmed.contains('@') {
        return false;
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() == 1 {
        return matches!(
            lower.as_str(),
            "admin"
                | "developers"
                | "developer"
                | "based"
                | "grants"
                | "working"
                | "features"
                | "components"
                | "ensure"
                | "in"
                | "on"
                | "for"
                | "from"
                | "by"
        );
    }

    false
}

pub fn collect_filtered_leaves<'a>(
    node: &'a ParseNode,
    ignored_labels: &[TreeLabel],
    ignored_pos_tags: &[PosTag],
) -> Vec<&'a Token> {
    let mut result = Vec::new();
    collect_filtered_leaves_inner(node, ignored_labels, ignored_pos_tags, &mut result);
    result
}

fn collect_filtered_leaves_inner<'a>(
    node: &'a ParseNode,
    ignored_labels: &[TreeLabel],
    ignored_pos_tags: &[PosTag],
    result: &mut Vec<&'a Token>,
) {
    match node {
        ParseNode::Leaf(token) => {
            if !ignored_pos_tags.contains(&token.tag) {
                result.push(token);
            }
        }
        ParseNode::Tree { label, children } => {
            if ignored_labels.contains(label) {
                return;
            }
            for child in children {
                collect_filtered_leaves_inner(child, ignored_labels, ignored_pos_tags, result);
            }
        }
    }
}

/// Drop copyright, holder, and author detections that originate from machine
/// subword-tokenizer data lines.
///
/// Hugging Face `merges.txt` BPE merge tables and `vocab.json` vocabularies
/// embed the `©` symbol and the literal word `copyright` as tokens, which the
/// detector would otherwise surface as dozens of spurious notices (e.g.
/// `"©": 102,` or `Ã © goo gle</w> fren ch</w>`). A detection is dropped when any
/// raw source line it spans is a tokenizer data fragment. Matching on the raw
/// source line (rather than the refined statement) is robust because holder
/// refinement strips the `</w>` markers and quotes that identify the data.
pub fn drop_tokenizer_data_fragment_detections(
    raw_lines: &[&str],
    tokenizer_merges_span: Option<(usize, usize)>,
    copyrights: &mut Vec<CopyrightDetection>,
    holders: &mut Vec<HolderDetection>,
    authors: &mut Vec<AuthorDetection>,
) {
    if raw_lines.is_empty() {
        return;
    }

    let spans_tokenizer_line = |start: usize, end: usize| -> bool {
        // A detection that overlaps the Hugging Face `tokenizer.json` `"merges"`
        // array is BPE merge-table junk; notices elsewhere in the document are
        // untouched. Only a multi-line (pretty-printed) span is used: a single
        // line cannot be split here, so a compact one-line document falls back to
        // the per-line fragment check below rather than dropping a same-line
        // notice that sits outside the merge data.
        if let Some((merges_start, merges_end)) = tokenizer_merges_span
            && merges_end > merges_start
            && start <= merges_end
            && end >= merges_start
        {
            return true;
        }
        (start..=end).any(|line_no| {
            raw_lines
                .get(line_no.saturating_sub(1))
                .is_some_and(|line| is_tokenizer_data_fragment(line))
        })
    };

    copyrights.retain(|c| !spans_tokenizer_line(c.start_line.get(), c.end_line.get()));
    holders.retain(|h| !spans_tokenizer_line(h.start_line.get(), h.end_line.get()));
    authors.retain(|a| !spans_tokenizer_line(a.start_line.get(), a.end_line.get()));
}

pub fn drop_scan_only_holders_from_copyright_scan_lines(
    raw_lines: &[&str],
    copyrights: &[CopyrightDetection],
    holders: &mut Vec<HolderDetection>,
) {
    if holders.is_empty() || raw_lines.is_empty() {
        return;
    }

    static COPYRIGHT_SCAN_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\bcopyright\s+scan(?:s|ner|ning)?\b").unwrap());

    let copyright_spans: HashSet<(usize, usize)> = copyrights
        .iter()
        .map(|c| (c.start_line.get(), c.end_line.get()))
        .collect();

    holders.retain(|holder| {
        let span = (holder.start_line.get(), holder.end_line.get());
        if copyright_spans.contains(&span) {
            return true;
        }
        if !holder.holder.eq_ignore_ascii_case("scan") {
            return true;
        }
        if holder.start_line.get() != holder.end_line.get() {
            return true;
        }

        raw_lines
            .get(holder.start_line.get() - 1)
            .is_none_or(|line| !COPYRIGHT_SCAN_RE.is_match(line))
    });
}

pub fn drop_path_fragment_holders_from_bare_c_code_lines(
    raw_lines: &[&str],
    copyrights: &[CopyrightDetection],
    holders: &mut Vec<HolderDetection>,
) {
    if holders.is_empty() || raw_lines.is_empty() {
        return;
    }

    static BARE_C_PATH_FRAGMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ix)
            ^\s*\(c\)\s*
            [A-Za-z_$][A-Za-z0-9_$]*
            (?:
                /[A-Za-z_$][A-Za-z0-9_$]*
              | \.[A-Za-z_$][A-Za-z0-9_$]*
              | \$[A-Za-z_$][A-Za-z0-9_$]*
            )+
            \s*$
            ",
        )
        .unwrap()
    });

    let copyright_spans: HashSet<(usize, usize)> = copyrights
        .iter()
        .map(|c| (c.start_line.get(), c.end_line.get()))
        .collect();

    holders.retain(|holder| {
        let span = (holder.start_line.get(), holder.end_line.get());
        if copyright_spans.contains(&span) {
            return true;
        }
        if holder.start_line.get() != holder.end_line.get() {
            return true;
        }
        if !is_path_like_code_fragment(&holder.holder) {
            return true;
        }

        raw_lines
            .get(holder.start_line.get() - 1)
            .is_none_or(|line| !BARE_C_PATH_FRAGMENT_RE.is_match(line))
    });
}

pub fn drop_test_label_false_positive_copyrights_and_holders(
    raw_lines: &[&str],
    copyrights: &mut Vec<CopyrightDetection>,
    holders: &mut Vec<HolderDetection>,
) {
    if raw_lines.is_empty() || (copyrights.is_empty() && holders.is_empty()) {
        return;
    }

    static ROW_HEADER_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ix)
            \([A-Z]\)\s+(?:row|column)\s+header
            | \|\s*\([A-Z]\)\s*\|\s*\d+\s*\|\s*\d+\s*\|\s*\([A-Z]\)\s+row\s+header
            ",
        )
        .unwrap()
    });
    static TEST_EXAMPLES_COPY_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)tests\s+using\s+the\s+examples\s+provided\s+by\s+\(c\)").unwrap()
    });

    let bad_lines: HashSet<usize> = raw_lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            (ROW_HEADER_RE.is_match(line) || TEST_EXAMPLES_COPY_RE.is_match(line))
                .then_some(idx + 1)
        })
        .collect();

    if bad_lines.is_empty() {
        return;
    }

    copyrights.retain(|c| !(c.start_line == c.end_line && bad_lines.contains(&c.start_line.get())));
    holders.retain(|h| !(h.start_line == h.end_line && bad_lines.contains(&h.start_line.get())));
}

/// Tags whose filtering should cause adjacent commas to be considered orphaned.
/// Only year-related tags: commas between years (e.g. "2006, 2007") become
/// orphaned when the years are removed. Email/URL commas are intentionally
/// excluded because they typically separate legitimate holder names
/// (e.g. "Name <email>, Name2").
pub const YEAR_LIKE_POS_TAGS: &[PosTag] = &[PosTag::Yr, PosTag::YrPlus, PosTag::BareYr];

static MALFORMED_COPYRIGHT_YEAR_RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)^
            (?:19\d{2}|20\d{2}|19\d{3}|20\d{3})
            (?:
                [\.,/\-~]
                (?:19\d{2}|20\d{2}|19\d{3}|20\d{3}|\d{2}|present|current_year|today\.year)
            )+
        $",
    )
    .unwrap()
});

pub fn is_year_like_token(token: &Token) -> bool {
    YEAR_LIKE_POS_TAGS.contains(&token.tag)
        || (token.tag == PosTag::Cd
            && MALFORMED_COPYRIGHT_YEAR_RANGE_RE.is_match(token.value.as_str()))
}

/// Year-related tree labels whose filtering orphans adjacent commas.
pub const YEAR_LIKE_LABELS: &[TreeLabel] = &[TreeLabel::YrRange, TreeLabel::YrAnd];

struct HolderLeafFilterState<'a> {
    result: Vec<&'a Token>,
    last_was_year_filtered: bool,
    last_filtered_email_or_url_line: Option<LineNumber>,
    last_filtered_email_was_angle_bracket: bool,
    pending_comma_after_filtered_email_or_url: Option<&'a Token>,
    last_filtered_was_paren_url: bool,
}

impl<'a> HolderLeafFilterState<'a> {
    fn new() -> Self {
        Self {
            result: Vec::new(),
            last_was_year_filtered: false,
            last_filtered_email_or_url_line: None,
            last_filtered_email_was_angle_bracket: false,
            pending_comma_after_filtered_email_or_url: None,
            last_filtered_was_paren_url: false,
        }
    }
}

/// Collect holder-filtered leaves with orphaned-comma removal.
///
/// Works like `collect_filtered_leaves` but additionally skips comma tokens
/// that become orphaned when year-related tokens/subtrees are filtered out.
pub fn collect_holder_filtered_leaves<'a>(
    node: &'a ParseNode,
    ignored_labels: &[TreeLabel],
    ignored_pos_tags: &[PosTag],
) -> Vec<&'a Token> {
    let mut state = HolderLeafFilterState::new();
    collect_holder_filtered_leaves_inner(node, ignored_labels, ignored_pos_tags, &mut state);
    if state.last_filtered_email_was_angle_bracket
        && let Some(comma) = state.pending_comma_after_filtered_email_or_url.take()
    {
        state.result.push(comma);
    }
    state.result
}

fn collect_holder_filtered_leaves_inner<'a>(
    node: &'a ParseNode,
    ignored_labels: &[TreeLabel],
    ignored_pos_tags: &[PosTag],
    state: &mut HolderLeafFilterState<'a>,
) {
    match node {
        ParseNode::Leaf(token) => {
            if ignored_pos_tags.contains(&token.tag) {
                if is_year_like_token(token) {
                    state.last_was_year_filtered = true;
                }
                if matches!(token.tag, PosTag::Email | PosTag::Url | PosTag::Url2) {
                    let ends_with_angle = token.value.ends_with('>') || token.value.ends_with(">,");
                    let is_paren_url = matches!(token.tag, PosTag::Url | PosTag::Url2)
                        && !ends_with_angle
                        && (token.value.ends_with(')') || token.value.ends_with("),"));
                    if is_paren_url {
                        state.last_filtered_was_paren_url = true;
                    } else {
                        state.last_filtered_was_paren_url = false;
                        state.last_filtered_email_or_url_line = Some(token.start_line);
                        state.last_filtered_email_was_angle_bracket = ends_with_angle;
                    }
                }
                return;
            }

            if let Some(comma) = state.pending_comma_after_filtered_email_or_url.take()
                && state.last_filtered_email_was_angle_bracket
            {
                state.result.push(comma);
            }

            if token.tag == PosTag::Cc && token.value == "," && state.last_was_year_filtered {
                state.last_filtered_was_paren_url = false;
                return;
            }

            if token.tag == PosTag::Cc && token.value == "," && state.last_filtered_was_paren_url {
                state.last_filtered_was_paren_url = false;
                state.last_filtered_email_or_url_line = None;
                state.last_filtered_email_was_angle_bracket = false;
                return;
            }

            if token.tag == PosTag::Cc
                && token.value == ","
                && state.last_filtered_email_or_url_line == Some(token.start_line)
            {
                state.pending_comma_after_filtered_email_or_url = Some(token);
                return;
            }

            if token.tag != PosTag::Cc || token.value != "," {
                state.last_was_year_filtered = false;
                state.last_filtered_email_or_url_line = None;
                state.last_filtered_was_paren_url = false;
            }
            state.result.push(token);
        }
        ParseNode::Tree { label, children } => {
            if ignored_labels.contains(label) {
                if YEAR_LIKE_LABELS.contains(label) {
                    state.last_was_year_filtered = true;
                }
                return;
            }
            for child in children {
                collect_holder_filtered_leaves_inner(
                    child,
                    ignored_labels,
                    ignored_pos_tags,
                    state,
                );
            }
        }
    }
}

/// Filter holder tokens from a flat slice, skipping orphaned commas after
/// year-related filtered tokens.
pub fn filter_holder_tokens_with_state<'a>(
    tokens: &[&'a Token],
    non_holder_tags: &[PosTag],
    predecessor_was_year_filtered: bool,
) -> Vec<&'a Token> {
    let mut result = Vec::new();
    let mut last_was_year_filtered = predecessor_was_year_filtered;
    let mut last_filtered_email_or_url_line: Option<LineNumber> = None;
    let mut last_filtered_email_was_angle_bracket = false;
    let mut last_filtered_was_paren_url = false;

    for (i, &token) in tokens.iter().enumerate() {
        if non_holder_tags.contains(&token.tag) {
            if is_year_like_token(token) {
                last_was_year_filtered = true;
            }
            if matches!(token.tag, PosTag::Email | PosTag::Url | PosTag::Url2) {
                let ends_with_angle = token.value.ends_with('>') || token.value.ends_with(">,");
                let is_paren_url = matches!(token.tag, PosTag::Url | PosTag::Url2)
                    && !ends_with_angle
                    && (token.value.ends_with(')') || token.value.ends_with("),"));
                if is_paren_url {
                    last_filtered_was_paren_url = true;
                } else {
                    last_filtered_was_paren_url = false;
                    last_filtered_email_or_url_line = Some(token.start_line);
                    last_filtered_email_was_angle_bracket = ends_with_angle;
                }
            }
            continue;
        }

        if token.tag == PosTag::Cc && token.value == "," {
            if last_was_year_filtered {
                last_filtered_was_paren_url = false;
                continue;
            }

            if last_filtered_was_paren_url {
                last_filtered_was_paren_url = false;
                last_filtered_email_or_url_line = None;
                last_filtered_email_was_angle_bracket = false;
                continue;
            }

            if last_filtered_email_or_url_line == Some(token.start_line)
                && !last_filtered_email_was_angle_bracket
            {
                let next_kept = tokens[i + 1..]
                    .iter()
                    .copied()
                    .find(|t| !non_holder_tags.contains(&t.tag));
                if next_kept.is_some_and(|t| t.start_line > token.start_line) {
                    last_filtered_email_or_url_line = None;
                    last_filtered_email_was_angle_bracket = false;
                    continue;
                }
            }
            last_filtered_email_or_url_line = None;
            last_filtered_email_was_angle_bracket = false;
        } else {
            last_was_year_filtered = false;
            last_filtered_email_or_url_line = None;
            last_filtered_email_was_angle_bracket = false;
            last_filtered_was_paren_url = false;
        }

        if token.tag != PosTag::Cc || token.value != "," {
            last_was_year_filtered = false;
        }

        result.push(token);
    }
    result
}

/// Strip trailing comma tokens from a holder token list.
pub fn strip_trailing_commas(tokens: &mut Vec<&Token>) {
    while tokens
        .last()
        .is_some_and(|t| t.tag == PosTag::Cc && t.value == ",")
    {
        tokens.pop();
    }
}

pub fn strip_all_rights_reserved(leaves: Vec<&Token>) -> Vec<&Token> {
    strip_all_rights_reserved_slice(&leaves)
}

pub fn strip_all_rights_reserved_slice<'a>(leaves: &[&'a Token]) -> Vec<&'a Token> {
    let mut filtered: Vec<&Token> = Vec::with_capacity(leaves.len());

    let mut i = 0;
    while i < leaves.len() {
        let token = leaves[i];
        if token.tag == PosTag::Reserved {
            if filtered.len() >= 2
                && filtered[filtered.len() - 1].tag == PosTag::Right
                && matches!(
                    filtered[filtered.len() - 2].tag,
                    PosTag::Nn | PosTag::Caps | PosTag::Nnp
                )
            {
                filtered.truncate(filtered.len() - 2);
            } else if filtered.len() >= 3
                && matches!(
                    filtered[filtered.len() - 1].tag,
                    PosTag::Nn | PosTag::Caps | PosTag::Nnp
                )
                && filtered[filtered.len() - 2].tag == PosTag::Right
                && matches!(
                    filtered[filtered.len() - 3].tag,
                    PosTag::Nn | PosTag::Caps | PosTag::Nnp
                )
            {
                filtered.truncate(filtered.len() - 3);
            }

            let mut j = i + 1;
            while j < leaves.len()
                && leaves[j].tag == PosTag::Cc
                && matches!(leaves[j].value.as_str(), "," | "." | ";" | ":")
            {
                j += 1;
            }

            let keep_after = leaves.get(j).is_some_and(|t| {
                matches!(
                    t.tag,
                    PosTag::By | PosTag::Copy | PosTag::Yr | PosTag::YrPlus | PosTag::BareYr
                )
            });
            if !keep_after {
                break;
            }
            i += 1;
            continue;
        }

        filtered.push(token);
        i += 1;
    }

    filtered
}

pub fn is_copyright_of_header(span: &[&Token]) -> bool {
    if span.len() < 3 {
        return false;
    }

    let first = span[0];
    let second = span[1];

    if first.tag != PosTag::Copy || !first.value.eq_ignore_ascii_case("copyright") {
        return false;
    }

    let starts_with_notice_of = if span.len() >= 4 {
        second.value.eq_ignore_ascii_case("notice")
            && span[2].tag == PosTag::Of
            && span[2].value.eq_ignore_ascii_case("of")
    } else {
        false
    };

    if !(second.tag == PosTag::Of && second.value.eq_ignore_ascii_case("of")
        || starts_with_notice_of)
    {
        return false;
    }

    let has_year = span
        .iter()
        .any(|t| matches!(t.tag, PosTag::Yr | PosTag::YrPlus | PosTag::BareYr));
    let has_c = span
        .iter()
        .any(|t| t.tag == PosTag::Copy && t.value.eq_ignore_ascii_case("(c)"));
    !has_year && !has_c
}
