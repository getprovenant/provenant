// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Copyright detection orchestrator.
//!
//! Runs the full detection pipeline: text → numbered lines → candidate groups
//! → tokens → parse tree → walk tree → refine → filter junk → detections.
//!
//! The grammar currently builds lower-level structures (Name, Company,
//! YrRange, etc.) but does not yet produce top-level COPYRIGHT/AUTHOR tree
//! nodes. This detector handles both cases:
//! - If the grammar produces COPYRIGHT/AUTHOR nodes, use them directly.
//! - Otherwise, scan the flat node sequence for COPY/AUTH tokens and
//!   collect spans heuristically.

use std::borrow::Cow;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use regex::Regex;

use super::candidates::collect_candidate_lines;
use super::detector_input_normalization::{
    maybe_expand_copyrighted_by_href_urls, normalize_split_input,
};
use super::lexer::get_tokens;
use super::line_tracking::{LineNumberIndex, PreparedLineCache};
use super::parser::{parse, parse_with_deadline};
use super::refiner::looks_like_bpe_merges_table;
#[cfg(test)]
use super::refiner::refine_copyright;
use super::types::{
    AuthorDetection, CopyrightDetection, HolderDetection, ParseNode, PosTag, TreeLabel,
};

const NON_COPYRIGHT_LABELS: &[TreeLabel] = &[];
const NON_HOLDER_LABELS: &[TreeLabel] = &[TreeLabel::YrRange, TreeLabel::YrAnd];
const NON_HOLDER_LABELS_MINI: &[TreeLabel] = &[TreeLabel::YrRange, TreeLabel::YrAnd];

const NON_HOLDER_POS_TAGS: &[PosTag] = &[
    PosTag::Copy,
    PosTag::Yr,
    PosTag::YrPlus,
    PosTag::BareYr,
    PosTag::Email,
    PosTag::Url,
    PosTag::Holder,
    PosTag::Is,
    PosTag::Held,
];

const NON_HOLDER_POS_TAGS_MINI: &[PosTag] = &[
    PosTag::Copy,
    PosTag::Yr,
    PosTag::YrPlus,
    PosTag::BareYr,
    PosTag::Is,
    PosTag::Held,
];

const NON_AUTHOR_POS_TAGS: &[PosTag] = &[
    PosTag::Copy,
    PosTag::Yr,
    PosTag::YrPlus,
    PosTag::BareYr,
    PosTag::Auth,
    PosTag::Auth2,
    PosTag::Auths,
    PosTag::AuthDot,
    PosTag::Contributors,
    PosTag::Commit,
    PosTag::SpdxContrib,
    PosTag::Holder,
    PosTag::Is,
    PosTag::Held,
];

const NON_COPYRIGHT_POS_TAGS: &[PosTag] = &[];

static NOT_COPYRIGHTED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bnot\s+copyrighted\b").unwrap());

#[derive(Default)]
struct TreeAnalysis {
    has_copy_like_token: bool,
    has_authorish_boundary_token: bool,
    has_year_token: bool,
    single_line: Option<usize>,
    is_single_line_group: bool,
}

fn analyze_tree(nodes: &[ParseNode]) -> TreeAnalysis {
    fn visit(node: &ParseNode, analysis: &mut TreeAnalysis) {
        match node {
            ParseNode::Leaf(token) => {
                analysis.has_copy_like_token |=
                    matches!(token.tag, PosTag::Copy | PosTag::SpdxContrib);
                analysis.has_authorish_boundary_token |= matches!(
                    token.tag,
                    PosTag::Auths | PosTag::AuthDot | PosTag::Contributors | PosTag::Commit
                );
                analysis.has_year_token |= token_utils::is_year_like_token(token);

                let line = token.start_line.get();
                match analysis.single_line {
                    Some(existing) if existing != line => analysis.is_single_line_group = false,
                    None => analysis.single_line = Some(line),
                    _ => {}
                }
            }
            ParseNode::Tree { children, .. } => {
                for child in children {
                    visit(child, analysis);
                }
            }
        }
    }

    let mut analysis = TreeAnalysis {
        is_single_line_group: true,
        ..TreeAnalysis::default()
    };

    for node in nodes {
        visit(node, &mut analysis);
    }

    analysis
}

/// Returns a tuple of (copyrights, holders, authors).
pub fn detect_copyrights_from_text(
    content: &str,
) -> (
    Vec<CopyrightDetection>,
    Vec<HolderDetection>,
    Vec<AuthorDetection>,
) {
    detect_copyrights_from_text_with_deadline(content, None)
}

