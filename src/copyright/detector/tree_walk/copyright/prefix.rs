// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Detection of prefix tokens that precede a copyright clause.
//!
//! These helpers recover copyright/portions/`(c)` and `not`-copyrighted markers
//! that the parser left as orphan leaves immediately before a copyright node so
//! they can be reattached when building the copyright statement.

use crate::copyright::detector;
use crate::copyright::types::{ParseNode, PosTag, Token, TreeLabel};

/// Recovers the leading `Portions created by the initial developer are`
/// prefix tokens (the Mozilla Public License attribution boilerplate) that
/// precede a copyright node, when present.
pub(super) fn mpl_portions_created_prefix_tokens<'a>(
    tree: &'a [ParseNode],
    idx: usize,
    copyright_node: &'a ParseNode,
    trailing_tokens: &[&'a Token],
) -> Option<Vec<&'a Token>> {
    let leaves = detector::token_utils::collect_all_leaves(copyright_node);
    let first = *leaves.first()?;
    if first.tag != PosTag::Copy || !first.value.eq_ignore_ascii_case("copyright") {
        return None;
    }

    let mut combined = leaves;
    combined.extend_from_slice(trailing_tokens);

    let has_initial = combined
        .iter()
        .any(|t| t.value.eq_ignore_ascii_case("initial"));
    let has_developer = combined.iter().any(|t| {
        t.value
            .as_str()
            .get(0.."developer".len())
            .is_some_and(|p| p.eq_ignore_ascii_case("developer"))
    });
    if !(has_initial && has_developer) {
        return None;
    }

    let line = first.start_line;
    let mut prev_rev: Vec<&Token> = Vec::with_capacity(7);
    let mut j = idx;
    while j > 0 && prev_rev.len() < 7 {
        j -= 1;
        let leaves = detector::token_utils::collect_all_leaves(&tree[j]);
        for &t in leaves.iter().rev() {
            if t.start_line != line {
                continue;
            }
            prev_rev.push(t);
            if prev_rev.len() == 7 {
                break;
            }
        }
    }

    if prev_rev.len() != 7 {
        return None;
    }
    prev_rev.reverse();

    let values: Vec<&str> = prev_rev.iter().map(|t| t.value.as_str()).collect();
    let matches = values[0].eq_ignore_ascii_case("portions")
        && values[1].eq_ignore_ascii_case("created")
        && values[2].eq_ignore_ascii_case("by")
        && values[3].eq_ignore_ascii_case("the")
        && values[4].eq_ignore_ascii_case("initial")
        && values[5].eq_ignore_ascii_case("developer")
        && values[6].eq_ignore_ascii_case("are");

    matches.then_some(prev_rev)
}

/// Recovers a single `Portions` prefix token immediately before a copyright
/// node, including the `Portions Copyright (c)` shape.
pub(super) fn single_portions_prefix_token<'a>(
    tree: &'a [ParseNode],
    idx: usize,
    copyright_node: &'a ParseNode,
) -> Option<&'a Token> {
    let first = *detector::token_utils::collect_all_leaves(copyright_node).first()?;
    if idx == 0 {
        return None;
    }

    if first.tag == PosTag::Copy && first.value.eq_ignore_ascii_case("copyright") {
        let ParseNode::Leaf(prev) = &tree[idx - 1] else {
            return None;
        };
        return (prev.tag == PosTag::Portions && prev.start_line == first.start_line)
            .then_some(prev);
    }

    if first.tag == PosTag::Copy && first.value.eq_ignore_ascii_case("(c)") && idx >= 2 {
        let ParseNode::Leaf(prev_copy) = &tree[idx - 1] else {
            return None;
        };
        if prev_copy.tag != PosTag::Copy
            || !prev_copy.value.eq_ignore_ascii_case("copyright")
            || prev_copy.start_line != first.start_line
        {
            return None;
        }

        let ParseNode::Leaf(prev_portions) = &tree[idx - 2] else {
            return None;
        };
        return (prev_portions.tag == PosTag::Portions
            && prev_portions.start_line == first.start_line)
            .then_some(prev_portions);
    }

    None
}

/// Recovers a `Copyright` prefix token left orphaned immediately before the
/// node at `idx`, including copy-only sibling trees.
pub(super) fn get_orphaned_copy_prefix(tree: &[ParseNode], idx: usize) -> Option<&Token> {
    if idx == 0 {
        return None;
    }
    let prev = &tree[idx - 1];
    if let ParseNode::Leaf(token) = prev
        && token.tag == PosTag::Copy
    {
        return Some(token);
    }
    if let ParseNode::Tree { label, children } = prev {
        match label {
            TreeLabel::NameCopy => {
                for child in children.iter().rev() {
                    if let ParseNode::Leaf(token) = child
                        && token.tag == PosTag::Copy
                    {
                        return Some(token);
                    }
                }
            }
            TreeLabel::Copyright | TreeLabel::Copyright2 => {
                let all_copy = children.iter().all(|c| {
                    matches!(c, ParseNode::Leaf(t) if t.tag == PosTag::Copy)
                        || matches!(c, ParseNode::Tree { label: l, .. }
                            if matches!(l, TreeLabel::Copyright | TreeLabel::Copyright2)
                                && is_copy_only_tree(c))
                });
                if all_copy {
                    for child in children.iter().rev() {
                        if let ParseNode::Leaf(token) = child
                            && token.tag == PosTag::Copy
                        {
                            return Some(token);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Recovers a leading `not` token before a copyright node so that a
/// `not copyrighted` statement is preserved when `allow_not_copyrighted_prefix`
/// is enabled.
pub(super) fn get_orphaned_not_prefix<'a>(
    tree: &'a [ParseNode],
    idx: usize,
    copyright_node: &ParseNode,
    allow_not_copyrighted_prefix: bool,
) -> Option<&'a Token> {
    if !allow_not_copyrighted_prefix {
        return None;
    }
    if idx == 0 {
        return None;
    }
    let first_line = detector::token_utils::collect_all_leaves(copyright_node)
        .first()
        .map(|t| t.start_line)?;
    let prev = &tree[idx - 1];
    if let ParseNode::Leaf(token) = prev
        && token.start_line == first_line
        && token.value.eq_ignore_ascii_case("not")
    {
        for n in &tree[..idx - 1] {
            for t in detector::token_utils::collect_all_leaves(n) {
                if t.start_line != first_line {
                    continue;
                }
                if matches!(t.tag, PosTag::Junk | PosTag::Dash | PosTag::Parens)
                    || looks_like_filename_prefix_token(t)
                {
                    continue;
                }
                return None;
            }
        }
        return Some(token);
    }
    None
}

fn looks_like_filename_prefix_token(token: &Token) -> bool {
    let v = token.value.as_str();
    if v == "--" {
        return true;
    }
    if !v.contains('.') {
        return false;
    }
    let (base, ext) = match v.rsplit_once('.') {
        Some(parts) => parts,
        None => return false,
    };
    if base.is_empty()
        || ext.is_empty()
        || ext.len() > 4
        || !ext.chars().all(|c| c.is_ascii_alphabetic())
    {
        return false;
    }
    v.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
}

fn is_copy_only_tree(node: &ParseNode) -> bool {
    match node {
        ParseNode::Leaf(t) => t.tag == PosTag::Copy,
        ParseNode::Tree { children, .. } => children.iter().all(is_copy_only_tree),
    }
}
