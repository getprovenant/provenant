// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Forward absorption of trailing tokens onto a copyright node.
//!
//! [`should_start_absorbing`] decides whether the node following a copyright
//! clause should be pulled into it, and [`collect_trailing_orphan_tokens`]
//! performs the bounded forward walk that gathers continuation tokens. The
//! remaining helpers absorb a following copyright clause or trailing year range.

use crate::copyright::detector;
use crate::copyright::types::{ParseNode, PosTag, Token, TreeLabel};
use crate::models::LineNumber;

use super::node_classify::{
    has_company_signal_nearby, has_name_like_within, has_same_line_confidential_proprietary_suffix,
    is_name_continuation, is_orphan_boundary, is_orphan_continuation,
    is_same_line_holder_suffix_prefix, is_same_line_legal_tail_boundary,
    is_year_only_copyright_clause_node, last_leaf_ends_with_comma,
};

/// Whether `token` closes an additional-holders marker — the `Oth` tag
/// ("others", "et al.", "et.al") or the `al`/`al.` of a split `et al.`
/// (tagged `AuthDot`). Bare `AuthDot` words like "authors." are excluded so
/// only the genuine "and unnamed others" marker terminates absorption.
fn is_additional_holders_marker_leaf(token: &Token) -> bool {
    match token.tag {
        PosTag::Oth => true,
        PosTag::AuthDot => token
            .value
            .trim_matches(|c: char| !c.is_ascii_alphanumeric())
            .eq_ignore_ascii_case("al"),
        _ => false,
    }
}

