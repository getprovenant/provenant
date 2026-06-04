// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Cross-node merges that combine a copyright clause with an adjacent clause.
//!
//! - [`merge_copyright_with_following_author`] folds an author line on the line
//!   directly after a copyright clause into the copyright statement.
//! - [`merge_year_only_copyright_clause_with_preceding_copyrighted_by`] joins a
//!   year-only copyright clause to a preceding `copyrighted ... by` clause.

use crate::copyright::detector;
use crate::copyright::types::{
    CopyrightDetection, HolderDetection, ParseNode, PosTag, Token, TreeLabel,
};
use crate::models::LineNumber;

use super::node_classify::is_year_only_copyright_clause_node;

pub(super) fn merge_copyright_with_following_author<'a>(
    copyright_node: &'a ParseNode,
    prefix_token: Option<&'a Token>,
    tree: &'a [ParseNode],
    author_idx: usize,
) -> Option<(CopyrightDetection, Option<HolderDetection>, usize)> {
    let author_node = &tree[author_idx];
    if author_node.label() != Some(TreeLabel::Author) {
        return None;
    }

    let author_leaves = detector::token_utils::collect_all_leaves(author_node);

    let auth_token = author_leaves
        .iter()
        .find(|t| matches!(t.tag, PosTag::Auth | PosTag::AuthDot))?;
    if auth_token.tag != PosTag::Auth {
        return None;
    }

    let cr_leaves_all = detector::token_utils::collect_all_leaves(copyright_node);
    let cr_last_line = cr_leaves_all
        .last()
        .map(|t| t.start_line)
        .unwrap_or(LineNumber::ONE);
    let author_first_line = auth_token.start_line;
    if author_first_line != cr_last_line + 1 {
        return None;
    }

    let mut author_tail: Vec<&Token> = Vec::new();
    author_tail.push(auth_token);
    for t in author_leaves.iter() {
        if t.start_line < author_first_line {
            continue;
        }
        if t.start_line == author_first_line {
            continue;
        }
        if matches!(
            t.tag,
            PosTag::Email
                | PosTag::EmailStart
                | PosTag::EmailEnd
                | PosTag::Url
                | PosTag::Url2
                | PosTag::At
                | PosTag::Dot
        ) {
            continue;
        }
        if matches!(
            t.tag,
            PosTag::Nnp
                | PosTag::Nn
                | PosTag::Caps
                | PosTag::Pn
                | PosTag::MixedCap
                | PosTag::Comp
                | PosTag::Uni
                | PosTag::Van
                | PosTag::Cc
        ) {
            author_tail.push(t);
        }
    }

    if author_tail.len() < 2 {
        return None;
    }

    let mut cr_tokens: Vec<&Token> = Vec::new();
    if let Some(prefix) = prefix_token {
        cr_tokens.push(prefix);
    }
    let cr_leaves = detector::token_utils::collect_filtered_leaves(
        copyright_node,
        detector::NON_COPYRIGHT_LABELS,
        detector::NON_COPYRIGHT_POS_TAGS,
    );
    let cr_leaves = detector::token_utils::strip_all_rights_reserved(cr_leaves);
    cr_tokens.extend(&cr_leaves);

    cr_tokens.extend(author_tail);

    let cr_det = detector::token_utils::build_copyright_from_tokens(&cr_tokens)?;

    Some((cr_det, None, 0))
}

pub(super) fn merge_year_only_copyright_clause_with_preceding_copyrighted_by(
    tree: &[ParseNode],
    copyright_idx: usize,
    copy_prefix: Option<&Token>,
    portions_prefix: Option<&Token>,
    mpl_prefix: Option<&[&Token]>,
) -> Option<(CopyrightDetection, HolderDetection)> {
    if copyright_idx >= tree.len() {
        return None;
    }
    let node = &tree[copyright_idx];
    if !is_year_only_copyright_clause_node(node) {
        return None;
    }

    let node_line = detector::token_utils::collect_all_leaves(node)
        .first()
        .map(|t| t.start_line)?;

    let mut copyrighted_idx: Option<usize> = None;
    let mut by_idx: Option<usize> = None;

    let start_search = copyright_idx.saturating_sub(14);
    for idx in (start_search..copyright_idx).rev() {
        let leaves = detector::token_utils::collect_all_leaves(&tree[idx]);
        if leaves.first().is_none_or(|t| t.start_line != node_line) {
            continue;
        }
        if let ParseNode::Leaf(token) = &tree[idx]
            && token.tag == PosTag::Copy
            && token.value.eq_ignore_ascii_case("copyrighted")
        {
            copyrighted_idx = Some(idx);
            break;
        }
    }
    let copyrighted_idx = copyrighted_idx?;

    for (idx, node) in tree
        .iter()
        .enumerate()
        .take(copyright_idx)
        .skip(copyrighted_idx + 1)
    {
        let leaves = detector::token_utils::collect_all_leaves(node);
        if leaves.first().is_none_or(|t| t.start_line != node_line) {
            break;
        }
        if let ParseNode::Leaf(token) = node
            && token.tag == PosTag::By
            && token.value.eq_ignore_ascii_case("by")
        {
            by_idx = Some(idx);
            break;
        }
    }
    let by_idx = by_idx?;

    if by_idx + 1 >= copyright_idx {
        return None;
    }

    let has_comma_boundary = (by_idx + 1..copyright_idx).any(|idx| {
        detector::token_utils::collect_all_leaves(&tree[idx])
            .iter()
            .any(|t| t.value == "," || t.tag == PosTag::Cc || t.value.ends_with(','))
    });
    if !has_comma_boundary {
        return None;
    }

    let mut cr_tokens: Vec<&Token> = Vec::new();
    if let Some(prefix) = copy_prefix {
        cr_tokens.push(prefix);
    }
    if let Some(prefix) = portions_prefix {
        cr_tokens.push(prefix);
    }
    if let Some(prefix) = mpl_prefix {
        cr_tokens.extend(prefix.iter().copied());
    }

    for node in tree.iter().take(copyright_idx + 1).skip(copyrighted_idx) {
        cr_tokens.extend(detector::token_utils::collect_all_leaves(node));
    }
    let cr_tokens = detector::token_utils::strip_all_rights_reserved(cr_tokens);
    let cr_det = detector::token_utils::build_copyright_from_tokens(&cr_tokens)?;

    let mut holder_tokens: Vec<&Token> = Vec::new();
    for node in tree.iter().take(copyright_idx).skip(by_idx + 1) {
        holder_tokens.extend(detector::token_utils::collect_all_leaves(node));
    }
    let holder_tokens = detector::token_utils::strip_all_rights_reserved(holder_tokens);
    let allow_single_word_contributors = holder_tokens
        .iter()
        .any(|t| detector::token_utils::is_year_like_token(t));
    let holder_det = detector::token_utils::build_holder_from_tokens(
        &holder_tokens,
        allow_single_word_contributors,
    )?;

    Some((cr_det, holder_det))
}
