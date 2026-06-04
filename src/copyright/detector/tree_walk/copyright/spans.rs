// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Flat-leaf span extraction over the whole token stream.
//!
//! These passes ignore parse-tree grouping and instead scan the flattened leaf
//! sequence for copyright (`Copyright`/`SPDX-Contributor`) and author spans.
//! [`extract_from_spans`] yields copyrights, holders, and authors;
//! [`extract_copyrights_from_spans`] yields only copyrights and holders.

use crate::copyright::detector;
use crate::copyright::types::{
    AuthorDetection, CopyrightDetection, HolderDetection, ParseNode, PosTag, Token,
};

pub fn extract_from_spans(
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

    let all_leaves: Vec<&Token> = tree.iter().flat_map(detector::collect_all_leaves).collect();

    if all_leaves.is_empty() {
        return (copyrights, holders, authors);
    }

    let mut i = 0;
    while i < all_leaves.len() {
        let token = all_leaves[i];

        let has_line_start_copyright_prefix =
            if token.tag == PosTag::Copy && token.value.eq_ignore_ascii_case("(c)") {
                let line = token.start_line;
                let mut found_copyright = false;
                for j in (0..i).rev() {
                    let t = all_leaves[j];
                    if t.start_line != line {
                        continue;
                    }
                    if !found_copyright {
                        if t.tag == PosTag::Copy && t.value.eq_ignore_ascii_case("copyright") {
                            found_copyright = true;
                            continue;
                        }
                        found_copyright = false;
                        break;
                    }
                    found_copyright = false;
                    break;
                }
                found_copyright
            } else {
                false
            };

        if token.tag == PosTag::Copy || token.tag == PosTag::SpdxContrib {
            if token.tag == PosTag::Copy
                && token.value.eq_ignore_ascii_case("(c)")
                && i > 0
                && all_leaves[i - 1].tag == PosTag::Portions
            {
                i += 1;
                continue;
            }

            if has_line_start_copyright_prefix {
                i += 1;
                continue;
            }
            let mut start = i;

            if token.tag == PosTag::Copy
                && token.value.eq_ignore_ascii_case("copyright")
                && start > 0
                && all_leaves[start - 1].tag == PosTag::Portions
                && all_leaves[start - 1].start_line == token.start_line
            {
                start -= 1;
            }

            if token.tag == PosTag::Copy
                && token.value.eq_ignore_ascii_case("(c)")
                && start > 0
                && all_leaves[start - 1].tag == PosTag::Copy
                && all_leaves[start - 1]
                    .value
                    .eq_ignore_ascii_case("copyright")
                && all_leaves[start - 1].start_line == token.start_line
                && start > 1
                && all_leaves[start - 2].tag == PosTag::Portions
                && all_leaves[start - 2].start_line == token.start_line
            {
                start -= 2;
            }

            if allow_not_copyrighted_prefix && start > 0 {
                let prev = all_leaves[start - 1];
                if prev.start_line == token.start_line && prev.value.eq_ignore_ascii_case("not") {
                    start -= 1;
                }
            }

            let copy_start = start;
            let copy_idx = i;
            i += 1;
            let mut allow_merge_following_copyright_clause = true;
            while i < all_leaves.len()
                && detector::token_utils::is_copyright_span_token(all_leaves[i])
            {
                if all_leaves[i].tag == PosTag::Copy && i > start + 1 {
                    if allow_merge_following_copyright_clause
                        && detector::token_utils::should_merge_following_copyright_clause(
                            &all_leaves,
                            start,
                            i,
                        )
                    {
                        allow_merge_following_copyright_clause = false;
                        i += 1;
                        continue;
                    }
                    if detector::token_utils::should_merge_following_c_sign_after_year(
                        &all_leaves,
                        start,
                        i,
                    ) {
                        i += 1;
                        continue;
                    }
                    break;
                }
                i += 1;
            }

            let mut skip_holder_from_span = false;

            if token.tag == PosTag::Copy
                && token.value.eq_ignore_ascii_case("(c)")
                && copy_start == copy_idx
                && all_leaves[copy_idx..i]
                    .iter()
                    .any(|t| detector::token_utils::is_year_like_token(t))
                && !all_leaves[copy_idx..i].iter().any(|t| {
                    matches!(
                        t.tag,
                        PosTag::Nnp
                            | PosTag::Nn
                            | PosTag::Caps
                            | PosTag::Pn
                            | PosTag::MixedCap
                            | PosTag::Comp
                            | PosTag::Uni
                    )
                })
            {
                let line = token.start_line;
                let has_holderish_before = all_leaves[..copy_idx]
                    .iter()
                    .rev()
                    .take_while(|t| t.start_line == line)
                    .any(|t| {
                        matches!(
                            t.tag,
                            PosTag::Nnp
                                | PosTag::Nn
                                | PosTag::Caps
                                | PosTag::Pn
                                | PosTag::MixedCap
                                | PosTag::Comp
                                | PosTag::Uni
                        )
                    });
                if has_holderish_before {
                    while start > 0
                        && all_leaves[start - 1].start_line == line
                        && detector::token_utils::is_copyright_span_token(all_leaves[start - 1])
                    {
                        start -= 1;
                    }
                    skip_holder_from_span = start < copy_start;
                }
            }

            let span = &all_leaves[start..i];
            if span.len() > 1 {
                let allow_single_word_contributors = span
                    .iter()
                    .any(|t| detector::token_utils::is_year_like_token(t));
                let filtered = detector::token_utils::strip_all_rights_reserved_slice(span);
                if let Some(det) = detector::token_utils::build_copyright_from_tokens(&filtered) {
                    copyrights.push(det);
                }

                if detector::token_utils::is_copyright_of_header(span) {
                    continue;
                }

                if !skip_holder_from_span {
                    let holder_span = filtered.as_slice();
                    let holder_tokens: Vec<&Token> = holder_span
                        .iter()
                        .copied()
                        .filter(|t| !detector::NON_HOLDER_POS_TAGS.contains(&t.tag))
                        .collect();
                    if let Some(det) = detector::token_utils::build_holder_from_tokens(
                        &holder_tokens,
                        allow_single_word_contributors,
                    ) {
                        holders.push(det);
                    } else {
                        let holder_tokens_mini: Vec<&Token> = holder_span
                            .iter()
                            .copied()
                            .filter(|t| !detector::NON_HOLDER_POS_TAGS_MINI.contains(&t.tag))
                            .collect();
                        if let Some(det) = detector::token_utils::build_holder_from_tokens(
                            &holder_tokens_mini,
                            allow_single_word_contributors,
                        ) {
                            holders.push(det);
                        }
                    }
                }
            }
        } else if matches!(
            token.tag,
            PosTag::Auth
                | PosTag::Auths
                | PosTag::AuthDot
                | PosTag::Contributors
                | PosTag::Commit
                | PosTag::SpdxContrib
        ) {
            let start = i;
            let start_line = token.start_line;
            i += 1;
            if i < all_leaves.len() && all_leaves[i].tag == PosTag::Of {
                continue;
            }
            while i < all_leaves.len() && detector::token_utils::is_author_span_token(all_leaves[i])
            {
                let t = all_leaves[i];
                if t.start_line != start_line {
                    let v = t
                        .value
                        .trim_matches(|c: char| c.is_ascii_punctuation())
                        .to_ascii_lowercase();
                    if matches!(v.as_str(), "date" | "purpose" | "description") {
                        break;
                    }
                    if matches!(
                        t.tag,
                        PosTag::Auth
                            | PosTag::Auths
                            | PosTag::AuthDot
                            | PosTag::Contributors
                            | PosTag::Commit
                            | PosTag::SpdxContrib
                    ) {
                        break;
                    }
                }
                i += 1;
            }

            let span = &all_leaves[start..i];
            if span.len() > 1 {
                let author_tokens: Vec<&Token> = span
                    .iter()
                    .copied()
                    .filter(|t| !detector::NON_AUTHOR_POS_TAGS.contains(&t.tag))
                    .collect();
                if let Some(det) = detector::token_utils::build_author_from_tokens(&author_tokens)
                    && !detector::token_utils::looks_like_bad_generic_author_candidate(&det.author)
                {
                    authors.push(det);
                }
            }
        } else {
            i += 1;
        }
    }

    (copyrights, holders, authors)
}

