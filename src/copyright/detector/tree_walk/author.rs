// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::sync::LazyLock;

use regex::Regex;

use crate::copyright::refiner::{is_junk_copyright, refine_author};
use crate::copyright::types::{AuthorDetection, ParseNode, PosTag, Token, TreeLabel};
use crate::models::LineNumber;

use crate::copyright::detector;

pub(super) fn extract_sectioned_authors_from_author_node(
    node: &ParseNode,
) -> Option<Vec<AuthorDetection>> {
    let all_leaves = detector::token_utils::collect_all_leaves(node);
    let mut header_lines: Vec<LineNumber> = Vec::new();
    for t in &all_leaves {
        let v = t
            .value
            .trim_matches(|c: char| c.is_ascii_punctuation())
            .to_ascii_lowercase();
        let is_section_header = v.starts_with("author")
            || v.starts_with("contributor")
            || v.starts_with("committer")
            || v.starts_with("maintainer");

        if (is_section_header
            || matches!(
                t.tag,
                PosTag::Auth
                    | PosTag::Auth2
                    | PosTag::Auths
                    | PosTag::AuthDot
                    | PosTag::Maint
                    | PosTag::Contributors
                    | PosTag::Commit
                    | PosTag::SpdxContrib
            ))
            && header_lines.last().copied() != Some(t.start_line)
        {
            header_lines.push(t.start_line);
        }
    }
    if header_lines.len() < 2 {
        return None;
    }

    let mut result: Vec<AuthorDetection> = Vec::new();
    for line in header_lines {
        let tokens: Vec<&Token> = all_leaves
            .iter()
            .copied()
            .filter(|t| t.start_line == line && !detector::NON_AUTHOR_POS_TAGS.contains(&t.tag))
            .collect();
        if let Some(det) = detector::token_utils::build_author_from_tokens(&tokens) {
            result.push(det);
        }
    }

    if result.len() >= 2 {
        Some(result)
    } else {
        None
    }
}

const AUTHOR_BY_KEYWORDS: &[&str] = &[
    "originally",
    "modified",
    "contributed",
    "adapted",
    "hacking",
    "ported",
    "patches",
];

fn is_line_initial_keyword(tree: &[ParseNode], idx: usize, keyword_line: LineNumber) -> bool {
    if idx == 0 {
        return true;
    }
    let prev = &tree[idx - 1];
    match prev {
        ParseNode::Tree { label, .. } => {
            if matches!(
                label,
                TreeLabel::Copyright | TreeLabel::Copyright2 | TreeLabel::Author
            ) {
                return true;
            }
            let leaves = detector::token_utils::collect_all_leaves(prev);
            leaves.last().is_none_or(|t| t.start_line != keyword_line)
        }
        ParseNode::Leaf(token) => token.start_line != keyword_line,
    }
}

pub(super) fn try_extract_orphaned_by_author(
    tree: &[ParseNode],
    idx: usize,
) -> Option<(AuthorDetection, usize)> {
    let node = &tree[idx];
    let (keyword, keyword_line) = match node {
        ParseNode::Leaf(token)
            if matches!(token.tag, PosTag::Junk | PosTag::Nn | PosTag::Auth2) =>
        {
            (token.value.to_lowercase(), token.start_line)
        }
        _ => return None,
    };

    if !AUTHOR_BY_KEYWORDS.contains(&keyword.as_str()) {
        return None;
    }

    if idx > 0 && !is_line_initial_keyword(tree, idx, keyword_line) {
        return None;
    }

    let by_idx = idx + 1;
    if by_idx >= tree.len() {
        return None;
    }
    match &tree[by_idx] {
        ParseNode::Leaf(token) if token.tag == PosTag::By => {}
        _ => return None,
    }

    let name_idx = by_idx + 1;
    if name_idx >= tree.len() {
        return None;
    }

    let mut author_tokens: Vec<&Token> = Vec::new();
    let mut consumed = name_idx - idx;

    let mut j = name_idx;
    while j < tree.len() {
        match &tree[j] {
            ParseNode::Tree {
                label:
                    TreeLabel::Name | TreeLabel::NameEmail | TreeLabel::NameYear | TreeLabel::Company,
                ..
            } => {
                let leaves = detector::token_utils::collect_filtered_leaves(
                    &tree[j],
                    &[TreeLabel::YrRange, TreeLabel::YrAnd],
                    detector::NON_AUTHOR_POS_TAGS,
                );
                author_tokens.extend(leaves);
                consumed = j - idx;
                j += 1;
            }
            ParseNode::Leaf(token)
                if matches!(
                    token.tag,
                    PosTag::Nnp | PosTag::Nn | PosTag::Email | PosTag::Url
                ) =>
            {
                if is_author_tail_preposition(token) {
                    break;
                }
                author_tokens.push(token);
                consumed = j - idx;
                j += 1;
            }
            _ => break,
        }
    }

    if author_tokens.is_empty() {
        return None;
    }

    let det = detector::token_utils::build_author_from_tokens(&author_tokens)?;
    Some((det, consumed))
}

