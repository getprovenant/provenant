// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! The primary node-by-node tree walk driver.
//!
//! [`extract_from_tree_nodes`] iterates the top-level parse nodes, attaches
//! recovered prefix tokens, absorbs trailing continuations, runs the cross-node
//! merges, and falls back to the author extractors. It produces the copyright,
//! holder, and author detections for the structured-tree pass.

use crate::copyright::detector;
use crate::copyright::types::{
    AuthorDetection, CopyrightDetection, HolderDetection, ParseNode, PosTag, Token, TreeLabel,
};

use super::super::author;
use super::absorb::{
    collect_following_copyright_clause_tokens, collect_trailing_orphan_tokens,
    get_trailing_year_range,
};
use super::merge::{
    merge_copyright_with_following_author,
    merge_year_only_copyright_clause_with_preceding_copyrighted_by,
};
use super::node_classify::{
    has_name_tree_within, is_name_continuation, is_orphan_boundary, is_orphan_continuation,
    is_orphan_copy_name_match, is_year_only_copyright_clause_node, last_leaf_ends_with_comma,
};
use super::prefix::{
    get_orphaned_copy_prefix, get_orphaned_not_prefix, mpl_portions_created_prefix_tokens,
    single_portions_prefix_token,
};

pub fn extract_from_tree_nodes(
    tree: &[ParseNode],
    allow_not_copyrighted_prefix: bool,
) -> (
    Vec<CopyrightDetection>,
    Vec<HolderDetection>,
    Vec<AuthorDetection>,
) {
    let mut copyrights: Vec<CopyrightDetection> = Vec::new();
    let mut holders: Vec<HolderDetection> = Vec::new();
    let mut authors: Vec<AuthorDetection> = Vec::new();

    let group_has_copyright = tree.iter().any(|n| {
        matches!(
            n.label(),
            Some(TreeLabel::Copyright) | Some(TreeLabel::Copyright2)
        )
    });

    let mut preceding_year_only_prefix: Option<Vec<&Token>> = None;

    let mut i = 0;
    while i < tree.len() {
        let node = &tree[i];
        let label = node.label();

        if matches!(
            label,
            Some(TreeLabel::Copyright) | Some(TreeLabel::Copyright2)
        ) {
            if preceding_year_only_prefix.is_none()
                && is_year_only_copyright_clause_node(node)
                && let Some(next_node) = tree.get(i + 1)
                && matches!(
                    next_node.label(),
                    Some(TreeLabel::Copyright) | Some(TreeLabel::Copyright2)
                )
                && !is_year_only_copyright_clause_node(next_node)
                && detector::token_utils::collect_all_leaves(node)
                    .first()
                    .is_some_and(|t| {
                        detector::token_utils::collect_all_leaves(next_node)
                            .first()
                            .is_some_and(|n| n.start_line == t.start_line + 1)
                    })
            {
                let leaves = detector::token_utils::collect_filtered_leaves(
                    node,
                    detector::NON_COPYRIGHT_LABELS,
                    detector::NON_COPYRIGHT_POS_TAGS,
                );
                let leaves = detector::token_utils::strip_all_rights_reserved(leaves);
                if !leaves.is_empty() {
                    preceding_year_only_prefix = Some(leaves);
                    i += 1;
                    continue;
                }
            }

            let allow_single_word_contributors = detector::token_utils::collect_all_leaves(node)
                .iter()
                .any(|t| detector::token_utils::is_year_like_token(t));
            let prefix_token = get_orphaned_copy_prefix(tree, i);
            let not_prefix = get_orphaned_not_prefix(tree, i, node, allow_not_copyrighted_prefix);
            let (mut trailing_tokens, mut skip) = collect_trailing_orphan_tokens(node, tree, i + 1);
            let mut trailing_copyright_only_tokens: Vec<&Token> = Vec::new();

            if trailing_tokens.is_empty() {
                let last_line = detector::token_utils::collect_all_leaves(node)
                    .last()
                    .map(|t| t.start_line);
                if let Some(last_line) = last_line {
                    let mut merged = false;

                    for offset in 1..=6 {
                        let idx = i + offset;
                        if idx >= tree.len() {
                            break;
                        }
                        let leaves = detector::token_utils::collect_all_leaves(&tree[idx]);
                        if leaves.first().is_none_or(|t| t.start_line != last_line) {
                            break;
                        }
                        let comma_boundary = if last_leaf_ends_with_comma(node) {
                            true
                        } else {
                            ((i + 1)..idx).any(|k| {
                                detector::token_utils::collect_all_leaves(&tree[k])
                                    .iter()
                                    .any(|t| {
                                        t.value == ","
                                            || t.tag == PosTag::Cc
                                            || t.value.ends_with(',')
                                    })
                            })
                        };
                        if !comma_boundary {
                            continue;
                        }

                        if is_year_only_copyright_clause_node(&tree[idx]) {
                            let combined: Vec<&Token> = tree
                                .iter()
                                .take(idx + 1)
                                .skip(i + 1)
                                .flat_map(detector::collect_all_leaves)
                                .collect();
                            trailing_copyright_only_tokens = combined;
                            skip = idx - (i + 1) + 1;
                            merged = true;
                            break;
                        }

                        if let ParseNode::Leaf(token) = &tree[idx]
                            && token.tag == PosTag::Copy
                            && token.value.eq_ignore_ascii_case("copyright")
                        {
                            let (clause_tokens, clause_skip) =
                                collect_following_copyright_clause_tokens(tree, idx, last_line);
                            if clause_tokens.is_empty() {
                                continue;
                            }

                            let mut combined: Vec<&Token> = tree
                                .iter()
                                .take(idx)
                                .skip(i + 1)
                                .flat_map(detector::collect_all_leaves)
                                .collect();
                            combined.extend(clause_tokens);
                            trailing_copyright_only_tokens = combined;
                            skip = (idx - (i + 1)) + clause_skip;
                            merged = true;
                            break;
                        }
                    }

                    if !merged
                        && last_leaf_ends_with_comma(node)
                        && i + 1 < tree.len()
                        && let ParseNode::Leaf(token) = &tree[i + 1]
                        && token.start_line == last_line
                    {
                        let is_comma_separated_holder_leaf =
                            matches!(token.tag, PosTag::MixedCap | PosTag::Comp)
                                || (matches!(token.tag, PosTag::Caps | PosTag::Nnp)
                                    && token.value.contains('-'));
                        if is_comma_separated_holder_leaf {
                            trailing_tokens.push(token);
                            skip = 1;
                        }
                    }
                }
            }

            if !trailing_tokens.is_empty() {
                let last_line = detector::token_utils::collect_all_leaves(node)
                    .last()
                    .map(|t| t.start_line);
                let last_token_has_comma = trailing_tokens.last().is_some_and(|t| {
                    t.value.ends_with(',') || t.value == "," || t.tag == PosTag::Cc
                });

                if last_token_has_comma && let Some(last_line) = last_line {
                    let after_idx = i + 1 + skip;
                    for clause_offset in 0..=2 {
                        let idx = after_idx + clause_offset;
                        if idx >= tree.len() {
                            break;
                        }
                        let leaves = detector::token_utils::collect_all_leaves(&tree[idx]);
                        if leaves.first().is_none_or(|t| t.start_line != last_line) {
                            break;
                        }

                        if is_year_only_copyright_clause_node(&tree[idx]) {
                            trailing_copyright_only_tokens
                                .extend(detector::token_utils::collect_all_leaves(&tree[idx]));
                            skip += clause_offset + 1;
                            break;
                        }

                        if let ParseNode::Leaf(token) = &tree[idx]
                            && token.tag == PosTag::Copy
                            && token.value.eq_ignore_ascii_case("copyright")
                        {
                            let (clause_tokens, clause_skip) =
                                collect_following_copyright_clause_tokens(tree, idx, last_line);
                            if !clause_tokens.is_empty() {
                                trailing_copyright_only_tokens.extend(clause_tokens);
                                skip += clause_offset + clause_skip;
                            }
                            break;
                        }
                    }
                }
            }
            let mpl_prefix = mpl_portions_created_prefix_tokens(tree, i, node, &trailing_tokens);
            let portions_prefix = single_portions_prefix_token(tree, i, node);

            if trailing_tokens.is_empty() && trailing_copyright_only_tokens.is_empty() {
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

                if !has_holder
                    && is_year_only_copyright_clause_node(node)
                    && let Some((cr_det, holder_det)) =
                        merge_year_only_copyright_clause_with_preceding_copyrighted_by(
                            tree,
                            i,
                            prefix_token,
                            portions_prefix,
                            mpl_prefix.as_deref(),
                        )
                {
                    copyrights.push(cr_det);
                    holders.push(holder_det);
                    i += 1;
                    continue;
                }

                if !has_holder
                    && i + 1 < tree.len()
                    && matches!(tree[i + 1], ParseNode::Leaf(ref t) if t.tag == PosTag::Uni)
                    && has_name_tree_within(tree, i + 2, 2)
                {
                    let mut cr_tokens: Vec<&Token> = Vec::new();
                    if let Some(prefix) = prefix_token {
                        cr_tokens.push(prefix);
                    }
                    if let Some(prefix) = portions_prefix {
                        cr_tokens.push(prefix);
                    }
                    if let Some(prefix) = mpl_prefix.as_ref() {
                        cr_tokens.extend(prefix.iter().copied());
                    }
                    let node_leaves = detector::token_utils::collect_filtered_leaves(
                        node,
                        detector::NON_COPYRIGHT_LABELS,
                        detector::NON_COPYRIGHT_POS_TAGS,
                    );
                    let node_leaves = detector::token_utils::strip_all_rights_reserved(node_leaves);
                    cr_tokens.extend(&node_leaves);

                    let mut extra_skip = 0;
                    let mut j = i + 1;
                    while j < tree.len()
                        && !is_orphan_boundary(&tree[j])
                        && is_orphan_continuation(&tree[j])
                    {
                        let leaves = detector::token_utils::collect_all_leaves(&tree[j]);
                        cr_tokens.extend(leaves);
                        j += 1;
                        extra_skip += 1;
                    }
                    let cr_tokens = detector::token_utils::strip_all_rights_reserved(cr_tokens);
                    if let Some(det) =
                        detector::token_utils::build_copyright_from_tokens(&cr_tokens)
                    {
                        copyrights.push(det);
                    }

                    let mut holder_tokens: Vec<&Token> = Vec::new();
                    let node_holder_leaves = detector::token_utils::collect_holder_filtered_leaves(
                        node,
                        detector::NON_HOLDER_LABELS,
                        detector::NON_HOLDER_POS_TAGS,
                    );
                    let node_holder_leaves =
                        detector::token_utils::strip_all_rights_reserved(node_holder_leaves);
                    holder_tokens.extend(&node_holder_leaves);
                    let mut k = i + 1;
                    while k < j {
                        let leaves = detector::token_utils::collect_all_leaves(&tree[k]);
                        holder_tokens.extend(leaves);
                        k += 1;
                    }
                    let holder_tokens =
                        detector::token_utils::strip_all_rights_reserved(holder_tokens);
                    if let Some(det) = detector::token_utils::build_holder_from_tokens(
                        &holder_tokens,
                        allow_single_word_contributors,
                    ) {
                        holders.push(det);
                    }

                    i += extra_skip;
                    i += 1;
                    continue;
                }

                if !has_holder
                    && i + 1 < tree.len()
                    && tree[i + 1].label() == Some(TreeLabel::Author)
                    && let Some((cr_det, h_det, skip)) =
                        merge_copyright_with_following_author(node, prefix_token, tree, i + 1)
                {
                    copyrights.push(cr_det);
                    if let Some(h) = h_det {
                        holders.push(h);
                    }
                    i += skip + 1;
                    i += 1;
                    continue;
                }

                if !has_holder && i + 1 < tree.len() {
                    let copyright_ends_with_year = {
                        let leaves = detector::token_utils::collect_all_leaves(node);
                        leaves
                            .last()
                            .is_some_and(|t| detector::token_utils::is_year_like_token(t))
                    };
                    let next_node = &tree[i + 1];
                    let next_line_ok = {
                        let last_line = detector::token_utils::collect_all_leaves(node)
                            .last()
                            .map(|t| t.start_line);
                        let first_next_line = detector::token_utils::collect_all_leaves(next_node)
                            .first()
                            .map(|t| t.start_line);
                        last_line.is_some_and(|l| first_next_line == Some(l + 1))
                    };
                    let next_is_holderish = match next_node {
                        ParseNode::Tree { label, .. } => matches!(
                            label,
                            TreeLabel::Name
                                | TreeLabel::NameCaps
                                | TreeLabel::NameYear
                                | TreeLabel::NameEmail
                                | TreeLabel::Company
                                | TreeLabel::AndCo
                                | TreeLabel::DashCaps
                        ),
                        ParseNode::Leaf(t) => matches!(
                            t.tag,
                            PosTag::Nnp
                                | PosTag::Caps
                                | PosTag::Comp
                                | PosTag::MixedCap
                                | PosTag::Uni
                                | PosTag::Pn
                                | PosTag::Email
                        ),
                    };

                    if copyright_ends_with_year && next_line_ok && next_is_holderish {
                        let name_node = next_node;
                        let mut cr_tokens: Vec<&Token> =
                            preceding_year_only_prefix.take().unwrap_or_default();
                        if let Some(prefix) = prefix_token {
                            cr_tokens.push(prefix);
                        }
                        if let Some(prefix) = portions_prefix {
                            cr_tokens.push(prefix);
                        }
                        if let Some(prefix) = mpl_prefix.as_ref() {
                            cr_tokens.extend(prefix.iter().copied());
                        }
                        let node_leaves = detector::token_utils::collect_filtered_leaves(
                            node,
                            detector::NON_COPYRIGHT_LABELS,
                            detector::NON_COPYRIGHT_POS_TAGS,
                        );
                        let node_leaves =
                            detector::token_utils::strip_all_rights_reserved(node_leaves);
                        cr_tokens.extend(&node_leaves);

                        let name_leaves = detector::token_utils::collect_all_leaves(name_node);
                        let mut holder_tokens: Vec<&Token> = name_leaves.clone();
                        cr_tokens.extend(&name_leaves);

                        let mut j = i + 2;
                        while j < tree.len()
                            && !is_orphan_boundary(&tree[j])
                            && is_name_continuation(&tree[j])
                        {
                            let leaves = detector::token_utils::collect_all_leaves(&tree[j]);
                            cr_tokens.extend(leaves.iter());
                            holder_tokens.extend(leaves);
                            j += 1;
                        }
                        let cr_tokens = detector::token_utils::strip_all_rights_reserved(cr_tokens);
                        if let Some(det) =
                            detector::token_utils::build_copyright_from_tokens(&cr_tokens)
                        {
                            copyrights.push(det);
                        }

                        let holder_tokens =
                            detector::token_utils::strip_all_rights_reserved(holder_tokens);
                        if let Some(det) = detector::token_utils::build_holder_from_tokens(
                            &holder_tokens,
                            allow_single_word_contributors,
                        ) {
                            holders.push(det);
                        }

                        i = j;
                        continue;
                    }
                }

                let trailing_yr = get_trailing_year_range(node, tree, i + 1);

                if let Some((yr_tokens, yr_skip)) = trailing_yr {
                    let mut cr_tokens: Vec<&Token> = Vec::new();
                    if let Some(prefix) = prefix_token {
                        cr_tokens.push(prefix);
                    }
                    if let Some(prefix) = portions_prefix {
                        cr_tokens.push(prefix);
                    }
                    if let Some(prefix) = mpl_prefix.as_ref() {
                        cr_tokens.extend(prefix.iter().copied());
                    }
                    let node_leaves = detector::token_utils::collect_filtered_leaves(
                        node,
                        detector::NON_COPYRIGHT_LABELS,
                        detector::NON_COPYRIGHT_POS_TAGS,
                    );
                    let node_leaves = detector::token_utils::strip_all_rights_reserved(node_leaves);
                    cr_tokens.extend(&node_leaves);
                    cr_tokens.extend(&yr_tokens);
                    let cr_tokens = detector::token_utils::strip_all_rights_reserved(cr_tokens);
                    if let Some(det) =
                        detector::token_utils::build_copyright_from_tokens(&cr_tokens)
                    {
                        copyrights.push(det);
                    }
                    let holder = detector::token_utils::build_holder_from_node(
                        node,
                        detector::NON_HOLDER_LABELS,
                        detector::NON_HOLDER_POS_TAGS,
                    );
                    if let Some(det) = holder {
                        holders.push(det);
                    } else if let Some(det) = detector::token_utils::build_holder_from_node(
                        node,
                        detector::NON_HOLDER_LABELS_MINI,
                        detector::NON_HOLDER_POS_TAGS_MINI,
                    ) {
                        holders.push(det);
                    }
                    i += yr_skip;
                } else {
                    let mut prefixes: Vec<&Token> =
                        preceding_year_only_prefix.take().unwrap_or_default();
                    if let Some(not) = not_prefix {
                        prefixes.push(not);
                    }
                    if let Some(prefix) = portions_prefix {
                        prefixes.push(prefix);
                    }
                    if let Some(prefix) = prefix_token {
                        prefixes.push(prefix);
                    }
                    if let Some(prefix) = mpl_prefix.as_ref() {
                        prefixes.extend(prefix.iter().copied());
                    }

                    let cr_ok = if let Some(det) = {
                        let leaves = detector::token_utils::collect_filtered_leaves(
                            node,
                            detector::NON_COPYRIGHT_LABELS,
                            detector::NON_COPYRIGHT_POS_TAGS,
                        );
                        let filtered = detector::token_utils::strip_all_rights_reserved(leaves);
                        let mut all_tokens: Vec<&Token> = Vec::new();
                        all_tokens.extend(&prefixes);
                        all_tokens.extend(filtered);
                        detector::token_utils::build_copyright_from_tokens(&all_tokens)
                    } {
                        copyrights.push(det);
                        true
                    } else {
                        false
                    };

                    if let Some(not) = not_prefix {
                        let mut holder_tokens: Vec<&Token> = vec![not];
                        let node_holder_leaves =
                            detector::token_utils::collect_holder_filtered_leaves(
                                node,
                                detector::NON_HOLDER_LABELS,
                                detector::NON_HOLDER_POS_TAGS,
                            );
                        let node_holder_leaves =
                            detector::token_utils::strip_all_rights_reserved(node_holder_leaves);
                        holder_tokens.extend(node_holder_leaves);
                        let holder_tokens =
                            detector::token_utils::strip_all_rights_reserved(holder_tokens);
                        if let Some(det) = detector::token_utils::build_holder_from_tokens(
                            &holder_tokens,
                            allow_single_word_contributors,
                        ) {
                            holders.push(det);
                        }
                    } else {
                        let holder = detector::token_utils::build_holder_from_copyright_node(
                            node,
                            detector::NON_HOLDER_LABELS,
                            detector::NON_HOLDER_POS_TAGS,
                        );
                        if let Some(det) = holder {
                            holders.push(det);
                        } else if let Some(det) =
                            detector::token_utils::build_holder_from_copyright_node(
                                node,
                                detector::NON_HOLDER_LABELS_MINI,
                                detector::NON_HOLDER_POS_TAGS_MINI,
                            )
                        {
                            holders.push(det);
                        }
                    }
                    if cr_ok && let Some(det) = author::extract_author_from_copyright_node(node) {
                        authors.push(det);
                    }
                }
            } else {
                let mut cr_tokens: Vec<&Token> = Vec::new();
                if let Some(prefix) = prefix_token {
                    cr_tokens.push(prefix);
                }
                if let Some(prefix) = portions_prefix {
                    cr_tokens.push(prefix);
                }
                if let Some(prefix) = mpl_prefix.as_ref() {
                    cr_tokens.extend(prefix.iter().copied());
                }
                let node_leaves = detector::token_utils::collect_filtered_leaves(
                    node,
                    detector::NON_COPYRIGHT_LABELS,
                    detector::NON_COPYRIGHT_POS_TAGS,
                );
                let node_leaves = detector::token_utils::strip_all_rights_reserved(node_leaves);
                cr_tokens.extend(&node_leaves);

                let mut short_cr_tokens = cr_tokens.clone();

                let copy_count = detector::token_utils::collect_all_leaves(node)
                    .iter()
                    .filter(|t| t.tag == PosTag::Copy)
                    .count();
                let emit_short_linux_variant = copy_count == 1
                    && trailing_tokens
                        .first()
                        .is_some_and(|t| t.tag == PosTag::Linux);

                cr_tokens.extend(&trailing_tokens);
                cr_tokens.extend(&trailing_copyright_only_tokens);

                let cr_tokens = detector::token_utils::strip_all_rights_reserved(cr_tokens);
                short_cr_tokens = detector::token_utils::strip_all_rights_reserved(short_cr_tokens);
                let full_cr = detector::token_utils::build_copyright_from_tokens(&cr_tokens);
                if let Some(det) = full_cr.as_ref() {
                    copyrights.push(det.clone());
                }
                if emit_short_linux_variant
                    && let Some(short_det) =
                        detector::token_utils::build_copyright_from_tokens(&short_cr_tokens)
                    && full_cr
                        .as_ref()
                        .is_none_or(|f| f.copyright != short_det.copyright)
                {
                    copyrights.push(short_det);
                }

                let mut holder_tokens: Vec<&Token> = Vec::new();
                let copy_line = detector::token_utils::collect_all_leaves(node)
                    .iter()
                    .filter(|t| t.tag == PosTag::Copy && t.value.eq_ignore_ascii_case("copyright"))
                    .map(|t| t.start_line)
                    .min();
                let keep_prefix_lines = copy_line
                    .map(|cl| detector::token_utils::signal_lines_before_copy_line(node, cl))
                    .unwrap_or_default();
                let node_holder_leaves = detector::token_utils::collect_holder_filtered_leaves(
                    node,
                    detector::NON_HOLDER_LABELS,
                    detector::NON_HOLDER_POS_TAGS,
                );
                let mut node_holder_leaves =
                    detector::token_utils::strip_all_rights_reserved(node_holder_leaves);
                if let Some(copy_line) = copy_line {
                    node_holder_leaves.retain(|t| {
                        t.start_line >= copy_line || keep_prefix_lines.contains(&t.start_line.get())
                    });
                }
                detector::token_utils::strip_trailing_commas(&mut node_holder_leaves);
                holder_tokens.extend(&node_holder_leaves);

                let mut short_holder_tokens = holder_tokens.clone();

                let node_ends_with_year = {
                    let all_leaves = detector::token_utils::collect_all_leaves(node);
                    let mut found = false;
                    for t in all_leaves.iter().rev() {
                        if t.tag == PosTag::Cc && t.value == "," {
                            continue;
                        }
                        if detector::token_utils::is_year_like_token(t) {
                            found = true;
                        }
                        break;
                    }
                    found
                };
                holder_tokens.extend(detector::token_utils::filter_holder_tokens_with_state(
                    &trailing_tokens,
                    detector::NON_HOLDER_POS_TAGS,
                    node_ends_with_year,
                ));
                let holder_tokens = detector::token_utils::strip_all_rights_reserved(holder_tokens);

                let full_holder = if let Some(det) = detector::token_utils::build_holder_from_tokens(
                    &holder_tokens,
                    allow_single_word_contributors,
                ) {
                    Some(det)
                } else {
                    let mut holder_tokens_mini: Vec<&Token> = Vec::new();
                    let node_holder_mini = detector::token_utils::collect_holder_filtered_leaves(
                        node,
                        detector::NON_HOLDER_LABELS_MINI,
                        detector::NON_HOLDER_POS_TAGS_MINI,
                    );
                    let mut node_holder_mini =
                        detector::token_utils::strip_all_rights_reserved(node_holder_mini);
                    if let Some(copy_line) = copy_line {
                        node_holder_mini.retain(|t| {
                            t.start_line >= copy_line
                                || keep_prefix_lines.contains(&t.start_line.get())
                        });
                    }
                    detector::token_utils::strip_trailing_commas(&mut node_holder_mini);
                    holder_tokens_mini.extend(&node_holder_mini);
                    let node_ends_with_year_mini = detector::token_utils::collect_all_leaves(node)
                        .last()
                        .is_some_and(|t| detector::token_utils::is_year_like_token(t));
                    holder_tokens_mini.extend(
                        detector::token_utils::filter_holder_tokens_with_state(
                            &trailing_tokens,
                            detector::NON_HOLDER_POS_TAGS_MINI,
                            node_ends_with_year_mini,
                        ),
                    );
                    let holder_tokens_mini =
                        detector::token_utils::strip_all_rights_reserved(holder_tokens_mini);
                    detector::token_utils::build_holder_from_tokens(
                        &holder_tokens_mini,
                        allow_single_word_contributors,
                    )
                };

                if let Some(det) = full_holder.as_ref() {
                    holders.push(det.clone());
                }

                if emit_short_linux_variant {
                    short_holder_tokens =
                        detector::token_utils::strip_all_rights_reserved(short_holder_tokens);
                    if let Some(short_det) = detector::token_utils::build_holder_from_tokens(
                        &short_holder_tokens,
                        allow_single_word_contributors,
                    ) && full_holder
                        .as_ref()
                        .is_none_or(|f| f.holder != short_det.holder)
                    {
                        holders.push(short_det);
                    }
                }
                i += skip;
            }
        } else if label == Some(TreeLabel::Author) {
            if let Some(dets) = author::extract_sectioned_authors_from_author_node(node) {
                authors.extend(dets);
                i += 1;
                continue;
            }
            if let Some((det, skip)) = author::build_author_with_trailing(node, tree, i + 1) {
                authors.push(det);
                i += skip;
            } else if let Some(det) = detector::token_utils::build_author_from_node(node) {
                authors.push(det);
            }
        } else if let ParseNode::Leaf(token) = node
            && token.tag == PosTag::Copy
        {
            let (name_node_idx, extra_copy_tokens) =
                if i + 1 < tree.len() && is_orphan_copy_name_match(&tree[i + 1]) {
                    (Some(i + 1), vec![])
                } else if i + 2 < tree.len()
                    && matches!(&tree[i + 1], ParseNode::Leaf(t) if t.tag == PosTag::Copy)
                    && is_orphan_copy_name_match(&tree[i + 2])
                {
                    let extra = if let ParseNode::Leaf(t) = &tree[i + 1] {
                        vec![t]
                    } else {
                        vec![]
                    };
                    (Some(i + 2), extra)
                } else {
                    (None, vec![])
                };

            if let Some(name_idx) = name_node_idx {
                let next = &tree[name_idx];
                let mut cr_tokens: Vec<&Token> = Vec::new();
                if let Some(prefix) = get_orphaned_copy_prefix(tree, i) {
                    cr_tokens.push(prefix);
                }
                if i > 0
                    && let ParseNode::Leaf(prev) = &tree[i - 1]
                    && prev.tag == PosTag::Portions
                    && prev.start_line == token.start_line
                {
                    cr_tokens.push(prev);
                }
                cr_tokens.push(token);
                cr_tokens.extend(extra_copy_tokens);
                let name_leaves = detector::token_utils::collect_filtered_leaves(
                    next,
                    detector::NON_COPYRIGHT_LABELS,
                    detector::NON_COPYRIGHT_POS_TAGS,
                );
                let name_leaves = detector::token_utils::strip_all_rights_reserved(name_leaves);
                cr_tokens.extend(&name_leaves);
                let allow_single_word_contributors = cr_tokens
                    .iter()
                    .any(|t| detector::token_utils::is_year_like_token(t));
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
                i = name_idx + 1;
                continue;
            }
        } else if let Some((det, skip)) = author::try_extract_orphaned_by_author(tree, i) {
            authors.push(det);
            i += skip;
        } else if let Some((det, skip)) = author::try_extract_date_by_author(tree, i) {
            authors.push(det);
            i += skip;
        } else if !group_has_copyright
            && let Some((det, skip)) = author::try_extract_by_name_email_author(tree, i)
        {
            authors.push(det);
            i += skip;
        }
        i += 1;
    }

    (copyrights, holders, authors)
}
