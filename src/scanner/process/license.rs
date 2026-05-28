// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::license_detection::LicenseDetection as InternalLicenseDetection;
use crate::license_detection::LicenseDetectionEngine;
use crate::license_detection::LicenseDetectionError;
use crate::license_detection::PositionSet;
use crate::license_detection::expression::parse_expression;
use crate::license_detection::index::LicenseIndex;
use crate::license_detection::models::LicenseMatch as InternalLicenseMatch;
use crate::license_detection::query::Query;
use crate::models::{
    FileInfoBuilder, LicenseDetection as PublicLicenseDetection, LineNumber, Match, ScanDiagnostic,
};
use crate::parsers::license_normalization::{
    DeclaredLicenseMatchMetadata, build_declared_license_data, normalize_spdx_expression,
};
use crate::scanner::LicenseScanOptions;
use crate::scanner::process::file_scan_error::FileScanError;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

const MAX_OUTPUT_MATCHED_TEXT_LINE_LENGTH: usize = 10_000;
const MAX_OUTPUT_MATCHED_TEXT_BYTES: usize = 128 * 1024;
const MATCHED_TEXT_TRUNCATION_MARKER: &str = "… [truncated]";

pub(super) struct LicenseExtractionInput<'a> {
    pub(super) path: &'a Path,
    pub(super) text_content: String,
    pub(super) license_engine: Option<Arc<LicenseDetectionEngine>>,
    pub(super) license_options: LicenseScanOptions,
    pub(super) from_binary_strings: bool,
    pub(super) timeout_seconds: f64,
    pub(super) deadline: Option<Instant>,
}