pub(super) fn try_extract_date_by_author(
    tree: &[ParseNode],
    idx: usize,
) -> Option<(AuthorDetection, usize)> {
    let node = &tree[idx];
    match node {
        ParseNode::Leaf(token) if token.tag == PosTag::By => {}
        _ => return None,
    }

    if idx == 0 {
        return None;
    }
    let prev_is_date = match &tree[idx - 1] {
        ParseNode::Leaf(token) => matches!(token.tag, PosTag::Yr | PosTag::BareYr),
        ParseNode::Tree { label, .. } => matches!(label, TreeLabel::YrRange | TreeLabel::YrAnd),
    };
    if !prev_is_date {
        return None;
    }

    let name_idx = idx + 1;
    if name_idx >= tree.len() {
        return None;
    }

    let mut author_tokens: Vec<&Token> = Vec::new();
    let mut consumed = name_idx - idx;

    let mut j = name_idx;
    while j < tree.len() {
        match &tree[j] {
            ParseNode::Tree {
                label:
                    TreeLabel::Name | TreeLabel::NameEmail | TreeLabel::NameYear | TreeLabel::Company,
                ..
            } => {
                let leaves = detector::token_utils::collect_filtered_leaves(
                    &tree[j],
                    &[TreeLabel::YrRange, TreeLabel::YrAnd],
                    detector::NON_AUTHOR_POS_TAGS,
                );
                author_tokens.extend(leaves);
                consumed = j - idx;
                j += 1;
            }
            ParseNode::Leaf(token)
                if matches!(
                    token.tag,
                    PosTag::Nnp | PosTag::Nn | PosTag::Email | PosTag::Url
                ) =>
            {
                if is_author_tail_preposition(token) {
                    break;
                }
                author_tokens.push(token);
                consumed = j - idx;
                j += 1;
            }
            _ => break,
        }
    }

    if author_tokens.is_empty() {
        return None;
    }

    let det = detector::token_utils::build_author_from_tokens(&author_tokens)?;
    if detector::token_utils::looks_like_bad_generic_author_candidate(&det.author) {
        return None;
    }
    Some((det, consumed))
}

fn is_author_tail_preposition(token: &Token) -> bool {
    token.tag == PosTag::Nn
        && matches!(
            token.value.to_ascii_lowercase().as_str(),
            "in" | "for" | "to" | "from" | "by"
        )
}

pub(super) fn try_extract_by_name_email_author(
    tree: &[ParseNode],
    idx: usize,
) -> Option<(AuthorDetection, usize)> {
    let by_token = match &tree[idx] {
        ParseNode::Leaf(token) if token.tag == PosTag::By => token,
        _ => return None,
    };

    // Require a "<contribution phrase> by ..." lead-in on the same line (e.g.
    // "Extensions by", "inotify back-end by"). A bare line-initial "by <name>"
    // with nothing before it is left out, matching the curated expectation that
    // isolated mid-comment attributions are not authors.
    let by_line = by_token.start_line;
    let same_line_preceding = tree[..idx]
        .iter()
        .flat_map(detector::collect_all_leaves)
        .filter(|leaf| leaf.start_line == by_line)
        .count();
    if same_line_preceding < 1 {
        return None;
    }

    let name_idx = idx + 1;
    let name_node = tree.get(name_idx)?;
    match name_node.label() {
        Some(
            TreeLabel::NameYear | TreeLabel::NameEmail | TreeLabel::Name | TreeLabel::NameCaps,
        ) => {}
        _ => return None,
    }

    let name_line = detector::token_utils::collect_all_leaves(name_node)
        .first()
        .map(|t| t.start_line)?;

    let mut author_tokens: Vec<&Token> = detector::token_utils::collect_filtered_leaves(
        name_node,
        &[TreeLabel::YrRange, TreeLabel::YrAnd],
        detector::NON_AUTHOR_POS_TAGS,
    );
    let mut last_consumed = name_idx;

    match author_tokens.iter().position(|t| t.tag == PosTag::Email) {
        // A personal name ends at its email address, so drop any trailing proper
        // nouns the grammar over-merged from the following clause (this is the
        // cross-line "<name> <email>. <NextWord> by ..." bleed).
        Some(pos) => author_tokens.truncate(pos + 1),
        // `<name>, <email>` shape: the email trails the name as a sibling leaf
        // rather than being folded into the NAME node.
        None => {
            let mut j = name_idx + 1;
            while let Some(ParseNode::Leaf(tok)) = tree.get(j) {
                if tok.start_line != name_line {
                    break;
                }
                match tok.tag {
                    PosTag::Cc if tok.value == "," => {
                        last_consumed = j;
                        j += 1;
                    }
                    PosTag::Email => {
                        author_tokens.push(tok);
                        last_consumed = j;
                        break;
                    }
                    _ => break,
                }
            }
        }
    }

    // Require an email as the strong authorship signal, mirroring ScanCode's
    // `AUTHOR: {<BY> <NAME-EMAIL>+}` / `AUTHOR: {<BY> <EMAIL>}` productions. The
    // email keeps this bare `by <name>` extraction from firing on ordinary prose.
    if !author_tokens.iter().any(|t| t.tag == PosTag::Email) {
        return None;
    }

    let det = detector::token_utils::build_author_from_tokens(&author_tokens)?;
    Some((det, last_consumed - idx))
}

