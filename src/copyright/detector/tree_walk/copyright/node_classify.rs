// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Pure predicates that classify parse nodes during the tree walk.
//!
//! These helpers decide whether a node continues a holder/name span, marks a
//! span boundary, is a year-only copyright clause, or carries a nearby signal
//! (company suffix, legal tail, holder-suffix keyword) within a short lookahead
//! window.

use crate::copyright::detector;
use crate::copyright::types::{ParseNode, PosTag, TreeLabel};
use crate::models::LineNumber;

pub(super) fn is_orphan_continuation(node: &ParseNode) -> bool {
    match node {
        ParseNode::Leaf(token) => matches!(
            token.tag,
            PosTag::Of
                | PosTag::Van
                | PosTag::Uni
                | PosTag::Yr
                | PosTag::YrPlus
                | PosTag::BareYr
                | PosTag::Nn
                | PosTag::Nnp
                | PosTag::Caps
                | PosTag::Cc
                | PosTag::Cd
                | PosTag::Cds
                | PosTag::Comp
                | PosTag::Dash
                | PosTag::Pn
                | PosTag::MixedCap
                | PosTag::In
                | PosTag::To
                | PosTag::By
                | PosTag::Oth
                | PosTag::Email
                | PosTag::Url
                | PosTag::Url2
                | PosTag::Linux
                | PosTag::Parens
        ),
        ParseNode::Tree { label, .. } => matches!(
            label,
            TreeLabel::Name
                | TreeLabel::NameEmail
                | TreeLabel::NameYear
                | TreeLabel::NameCaps
                | TreeLabel::Company
                | TreeLabel::AndCo
                | TreeLabel::YrRange
                | TreeLabel::YrAnd
                | TreeLabel::DashCaps
        ),
    }
}

pub(super) fn is_name_continuation(node: &ParseNode) -> bool {
    match node {
        ParseNode::Leaf(token) => matches!(
            token.tag,
            PosTag::Nnp
                | PosTag::Caps
                | PosTag::Comp
                | PosTag::MixedCap
                | PosTag::Cc
                | PosTag::Dash
                | PosTag::Of
                | PosTag::Van
                | PosTag::Linux
                | PosTag::Email
                | PosTag::Url
                | PosTag::Url2
        ),
        ParseNode::Tree { label, .. } => matches!(
            label,
            TreeLabel::Name
                | TreeLabel::NameEmail
                | TreeLabel::NameCaps
                | TreeLabel::NameYear
                | TreeLabel::Company
                | TreeLabel::AndCo
                | TreeLabel::DashCaps
        ),
    }
}

pub(super) fn is_same_line_holder_suffix_prefix(
    tree: &[ParseNode],
    idx: usize,
    line: LineNumber,
) -> bool {
    let Some(node) = tree.get(idx) else {
        return false;
    };
    let leaves = detector::token_utils::collect_all_leaves(node);
    let Some(first_token) = leaves.first() else {
        return false;
    };
    if first_token.start_line != line {
        return false;
    }

    let is_name_like_prefix = matches!(
        first_token.tag,
        PosTag::Nnp
            | PosTag::Nn
            | PosTag::Caps
            | PosTag::Comp
            | PosTag::MixedCap
            | PosTag::Uni
            | PosTag::Pn
            | PosTag::Ou
            | PosTag::Of
            | PosTag::Van
    );
    if !is_name_like_prefix {
        return false;
    }

    let end = std::cmp::min(idx + 6, tree.len());
    tree[idx..end].iter().any(|node| {
        detector::token_utils::collect_all_leaves(node)
            .iter()
            .any(|token| {
                token.start_line == line
                    && matches!(
                        token.tag,
                        PosTag::Auths | PosTag::AuthDot | PosTag::Contributors | PosTag::Commit
                    )
            })
    })
}

pub(super) fn has_same_line_confidential_proprietary_suffix(
    copyright_node: &ParseNode,
    tree: &[ParseNode],
    start: usize,
    line: LineNumber,
) -> bool {
    let node_has_confidential = detector::token_utils::collect_all_leaves(copyright_node)
        .iter()
        .any(|t| t.start_line == line && t.value.eq_ignore_ascii_case("Confidential"));
    if !node_has_confidential {
        return false;
    }

    let end = std::cmp::min(start + 6, tree.len());
    tree[start + 1..end].iter().any(|node| {
        detector::token_utils::collect_all_leaves(node)
            .iter()
            .any(|token| {
                token.start_line == line
                    && token
                        .value
                        .trim_end_matches(|c: char| c.is_ascii_punctuation())
                        .eq_ignore_ascii_case("proprietary")
            })
    })
}