pub(super) fn extract_license_information(
    file_info_builder: &mut FileInfoBuilder,
    scan_diagnostics: &mut Vec<ScanDiagnostic>,
    input: LicenseExtractionInput<'_>,
) -> Result<(), FileScanError> {
    let LicenseExtractionInput {
        path,
        text_content,
        license_engine,
        license_options,
        from_binary_strings,
        timeout_seconds,
        deadline,
    } = input;

    let Some(engine) = license_engine else {
        return Ok(());
    };

    if deadline.is_some() {
        let detection_result = if license_options.min_score == 0 {
            engine.detect_with_kind_and_source_with_deadline(
                &text_content,
                license_options.unknown_licenses,
                from_binary_strings,
                &path.to_string_lossy(),
                deadline,
            )
        } else {
            engine.detect_with_kind_and_source_with_score_and_deadline(
                &text_content,
                license_options.unknown_licenses,
                from_binary_strings,
                &path.to_string_lossy(),
                f32::from(license_options.min_score),
                deadline,
            )
        };

        match detection_result {
            Ok(detections) => {
                let query = match Query::from_extracted_text_with_deadline(
                    &text_content,
                    engine.index(),
                    from_binary_strings,
                    deadline,
                ) {
                    Ok(query) => Some(query),
                    Err(LicenseDetectionError::Timeout) => {
                        return Err(license_detection_timeout(timeout_seconds));
                    }
                };
                process_successful_detections(
                    file_info_builder,
                    &detections,
                    query.as_ref(),
                    &text_content,
                    path,
                    license_options,
                    engine.index(),
                );
            }
            Err(LicenseDetectionError::Timeout) => {
                return Err(license_detection_timeout(timeout_seconds));
            }
        }
    } else {
        let detection_result = if license_options.min_score == 0 {
            engine.detect_with_kind_and_source(
                &text_content,
                license_options.unknown_licenses,
                from_binary_strings,
                &path.to_string_lossy(),
            )
        } else {
            engine.detect_with_kind_and_source_with_score(
                &text_content,
                license_options.unknown_licenses,
                from_binary_strings,
                &path.to_string_lossy(),
                f32::from(license_options.min_score),
            )
        };

        match detection_result {
            Ok(detections) => {
                let query =
                    Query::from_extracted_text(&text_content, engine.index(), from_binary_strings)
                        .ok();
                process_successful_detections(
                    file_info_builder,
                    &detections,
                    query.as_ref(),
                    &text_content,
                    path,
                    license_options,
                    engine.index(),
                );
            }
            Err(e) => {
                scan_diagnostics.push(ScanDiagnostic::error(format!(
                    "License detection failed: {}",
                    e
                )));
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct NixLicenseDeclaration {
    symbol: String,
    matched_text: String,
    start_line: LineNumber,
    end_line: LineNumber,
}

fn supplement_nix_manifest_license_detections(
    path: &Path,
    text_content: &str,
    existing_detections: &[PublicLicenseDetection],
) -> Vec<PublicLicenseDetection> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("nix")
        || !text_content.contains("license =")
    {
        return Vec::new();
    }

    let mut existing_license_keys = collect_detected_license_keys(existing_detections);
    let mut synthesized = Vec::new();

    for declaration in extract_nix_manifest_license_declarations(text_content) {
        let Some(spdx_expression) = nix_license_symbol_to_spdx(&declaration.symbol) else {
            continue;
        };
        let Some(normalized) = normalize_spdx_expression(spdx_expression) else {
            continue;
        };
        let declared_license_keys =
            collect_expression_keys(&normalized.declared_license_expression);
        let declared_spdx_keys =
            collect_expression_keys(&normalized.declared_license_expression_spdx);
        let all_declared_keys = declared_license_keys
            .iter()
            .chain(declared_spdx_keys.iter())
            .cloned()
            .collect::<HashSet<_>>();
        if !all_declared_keys.is_empty()
            && all_declared_keys
                .iter()
                .all(|key| existing_license_keys.contains(key))
        {
            continue;
        }

        let (_, _, detections) = build_declared_license_data(
            normalized,
            DeclaredLicenseMatchMetadata::new(
                &declaration.matched_text,
                declaration.start_line,
                declaration.end_line,
            ),
        );

        for key in all_declared_keys {
            existing_license_keys.insert(key);
        }
        synthesized.extend(detections);
    }

    synthesized
}

fn collapse_repeated_sourcemap_license_detections(
    path: &Path,
    detections: Vec<PublicLicenseDetection>,
) -> Vec<PublicLicenseDetection> {
    if !crate::utils::sourcemap::is_sourcemap(path) || detections.len() <= 1 {
        return detections;
    }

    let mut deduplicated: Vec<PublicLicenseDetection> = Vec::new();

    for detection in detections {
        if let Some(existing) = deduplicated.iter_mut().find(|existing| {
            existing.license_expression == detection.license_expression
                && existing.license_expression_spdx == detection.license_expression_spdx
        }) {
            merge_public_detection(existing, detection);
        } else {
            deduplicated.push(detection);
        }
    }

    let concrete_detections = deduplicated
        .iter()
        .filter(|detection| is_concrete_sourcemap_detection(detection))
        .cloned()
        .collect::<Vec<_>>();
    if concrete_detections.len() <= 1 {
        return deduplicated;
    }

    let combined = combine_sourcemap_detections(concrete_detections);
    let mut combined = Some(combined);
    let mut collapsed = Vec::with_capacity(deduplicated.len());

    for detection in deduplicated {
        if is_concrete_sourcemap_detection(&detection) {
            if let Some(combined) = combined.take() {
                collapsed.push(combined);
            }
            continue;
        }

        collapsed.push(detection);
    }

    collapsed
}

fn is_concrete_sourcemap_detection(detection: &PublicLicenseDetection) -> bool {
    !detection_is_unknown_reference(detection)
        && (!detection.license_expression.is_empty()
            || !detection.license_expression_spdx.is_empty())
}

fn combine_sourcemap_detections(detections: Vec<PublicLicenseDetection>) -> PublicLicenseDetection {
    let mut detections = detections.into_iter();
    let mut combined = detections
        .next()
        .expect("sourcemap combination requires at least one detection");
    let mut combined_license_expressions = vec![combined.license_expression.clone()];
    let mut combined_spdx_expressions = if combined.license_expression_spdx.is_empty() {
        Vec::new()
    } else {
        vec![combined.license_expression_spdx.clone()]
    };

    for detection in detections {
        combined_license_expressions.push(detection.license_expression.clone());
        if !detection.license_expression_spdx.is_empty() {
            combined_spdx_expressions.push(detection.license_expression_spdx.clone());
        }
        merge_public_detection(&mut combined, detection);
    }

    combined.license_expression =
        crate::utils::spdx::combine_license_expressions_preserving_structure(
            combined_license_expressions,
        )
        .unwrap_or_else(|| combined.license_expression.clone());
    combined.license_expression_spdx =
        crate::utils::spdx::combine_license_expressions_preserving_structure(
            combined_spdx_expressions,
        )
        .unwrap_or_else(|| combined.license_expression_spdx.clone());
    combined
}

fn merge_public_detection(
    existing: &mut PublicLicenseDetection,
    detection: PublicLicenseDetection,
) {
    for detection_log in detection.detection_log {
        if !existing.detection_log.contains(&detection_log) {
            existing.detection_log.push(detection_log);
        }
    }

    for matched_region in detection.matches {
        if !existing.matches.contains(&matched_region) {
            existing.matches.push(matched_region);
        }
    }

    if existing.identifier.is_empty() {
        existing.identifier = detection.identifier;
    }
}

fn collect_detected_license_keys(detections: &[PublicLicenseDetection]) -> HashSet<String> {
    let mut keys = HashSet::new();
    for detection in detections {
        keys.extend(collect_expression_keys(&detection.license_expression));
        keys.extend(collect_expression_keys(&detection.license_expression_spdx));
    }
    keys
}

fn collect_expression_keys(expression: &str) -> HashSet<String> {
    parse_expression(expression)
        .ok()
        .map(|parsed| {
            parsed
                .license_keys()
                .into_iter()
                .map(|key| key.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

fn extract_nix_manifest_license_declarations(text: &str) -> Vec<NixLicenseDeclaration> {
    let mut declarations = Vec::new();
    let mut in_license_list = false;

    for (index, line) in text.lines().enumerate() {
        let line_number = LineNumber::new(index + 1).expect("line number should be valid");
        let line_without_comment = line.split('#').next().unwrap_or("");
        let trimmed = line_without_comment.trim();

        if in_license_list {
            let closes_list = trimmed.contains("];");
            for symbol in tokenize_nix_license_symbols(trimmed) {
                declarations.push(NixLicenseDeclaration {
                    matched_text: symbol.clone(),
                    symbol,
                    start_line: line_number,
                    end_line: line_number,
                });
            }
            if closes_list {
                in_license_list = false;
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("license = lib.licenses.") {
            if let Some(symbol) = parse_nix_license_symbol(rest) {
                declarations.push(NixLicenseDeclaration {
                    matched_text: symbol.clone(),
                    symbol,
                    start_line: line_number,
                    end_line: line_number,
                });
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("license = with lib.licenses; [") {
            in_license_list = !trimmed.contains("];");
            for symbol in tokenize_nix_license_symbols(rest) {
                declarations.push(NixLicenseDeclaration {
                    matched_text: symbol.clone(),
                    symbol,
                    start_line: line_number,
                    end_line: line_number,
                });
            }
            if trimmed.contains("];") {
                in_license_list = false;
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("license = licenses.") {
            if let Some(symbol) = parse_nix_license_symbol(rest) {
                declarations.push(NixLicenseDeclaration {
                    matched_text: format!("licenses.{symbol}"),
                    symbol,
                    start_line: line_number,
                    end_line: line_number,
                });
            }
            continue;
        }
    }

    declarations
}

fn parse_nix_license_symbol(input: &str) -> Option<String> {
    let mut symbol = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '+' | '-') {
            symbol.push(ch);
        } else {
            break;
        }
    }
    (!symbol.is_empty()).then_some(symbol)
}

fn tokenize_nix_license_symbols(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .filter_map(parse_nix_license_symbol)
        .collect()
}

fn nix_license_symbol_to_spdx(symbol: &str) -> Option<&'static str> {
    match symbol {
        "asl20" => Some("Apache-2.0"),
        "bsd2" => Some("BSD-2-Clause"),
        "bsd3" => Some("BSD-3-Clause"),
        "gpl1Only" => Some("GPL-1.0-only"),
        "gpl1Plus" => Some("GPL-1.0-or-later"),
        "gpl2" | "gpl2Only" => Some("GPL-2.0-only"),
        "gpl2Plus" => Some("GPL-2.0-or-later"),
        "gpl3" | "gpl3Only" => Some("GPL-3.0-only"),
        "gpl3Plus" => Some("GPL-3.0-or-later"),
        "lgpl21" => Some("LGPL-2.1-only"),
        "lgpl21Plus" => Some("LGPL-2.1-or-later"),
        "lgpl3" => Some("LGPL-3.0-only"),
        "lgpl3Plus" => Some("LGPL-3.0-or-later"),
        "mit" => Some("MIT"),
        "mpl11" => Some("MPL-1.1"),
        "mpl20" => Some("MPL-2.0"),
        _ => None,
    }
}

fn process_successful_detections(
    file_info_builder: &mut FileInfoBuilder,
    detections: &[InternalLicenseDetection],
    query: Option<&Query<'_>>,
    text_content: &str,
    path: &Path,
    license_options: LicenseScanOptions,
    index: &LicenseIndex,
) {
    let mut detections = detections.to_vec();
    promote_legal_notice_low_quality_detections(&mut detections, path);

    let mut model_detections = Vec::new();
    let mut model_clues = Vec::new();

    for detection in &detections {
        let (public_detection, clue_matches) = convert_detection_to_model(
            detection,
            license_options,
            text_content,
            query,
            Some(index),
        );

        if let Some(public_detection) = public_detection {
            model_detections.push(public_detection);
        }

        model_clues.extend(clue_matches);
    }

    prune_contextual_short_reference_matches(
        path,
        text_content,
        license_options.include_diagnostics,
        &mut model_detections,
        &mut model_clues,
    );
    model_detections.extend(supplement_nix_manifest_license_detections(
        path,
        text_content,
        &model_detections,
    ));
    expand_dual_licensed_under_readme_choice_detections(path, text_content, &mut model_detections);
    prune_redundant_readme_conjunctive_detections(path, &mut model_detections);
    model_detections = collapse_repeated_sourcemap_license_detections(path, model_detections);

    if !model_detections.is_empty() {
        let expressions: Option<Vec<String>> = model_detections
            .iter()
            .map(|detection| {
                (!detection.license_expression_spdx.is_empty())
                    .then(|| detection.license_expression_spdx.clone())
            })
            .collect();

        if let Some(expressions) = expressions {
            let combined =
                crate::utils::spdx::select_primary_license_expression_strict(expressions.clone())
                    .or_else(|| {
                        crate::utils::spdx::combine_license_expressions_preserving_structure_strict(
                            expressions,
                        )
                    });
            if let Some(expr) = combined {
                file_info_builder.detected_license_expression(Some(expr));
            }
        }
    }

    file_info_builder.license_detections(model_detections);
    file_info_builder.license_clues(model_clues);
    file_info_builder.percentage_of_license_text(
        query.map(|query| compute_percentage_of_license_text(query, &detections)),
    );
}

fn license_detection_timeout(timeout_seconds: f64) -> FileScanError {
    FileScanError::from_license_detection_timeout(timeout_seconds)
}

fn convert_detection_to_model(
    detection: &InternalLicenseDetection,
    license_options: LicenseScanOptions,
    text_content: &str,
    query: Option<&Query<'_>>,
    index: Option<&LicenseIndex>,
) -> (Option<PublicLicenseDetection>, Vec<Match>) {
    let matches: Vec<Match> = detection
        .matches
        .iter()
        .map(|m| convert_match_to_model(m, license_options, text_content, query))
        .collect();

    if let Some(license_expression) = detection.license_expression.clone() {
        (
            Some(PublicLicenseDetection {
                license_expression,
                license_expression_spdx: normalize_optional_spdx_expression(
                    detection.license_expression_spdx.as_deref(),
                ),
                matches,
                detection_log: if license_options.include_diagnostics {
                    detection.detection_log.clone()
                } else {
                    Vec::new()
                },
                identifier: detection.identifier.clone().unwrap_or_default(),
            }),
            Vec::new(),
        )
    } else if let Some(public_detection) = index.and_then(|index| {
        promote_reference_url_clue_detection(detection, license_options, text_content, query, index)
    }) {
        (Some(public_detection), Vec::new())
    } else {
        (None, matches)
    }
}

fn promote_reference_url_clue_detection(
    detection: &InternalLicenseDetection,
    license_options: LicenseScanOptions,
    text_content: &str,
    query: Option<&Query<'_>>,
    index: &LicenseIndex,
) -> Option<PublicLicenseDetection> {
    let query = query?;

    let promoted_matches: Vec<&InternalLicenseMatch> = detection
        .matches
        .iter()
        .filter(|license_match| match_has_exact_reference_url(query, license_match, index))
        .collect();

    if promoted_matches.is_empty() {
        return None;
    }

    let license_expression = crate::utils::spdx::combine_license_expressions_preserving_structure(
        promoted_matches
            .iter()
            .map(|license_match| license_match.license_expression.clone()),
    )?;
    let license_expression_spdx = promoted_matches
        .iter()
        .map(|license_match| license_match.license_expression_spdx.clone())
        .collect::<Option<Vec<_>>>()
        .and_then(crate::utils::spdx::combine_license_expressions_preserving_structure_strict)
        .unwrap_or_default();
    let matches = promoted_matches
        .into_iter()
        .map(|license_match| {
            convert_match_to_model(license_match, license_options, text_content, Some(query))
        })
        .collect();

    Some(PublicLicenseDetection {
        license_expression,
        license_expression_spdx,
        matches,
        detection_log: if license_options.include_diagnostics {
            vec!["promoted-reference-url-license-clue".to_string()]
        } else {
            Vec::new()
        },
        identifier: detection.identifier.clone().unwrap_or_default(),
    })
}

fn promote_legal_notice_low_quality_detections(
    detections: &mut [InternalLicenseDetection],
    path: &Path,
) {
    if !is_legal_notice_like_path(path) {
        return;
    }

    let has_concrete_detection = detections
        .iter()
        .any(|detection| detection.license_expression.is_some());
    if !has_concrete_detection {
        return;
    }

    for detection in detections {
        if detection.license_expression.is_some()
            || !detection
                .detection_log
                .iter()
                .any(|log| log == "low-quality-match-fragments")
            || detection.matches.is_empty()
        {
            continue;
        }

        if !detection.matches.iter().all(|license_match| {
            !license_match.is_license_clue()
                && !license_match.license_expression.is_empty()
                && !license_match.license_expression.contains("unknown")
        }) {
            continue;
        }

        let Some(license_expression) =
            crate::utils::spdx::combine_license_expressions_preserving_structure(
                detection
                    .matches
                    .iter()
                    .map(|license_match| license_match.license_expression.clone())
                    .collect::<Vec<_>>(),
            )
        else {
            continue;
        };
        let license_expression_spdx = detection
            .matches
            .iter()
            .map(|license_match| license_match.license_expression_spdx.clone())
            .collect::<Option<Vec<_>>>()
            .and_then(crate::utils::spdx::combine_license_expressions_preserving_structure_strict);

        detection.license_expression = Some(license_expression);
        detection.license_expression_spdx = license_expression_spdx;
        detection
            .detection_log
            .push("promoted-low-quality-legal-notice".to_string());
    }
}

fn is_legal_notice_like_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(base_name) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };

    let name = name.to_ascii_lowercase();
    let base_name = base_name.to_ascii_lowercase();
    ["notice", "copyright", "copying", "license", "licence"]
        .iter()
        .any(|pattern| {
            name.starts_with(pattern)
                || name.ends_with(pattern)
                || base_name.starts_with(pattern)
                || base_name.ends_with(pattern)
        })
}

fn prune_redundant_readme_conjunctive_detections(
    path: &Path,
    detections: &mut Vec<PublicLicenseDetection>,
) {
    if !is_readme_like_path(path) || detections.len() < 2 {
        return;
    }

    let concrete_ranges: Vec<(usize, usize)> = detections
        .iter()
        .filter(|detection| !detection_is_unknown_reference(detection))
        .filter_map(detection_line_range)
        .collect();

    let alternative_key_sets: Vec<(HashSet<String>, (usize, usize))> = detections
        .iter()
        .filter_map(|detection| {
            let expression = detection_expression_ast(detection)?;
            expression_contains_or(&expression).then_some((detection, expression))
        })
        .filter_map(|(detection, expression)| {
            let range = detection_line_range(detection)?;
            Some((
                expression
                    .license_keys()
                    .into_iter()
                    .map(|key| key.to_ascii_lowercase())
                    .collect(),
                range,
            ))
        })
        .collect();

    if alternative_key_sets.is_empty() {
        return;
    }

    detections.retain(|detection| {
        if detection_is_unknown_reference(detection)
            && let Some((start, end)) = detection_line_range(detection)
            && concrete_ranges.iter().any(|(other_start, other_end)| {
                ranges_overlap(start, end, *other_start, *other_end)
            })
        {
            return false;
        }

        let Some(expression) = detection_expression_ast(detection) else {
            return true;
        };

        if expression_contains_or(&expression) || !expression_contains_and(&expression) {
            return true;
        }

        let Some((start, end)) = detection_line_range(detection) else {
            return true;
        };

        let keys: HashSet<String> = expression
            .license_keys()
            .into_iter()
            .map(|key| key.to_ascii_lowercase())
            .collect();

        !alternative_key_sets
            .iter()
            .any(|(candidate_keys, (other_start, other_end))| {
                *candidate_keys == keys && ranges_overlap(start, end, *other_start, *other_end)
            })
    });
}

fn prune_contextual_short_reference_matches(
    path: &Path,
    text_content: &str,
    include_diagnostics: bool,
    detections: &mut Vec<PublicLicenseDetection>,
    clues: &mut Vec<Match>,
) {
    let source_lines: Vec<&str> = text_content.lines().collect();
    if source_lines.is_empty() {
        return;
    }

    clues.retain(|detection_match| {
        !should_prune_contextual_short_reference_match(detection_match, &source_lines)
    });

    detections.retain_mut(|detection| {
        let original_match_count = detection.matches.len();
        detection.matches.retain(|detection_match| {
            !should_prune_contextual_short_reference_match(detection_match, &source_lines)
        });

        if detection.matches.is_empty() {
            return false;
        }

        if detection.matches.len() != original_match_count {
            refresh_public_detection_after_context_prune(detection, path, include_diagnostics);
        }

        true
    });
}

fn refresh_public_detection_after_context_prune(
    detection: &mut PublicLicenseDetection,
    path: &Path,
    include_diagnostics: bool,
) {
    detection.license_expression =
        crate::utils::spdx::combine_license_expressions_preserving_structure(
            detection
                .matches
                .iter()
                .map(|detection_match| detection_match.license_expression.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|| detection.matches[0].license_expression.clone());
    detection.license_expression_spdx =
        crate::utils::spdx::combine_license_expressions_preserving_structure_strict(
            detection
                .matches
                .iter()
                .map(|detection_match| detection_match.license_expression_spdx.clone())
                .filter(|expression| !expression.is_empty())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();
    detection.identifier = String::new();
    if include_diagnostics
        && !detection
            .detection_log
            .iter()
            .any(|log| log == "contextual-short-license-mention-pruned")
    {
        detection
            .detection_log
            .push("contextual-short-license-mention-pruned".to_string());
    }
    crate::models::file_info::enrich_license_detection_provenance(
        detection,
        &path.to_string_lossy(),
    );
}

fn should_prune_contextual_short_reference_match(
    detection_match: &Match,
    source_lines: &[&str],
) -> bool {
    if !(is_short_reference_like_public_match(detection_match)
        || is_unknown_reference_like_public_match(detection_match))
    {
        return false;
    }

    let start_line = detection_match.start_line.get();
    let end_line = detection_match.end_line.get().min(source_lines.len());
    if start_line == 0 || start_line > end_line || start_line > source_lines.len() {
        return false;
    }

    (start_line..=end_line).any(|line_number| {
        is_markdown_license_table_row(line_number, source_lines)
            || is_negative_or_comparative_license_mention_line(source_lines[line_number - 1])
    })
}

fn is_short_reference_like_public_match(detection_match: &Match) -> bool {
    detection_match.matched_length.unwrap_or(usize::MAX) <= 3
}

fn is_unknown_reference_like_public_match(detection_match: &Match) -> bool {
    detection_match.license_expression == "unknown-license-reference"
        || detection_match.license_expression_spdx
            == "LicenseRef-scancode-unknown-license-reference"
        || (!detection_match.rule_identifier.is_empty()
            && detection_match
                .rule_identifier
                .to_ascii_lowercase()
                .contains("unknown"))
}

fn is_markdown_license_table_row(line_number: usize, source_lines: &[&str]) -> bool {
    let Some(line) = source_lines.get(line_number.saturating_sub(1)) else {
        return false;
    };
    if !is_markdown_table_line(line) {
        return false;
    }

    let mut block_start = line_number - 1;
    while block_start > 0 && is_markdown_table_line(source_lines[block_start - 1]) {
        block_start -= 1;
    }

    let Some(header_line) = source_lines.get(block_start) else {
        return false;
    };
    let Some(separator_line) = source_lines.get(block_start + 1) else {
        return false;
    };

    header_line.to_ascii_lowercase().contains("license")
        && is_markdown_table_separator_line(separator_line)
        && line_number > block_start + 2
}

fn is_markdown_table_line(line: &str) -> bool {
    let trimmed = strip_markdown_table_comment_prefix(line).trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

fn is_markdown_table_separator_line(line: &str) -> bool {
    let trimmed = strip_markdown_table_comment_prefix(line).trim();
    is_markdown_table_line(trimmed)
        && trimmed
            .chars()
            .all(|ch| matches!(ch, '|' | '-' | ':' | ' ' | '\t'))
}

fn strip_markdown_table_comment_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    for prefix in ["//!", "///", "//", "*"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.trim_start();
        }
    }
    trimmed
}

fn is_negative_or_comparative_license_mention_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    (lower.contains("no ") && lower.contains("restriction"))
        || (line_has_possessive_without_license_phrase(&lower))
        || (lower.contains("unlike ") && line.contains('(') && line.contains(')'))
        || lower.contains("-licensed alternative")
        || lower.contains("-licensed alternatives")
        || line_has_negated_license_shorthand_list(line)
}

fn line_has_possessive_without_license_phrase(lower: &str) -> bool {
    lower.contains("without its ") && lower.contains(" license")
        || lower.contains("without their ") && lower.contains(" license")
}

fn line_has_negated_license_shorthand_list(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("no ") && line.contains('/') && line.chars().any(|ch| ch.is_ascii_uppercase())
}

fn expand_dual_licensed_under_readme_choice_detections(
    path: &Path,
    text_content: &str,
    detections: &mut Vec<PublicLicenseDetection>,
) {
    if !is_readme_like_path(path) || !has_dual_licensed_under_notice_text(text_content) {
        return;
    }

    let mut extras = Vec::new();

    for detection in detections.iter() {
        if !detection.license_expression_spdx.contains(" OR ") {
            continue;
        }

        let Some((start, end)) = detection_line_range(detection) else {
            continue;
        };

        for detection_match in &detection.matches {
            let single_spdx = detection_match.license_expression_spdx.trim();
            if single_spdx.is_empty()
                || single_spdx.contains(" OR ")
                || single_spdx.contains(" AND ")
                || single_spdx.contains(" WITH ")
            {
                continue;
            }

            let already_present = detections.iter().chain(extras.iter()).any(|existing| {
                existing.license_expression_spdx == single_spdx
                    && detection_line_range(existing).is_some_and(|range| range == (start, end))
            });
            if already_present {
                continue;
            }

            extras.push(PublicLicenseDetection {
                license_expression: detection_match.license_expression.clone(),
                license_expression_spdx: detection_match.license_expression_spdx.clone(),
                matches: vec![detection_match.clone()],
                detection_log: detection.detection_log.clone(),
                identifier: String::new(),
            });
        }
    }

    detections.extend(extras);
}

fn is_readme_like_path(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case("readme"))
}

fn has_dual_licensed_under_notice_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("dual-licensed under") || lower.contains("dual licensed under")
}

fn detection_expression_ast(
    detection: &PublicLicenseDetection,
) -> Option<crate::license_detection::expression::LicenseExpression> {
    let expression = if !detection.license_expression_spdx.is_empty() {
        detection.license_expression_spdx.as_str()
    } else if !detection.license_expression.is_empty() {
        detection.license_expression.as_str()
    } else {
        return None;
    };

    parse_expression(expression).ok()
}

fn detection_is_unknown_reference(detection: &PublicLicenseDetection) -> bool {
    detection.license_expression == "unknown-license-reference"
        || detection.license_expression_spdx == "LicenseRef-scancode-unknown-license-reference"
}

fn detection_line_range(detection: &PublicLicenseDetection) -> Option<(usize, usize)> {
    let start = detection.matches.iter().map(|m| m.start_line.get()).min()?;
    let end = detection.matches.iter().map(|m| m.end_line.get()).max()?;
    Some((start, end))
}

fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start <= b_end && b_start <= a_end
}

fn expression_contains_or(
    expression: &crate::license_detection::expression::LicenseExpression,
) -> bool {
    match expression {
        crate::license_detection::expression::LicenseExpression::Or { .. } => true,
        crate::license_detection::expression::LicenseExpression::And { left, right }
        | crate::license_detection::expression::LicenseExpression::With { left, right } => {
            expression_contains_or(left) || expression_contains_or(right)
        }
        _ => false,
    }
}

fn expression_contains_and(
    expression: &crate::license_detection::expression::LicenseExpression,
) -> bool {
    match expression {
        crate::license_detection::expression::LicenseExpression::And { .. } => true,
        crate::license_detection::expression::LicenseExpression::Or { left, right }
        | crate::license_detection::expression::LicenseExpression::With { left, right } => {
            expression_contains_and(left) || expression_contains_and(right)
        }
        _ => false,
    }
}

fn match_has_exact_reference_url(
    query: &Query<'_>,
    license_match: &InternalLicenseMatch,
    index: &LicenseIndex,
) -> bool {
    let Some(license) = index.licenses_by_key.get(&license_match.license_expression) else {
        return false;
    };

    if license.reference_urls.is_empty() {
        return false;
    }

    let matched_text = license_match.matched_text.clone().unwrap_or_else(|| {
        query.matched_text(license_match.start_line.get(), license_match.end_line.get())
    });
    let normalized_text = normalize_reference_url_candidate(&matched_text);
    if normalized_text.is_empty() {
        return false;
    }

    license.reference_urls.iter().any(|reference_url| {
        let normalized_reference = normalize_reference_url_candidate(reference_url);
        !normalized_reference.is_empty() && normalized_text.contains(&normalized_reference)
    })
}

fn normalize_reference_url_candidate(text: &str) -> String {
    text.trim().trim_end_matches('/').to_ascii_lowercase()
}

fn extract_output_matched_text(
    license_match: &InternalLicenseMatch,
    text_content: &str,
    query: Option<&Query<'_>>,
) -> String {
    if let Some(matched_text) = &license_match.matched_text {
        return cap_output_matched_text(matched_text.clone());
    }

    let start_line = license_match.start_line.get();
    let end_line = license_match.end_line.get();

    if line_range_has_oversized_line(
        text_content,
        start_line,
        end_line,
        MAX_OUTPUT_MATCHED_TEXT_LINE_LENGTH,
    ) {
        if let Some(compact_text) = compact_matched_text_from_query(query, license_match) {
            return cap_output_matched_text(compact_text);
        }

        return cap_output_matched_text(bounded_matched_text_from_text(
            text_content,
            start_line,
            end_line,
        ));
    }

    let whole_line =
        crate::license_detection::query::matched_text_from_text(text_content, start_line, end_line);

    if whole_line.len() > MAX_OUTPUT_MATCHED_TEXT_BYTES
        && let Some(compact_text) = compact_matched_text_from_query(query, license_match)
    {
        return cap_output_matched_text(compact_text);
    }

    cap_output_matched_text(whole_line)
}

fn compact_matched_text_from_query(
    query: Option<&Query<'_>>,
    license_match: &InternalLicenseMatch,
) -> Option<String> {
    let query = query?;
    let matched_positions: PositionSet = license_match.query_span().iter().collect();
    let start_pos = matched_positions.iter().min()?;
    let end_pos = matched_positions.iter().max()?;

    Some(crate::license_detection::query::matched_text_from_tokens(
        &query.text,
        query,
        &matched_positions,
        start_pos,
        end_pos,
        license_match.start_line.get(),
        license_match.end_line.get(),
    ))
}

fn line_range_has_oversized_line(
    text: &str,
    start_line: usize,
    end_line: usize,
    max_line_length: usize,
) -> bool {
    if start_line == 0 || end_line == 0 || start_line > end_line {
        return false;
    }

    text.lines().enumerate().any(|(idx, line)| {
        let line_num = idx + 1;
        line_num >= start_line && line_num <= end_line && line.len() > max_line_length
    })
}

fn bounded_matched_text_from_text(text: &str, start_line: usize, end_line: usize) -> String {
    matched_text_from_text_with_line_cap(
        text,
        start_line,
        end_line,
        MAX_OUTPUT_MATCHED_TEXT_LINE_LENGTH,
    )
}

fn matched_text_from_text_with_line_cap(
    text: &str,
    start_line: usize,
    end_line: usize,
    max_line_length: usize,
) -> String {
    if start_line == 0 || end_line == 0 || start_line > end_line {
        return String::new();
    }

    let mut selected_lines = Vec::new();

    for (idx, line) in text.split_inclusive('\n').enumerate() {
        let line_num = idx + 1;
        if line_num < start_line || line_num > end_line {
            continue;
        }

        let (line_text, line_ending) = split_line_ending(line);
        let capped_line = if line_text.len() > max_line_length {
            truncate_with_marker(line_text, max_line_length)
        } else {
            line_text.to_string()
        };

        selected_lines.push((capped_line, line_ending.to_string()));
    }

    let total_lines = selected_lines.len();
    let mut rendered = String::new();
    for (idx, (line_text, line_ending)) in selected_lines.into_iter().enumerate() {
        rendered.push_str(&line_text);
        if idx + 1 < total_lines {
            rendered.push_str(&line_ending);
        }
    }

    rendered
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(line) = line.strip_suffix("\r\n") {
        (line, "\r\n")
    } else if let Some(line) = line.strip_suffix('\n') {
        (line, "\n")
    } else {
        (line, "")
    }
}

fn cap_output_matched_text(text: String) -> String {
    if text.len() <= MAX_OUTPUT_MATCHED_TEXT_BYTES {
        return text;
    }

    truncate_with_marker(&text, MAX_OUTPUT_MATCHED_TEXT_BYTES)
}

fn truncate_with_marker(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }

    if max_bytes <= MATCHED_TEXT_TRUNCATION_MARKER.len() {
        return truncate_at_char_boundary(MATCHED_TEXT_TRUNCATION_MARKER, max_bytes).to_string();
    }

    let prefix = truncate_at_char_boundary(
        text,
        max_bytes.saturating_sub(MATCHED_TEXT_TRUNCATION_MARKER.len()),
    );
    format!("{prefix}{MATCHED_TEXT_TRUNCATION_MARKER}")
}

fn truncate_at_char_boundary(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }

    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }

    &text[..end]
}

fn convert_match_to_model(
    m: &crate::license_detection::models::LicenseMatch,
    license_options: LicenseScanOptions,
    text_content: &str,
    query: Option<&Query<'_>>,
) -> Match {
    let rule_url = if m.rule_url.is_empty() {
        None
    } else {
        Some(m.rule_url.clone())
    };
    let matched_text = if license_options.include_text {
        Some(extract_output_matched_text(m, text_content, query))
    } else {
        None
    };
    let matched_text_diagnostics = if license_options.include_text_diagnostics {
        query.map(|query| matched_text_diagnostics_from_match(query, m))
    } else {
        None
    };
    Match {
        license_expression: m.license_expression.clone(),
        license_expression_spdx: normalize_optional_spdx_expression(
            m.license_expression_spdx.as_deref(),
        ),
        from_file: m.from_file.clone(),
        start_line: m.start_line,
        end_line: m.end_line,
        matcher: m.matcher,
        score: m.score,
        matched_length: Some(m.matched_length),
        match_coverage: Some((f64::from(m.coverage()) * 100.0).round() / 100.0),
        rule_relevance: Some(m.rule_relevance),
        rule_identifier: m.rule_identifier.clone(),
        rule_url,
        matched_text,
        referenced_filenames: m.referenced_filenames.clone(),
        matched_text_diagnostics,
    }
}

fn normalize_optional_spdx_expression(expression: Option<&str>) -> String {
    let Some(expression) = expression
        .map(str::trim)
        .filter(|expression| !expression.is_empty())
    else {
        return String::new();
    };

    crate::utils::spdx::combine_license_expressions_preserving_structure_strict(std::iter::once(
        expression.to_string(),
    ))
    .unwrap_or_default()
}

fn compute_percentage_of_license_text(
    query: &Query<'_>,
    detections: &[InternalLicenseDetection],
) -> f64 {
    let matched_positions: std::collections::HashSet<usize> = detections
        .iter()
        .flat_map(|detection| detection.matches.iter())
        .flat_map(|m| m.query_span().iter())
        .collect();

    let query_tokens_length = query.tokens.len() + query.unknowns_by_pos.values().sum::<usize>();
    if query_tokens_length == 0 {
        return 0.0;
    }

    let percentage = (matched_positions.len() as f64 / query_tokens_length as f64) * 100.0;
    (percentage * 100.0).round() / 100.0
}

fn matched_text_diagnostics_from_match(
    query: &Query<'_>,
    license_match: &InternalLicenseMatch,
) -> String {
    let matched_positions: PositionSet = license_match.query_span().iter().collect();
    let Some(start_pos) = matched_positions.iter().min() else {
        return bounded_matched_text_from_text(
            &query.text,
            license_match.start_line.get(),
            license_match.end_line.get(),
        );
    };
    let Some(end_pos) = matched_positions.iter().max() else {
        return bounded_matched_text_from_text(
            &query.text,
            license_match.start_line.get(),
            license_match.end_line.get(),
        );
    };

    cap_output_matched_text(
        crate::license_detection::query::matched_text_diagnostics_from_text(
            &query.text,
            query,
            &matched_positions,
            start_pos,
            end_pos,
            license_match.start_line.get(),
            license_match.end_line.get(),
        ),
    )
}

#[cfg(test)]
#[path = "license_test.rs"]
mod tests;