pub(super) fn build_author_with_trailing(
    node: &ParseNode,
    tree: &[ParseNode],
    start: usize,
) -> Option<(AuthorDetection, usize)> {
    if start >= tree.len() {
        return None;
    }
    match &tree[start] {
        ParseNode::Leaf(token) if matches!(token.tag, PosTag::Email | PosTag::Url) => {}
        _ => return None,
    }

    let all_leaves = detector::token_utils::collect_all_leaves(node);
    let last_leaf = all_leaves.last()?;
    let last_is_email_with_comma =
        matches!(last_leaf.tag, PosTag::Email | PosTag::Url) && last_leaf.value.ends_with(',');
    if !last_is_email_with_comma {
        return None;
    }

    let mut author_tokens: Vec<&Token> = detector::token_utils::collect_filtered_leaves(
        node,
        &[TreeLabel::YrRange, TreeLabel::YrAnd],
        detector::NON_AUTHOR_POS_TAGS,
    );

    let mut j = start;
    while j < tree.len() {
        match &tree[j] {
            ParseNode::Leaf(token)
                if matches!(token.tag, PosTag::Email | PosTag::Url | PosTag::Cc) =>
            {
                if !detector::NON_AUTHOR_POS_TAGS.contains(&token.tag) {
                    author_tokens.push(token);
                }
                j += 1;
            }
            _ => break,
        }
    }

    let skip = j - start;
    if skip == 0 {
        return None;
    }
    let det = detector::token_utils::build_author_from_tokens(&author_tokens)?;
    Some((det, skip))
}

pub(super) fn extract_author_from_copyright_node(node: &ParseNode) -> Option<AuthorDetection> {
    static INLINE_ATTRIBUTION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)(?:\(|\b)(?:written|authored|created|developed)\s+by\s+(?P<who>[A-Z][^()]*?)(?:\)|$)",
        )
        .unwrap()
    });

    let all_leaves = detector::token_utils::collect_all_leaves(node);
    if all_leaves.len() < 2 {
        return None;
    }

    let raw_text = detector::token_utils::normalize_whitespace(
        &all_leaves
            .iter()
            .map(|t| t.value.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    );
    if let Some(cap) = INLINE_ATTRIBUTION_RE.captures(&raw_text) {
        let who = cap.name("who").map(|m| m.as_str()).unwrap_or("").trim();
        if !who.is_empty()
            && let Some(author) = refine_author(who)
        {
            let start_line = all_leaves.first()?.start_line;
            let end_line = all_leaves.last()?.start_line;
            return Some(AuthorDetection {
                author,
                start_line,
                end_line,
            });
        }
    }

    let auth_idx = all_leaves.iter().position(|t| {
        matches!(
            t.tag,
            PosTag::Auth | PosTag::Auth2 | PosTag::Auths | PosTag::AuthDot
        )
    })?;

    if auth_idx > 0 && all_leaves[auth_idx].start_line == all_leaves[auth_idx - 1].start_line {
        return None;
    }

    let auth_line = all_leaves[auth_idx].start_line;
    let after_auth = &all_leaves[auth_idx + 1..];

    let has_name_on_same_line = after_auth.iter().any(|t| {
        t.start_line == auth_line
            && !detector::NON_AUTHOR_POS_TAGS.contains(&t.tag)
            && !matches!(t.tag, PosTag::Email | PosTag::Url)
    });
    if !has_name_on_same_line {
        return None;
    }

    let has_email = after_auth.iter().any(|t| t.tag == PosTag::Email);
    if !has_email {
        return None;
    }

    let author_tokens: Vec<&Token> = after_auth
        .iter()
        .copied()
        .filter(|t| !detector::NON_AUTHOR_POS_TAGS.contains(&t.tag))
        .collect();

    detector::token_utils::build_author_from_tokens(&author_tokens)
}