pub fn should_start_absorbing(
    copyright_node: &ParseNode,
    tree: &[ParseNode],
    start: usize,
) -> bool {
    if start >= tree.len() {
        return false;
    }
    let first = &tree[start];

    let last_line = detector::token_utils::collect_all_leaves(copyright_node)
        .last()
        .map(|t| t.start_line);

    if last_line.is_some()
        && last_line
            == detector::token_utils::collect_all_leaves(first)
                .first()
                .map(|t| t.start_line)
    {
        let last_tag = detector::token_utils::collect_all_leaves(copyright_node)
            .last()
            .map(|t| t.tag);
        if matches!(
            last_tag,
            Some(PosTag::Auths)
                | Some(PosTag::AuthDot)
                | Some(PosTag::Contributors)
                | Some(PosTag::Commit)
        ) {
            if is_orphan_continuation(first) {
                return true;
            }
            if let ParseNode::Leaf(token) = first
                && token.value.eq_ignore_ascii_case("as")
            {
                return true;
            }
        }
    }

    if let ParseNode::Leaf(token) = first
        && matches!(
            token.tag,
            PosTag::Auths | PosTag::AuthDot | PosTag::Contributors | PosTag::Commit
        )
    {
        let same_line = last_line.is_some_and(|l| l == token.start_line);
        let node_has_year = detector::token_utils::collect_all_leaves(copyright_node)
            .iter()
            .any(|t| detector::token_utils::is_year_like_token(t));
        let has_holder_like_tokens = detector::token_utils::collect_all_leaves(copyright_node)
            .iter()
            .any(|t| {
                matches!(
                    t.tag,
                    PosTag::Nnp
                        | PosTag::Caps
                        | PosTag::Comp
                        | PosTag::MixedCap
                        | PosTag::Uni
                        | PosTag::Pn
                        | PosTag::Ou
                        | PosTag::Url
                        | PosTag::Url2
                        | PosTag::Email
                )
            });
        if same_line && (has_holder_like_tokens || node_has_year) {
            return true;
        }
    }

    if let ParseNode::Tree {
        label: TreeLabel::Author | TreeLabel::AndAuth,
        ..
    } = first
    {
        let leaves = detector::token_utils::collect_all_leaves(first);
        let same_line =
            !leaves.is_empty() && leaves.iter().all(|t| last_line == Some(t.start_line));
        let has_author_keyword = leaves.iter().any(|t| {
            matches!(
                t.tag,
                PosTag::Auths | PosTag::AuthDot | PosTag::Contributors | PosTag::Commit
            )
        });
        if same_line && has_author_keyword {
            let node_has_year = detector::token_utils::collect_all_leaves(copyright_node)
                .iter()
                .any(|t| detector::token_utils::is_year_like_token(t));
            if node_has_year {
                return true;
            }
        }
    }

    if let ParseNode::Leaf(token) = first
        && token.tag == PosTag::Uni
        && last_line.is_some_and(|l| l == token.start_line)
    {
        return true;
    }

    if let ParseNode::Leaf(token) = first
        && token.tag == PosTag::By
        && last_line.is_some_and(|l| l == token.start_line)
    {
        let node_has_holder = detector::token_utils::build_holder_from_node(
            copyright_node,
            detector::NON_HOLDER_LABELS,
            detector::NON_HOLDER_POS_TAGS,
        )
        .is_some();
        if !node_has_holder && has_name_like_within(tree, start + 1, 3) {
            return true;
        }
    }

    if let ParseNode::Leaf(token) = first
        && token.tag == PosTag::Cd
        && last_line.is_some_and(|l| l == token.start_line)
    {
        let end = std::cmp::min(start + 5, tree.len());
        let has_company_suffix = tree[start..end].iter().any(|n| {
            detector::token_utils::collect_all_leaves(n)
                .iter()
                .any(|t| t.tag == PosTag::Comp)
        });
        let has_comma_boundary = token.value.ends_with(',')
            || tree.get(start + 1).is_some_and(|n| {
                detector::token_utils::collect_all_leaves(n)
                    .iter()
                    .any(|t| t.value == ",")
            });
        if has_company_suffix && has_comma_boundary {
            return true;
        }
    }

    if let ParseNode::Leaf(token) = first
        && token.tag == PosTag::Dash
        && last_line.is_some_and(|l| l == token.start_line)
    {
        let end = std::cmp::min(start + 5, tree.len());
        let has_email = tree[start..end].iter().any(|n| {
            detector::token_utils::collect_all_leaves(n)
                .iter()
                .any(|t| t.tag == PosTag::Email)
        });
        if has_email {
            return true;
        }
    }

    if let ParseNode::Leaf(token) = first
        && token.value.eq_ignore_ascii_case("as")
    {
        let end = std::cmp::min(start + 8, tree.len());
        let has_expected_title = tree[start..end].iter().any(|n| {
            detector::token_utils::collect_all_leaves(n)
                .iter()
                .any(|t| {
                    t.value.eq_ignore_ascii_case("secretary")
                        || t.value.eq_ignore_ascii_case("administrator")
                })
        });
        if has_expected_title {
            let same_line = last_line.is_some_and(|l| l == token.start_line);
            let has_holder_like_tokens = detector::token_utils::collect_all_leaves(copyright_node)
                .iter()
                .any(|t| {
                    matches!(
                        t.tag,
                        PosTag::Nnp
                            | PosTag::Caps
                            | PosTag::Comp
                            | PosTag::MixedCap
                            | PosTag::Uni
                            | PosTag::Pn
                            | PosTag::Ou
                    )
                });
            if same_line && has_holder_like_tokens {
                return true;
            }
        }
    }

    if is_year_only_copyright_clause_node(copyright_node)
        && let ParseNode::Leaf(token) = first
        && token.tag == PosTag::Nn
        && last_line.is_some_and(|l| l == token.start_line)
        && token.value == "Name"
    {
        return true;
    }

    if let ParseNode::Leaf(token) = first
        && last_line.is_some_and(|l| l == token.start_line)
        && matches!(
            token.tag,
            PosTag::Nnp
                | PosTag::Nn
                | PosTag::Caps
                | PosTag::Comp
                | PosTag::MixedCap
                | PosTag::Uni
                | PosTag::Pn
                | PosTag::Ou
                | PosTag::Url
                | PosTag::Url2
        )
    {
        let end = std::cmp::min(start + 6, tree.len());
        let suffix_boundary_on_same_line = tree[start..end].iter().any(|n| {
            detector::token_utils::collect_all_leaves(n)
                .iter()
                .any(|t| {
                    t.start_line == token.start_line
                        && matches!(
                            t.tag,
                            PosTag::Auths | PosTag::AuthDot | PosTag::Contributors | PosTag::Commit
                        )
                })
        });
        if suffix_boundary_on_same_line {
            return true;
        }
    }

    if let ParseNode::Leaf(token) = first
        && last_line.is_some_and(|l| l == token.start_line)
        && (token.value == "," || token.tag == PosTag::Cc)
    {
        let end = std::cmp::min(start + 6, tree.len());
        let has_expected_continuation = tree[start + 1..end].iter().any(|n| {
            is_name_continuation(n)
                || matches!(n.label(), Some(TreeLabel::YrRange) | Some(TreeLabel::YrAnd))
                || detector::token_utils::collect_all_leaves(n)
                    .iter()
                    .any(|t| t.tag == PosTag::Maint)
        });
        let has_holder_suffix_prefix =
            tree[start + 1..end].iter().enumerate().any(|(offset, _)| {
                is_same_line_holder_suffix_prefix(tree, start + 1 + offset, token.start_line)
            });
        let has_confidential_proprietary_suffix = has_same_line_confidential_proprietary_suffix(
            copyright_node,
            tree,
            start,
            token.start_line,
        );
        if has_expected_continuation
            || has_holder_suffix_prefix
            || has_confidential_proprietary_suffix
        {
            return true;
        }
    }

    if copyright_node.label() == Some(TreeLabel::Copyright2)
        && let ParseNode::Tree {
            label: TreeLabel::NameCaps,
            ..
        } = first
    {
        let leaves = detector::token_utils::collect_all_leaves(first);
        let same_line = !leaves.is_empty()
            && last_line.is_some_and(|l| leaves.first().is_some_and(|t| t.start_line == l));
        let node_has_year = detector::token_utils::collect_all_leaves(copyright_node)
            .iter()
            .any(|t| detector::token_utils::is_year_like_token(t));
        let last_tag = detector::token_utils::collect_all_leaves(copyright_node)
            .last()
            .map(|t| t.tag);
        if same_line && node_has_year && matches!(last_tag, Some(PosTag::Caps)) {
            return true;
        }
    }

    let strong_first = match first {
        ParseNode::Leaf(token) if token.tag == PosTag::Of || token.tag == PosTag::Van => {
            has_name_like_within(tree, start + 1, 2)
        }
        ParseNode::Tree { label, .. } => matches!(
            label,
            TreeLabel::Name
                | TreeLabel::NameEmail
                | TreeLabel::NameYear
                | TreeLabel::Company
                | TreeLabel::AndCo
                | TreeLabel::NameCaps
                | TreeLabel::DashCaps
        ),
        _ => false,
    };

    if strong_first {
        return true;
    }

    if last_leaf_ends_with_comma(copyright_node) {
        let node_has_year = detector::token_utils::collect_all_leaves(copyright_node)
            .iter()
            .any(|t| detector::token_utils::is_year_like_token(t));
        if node_has_year {
            let is_name_like_first = match first {
                ParseNode::Leaf(token) => matches!(
                    token.tag,
                    PosTag::Nnp | PosTag::Caps | PosTag::Comp | PosTag::Uni | PosTag::MixedCap
                ),
                _ => false,
            };
            if is_name_like_first {
                return has_company_signal_nearby(tree, start);
            }
        }
    }

    let is_name_like_first = match first {
        ParseNode::Leaf(token) => matches!(
            token.tag,
            PosTag::Nnp | PosTag::Caps | PosTag::Cd | PosTag::Cds | PosTag::Comp | PosTag::MixedCap
        ),
        _ => false,
    };
    if is_name_like_first {
        let same_line = last_line.is_some_and(|line| {
            detector::token_utils::collect_all_leaves(first)
                .first()
                .is_some_and(|token| token.start_line == line)
        });
        let node_has_holder = detector::token_utils::build_holder_from_node(
            copyright_node,
            detector::NON_HOLDER_LABELS,
            detector::NON_HOLDER_POS_TAGS,
        )
        .is_some();
        if same_line
            && node_has_holder
            && is_same_line_legal_tail_boundary(
                tree,
                start + 1,
                last_line.expect("same_line checked"),
            )
        {
            return true;
        }

        return has_company_signal_nearby(tree, start);
    }

    if let ParseNode::Leaf(token) = first
        && token.tag == PosTag::Linux
        && last_line.is_some_and(|l| l == token.start_line)
        && has_company_signal_nearby(tree, start)
    {
        let copy_count = detector::token_utils::collect_all_leaves(copyright_node)
            .iter()
            .filter(|t| t.tag == PosTag::Copy)
            .count();
        if copy_count != 1 {
            return false;
        }
        let has_holder_like_tokens = detector::token_utils::collect_all_leaves(copyright_node)
            .iter()
            .any(|t| {
                matches!(
                    t.tag,
                    PosTag::Nnp
                        | PosTag::Caps
                        | PosTag::Comp
                        | PosTag::MixedCap
                        | PosTag::Uni
                        | PosTag::Pn
                        | PosTag::Ou
                        | PosTag::Url
                        | PosTag::Url2
                        | PosTag::Email
                )
            });
        if has_holder_like_tokens {
            return true;
        }
    }

    false
}