/// A ruler / separator line made entirely of repeated punctuation (`----`,
/// `====`, `****`) with no alphanumeric content.
fn is_ruler_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.chars().count() >= 3
        && !trimmed.chars().any(|c| c.is_alphanumeric())
        && trimmed
            .chars()
            .filter(|c| matches!(c, '-' | '=' | '_' | '*' | '~' | '#' | '+'))
            .count()
            >= 3
}

/// Whether a group's lines, detected on their own, yield a copyright that names
/// a holder. Runs the full detector on the reconstructed raw text rather than a
/// single extraction pass, so the verdict matches what the main loop produces (a
/// holder-less year-only result — as a junk line gives — returns false).
fn segment_yields_copyright_with_holder(segment: &[(usize, String)], raw_lines: &[&str]) -> bool {
    let text = segment
        .iter()
        .filter_map(|(ln, _)| raw_lines.get(*ln - 1).copied())
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        return false;
    }
    let (copyrights, holders, _) = detect_copyrights_from_text(&text);
    !copyrights.is_empty() && !holders.is_empty()
}

/// Split a candidate group at ruler lines, but only where the lines before a
/// ruler already form a copyright with a holder. This stops a complete notice
/// from bleeding across a divider into the following line, while leaving a junk
/// line (which yields only a bare year when isolated) merged with its neighbours.
fn split_groups_at_rulers(
    groups: Vec<Vec<(usize, String)>>,
    raw_lines: &[&str],
) -> Vec<Vec<(usize, String)>> {
    // The grouping look-ahead blanks a ruler's text but keeps its entry, so
    // match it by raw line number rather than the (empty) group text.
    let is_ruler_at = |ln: usize| {
        raw_lines
            .get(ln - 1)
            .is_some_and(|line| is_ruler_line(line))
    };
    let mut out: Vec<Vec<(usize, String)>> = Vec::with_capacity(groups.len());
    for group in groups {
        // Cut at every qualifying ruler, not just the first: a group may hold
        // more than one copyright/divider pair. A ruler whose preceding segment
        // is not a copyright-with-holder is left in place (the segment keeps
        // scanning), so junk lines stay merged with their neighbours.
        let mut segment_start = 0;
        for i in 0..group.len() {
            if i > segment_start
                && is_ruler_at(group[i].0)
                && segment_yields_copyright_with_holder(&group[segment_start..i], raw_lines)
            {
                out.push(group[segment_start..i].to_vec());
                segment_start = i + 1;
            }
        }
        if segment_start == 0 {
            out.push(group);
        } else if segment_start < group.len() {
            out.push(group[segment_start..].to_vec());
        }
    }
    out
}