pub fn extract_orphaned_by_authors(tree: &[ParseNode]) -> Vec<AuthorDetection> {
    let mut authors: Vec<AuthorDetection> = Vec::new();

    let mut i = 0;
    while i < tree.len() {
        if let Some((det, skip)) = try_extract_orphaned_by_author(tree, i) {
            authors.push(det);
            i += skip;
        } else if let Some((det, skip)) = try_extract_date_by_author(tree, i) {
            authors.push(det);
            i += skip;
        }
        i += 1;
    }

    authors
}

pub fn fix_truncated_contributors_authors(tree: &[ParseNode], authors: &mut Vec<AuthorDetection>) {
    let all_leaves: Vec<&Token> = tree.iter().flat_map(detector::collect_all_leaves).collect();

    for author in authors.iter_mut() {
        let author_line = author.end_line;
        let trailing_contributors = all_leaves.iter().find(|t| {
            t.tag == PosTag::Contributors
                && t.start_line == author_line
                && t.value.to_ascii_lowercase().starts_with("contributor")
        });
        let Some(trailing_contributors) = trailing_contributors else {
            continue;
        };

        if author.author.ends_with("and its") || author.author.ends_with("and her") {
            author.author.push_str(" contributors");
            continue;
        }

        if author.author.to_ascii_lowercase().contains("contributor") {
            continue;
        }

        if author.author.contains(',') {
            continue;
        }

        author.author = restore_trailing_contributors_suffix(
            &author.author,
            trailing_contributors
                .value
                .trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace()),
        );
    }

    let mut i = 0;
    while i < all_leaves.len() {
        let token = all_leaves[i];
        if token.tag == PosTag::Auth2 && i + 1 < all_leaves.len() {
            let next = all_leaves[i + 1];
            if next.tag == PosTag::By {
                let name_start = i + 2;
                let mut end = name_start;
                let mut found_contributors = false;
                while end < all_leaves.len() {
                    let t = all_leaves[end];
                    if t.tag == PosTag::Contributors {
                        found_contributors = true;
                        end += 1;
                        break;
                    }
                    if matches!(
                        t.tag,
                        PosTag::EmptyLine
                            | PosTag::Junk
                            | PosTag::Copy
                            | PosTag::Auth
                            | PosTag::Auth2
                            | PosTag::Auths
                            | PosTag::Maint
                    ) {
                        break;
                    }
                    end += 1;
                }
                if found_contributors && end > name_start {
                    let name_tokens: Vec<&Token> = all_leaves[name_start..end]
                        .iter()
                        .copied()
                        .filter(|t| !detector::NON_AUTHOR_POS_TAGS.contains(&t.tag))
                        .collect();
                    if !name_tokens.is_empty() {
                        let name_str =
                            detector::token_utils::normalized_tokens_to_string(&name_tokens);
                        let refined = refine_author(&name_str);
                        if let Some(mut author_text) = refined {
                            if !author_text.ends_with("contributors") {
                                author_text.push_str(" contributors");
                            }
                            let already_detected = authors.iter().any(|a| a.author == author_text);
                            if !already_detected && !is_junk_copyright(&author_text) {
                                authors.push(AuthorDetection {
                                    author: author_text,
                                    start_line: all_leaves[name_start].start_line,
                                    end_line: all_leaves[end - 1].start_line,
                                });
                            }
                        }
                    }
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
}

fn restore_trailing_contributors_suffix(author: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        return author.to_string();
    }

    if let Some(email_start) = author.rfind(" <") {
        let name = author[..email_start].trim_end();
        let email = &author[email_start..];
        return format!("{name} {suffix}{email}");
    }

    if let Some(email_start) = author.rfind(" (")
        && author.ends_with(')')
        && author[email_start + 2..author.len() - 1].contains('@')
    {
        let name = author[..email_start].trim_end();
        let email = &author[email_start..];
        return format!("{name} {suffix}{email}");
    }

    format!("{author} {suffix}")
}