pub fn collect_trailing_orphan_tokens<'a>(
    copyright_node: &'a ParseNode,
    tree: &'a [ParseNode],
    start: usize,
) -> (Vec<&'a Token>, usize) {
    if !should_start_absorbing(copyright_node, tree, start) {
        return (Vec::new(), 0);
    }

    fn is_allowed_holder_suffix_boundary_on_same_line(
        copyright_node: &ParseNode,
        node: &ParseNode,
    ) -> bool {
        let last_line = detector::token_utils::collect_all_leaves(copyright_node)
            .last()
            .map(|t| t.start_line);
        let Some(last_line) = last_line else {
            return false;
        };

        match node {
            ParseNode::Leaf(token) => {
                token.start_line == last_line
                    && matches!(
                        token.tag,
                        PosTag::Auths
                            | PosTag::AuthDot
                            | PosTag::Maint
                            | PosTag::Contributors
                            | PosTag::Commit
                    )
            }
            ParseNode::Tree {
                label: TreeLabel::Author | TreeLabel::AndAuth,
                ..
            } => {
                let leaves = detector::token_utils::collect_all_leaves(node);
                !leaves.is_empty()
                    && leaves.iter().all(|t| t.start_line == last_line)
                    && leaves.iter().any(|t| {
                        matches!(
                            t.tag,
                            PosTag::Auths
                                | PosTag::AuthDot
                                | PosTag::Maint
                                | PosTag::Contributors
                                | PosTag::Commit
                        )
                    })
            }
            _ => false,
        }
    }

    let mut tokens: Vec<&Token> = Vec::new();
    let mut j = start;

    let last_line = detector::token_utils::collect_all_leaves(copyright_node)
        .last()
        .map(|t| t.start_line);

    // Line on which an additional-holders marker ("et al", "and others") was
    // absorbed, if any. Such a marker conventionally means "and unnamed
    // others", so nothing meaningful follows it in a copyright statement.
    // Once it is reached, forward absorption must not cross onto a later line
    // (into an SPDX tag, code, or prose) regardless of what that line holds.
    let mut additional_holders_marker_line: Option<LineNumber> = None;

    while j < tree.len() {
        let node = &tree[j];

        if let Some(marker_line) = additional_holders_marker_line
            && detector::token_utils::collect_all_leaves(node)
                .first()
                .is_some_and(|t| t.start_line > marker_line)
        {
            break;
        }

        if let Some(last_line) = last_line
            && matches!(
                node.label(),
                Some(TreeLabel::Copyright) | Some(TreeLabel::Copyright2)
            )
        {
            let leaves = detector::token_utils::collect_all_leaves(node);
            if leaves.first().is_some_and(|t| t.start_line > last_line) {
                break;
            }
        }

        let allowed_suffix = is_allowed_holder_suffix_boundary_on_same_line(copyright_node, node);
        let allowed_suffix_prefix =
            last_line.is_some_and(|line| is_same_line_holder_suffix_prefix(tree, j, line));

        let allow_junk_file = match node {
            ParseNode::Leaf(token)
                if token.tag == PosTag::Junk && token.value.eq_ignore_ascii_case("file") =>
            {
                tokens
                    .last()
                    .is_some_and(|prev| prev.value.eq_ignore_ascii_case("AUTHORS"))
            }
            _ => false,
        };

        if is_orphan_boundary(node) && !allowed_suffix && !allowed_suffix_prefix && !allow_junk_file
        {
            break;
        }

        if !is_orphan_continuation(node)
            && !allowed_suffix
            && !allowed_suffix_prefix
            && !allow_junk_file
        {
            break;
        }

        let leaves = detector::token_utils::collect_all_leaves(node);
        let already_have_url = tokens
            .iter()
            .any(|t| matches!(t.tag, PosTag::Url | PosTag::Url2));
        let leaves_have_url = leaves
            .iter()
            .any(|t| matches!(t.tag, PosTag::Url | PosTag::Url2));
        if already_have_url && leaves_have_url {
            break;
        }

        if let Some(marker) = leaves.iter().find(|t| is_additional_holders_marker_leaf(t)) {
            additional_holders_marker_line = Some(marker.start_line);
        }

        tokens.extend(leaves);
        j += 1;
    }

    let skip = j - start;
    (tokens, skip)
}