pub fn detect_copyrights_from_text_with_deadline(
    content: &str,
    max_runtime: Option<Duration>,
) -> (
    Vec<CopyrightDetection>,
    Vec<HolderDetection>,
    Vec<AuthorDetection>,
) {
    let mut copyrights = Vec::new();
    let mut holders = Vec::new();
    let mut authors = Vec::new();
    let deadline = max_runtime.and_then(|d| Instant::now().checked_add(d));

    if content.is_empty() {
        return (copyrights, holders, authors);
    }

    // BPE tokenizer merge tables (e.g. `merges.txt`) embed the `©` symbol and
    // mojibake byte pairs as merge rules but carry no genuine copyright notice.
    if looks_like_bpe_merges_table(content) {
        return (copyrights, holders, authors);
    }

    let normalized = normalize_split_input(content);
    let expanded = maybe_expand_copyrighted_by_href_urls(normalized.as_ref());
    let did_expand_href = matches!(expanded, Cow::Owned(_));
    let content = expanded.as_ref();
    let line_number_index = LineNumberIndex::new(content);

    let allow_not_copyrighted_prefix = NOT_COPYRIGHTED_RE.find_iter(content).count() == 1;

    let raw_lines: Vec<&str> = content.lines().collect();
    let prepared_cache = PreparedLineCache::new(&raw_lines);

    if raw_lines.is_empty() {
        return (copyrights, holders, authors);
    }

    let groups =
        collect_candidate_lines(raw_lines.iter().enumerate().map(|(i, line)| (i + 1, *line)));
    let groups = split_groups_at_rulers(groups, &raw_lines);

    let mut seen = seen_text::SeenTextSets::from_existing(&copyrights, &holders, &authors);

    for group in &groups {
        if deadline_exceeded(deadline) {
            break;
        }

        if group.is_empty() {
            continue;
        }

        let tokens = get_tokens(group);
        if tokens.is_empty() {
            continue;
        }

        let tree = if deadline.is_some() {
            parse_with_deadline(tokens, deadline)
        } else {
            parse(tokens)
        };

        let has_top_level_nodes = tree.iter().any(|n| {
            matches!(
                n.label(),
                Some(TreeLabel::Copyright) | Some(TreeLabel::Copyright2) | Some(TreeLabel::Author)
            )
        });
        let analysis = analyze_tree(&tree);

        if has_top_level_nodes {
            let (new_c, new_h, new_a) =
                extract_from_tree_nodes(&tree, allow_not_copyrighted_prefix);
            seen.register_copyrights(&new_c);
            seen.register_holders(&new_h);
            seen.register_authors(&new_a);
            let copyrights_before = copyrights.len();
            copyrights.extend(new_c);
            holders.extend(new_h);
            authors.extend(new_a);

            if let Some(det) = extract_original_author_additional_contributors(&tree)
                && seen.authors.insert(det.author.clone())
            {
                authors.push(det);
            }

            if copyrights.len() == copyrights_before
                && analysis.has_copy_like_token
                && analysis.has_authorish_boundary_token
                && analysis.is_single_line_group
            {
                let (new_c, new_h) = extract_bare_copyrights(&tree);
                seen.register_copyrights(&new_c);
                seen.register_holders(&new_h);
                copyrights.extend(new_c);
                holders.extend(new_h);
                let (new_c, new_h) =
                    extract_copyrights_from_spans(&tree, allow_not_copyrighted_prefix);
                seen.register_copyrights(&new_c);
                seen.register_holders(&new_h);
                copyrights.extend(new_c);
                holders.extend(new_h);
            }

            if copyrights.len() == copyrights_before
                && analysis.has_copy_like_token
                && analysis.has_year_token
            {
                let (new_c, new_h) =
                    extract_copyrights_from_spans(&tree, allow_not_copyrighted_prefix);
                seen.register_copyrights(&new_c);
                seen.register_holders(&new_h);
                copyrights.extend(new_c);
                holders.extend(new_h);
            }
        } else {
            let (new_c, new_h) = extract_bare_copyrights(&tree);
            seen.register_copyrights(&new_c);
            seen.register_holders(&new_h);
            copyrights.extend(new_c);
            holders.extend(new_h);
            let (new_c, new_h, new_a) = extract_from_spans(&tree, allow_not_copyrighted_prefix);
            seen.register_copyrights(&new_c);
            seen.register_holders(&new_h);
            seen.register_authors(&new_a);
            copyrights.extend(new_c);
            holders.extend(new_h);
            authors.extend(new_a);
            let mut new_a = extract_orphaned_by_authors(&tree);
            seen.dedup_new_authors(&mut new_a, 0);
            authors.extend(new_a);

            if let Some(det) = extract_original_author_additional_contributors(&tree)
                && seen.authors.insert(det.author.clone())
            {
                authors.push(det);
            }
        }

        // Run after each group is processed so it can fix authors detected
        // through any extraction path.
        fix_truncated_contributors_authors(&tree, &mut authors);
        seen.rebuild_authors_from(&authors);
        let (mut new_c, mut new_h) = extract_holder_is_name(&tree);
        seen.dedup_new_copyrights(&mut new_c, 0);
        seen.dedup_new_holders(&mut new_h, 0);
        copyrights.extend(new_c);
        holders.extend(new_h);
        apply_written_by_for_markers(group, &mut copyrights, &mut holders);
        extend_multiline_copyright_c_year_holder_continuations(
            group,
            &mut copyrights,
            &mut holders,
        );
        extend_multiline_copyright_c_no_year_names(group, &mut copyrights[..], &mut holders[..]);
        extend_authors_see_url_copyrights(group, &mut copyrights[..], &mut holders[..]);
        extend_leading_dash_suffixes(group, &mut copyrights[..], &mut holders[..]);
        extend_dash_obfuscated_email_suffixes(&raw_lines, group, &mut copyrights[..], &holders[..]);
        extend_trailing_copy_year_suffixes(&raw_lines, group, &mut copyrights[..]);
        extend_trailing_lowercase_holder_suffixes(
            &raw_lines,
            group,
            &mut copyrights[..],
            &mut holders,
        );
        extend_w3c_registered_org_list_suffixes(group, &mut copyrights[..], &mut holders[..]);
        extend_software_in_the_public_interest_holder(group, &mut copyrights, &mut holders);
    }

    let mut fallback = fallback_year_only_copyrights(&groups);
    seen.dedup_new_copyrights(&mut fallback, 0);
    fallback.retain(|det| {
        !copyrights.iter().any(|c| {
            c.copyright
                .to_ascii_lowercase()
                .contains(&det.copyright.to_ascii_lowercase())
        })
    });
    copyrights.extend(fallback);

    if deadline_exceeded(deadline) {
        refine_final_copyrights(&mut copyrights);
        postprocess_transforms::refine_final_authors(&mut authors);
        dedupe_exact_span_copyrights(&mut copyrights);
        dedupe_exact_span_holders(&mut holders);
        dedupe_exact_span_authors(&mut authors);
        return (copyrights, holders, authors);
    }

    let prepared_lines = prepared_cache.materialize();

    phases::run_phase_primary_extractions(
        content,
        &groups,
        &line_number_index,
        &prepared_lines,
        &mut copyrights,
        &mut holders,
        &mut seen,
    );

    phases::run_phase_postprocess(
        content,
        &raw_lines,
        &prepared_lines,
        did_expand_href,
        &mut copyrights,
        &mut holders,
        &mut authors,
        &mut seen,
    );

    refine_final_copyrights(&mut copyrights);
    postprocess_transforms::refine_final_authors(&mut authors);
    postprocess_transforms::drop_trademark_boilerplate_multiline_extensions(
        &raw_lines,
        &mut copyrights,
        &mut holders,
    );
    postprocess_transforms::drop_same_span_license_tail_variants(&mut copyrights, &mut holders);
    postprocess_transforms::drop_shadowed_bare_c_from_year_fragments(&mut copyrights, &mut holders);
    drop_path_fragment_holders_from_bare_c_code_lines(&raw_lines, &copyrights, &mut holders);
    drop_scan_only_holders_from_copyright_scan_lines(&raw_lines, &copyrights, &mut holders);
    drop_test_label_false_positive_copyrights_and_holders(
        &raw_lines,
        &mut copyrights,
        &mut holders,
    );
    drop_tokenizer_data_fragment_detections(
        &raw_lines,
        &mut copyrights,
        &mut holders,
        &mut authors,
    );

    for group in &groups {
        extend_dash_obfuscated_email_suffixes(&raw_lines, group, &mut copyrights[..], &holders[..]);
    }
    restore_linux_foundation_copyrights_from_raw_lines(&raw_lines, &mut copyrights);
    copyrights.retain(|c| {
        let trimmed = c.copyright.trim();
        !trimmed.eq_ignore_ascii_case("copyright holders")
            && !trimmed.eq_ignore_ascii_case("the above copyright holders")
    });

    holders.extend(add_missing_holders_for_bare_c_name_year_suffixes(
        &copyrights,
    ));
    holders
        .extend(postprocess_transforms::add_missing_holders_for_iso_date_copyrights(&copyrights));

    dedupe_exact_span_holders(&mut holders);

    dedupe_exact_span_copyrights(&mut copyrights);
    dedupe_exact_span_holders(&mut holders);
    dedupe_exact_span_authors(&mut authors);

    (copyrights, holders, authors)
}

