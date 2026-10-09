// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Test-only diagnostics for attributing group-loop detections to an extractor.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectionOrigin {
    ParsedTree,
    BareTreeFallback,
    SpanFallback,
}

#[derive(Debug)]
struct OriginDetections {
    origin: DetectionOrigin,
    copyrights: Vec<CopyrightDetection>,
    holders: Vec<HolderDetection>,
    authors: Vec<AuthorDetection>,
}

#[derive(Debug)]
struct CandidateGroupTrace {
    lines: Vec<usize>,
    has_top_level_nodes: bool,
    tokens: Vec<(String, PosTag)>,
    detections: Vec<OriginDetections>,
}

/// Attribute the three grammar-adjacent extraction routes without changing the
/// production result types or serialized scan output.
///
/// Each fallback is evaluated independently. This deliberately answers which
/// route *can* manufacture a candidate; it is not a replay of later deduplication
/// or post-processing.
fn trace_candidate_groups(content: &str) -> Vec<CandidateGroupTrace> {
    let normalized = normalize_split_input(content);
    let expanded = maybe_expand_copyrighted_by_href_urls(normalized.as_ref());
    let content = expanded.as_ref();
    let allow_not_copyrighted_prefix = NOT_COPYRIGHTED_RE.find_iter(content).count() == 1;
    let raw_lines: Vec<&str> = content.lines().collect();
    let groups = collect_candidate_lines(
        raw_lines
            .iter()
            .enumerate()
            .map(|(index, line)| (index + 1, *line)),
    );
    let groups = split_groups_at_rulers(groups, &raw_lines, None);

    groups
        .into_iter()
        .filter_map(|group| {
            let tokens = get_tokens(&group);
            if tokens.is_empty() {
                return None;
            }
            let token_summary = tokens
                .iter()
                .map(|token| (token.value.clone(), token.tag))
                .collect();
            let tree = parse(tokens);
            let has_top_level_nodes = tree.iter().any(|node| {
                matches!(
                    node.label(),
                    Some(TreeLabel::Copyright)
                        | Some(TreeLabel::Copyright2)
                        | Some(TreeLabel::Author)
                )
            });

            let (tree_copyrights, tree_holders, tree_authors) =
                extract_from_tree_nodes(&tree, allow_not_copyrighted_prefix);
            let (bare_copyrights, bare_holders) = extract_bare_copyrights(&tree);
            let (span_copyrights, span_holders, span_authors) =
                extract_from_spans(&tree, allow_not_copyrighted_prefix);

            Some(CandidateGroupTrace {
                lines: group.iter().map(|(line, _)| *line).collect(),
                has_top_level_nodes,
                tokens: token_summary,
                detections: vec![
                    OriginDetections {
                        origin: DetectionOrigin::ParsedTree,
                        copyrights: tree_copyrights,
                        holders: tree_holders,
                        authors: tree_authors,
                    },
                    OriginDetections {
                        origin: DetectionOrigin::BareTreeFallback,
                        copyrights: bare_copyrights,
                        holders: bare_holders,
                        authors: Vec::new(),
                    },
                    OriginDetections {
                        origin: DetectionOrigin::SpanFallback,
                        copyrights: span_copyrights,
                        holders: span_holders,
                        authors: span_authors,
                    },
                ],
            })
        })
        .collect()
}

#[test]
fn test_nameslist_rows_are_rejected_by_span_fallback() {
    let input = concat!(
        "00A8\tDIAERESIS\n",
        "\t* this is a spacing character\n",
        "\tx (combining diaeresis - 0308)\n",
        "\t# 0020 0308\n",
        "00A9\tCOPYRIGHT SIGN\n",
        "\tx (sound recording copyright - 2117)\n",
        "\tx (circled latin capital letter c - 24B8)\n",
        "\tx (copyleft symbol - 1F12F)\n",
        "\tx (mask work symbol - 1F1AD)\n",
        "00AA\tFEMININE ORDINAL INDICATOR\n",
    );

    let traces = trace_candidate_groups(input);
    let trace = traces
        .iter()
        .find(|trace| trace.lines.contains(&6))
        .expect("candidate group containing the copyright cross-reference");

    assert!(!trace.has_top_level_nodes, "trace: {trace:#?}");
    let parsed = trace
        .detections
        .iter()
        .find(|detections| detections.origin == DetectionOrigin::ParsedTree)
        .expect("parsed-tree origin");
    assert!(parsed.copyrights.is_empty(), "trace: {trace:#?}");

    let span = trace
        .detections
        .iter()
        .find(|detections| detections.origin == DetectionOrigin::SpanFallback)
        .expect("span-fallback origin");
    assert!(span.copyrights.is_empty(), "trace: {trace:#?}");
    assert!(span.holders.is_empty(), "trace: {trace:#?}");
    assert!(span.authors.is_empty(), "trace: {trace:#?}");
}

#[test]
fn test_real_notice_records_positive_grammar_or_fallback_evidence() {
    let traces = trace_candidate_groups("Copyright (c) 2024 Acme Research, Inc.");
    let trace = traces.first().expect("candidate group");

    assert!(
        trace
            .tokens
            .iter()
            .any(|(_, tag)| matches!(tag, PosTag::Yr | PosTag::YrPlus | PosTag::BareYr)),
        "trace: {trace:#?}"
    );
    assert!(
        trace
            .detections
            .iter()
            .any(|detections| !detections.copyrights.is_empty()),
        "trace: {trace:#?}"
    );
}