pub(super) fn collect_following_copyright_clause_tokens(
    tree: &[ParseNode],
    start: usize,
    line: LineNumber,
) -> (Vec<&Token>, usize) {
    if start >= tree.len() {
        return (Vec::new(), 0);
    }

    match &tree[start] {
        ParseNode::Leaf(token)
            if token.tag == PosTag::Copy && token.value.eq_ignore_ascii_case("copyright") => {}
        _ => return (Vec::new(), 0),
    }

    let mut tokens: Vec<&Token> = Vec::new();
    let mut j = start;
    let max_nodes = std::cmp::min(start + 16, tree.len());

    while j < max_nodes {
        let node = &tree[j];
        let leaves = detector::token_utils::collect_all_leaves(node);
        if leaves.first().is_none_or(|t| t.start_line != line) {
            break;
        }

        if j != start && is_orphan_boundary(node) {
            break;
        }

        tokens.extend(leaves);
        j += 1;
    }

    let skip = j - start;
    let has_year = tokens
        .iter()
        .any(|t| detector::token_utils::is_year_like_token(t));

    if !has_year {
        return (Vec::new(), 0);
    }

    let has_name_like = tokens.iter().any(|t| {
        matches!(
            t.tag,
            PosTag::Nnp
                | PosTag::Caps
                | PosTag::Comp
                | PosTag::MixedCap
                | PosTag::Uni
                | PosTag::Pn
                | PosTag::Ou
                | PosTag::Email
                | PosTag::Url
                | PosTag::Url2
        )
    });
    if has_name_like {
        return (Vec::new(), 0);
    }

    (tokens, skip)
}

pub(super) fn get_trailing_year_range<'a>(
    copyright_node: &ParseNode,
    tree: &'a [ParseNode],
    start: usize,
) -> Option<(Vec<&'a Token>, usize)> {
    if start >= tree.len() {
        return None;
    }
    let next = &tree[start];
    let is_yr_tree = matches!(
        next.label(),
        Some(TreeLabel::YrRange) | Some(TreeLabel::YrAnd)
    );
    if !is_yr_tree {
        return None;
    }
    let node_has_year = detector::token_utils::collect_all_leaves(copyright_node)
        .iter()
        .any(|t| detector::token_utils::is_year_like_token(t));
    if node_has_year {
        return None;
    }
    let yr_tokens = detector::token_utils::collect_all_leaves(next);
    Some((yr_tokens, 1))
}