#[cfg(test)]
#[derive(Debug)]
pub(super) struct PhaseBoundaryDetections {
    pub(super) after_group_loop_copyrights: Vec<CopyrightDetection>,
    pub(super) after_group_loop_holders: Vec<HolderDetection>,
    pub(super) after_primary_copyrights: Vec<CopyrightDetection>,
    pub(super) after_primary_holders: Vec<HolderDetection>,
    pub(super) after_postprocess_copyrights: Vec<CopyrightDetection>,
    pub(super) after_postprocess_holders: Vec<HolderDetection>,
}

#[cfg(test)]
pub(super) fn detect_copyright_phase_boundaries(content: &str) -> PhaseBoundaryDetections {
    let mut copyrights = Vec::new();
    let mut holders = Vec::new();
    let mut authors = Vec::new();

    if content.is_empty() {
        return PhaseBoundaryDetections {
            after_group_loop_copyrights: Vec::new(),
            after_group_loop_holders: Vec::new(),
            after_primary_copyrights: Vec::new(),
            after_primary_holders: Vec::new(),
            after_postprocess_copyrights: Vec::new(),
            after_postprocess_holders: Vec::new(),
        };
    }

    let normalized = normalize_split_input(content);
    let expanded = maybe_expand_copyrighted_by_href_urls(normalized.as_ref());
    let did_expand_href = matches!(expanded, Cow::Owned(_));
    let content = expanded.as_ref();
    let line_number_index = LineNumberIndex::new(content);

    let allow_not_copyrighted_prefix = NOT_COPYRIGHTED_RE.find_iter(content).count() == 1;
    let raw_lines: Vec<&str> = content.lines().collect();
    let prepared_cache = PreparedLineCache::new(&raw_lines);

    let groups =
        collect_candidate_lines(raw_lines.iter().enumerate().map(|(i, line)| (i + 1, *line)));
    let mut seen = seen_text::SeenTextSets::from_existing(&copyrights, &holders, &authors);

    for group in &groups {
        if group.is_empty() {
            continue;
        }

        let tokens = get_tokens(group);
        if tokens.is_empty() {
            continue;
        }

        let tree = parse(tokens);
        let has_top_level_nodes = tree.iter().any(|n| {
            matches!(
                n.label(),
                Some(TreeLabel::Copyright) | Some(TreeLabel::Copyright2) | Some(TreeLabel::Author)
            )
        });
        let analysis = analyze_tree(&tree);

        if has_top_level_nodes {
            let (new_c, new_h, new_a) =
                extract_from_tree_nodes(&tree, allow_not_copyrighted_prefix);
            seen.register_copyrights(&new_c);
            seen.register_holders(&new_h);
            seen.register_authors(&new_a);
            let copyrights_before = copyrights.len();
            copyrights.extend(new_c);
            holders.extend(new_h);
            authors.extend(new_a);

            if let Some(det) = extract_original_author_additional_contributors(&tree)
                && seen.authors.insert(det.author.clone())
            {
                authors.push(det);
            }

            if copyrights.len() == copyrights_before
                && analysis.has_copy_like_token
                && analysis.has_authorish_boundary_token
                && analysis.is_single_line_group
            {
                let (new_c, new_h) = extract_bare_copyrights(&tree);
                seen.register_copyrights(&new_c);
                seen.register_holders(&new_h);
                copyrights.extend(new_c);
                holders.extend(new_h);
                let (new_c, new_h) =
                    extract_copyrights_from_spans(&tree, allow_not_copyrighted_prefix);
                seen.register_copyrights(&new_c);
                seen.register_holders(&new_h);
                copyrights.extend(new_c);
                holders.extend(new_h);
            }

            if copyrights.len() == copyrights_before
                && analysis.has_copy_like_token
                && analysis.has_year_token
            {
                let (new_c, new_h) =
                    extract_copyrights_from_spans(&tree, allow_not_copyrighted_prefix);
                seen.register_copyrights(&new_c);
                seen.register_holders(&new_h);
                copyrights.extend(new_c);
                holders.extend(new_h);
            }
        } else {
            let (new_c, new_h) = extract_bare_copyrights(&tree);
            seen.register_copyrights(&new_c);
            seen.register_holders(&new_h);
            copyrights.extend(new_c);
            holders.extend(new_h);
            let (new_c, new_h, new_a) = extract_from_spans(&tree, allow_not_copyrighted_prefix);
            seen.register_copyrights(&new_c);
            seen.register_holders(&new_h);
            seen.register_authors(&new_a);
            copyrights.extend(new_c);
            holders.extend(new_h);
            authors.extend(new_a);
            let mut new_a = extract_orphaned_by_authors(&tree);
            seen.dedup_new_authors(&mut new_a, 0);
            authors.extend(new_a);

            if let Some(det) = extract_original_author_additional_contributors(&tree)
                && seen.authors.insert(det.author.clone())
            {
                authors.push(det);
            }
        }

        fix_truncated_contributors_authors(&tree, &mut authors);
        seen.rebuild_authors_from(&authors);
        let (mut new_c, mut new_h) = extract_holder_is_name(&tree);
        seen.dedup_new_copyrights(&mut new_c, 0);
        seen.dedup_new_holders(&mut new_h, 0);
        copyrights.extend(new_c);
        holders.extend(new_h);
        apply_written_by_for_markers(group, &mut copyrights, &mut holders);
        extend_multiline_copyright_c_year_holder_continuations(
            group,
            &mut copyrights,
            &mut holders,
        );
        extend_multiline_copyright_c_no_year_names(group, &mut copyrights[..], &mut holders[..]);
        extend_authors_see_url_copyrights(group, &mut copyrights[..], &mut holders[..]);
        extend_leading_dash_suffixes(group, &mut copyrights[..], &mut holders[..]);
        extend_dash_obfuscated_email_suffixes(&raw_lines, group, &mut copyrights[..], &holders[..]);
        extend_trailing_copy_year_suffixes(&raw_lines, group, &mut copyrights[..]);
        extend_trailing_lowercase_holder_suffixes(
            &raw_lines,
            group,
            &mut copyrights[..],
            &mut holders,
        );
        extend_w3c_registered_org_list_suffixes(group, &mut copyrights[..], &mut holders[..]);
        extend_software_in_the_public_interest_holder(group, &mut copyrights, &mut holders);
    }

    let mut fallback = fallback_year_only_copyrights(&groups);
    seen.dedup_new_copyrights(&mut fallback, 0);
    fallback.retain(|det| {
        !copyrights.iter().any(|c| {
            c.copyright
                .to_ascii_lowercase()
                .contains(&det.copyright.to_ascii_lowercase())
        })
    });
    copyrights.extend(fallback);

    let after_group_loop_copyrights = copyrights.clone();
    let after_group_loop_holders = holders.clone();

    let prepared_lines = prepared_cache.materialize();

    phases::run_phase_primary_extractions(
        content,
        &groups,
        &line_number_index,
        &prepared_lines,
        &mut copyrights,
        &mut holders,
        &mut seen,
    );

    let after_primary_copyrights = copyrights.clone();
    let after_primary_holders = holders.clone();

    phases::run_phase_postprocess(
        content,
        &raw_lines,
        &prepared_lines,
        did_expand_href,
        &mut copyrights,
        &mut holders,
        &mut authors,
        &mut seen,
    );

    PhaseBoundaryDetections {
        after_group_loop_copyrights,
        after_group_loop_holders,
        after_primary_copyrights,
        after_primary_holders,
        after_postprocess_copyrights: copyrights,
        after_postprocess_holders: holders,
    }
}

