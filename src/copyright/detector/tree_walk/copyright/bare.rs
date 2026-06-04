// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Standalone extraction passes that do not require the orphan-absorbing walk.
//!
//! - [`extract_holder_is_name`] handles `Copyright holder is <name>` phrasing.
//! - [`extract_bare_copyrights`] handles a bare `Copyright <name>` clause that
//!   the parser did not group into a single copyright node.

use crate::copyright::detector;
use crate::copyright::types::{
    CopyrightDetection, HolderDetection, ParseNode, PosTag, Token, TreeLabel,
};
use crate::models::LineNumber;

pub fn extract_holder_is_name(
    tree: &[ParseNode],
) -> (Vec<CopyrightDetection>, Vec<HolderDetection>) {
    let mut copyrights: Vec<CopyrightDetection> = Vec::new();
    let mut holders: Vec<HolderDetection> = Vec::new();

    let mut i = 0;
    while i < tree.len() {
        if let ParseNode::Leaf(token) = &tree[i]
            && token.tag == PosTag::Holder
            && i + 2 < tree.len()
            && let ParseNode::Leaf(is_token) = &tree[i + 1]
            && is_token.tag == PosTag::Is
            && matches!(
                tree[i + 2].label(),
                Some(TreeLabel::Name)
                    | Some(TreeLabel::NameEmail)
                    | Some(TreeLabel::NameYear)
                    | Some(TreeLabel::NameCaps)
                    | Some(TreeLabel::Company)
            )
        {
            let name_leaves = detector::token_utils::collect_filtered_leaves(
                &tree[i + 2],
                detector::NON_COPYRIGHT_LABELS,
                detector::NON_COPYRIGHT_POS_TAGS,
            );
            let name_leaves_stripped =
                detector::token_utils::strip_all_rights_reserved(name_leaves);
            let mut cr_tokens: Vec<&Token> = vec![token, is_token];
            cr_tokens.extend(&name_leaves_stripped);
            if let Some(det) = detector::token_utils::build_copyright_from_tokens(&cr_tokens) {
                copyrights.push(det);
            }

            let holder_leaves = detector::token_utils::collect_holder_filtered_leaves(
                &tree[i + 2],
                detector::NON_HOLDER_LABELS,
                detector::NON_HOLDER_POS_TAGS,
            );
            let holder_leaves = detector::token_utils::strip_all_rights_reserved(holder_leaves);
            if let Some(det) =
                detector::token_utils::build_holder_from_tokens(&holder_leaves, false)
            {
                holders.push(det);
            }
            i += 3;
            continue;
        }
        i += 1;
    }

    (copyrights, holders)
}

pub fn extract_bare_copyrights(
    tree: &[ParseNode],
) -> (Vec<CopyrightDetection>, Vec<HolderDetection>) {
    fn has_line_start_copyright_prefix(tree: &[ParseNode], idx: usize, line: LineNumber) -> bool {
        let mut found_copyright = false;
        for j in (0..idx).rev() {
            for t in detector::token_utils::collect_all_leaves(&tree[j])
                .iter()
                .rev()
            {
                if t.start_line != line {
                    continue;
                }
                if !found_copyright {
                    if t.tag == PosTag::Copy && t.value.eq_ignore_ascii_case("copyright") {
                        found_copyright = true;
                        continue;
                    }
                    return false;
                }
                return false;
            }
        }
        found_copyright
    }

    let mut copyrights: Vec<CopyrightDetection> = Vec::new();
    let mut holders: Vec<HolderDetection> = Vec::new();

    let mut i = 0;
    while i < tree.len() {
        if let ParseNode::Leaf(token) = &tree[i]
            && token.tag == PosTag::Copy
            && i + 1 < tree.len()
        {
            if token.value.eq_ignore_ascii_case("(c)")
                && has_line_start_copyright_prefix(tree, i, token.start_line)
            {
                i += 1;
                continue;
            }

            let next = &tree[i + 1];
            if matches!(
                next.label(),
                Some(TreeLabel::NameYear)
                    | Some(TreeLabel::Name)
                    | Some(TreeLabel::NameEmail)
                    | Some(TreeLabel::NameCaps)
                    | Some(TreeLabel::Company)
            ) {
                let portions_prefix = if i > 0
                    && let ParseNode::Leaf(prev) = &tree[i - 1]
                    && prev.tag == PosTag::Portions
                {
                    Some(prev)
                } else {
                    None
                };

                let mut cr_tokens: Vec<&Token> = Vec::new();
                if let Some(prefix) = portions_prefix {
                    cr_tokens.push(prefix);
                }
                cr_tokens.push(token);
                let name_leaves = detector::token_utils::collect_filtered_leaves(
                    next,
                    detector::NON_COPYRIGHT_LABELS,
                    detector::NON_COPYRIGHT_POS_TAGS,
                );
                let name_leaves = detector::token_utils::strip_all_rights_reserved(name_leaves);
                let allow_single_word_contributors = name_leaves
                    .iter()
                    .any(|t| detector::token_utils::is_year_like_token(t));
                cr_tokens.extend(&name_leaves);

                let mut extra_skip = 0usize;
                let mut j = i + 2;
                while j < tree.len() {
                    match &tree[j] {
                        ParseNode::Leaf(t)
                            if t.start_line == token.start_line
                                && matches!(
                                    t.tag,
                                    PosTag::Cc | PosTag::Email | PosTag::Url | PosTag::Url2
                                ) =>
                        {
                            cr_tokens.push(t);
                            j += 1;
                            extra_skip += 1;
                        }
                        _ => break,
                    }
                }
                if let Some(det) = detector::token_utils::build_copyright_from_tokens(&cr_tokens) {
                    copyrights.push(det);
                }

                let holder_leaves = detector::token_utils::collect_holder_filtered_leaves(
                    next,
                    detector::NON_HOLDER_LABELS,
                    detector::NON_HOLDER_POS_TAGS,
                );
                let holder_leaves = detector::token_utils::strip_all_rights_reserved(holder_leaves);
                if let Some(det) = detector::token_utils::build_holder_from_tokens(
                    &holder_leaves,
                    allow_single_word_contributors,
                ) {
                    holders.push(det);
                } else {
                    let holder_mini = detector::token_utils::collect_holder_filtered_leaves(
                        next,
                        detector::NON_HOLDER_LABELS_MINI,
                        detector::NON_HOLDER_POS_TAGS_MINI,
                    );
                    let holder_mini = detector::token_utils::strip_all_rights_reserved(holder_mini);
                    if let Some(det) = detector::token_utils::build_holder_from_tokens(
                        &holder_mini,
                        allow_single_word_contributors,
                    ) {
                        holders.push(det);
                    }
                }
                i += 2 + extra_skip;
                continue;
            }
        }
        i += 1;
    }

    (copyrights, holders)
}