pub(super) fn is_orphan_copy_name_match(node: &ParseNode) -> bool {
    match node.label() {
        Some(TreeLabel::NameYear) | Some(TreeLabel::NameEmail) | Some(TreeLabel::Company) => true,
        Some(TreeLabel::Name | TreeLabel::NameCaps) => {
            let leaves = detector::token_utils::collect_all_leaves(node);
            leaves
                .iter()
                .any(|t| detector::token_utils::is_year_like_token(t))
        }
        _ => false,
    }
}

pub(super) fn is_orphan_boundary(node: &ParseNode) -> bool {
    match node {
        ParseNode::Leaf(token) => matches!(
            token.tag,
            PosTag::EmptyLine
                | PosTag::Copy
                | PosTag::Auth
                | PosTag::Auth2
                | PosTag::Auths
                | PosTag::AuthDot
                | PosTag::Maint
                | PosTag::Contributors
                | PosTag::Commit
                | PosTag::SpdxContrib
                | PosTag::Junk
        ),
        ParseNode::Tree { label, .. } => matches!(
            label,
            TreeLabel::Copyright
                | TreeLabel::Copyright2
                | TreeLabel::Author
                | TreeLabel::AllRightReserved
        ),
    }
}

pub(super) fn has_name_tree_within(tree: &[ParseNode], start: usize, lookahead: usize) -> bool {
    let end = std::cmp::min(start + lookahead, tree.len());
    for node in &tree[start..end] {
        if let ParseNode::Tree { label, .. } = node
            && matches!(
                label,
                TreeLabel::Name | TreeLabel::Company | TreeLabel::NameEmail
            )
        {
            return true;
        }
    }
    false
}

pub(super) fn has_name_like_within(tree: &[ParseNode], start: usize, lookahead: usize) -> bool {
    let end = std::cmp::min(start + lookahead, tree.len());
    for node in &tree[start..end] {
        match node {
            ParseNode::Leaf(token) => {
                if matches!(
                    token.tag,
                    PosTag::Uni | PosTag::Nnp | PosTag::Caps | PosTag::Comp
                ) {
                    return true;
                }
            }
            ParseNode::Tree { label, .. } => {
                if matches!(
                    label,
                    TreeLabel::Name | TreeLabel::Company | TreeLabel::NameEmail
                ) {
                    return true;
                }
            }
        }
    }
    false
}

pub(super) fn has_company_signal_nearby(tree: &[ParseNode], start: usize) -> bool {
    let end = std::cmp::min(start + 3, tree.len());
    for node in &tree[start..end] {
        match node {
            ParseNode::Leaf(token) => {
                if matches!(token.tag, PosTag::Comp) {
                    return true;
                }
            }
            ParseNode::Tree { label, .. } => {
                if matches!(label, TreeLabel::Company) {
                    return true;
                }
            }
        }
    }
    false
}

pub(super) fn is_same_line_legal_tail_boundary(
    tree: &[ParseNode],
    start: usize,
    line: LineNumber,
) -> bool {
    let end = std::cmp::min(start + 3, tree.len());
    tree[start..end].iter().any(|node| match node {
        ParseNode::Leaf(token) => {
            token.start_line == line
                && token.tag == PosTag::Junk
                && !token.value.eq_ignore_ascii_case("file")
        }
        _ => false,
    })
}

pub(super) fn last_leaf_ends_with_comma(node: &ParseNode) -> bool {
    let leaves = detector::token_utils::collect_all_leaves(node);
    leaves.last().is_some_and(|t| t.value.ends_with(','))
}

pub(super) fn is_year_only_copyright_clause_node(node: &ParseNode) -> bool {
    if !matches!(
        node.label(),
        Some(TreeLabel::Copyright) | Some(TreeLabel::Copyright2)
    ) {
        return false;
    }

    let leaves = detector::token_utils::collect_all_leaves(node);
    let has_year = leaves
        .iter()
        .any(|t| detector::token_utils::is_year_like_token(t));
    if !has_year {
        return false;
    }

    let has_holder = detector::token_utils::build_holder_from_node(
        node,
        detector::NON_HOLDER_LABELS,
        detector::NON_HOLDER_POS_TAGS,
    )
    .is_some()
        || detector::token_utils::build_holder_from_node(
            node,
            detector::NON_HOLDER_LABELS_MINI,
            detector::NON_HOLDER_POS_TAGS_MINI,
        )
        .is_some();
    !has_holder
}