mod author_heuristics;
mod pattern_extract;
mod phases;
mod postprocess_transforms;
mod seen_text;
mod token_utils;
mod tree_walk;

use pattern_extract::{
    extend_software_in_the_public_interest_holder, fallback_year_only_copyrights,
};
use postprocess_transforms::{
    add_missing_holders_for_bare_c_name_year_suffixes, deadline_exceeded,
    dedupe_exact_span_authors, dedupe_exact_span_copyrights, dedupe_exact_span_holders,
    extend_authors_see_url_copyrights, extend_dash_obfuscated_email_suffixes,
    extend_leading_dash_suffixes, extend_multiline_copyright_c_no_year_names,
    extend_multiline_copyright_c_year_holder_continuations, extend_trailing_copy_year_suffixes,
    extend_trailing_lowercase_holder_suffixes, extend_w3c_registered_org_list_suffixes,
    refine_final_copyrights, restore_linux_foundation_copyrights_from_raw_lines,
};
pub(super) use token_utils::collect_all_leaves;
use token_utils::{
    apply_written_by_for_markers, drop_path_fragment_holders_from_bare_c_code_lines,
    drop_scan_only_holders_from_copyright_scan_lines,
    drop_test_label_false_positive_copyrights_and_holders, drop_tokenizer_data_fragment_detections,
    extract_original_author_additional_contributors,
};
use tree_walk::{
    extract_bare_copyrights, extract_copyrights_from_spans, extract_from_spans,
    extract_from_tree_nodes, extract_holder_is_name, extract_orphaned_by_authors,
    fix_truncated_contributors_authors,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "tests_false_positives.rs"]
mod tests_false_positives;

#[cfg(test)]
#[path = "tests_structured_metadata.rs"]
mod tests_structured_metadata;

#[cfg(test)]
#[path = "tests_author_pipeline.rs"]
mod tests_author_pipeline;

#[cfg(test)]
#[path = "tests_multiline_repairs.rs"]
mod tests_multiline_repairs;

#[cfg(test)]
#[path = "tests_parser_internals.rs"]
mod tests_parser_internals;

#[cfg(test)]
#[path = "tests_copyright_holder_pipeline.rs"]
mod tests_copyright_holder_pipeline;