pub fn extract_copyrights_from_spans(
    tree: &[ParseNode],
    allow_not_copyrighted_prefix: bool,
) -> (Vec<CopyrightDetection>, Vec<HolderDetection>) {
    let mut copyrights: Vec<CopyrightDetection> = Vec::new();
    let mut holders: Vec<HolderDetection> = Vec::new();

    let all_leaves: Vec<&Token> = tree.iter().flat_map(detector::collect_all_leaves).collect();
    if all_leaves.is_empty() {
        return (copyrights, holders);
    }

    let mut i = 0;
    while i < all_leaves.len() {
        let token = all_leaves[i];

        if token.tag == PosTag::Copy || token.tag == PosTag::SpdxContrib {
            if token.tag == PosTag::Copy
                && token.value.eq_ignore_ascii_case("(c)")
                && i > 0
                && all_leaves[i - 1].tag == PosTag::Portions
            {
                i += 1;
                continue;
            }

            let mut start = i;

            if token.tag == PosTag::Copy
                && token.value.eq_ignore_ascii_case("copyright")
                && start > 0
                && all_leaves[start - 1].tag == PosTag::Portions
                && all_leaves[start - 1].start_line == token.start_line
            {
                start -= 1;
            }

            if token.tag == PosTag::Copy
                && token.value.eq_ignore_ascii_case("(c)")
                && start > 0
                && all_leaves[start - 1].tag == PosTag::Copy
                && all_leaves[start - 1]
                    .value
                    .eq_ignore_ascii_case("copyright")
                && all_leaves[start - 1].start_line == token.start_line
            {
                start -= 1;

                if start > 0
                    && all_leaves[start - 1].tag == PosTag::Portions
                    && all_leaves[start - 1].start_line == token.start_line
                {
                    start -= 1;
                }
            }

            if allow_not_copyrighted_prefix && start > 0 {
                let prev = all_leaves[start - 1];
                if prev.start_line == token.start_line && prev.value.eq_ignore_ascii_case("not") {
                    start -= 1;
                }
            }

            let copy_start = start;
            let copy_idx = i;
            i += 1;
            let mut allow_merge_following_copyright_clause = true;
            while i < all_leaves.len()
                && detector::token_utils::is_copyright_span_token(all_leaves[i])
            {
                if all_leaves[i].tag == PosTag::Copy && i > start + 1 {
                    if allow_merge_following_copyright_clause
                        && detector::token_utils::should_merge_following_copyright_clause(
                            &all_leaves,
                            start,
                            i,
                        )
                    {
                        allow_merge_following_copyright_clause = false;
                        i += 1;
                        continue;
                    }
                    if detector::token_utils::should_merge_following_c_sign_after_year(
                        &all_leaves,
                        start,
                        i,
                    ) {
                        i += 1;
                        continue;
                    }
                    break;
                }
                i += 1;
            }

            let mut skip_holder_from_span = false;

            if token.tag == PosTag::Copy
                && token.value.eq_ignore_ascii_case("(c)")
                && copy_start == copy_idx
                && all_leaves[copy_idx..i]
                    .iter()
                    .any(|t| detector::token_utils::is_year_like_token(t))
                && !all_leaves[copy_idx..i].iter().any(|t| {
                    matches!(
                        t.tag,
                        PosTag::Nnp
                            | PosTag::Nn
                            | PosTag::Caps
                            | PosTag::Pn
                            | PosTag::MixedCap
                            | PosTag::Comp
                            | PosTag::Uni
                    )
                })
            {
                let line = token.start_line;
                let has_holderish_before = all_leaves[..copy_idx]
                    .iter()
                    .rev()
                    .take_while(|t| t.start_line == line)
                    .any(|t| {
                        matches!(
                            t.tag,
                            PosTag::Nnp
                                | PosTag::Nn
                                | PosTag::Caps
                                | PosTag::Pn
                                | PosTag::MixedCap
                                | PosTag::Comp
                                | PosTag::Uni
                        )
                    });
                if has_holderish_before {
                    while start > 0
                        && all_leaves[start - 1].start_line == line
                        && detector::token_utils::is_copyright_span_token(all_leaves[start - 1])
                    {
                        start -= 1;
                    }
                    skip_holder_from_span = start < copy_start;
                }
            }

            let span = &all_leaves[start..i];
            if span.len() > 1 {
                let allow_single_word_contributors = span
                    .iter()
                    .any(|t| detector::token_utils::is_year_like_token(t));

                let filtered = detector::token_utils::strip_all_rights_reserved_slice(span);
                if let Some(det) = detector::token_utils::build_copyright_from_tokens(&filtered) {
                    copyrights.push(det);
                }

                if detector::token_utils::is_copyright_of_header(span) {
                    continue;
                }

                if !skip_holder_from_span {
                    let holder_span = filtered.as_slice();
                    let holder_tokens: Vec<&Token> = holder_span
                        .iter()
                        .copied()
                        .filter(|t| !detector::NON_HOLDER_POS_TAGS.contains(&t.tag))
                        .collect();
                    if let Some(det) = detector::token_utils::build_holder_from_tokens(
                        &holder_tokens,
                        allow_single_word_contributors,
                    ) {
                        holders.push(det);
                    } else {
                        let holder_tokens_mini: Vec<&Token> = holder_span
                            .iter()
                            .copied()
                            .filter(|t| !detector::NON_HOLDER_POS_TAGS_MINI.contains(&t.tag))
                            .collect();
                        if let Some(det) = detector::token_utils::build_holder_from_tokens(
                            &holder_tokens_mini,
                            allow_single_word_contributors,
                        ) {
                            holders.push(det);
                        }
                    }
                }
            }
        } else {
            i += 1;
        }
    }

    (copyrights, holders)
}
