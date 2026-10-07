// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use crate::copyright::line_tracking::PreparedLines;
use crate::copyright::refiner::has_copyright_year;
use crate::copyright::types::{AuthorDetection, CopyrightDetection, HolderDetection};
use crate::models::LineNumber;
use regex::Regex;
use std::sync::LazyLock;
use std::time::Instant;

use super::super::seen_text::SeenTextSets;

// Copyright postprocess phase fn; the long argument list threads the shared detection-pipeline state.
#[allow(clippy::too_many_arguments)]
fn run_initial_detection_repairs(
    content: &str,
    prepared_cache: &PreparedLines<'_>,
    copyrights: &mut Vec<CopyrightDetection>,
    holders: &mut Vec<HolderDetection>,
    seen: &mut SeenTextSets,
) {
    let (mut new_c, mut new_h) =
        super::postprocess_transforms::extract_question_mark_year_copyrights(prepared_cache);
    seen.dedup_new_copyrights(&mut new_c, 0);
    seen.dedup_new_holders(&mut new_h, 0);
    copyrights.extend(new_c);
    holders.extend(new_h);

    if super::pattern_extract::is_lppl_license_document(content) {
        holders.retain(|h| h.holder != "M. Y.");
    }

    super::pattern_extract::drop_arch_floppy_h_bare_1995(content, copyrights);
    super::pattern_extract::drop_batman_adv_contributors_copyright(content, copyrights, holders);
    seen.rebuild_copyrights_from(copyrights);
    seen.rebuild_holders_from(holders);

    super::postprocess_transforms::split_embedded_copyright_detections(copyrights, holders);
    let new_h = super::postprocess_transforms::add_missing_holders_from_email_bearing_copyrights(
        &copyrights[..],
        &holders[..],
    );
    holders.extend(new_h);
    let new_h =
        super::postprocess_transforms::add_missing_holders_from_lowercase_hyphenated_url_copyrights(
            &copyrights[..],
        );
    holders.extend(new_h);
    super::postprocess_transforms::extend_bare_c_year_detections_to_line_end_for_multi_c_lines(
        prepared_cache,
        copyrights,
        holders,
    );
    super::postprocess_transforms::replace_holders_with_embedded_c_year_markers(
        copyrights, holders,
    );
    let new_h = super::postprocess_transforms::add_missing_holders_for_debian_modifications(
        content,
        &copyrights[..],
    );
    holders.extend(new_h);
    super::postprocess_transforms::fix_sundry_contributors_truncation(
        prepared_cache,
        copyrights,
        holders,
    );
    super::token_utils::restore_bare_holder_angle_emails(copyrights, holders);
    super::postprocess_transforms::drop_trailing_software_line_from_holders(
        prepared_cache,
        holders,
    );
    super::postprocess_transforms::drop_url_embedded_c_symbol_false_positive_holders(
        content, holders,
    );

    let c_before = copyrights.len();
    let h_before = holders.len();
    super::postprocess_transforms::recover_template_literal_year_range_copyrights(
        content, copyrights, holders,
    );
    seen.dedup_new_copyrights(copyrights, c_before);
    seen.dedup_new_holders(holders, h_before);
}

// Copyright postprocess phase fn; the long argument list threads the shared detection-pipeline state.
#[allow(clippy::too_many_arguments)]
fn run_author_extraction_and_repairs(
    content: &str,
    raw_lines: &[&str],
    prepared_cache: &PreparedLines<'_>,
    copyrights: &mut Vec<CopyrightDetection>,
    holders: &mut Vec<HolderDetection>,
    authors: &mut Vec<AuthorDetection>,
    seen: &mut SeenTextSets,
) {
    let a_before_markup = authors.len();
    super::author_heuristics::extract_markup_authors(content, authors);
    seen.authors
        .extend(authors[a_before_markup..].iter().map(|a| a.author.clone()));

    let mut new_a = super::author_heuristics::extract_rst_field_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_toml_author_assignment_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let a_before = authors.len();
    super::author_heuristics::merge_metadata_author_and_email_lines(prepared_cache, authors);
    super::author_heuristics::drop_metadata_field_listing_authors(prepared_cache, authors);
    seen.dedup_new_authors(authors, a_before);

    let mut new_a = super::author_heuristics::extract_debian_maintainer_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_maintainers_label_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_maintained_by_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_package_comment_named_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_created_by_project_author(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let a_before = authors.len();
    super::author_heuristics::extract_created_by_authors(prepared_cache, authors);
    seen.dedup_new_authors(authors, a_before);
    seen.rebuild_authors_from(authors);

    let a_before = authors.len();
    super::author_heuristics::extract_written_by_comma_and_copyright_authors(
        prepared_cache,
        authors,
    );
    seen.dedup_new_authors(authors, a_before);
    seen.rebuild_authors_from(authors);

    let mut new_a =
        super::author_heuristics::extract_line_local_attribution_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_subject_attribution_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let a_before = authors.len();
    super::author_heuristics::extract_multiline_written_by_author_blocks(prepared_cache, authors);
    seen.dedup_new_authors(authors, a_before);
    seen.rebuild_authors_from(authors);

    let mut new_a = super::author_heuristics::extract_name_contributed_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a =
        super::author_heuristics::extract_dash_bullet_attribution_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_plaintext_roster_by_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a =
        super::author_heuristics::extract_markdown_contributor_roster_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_written_on_top_of_by_authors(content);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_json_excerpt_developed_by_authors(content);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a =
        super::author_heuristics::extract_modified_portion_developed_by_authors(content);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a =
        super::author_heuristics::extract_was_developed_by_author_blocks(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_developed_by_sentence_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_developed_by_phrase_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a =
        super::author_heuristics::extract_developed_by_contributors_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_notice_developed_by_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a =
        super::author_heuristics::extract_with_additional_hacking_by_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_parenthesized_inline_by_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let a_before = authors.len();
    super::author_heuristics::extract_developed_and_created_by_authors(prepared_cache, authors);
    seen.dedup_new_authors(authors, a_before);
    seen.rebuild_authors_from(authors);

    let a_before = authors.len();
    super::author_heuristics::extract_author_colon_blocks(prepared_cache, authors);
    seen.dedup_new_authors(authors, a_before);
    seen.rebuild_authors_from(authors);

    let (mut new_c, mut new_h, mut new_a) = super::author_heuristics::extract_module_author_macros(
        content,
        &copyrights[..],
        &holders[..],
    );
    seen.dedup_new_copyrights(&mut new_c, 0);
    seen.dedup_new_holders(&mut new_h, 0);
    seen.dedup_new_authors(&mut new_a, 0);
    copyrights.extend(new_c);
    holders.extend(new_h);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_code_written_by_author_blocks(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_converted_to_by_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);
    seen.rebuild_authors_from(authors);

    let mut new_a = super::author_heuristics::extract_various_bugfixes_and_enhancements_by_authors(
        prepared_cache,
    );
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);
    seen.rebuild_authors_from(authors);

    let mut new_a = super::author_heuristics::extract_dense_name_email_author_lists(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    super::author_heuristics::drop_author_colon_lines_absorbed_into_year_only_copyrights(
        prepared_cache,
        copyrights,
        authors,
    );
    super::author_heuristics::drop_authors_embedded_in_copyrights(copyrights, authors);
    super::author_heuristics::drop_authors_from_copyright_by_lines(prepared_cache, authors);
    super::author_heuristics::drop_merged_dash_bullet_attribution_authors(authors);
    seen.rebuild_authors_from(authors);
    super::postprocess_transforms::drop_created_by_camelcase_identifier_authors(
        prepared_cache,
        authors,
    );
    super::author_heuristics::drop_shadowed_compound_email_authors(authors);
    super::author_heuristics::drop_shadowed_prefix_authors(authors);
    super::author_heuristics::drop_same_span_contact_sentence_overruns(authors);
    seen.rebuild_authors_from(authors);

    super::postprocess_transforms::merge_implemented_by_lines(
        prepared_cache,
        copyrights,
        holders,
        authors,
    );
    super::postprocess_transforms::split_written_by_copyrights_into_holder_prefixed_clauses(
        prepared_cache,
        copyrights,
        holders,
        authors,
    );
    super::author_heuristics::drop_written_by_authors_preceded_by_copyright(
        prepared_cache,
        authors,
    );
    super::author_heuristics::drop_ref_markup_authors(authors);
    seen.rebuild_authors_from(authors);

    let mut new_a = super::author_heuristics::extract_json_author_object_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    super::author_heuristics::normalize_json_blob_authors(raw_lines, authors);
    seen.authors = authors.iter().map(|a| a.author.clone()).collect();

    let mut new_a = super::author_heuristics::extract_json_credit_array_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_yaml_credit_array_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);
    super::author_heuristics::drop_out_of_scope_structured_credit_authors(raw_lines, authors);
    seen.rebuild_authors_from(authors);

    let mut new_a = super::author_heuristics::extract_embedded_yaml_credit_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a =
        super::postprocess_transforms::extract_following_authors_holders(raw_lines, prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    super::author_heuristics::drop_json_code_example_authors(raw_lines, authors);
    super::author_heuristics::drop_markup_element_value_authors(raw_lines, authors);
    seen.rebuild_authors_from(authors);

    let mut new_a = super::author_heuristics::extract_name_contributed_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a = super::author_heuristics::extract_changes_by_authors(prepared_cache);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    super::author_heuristics::repair_chained_attribution_authors(prepared_cache, authors);
    seen.rebuild_authors_from(authors);

    let mut new_a = super::author_heuristics::extract_comment_author_label_authors(raw_lines);
    new_a.retain(|candidate| {
        !authors
            .iter()
            .any(|author| author.start_line == candidate.start_line)
    });
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);
    super::author_heuristics::drop_markup_element_value_authors(raw_lines, authors);
    super::author_heuristics::drop_markup_declaration_authors(raw_lines, authors);
    super::author_heuristics::drop_authors_after_sentence_final_label(raw_lines, authors);
    super::author_heuristics::drop_subject_role_object_authors(raw_lines, authors);
    super::author_heuristics::drop_weak_prose_authors(raw_lines, authors);
    super::author_heuristics::repair_complete_by_line_author_boundaries(raw_lines, authors);
    super::author_heuristics::repair_hyphenated_prose_tail_authors(raw_lines, authors);
    super::author_heuristics::drop_embedded_authors_title_phrases(raw_lines, authors);
    seen.rebuild_authors_from(authors);

    let mut new_a = super::author_heuristics::extract_pod_author_section_contact_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a =
        super::author_heuristics::extract_pod_author_section_contactless_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);

    let mut new_a =
        super::author_heuristics::extract_pod_author_section_narrative_credit_authors(raw_lines);
    seen.dedup_new_authors(&mut new_a, 0);
    authors.extend(new_a);
}

// Copyright postprocess phase fn; the long argument list threads the shared detection-pipeline state.
#[allow(clippy::too_many_arguments)]
fn run_mid_pipeline_repairs(
    content: &str,
    raw_lines: &[&str],
    prepared_cache: &PreparedLines<'_>,
    did_expand_href: bool,
    copyrights: &mut Vec<CopyrightDetection>,
    holders: &mut Vec<HolderDetection>,
    authors: &mut Vec<AuthorDetection>,
    seen: &mut SeenTextSets,
) {
    super::postprocess_transforms::merge_multiline_copyrighted_by_with_trailing_copyright_clause(
        did_expand_href,
        content,
        copyrights,
    );
    super::postprocess_transforms::extend_copyrights_with_next_line_parenthesized_obfuscated_email(
        prepared_cache,
        copyrights,
    );
    super::postprocess_transforms::extend_copyrights_with_following_all_rights_reserved_line(
        raw_lines, copyrights,
    );

    super::postprocess_transforms::drop_symbol_year_only_copyrights(content, copyrights);

    super::postprocess_transforms::drop_from_source_attribution_copyrights(copyrights, holders);
    seen.rebuild_copyrights_from(copyrights);
    seen.rebuild_holders_from(holders);

    let c_before = copyrights.len();
    let h_before = holders.len();
    super::postprocess_transforms::fix_shm_inline_copyrights(prepared_cache, copyrights, holders);
    seen.dedup_new_copyrights(copyrights, c_before);
    seen.dedup_new_holders(holders, h_before);

    let c_before = copyrights.len();
    let h_before = holders.len();
    super::postprocess_transforms::fix_n_tty_linus_torvalds_written_by_clause(
        content, copyrights, holders,
    );
    seen.dedup_new_copyrights(copyrights, c_before);
    seen.dedup_new_holders(holders, h_before);

    let c_before = copyrights.len();
    let h_before = holders.len();
    super::postprocess_transforms::merge_freebird_c_inc_urls(prepared_cache, copyrights, holders);
    seen.dedup_new_copyrights(copyrights, c_before);
    seen.dedup_new_holders(holders, h_before);

    super::postprocess_transforms::merge_debugging390_best_viewed_suffix(
        prepared_cache,
        copyrights,
        holders,
    );
    super::postprocess_transforms::merge_fsf_gdb_notice_lines(prepared_cache, copyrights, holders);
    super::postprocess_transforms::merge_axis_ethereal_suffix(prepared_cache, copyrights, holders);

    let c_before = copyrights.len();
    let h_before = holders.len();
    super::postprocess_transforms::merge_kirkwood_converted_to(prepared_cache, copyrights, holders);
    seen.dedup_new_copyrights(copyrights, c_before);
    seen.dedup_new_holders(holders, h_before);

    let c_before = copyrights.len();
    let h_before = holders.len();
    let a_before = authors.len();
    super::postprocess_transforms::split_reworked_by_suffixes(
        content, copyrights, holders, authors,
    );
    seen.dedup_new_copyrights(copyrights, c_before);
    seen.dedup_new_holders(holders, h_before);
    seen.dedup_new_authors(authors, a_before);

    let c_before = copyrights.len();
    let h_before = holders.len();
    let a_before = authors.len();
    super::postprocess_transforms::split_author_project_copyright_metadata_blocks(
        copyrights, holders, authors,
    );
    seen.dedup_new_copyrights(copyrights, c_before);
    seen.dedup_new_holders(holders, h_before);
    seen.dedup_new_authors(authors, a_before);

    super::postprocess_transforms::drop_static_char_string_copyrights(content, copyrights, holders);
    super::postprocess_transforms::drop_combined_period_holders(holders);
    super::pattern_extract::drop_shadowed_prefix_holders(holders);
    super::pattern_extract::strip_trailing_c_year_suffix_from_comma_and_others(copyrights);
    super::pattern_extract::drop_bare_c_shadowed_by_non_copyright_prefixes(copyrights);
    seen.rebuild_copyrights_from(copyrights);
    seen.rebuild_holders_from(holders);
}

// Copyright postprocess phase fn; the long argument list threads the shared detection-pipeline state.
#[allow(clippy::too_many_arguments)]
fn run_late_pattern_extractions(
    content: &str,
    prepared_cache: &PreparedLines<'_>,
    copyrights: &mut Vec<CopyrightDetection>,
    holders: &mut Vec<HolderDetection>,
    seen: &mut SeenTextSets,
) {
    let (mut new_c, mut new_h) =
        super::pattern_extract::extract_name_before_rewrited_by_copyrights(prepared_cache);
    seen.dedup_new_copyrights(&mut new_c, 0);
    seen.dedup_new_holders(&mut new_h, 0);
    copyrights.extend(new_c);
    holders.extend(new_h);

    let (mut new_c, mut new_h) =
        super::pattern_extract::extract_developed_at_software_copyrights(prepared_cache);
    seen.dedup_new_copyrights(&mut new_c, 0);
    seen.dedup_new_holders(&mut new_h, 0);
    copyrights.extend(new_c);
    holders.extend(new_h);

    let c_before = copyrights.len();
    let h_before = holders.len();
    super::pattern_extract::extract_confidential_proprietary_copyrights(
        prepared_cache,
        copyrights,
        holders,
    );
    seen.dedup_new_copyrights(copyrights, c_before);
    seen.dedup_new_holders(holders, h_before);

    super::pattern_extract::drop_shadowed_bare_c_holders_with_year_prefixed_copyrights(
        copyrights, holders,
    );
    super::pattern_extract::drop_shadowed_dashless_holders(holders);
    seen.rebuild_copyrights_from(copyrights);
    seen.rebuild_holders_from(holders);

    let mut new_h =
        super::pattern_extract::extract_initials_holders_from_copyrights(&copyrights[..]);
    seen.dedup_new_holders(&mut new_h, 0);
    holders.extend(new_h);

    super::pattern_extract::strip_trailing_the_source_suffixes(copyrights);
    super::pattern_extract::truncate_stichting_mathematisch_centrum_amsterdam_netherlands(
        copyrights, holders,
    );

    super::postprocess_transforms::strip_inc_suffix_from_holders_for_today_year_copyrights(
        copyrights, holders,
    );

    super::postprocess_transforms::apply_openoffice_org_report_builder_bin_normalizations(
        content, copyrights, holders,
    );
}

fn drop_placeholder_and_code_junk_by_raw_line(
    raw_lines: &[&str],
    copyrights: &mut Vec<CopyrightDetection>,
    holders: &mut Vec<HolderDetection>,
) {
    let is_junk_line = |line_number: LineNumber| {
        let idx = line_number.get().saturating_sub(1);
        let Some(raw) = raw_lines.get(idx) else {
            return false;
        };
        let trimmed = raw.trim();
        is_embedded_c_sign_code_fragment_line(trimmed)
            || is_copyright_edit_note_line(trimmed)
            || is_copyright_holder_placeholder_line(trimmed)
            || is_pattern_match_binding_line(trimmed)
            || is_notice_template_line(trimmed)
            || is_delimited_data_row_line(trimmed)
            || is_documentation_tag_reference_line(trimmed)
    };
    // The tagged type is the copyright field's own: on its line, or on the next one.
    let declares_tagged_type = |start: LineNumber| {
        raw_lines
            .iter()
            .skip(start.get().saturating_sub(1))
            .take(2)
            .any(|raw| is_tagged_type_field_declaration_line(raw.trim()))
    };

    copyrights.retain(|c| !is_junk_line(c.start_line) && !declares_tagged_type(c.start_line));
    holders.retain(|h| !is_junk_line(h.start_line) && !declares_tagged_type(h.start_line));
}

/// Every form a copyright marker takes at the head of a notice.
static COPYRIGHT_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bcopyright\b|\bcopr\.?|\(c\)|\u{a9}").expect("valid marker regex")
});

/// Whether the line is a positional data row: many short `;`-delimited fields,
/// as in a Unicode character-database entry, which makes a `copyright` word on
/// it a data cell rather than a notice. Every field has to read as a plain cell —
/// short, and free of the abbreviating periods of prose (`Inc.`) and of code
/// punctuation — which keeps a semicolon-separated notice and a banner on a
/// minified stylesheet out of scope, and a dated notice is never a data row
/// however many fields it is punctuated into.
fn is_delimited_data_row_line(line: &str) -> bool {
    const MIN_FIELDS: usize = 8;
    const MAX_FIELD_LEN: usize = 48;
    const CODE_PUNCTUATION: &[char] = &[
        '{', '}', '(', ')', '[', ']', '/', '\\', '*', '=', ':', '|', '"', '.',
    ];

    if has_copyright_year(line) {
        return false;
    }

    let mut fields = 0usize;
    for field in line.split(';') {
        let field = field.trim();
        if field.len() > MAX_FIELD_LEN || field.contains(CODE_PUNCTUATION) {
            return false;
        }
        fields += 1;
    }

    fields >= MIN_FIELDS
}

/// Whether the line names a documentation tag (`` `@copyright' `` in edoc,
/// javadoc, or doxygen prose) rather than asserting a notice. The tag is closed
/// off by its own quote or inline-code delimiter, so it carries no party name,
/// and the closing delimiter is what keeps an open quote that does introduce one
/// (`"@Copyright` above a chip label's party and year) out of scope.
///
/// A marker elsewhere on the line that does open on a party name is a real
/// notice, dated or not, and keeps the line — the tag alone has to account for
/// every notice-looking marker on it.
fn is_documentation_tag_reference_line(line: &str) -> bool {
    static QUOTED_DOC_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)["'`\u{2018}\u{201c}][@\\]copyright["'`\u{2019}\u{201d}]"#)
            .expect("valid doc tag regex")
    });

    QUOTED_DOC_TAG_RE.is_match(line)
        && !has_copyright_year(line)
        && !COPYRIGHT_MARKER_RE.find_iter(line).any(|marker| {
            !line[..marker.start()].ends_with(['@', '\\'])
                && opens_on_party_name(&line[marker.end()..])
        })
}

/// Whether the text after a marker opens on a capitalized party name, skipping
/// the bare `c` of a `(c)` sign and any punctuation between them.
fn opens_on_party_name(tail: &str) -> bool {
    tail.split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_alphanumeric()))
        .find(|word| !word.is_empty() && !word.eq_ignore_ascii_case("c"))
        .is_some_and(|word| word.starts_with(char::is_uppercase))
}

/// Whether the line declares a tagged type field (`[0] IMPLICIT SET OF`), the
/// ASN.1 notation that makes a neighboring `copyright` field name a schema
/// member rather than a notice.
fn is_tagged_type_field_declaration_line(line: &str) -> bool {
    static TAGGED_TYPE_FIELD_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\[\s*\d+\s*\]\s+(?:IMPLICIT|EXPLICIT)\b").expect("valid tagged type regex")
    });

    TAGGED_TYPE_FIELD_RE.is_match(line)
}

fn is_copyright_holder_placeholder_line(line: &str) -> bool {
    static COPYRIGHT_HOLDER_PLACEHOLDER_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ix)
            ^(?:
                [\p{L}0-9._-]+(?:'s)?\s+COPYRIGHT\s+HOLDER
                |
                copyright
                (?:\s*\(c\))?
                (?:\s+(?:19\d{2}|20\d{2})(?:-(?:19\d{2}|20\d{2}))?)?
                \s+[\p{L}0-9._-]+(?:'s)?\s+COPYRIGHT\s+HOLDER
            )$
            ",
        )
        .unwrap()
    });

    COPYRIGHT_HOLDER_PLACEHOLDER_RE.is_match(line.trim())
}

/// Whether the line shows copyright *templates* only: every copyright marker on
/// it has a `YYYY` placeholder in its year slot. Documentation that quotes the
/// required header reads that way, and the line — not the value — is what shows
/// it, because refinement strips the placeholder off the value
/// (`` `Copyright Acme AB YYYY.` `` yields the apparently real
/// `Copyright Acme AB`, whatever spacing or punctuation the source used).
///
/// Every marker must be a template: a detection records no column and so cannot
/// be tied to one marker, which means a line that also asserts a real notice —
/// dated or not — keeps all of its detections. The one marker that opens no
/// notice at all is `copyright notice`, a term of art naming the form rather
/// than claiming it.
fn is_notice_template_line(line: &str) -> bool {
    let mut saw_template = false;
    for marker in COPYRIGHT_MARKER_RE.find_iter(line) {
        let tail = &line[marker.end()..];
        if names_a_notice_form(tail) {
            continue;
        }
        if !year_slot_is_placeholder(tail) {
            return false;
        }
        saw_template = true;
    }

    saw_template
}

/// Whether the marker is the term of art `copyright notice(s)` — the phrase
/// refers to notices instead of asserting one, so it never supplies a party name.
fn names_a_notice_form(tail: &str) -> bool {
    tail.split_whitespace().next().is_some_and(|word| {
        matches!(
            word.trim_end_matches([',', ':', ';', '.']),
            "notice" | "notices"
        )
    })
}

/// Whether the year slot of the notice opening at a marker holds a `YYYY`
/// placeholder. The slot is the first year-like token in the run of party-name
/// tokens after the marker, and the run stops at the end of the notice: at a
/// lowercase prose word, or at a sentence or clause boundary. So `YYYY` in the
/// text following a notice never supplies that notice's year, whether the text
/// is lowercase (`Copyright Acme Corp. Release dates use the YYYY format.`),
/// title-cased (`... Corp. Release Dates Use YYYY`), or behind a semicolon or
/// colon (`... Corp; dates use YYYY`).
///
/// A fill-in slot written longhand collapses to `YYYY` before the walk and takes
/// the same positional path. Only two forms count: a substitution variable
/// (`[$date-of-software]`, `${year}`), which is text some tool is meant to
/// replace, and a slot that instructs the reader to supply the date
/// (`<insert year>`). A bare slot name (`<year>`, `{YEAR}`) is deliberately out
/// of scope — it is how a project writes its own credit line, and the party it
/// names is real.
fn year_slot_is_placeholder(tail: &str) -> bool {
    static DATE_FILL_IN_SLOT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ix)
            \$ \{? [\w.-]* \b (?: year | date ) \b [\w.-]*
            |
            [\[<{] [^\[\]<>{}]{0,20}
            \b (?: insert | enter | fill | specify | your ) \b
            [^\[\]<>{}]{0,20} \b (?: year | date ) \b [^\[\]<>{}]{0,20} [\]>}]
            ",
        )
        .expect("valid date fill-in slot regex")
    });
    static PLACEHOLDER_YEAR_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\by{4}\b").expect("valid placeholder year regex"));
    static PLAIN_YEAR_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b(?:19|20)\d{2}\b").expect("valid plain year regex"));

    let tail = DATE_FILL_IN_SLOT_RE.replace_all(tail, " YYYY ");

    let mut previous_ended_sentence = false;
    let mut previous_ended_clause = false;
    for token in tail.split_whitespace() {
        let opens_new_sentence =
            previous_ended_sentence && token.chars().next().is_some_and(char::is_uppercase);
        if previous_ended_clause || opens_new_sentence {
            return false;
        }
        if PLACEHOLDER_YEAR_RE.is_match(token) {
            return true;
        }
        if PLAIN_YEAR_RE.is_match(token) || !is_party_name_token(token) {
            return false;
        }
        // `Inc.`/`Ltd.` end a token without ending the notice, so a period only
        // closes the sentence when the next token opens one.
        previous_ended_sentence = token.ends_with('.');
        previous_ended_clause = token.ends_with([';', ':']);
    }

    false
}

/// Whether a token can belong to the party name between a marker and its year:
/// a capitalized word, a bare `c` from a `(c)` sign, or pure punctuation such as
/// a dash or `&`. Anything else — notably a lowercase prose word — is not part of
/// the notice's party name.
fn is_party_name_token(token: &str) -> bool {
    let word = token.trim_matches(|c: char| !c.is_alphanumeric());
    word.is_empty()
        || word.eq_ignore_ascii_case("c")
        || word.chars().next().is_some_and(char::is_uppercase)
}

/// Whether the line binds values with a pattern-match or short-declaration
/// operator (Erlang `#{<<"path">> := Path} = Copyright,`, Go `x := y`), which
/// makes a bare `Copyright` token on it a variable rather than a notice. A
/// quoted notice being assigned, or a year anywhere on the line, keeps it.
fn is_pattern_match_binding_line(line: &str) -> bool {
    line.contains(":=") && !has_quoted_copyright(line) && !has_copyright_year(line)
}

/// Whether one quoted segment of the line spells out a copyright notice, which
/// makes it assigned text rather than an identifier. A quoted key name is not
/// such a notice: Erlang's `~"copyrights" := Cs` binds a map key, and JSON-shaped
/// keys read the same way. Segments are tested one by one, so two neighbouring
/// strings cannot combine into a marker that neither of them contains.
fn has_quoted_copyright(line: &str) -> bool {
    quoted_segments(line).iter().any(|segment| {
        segment.to_ascii_lowercase().contains("copyright") && !is_bare_identifier(segment)
    })
}

/// Whether `s` is a lone identifier-shaped token — the shape of a map key or
/// struct field (`copyrights`, `copyright_notice`), never of a notice, which
/// always carries punctuation or a party name alongside its marker.
fn is_bare_identifier(s: &str) -> bool {
    let trimmed = s.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-'))
}

/// The quoted string literals of `line`, without their delimiters. Walking the
/// line tracks which delimiter opened the string and which characters are
/// escaped, so neither an escaped quote nor an escaped backslash before a real
/// one shifts the boundaries; an unterminated final string still yields its text.
fn quoted_segments(line: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut open_quote: Option<char> = None;
    let mut escaped = false;
    let mut current = String::new();
    for ch in line.chars() {
        if escaped {
            escaped = false;
            if open_quote.is_some() {
                current.push(ch);
            }
        } else if ch == '\\' {
            escaped = true;
        } else if matches!(ch, '"' | '\'') {
            match open_quote {
                Some(opener) if opener == ch => {
                    open_quote = None;
                    segments.push(std::mem::take(&mut current));
                }
                Some(_) => current.push(ch),
                None => open_quote = Some(ch),
            }
        } else if open_quote.is_some() {
            current.push(ch);
        }
    }
    if open_quote.is_some() {
        segments.push(current);
    }

    segments
}

fn is_embedded_c_sign_code_fragment_line(line: &str) -> bool {
    static EMBEDDED_C_SIGN_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b[A-Za-z_][A-Za-z0-9_]*\(\s*c\s*\)\s*(?:;|=|->)").unwrap()
    });

    EMBEDDED_C_SIGN_CALL_RE.is_match(line.trim())
}

fn is_copyright_edit_note_line(line: &str) -> bool {
    static COPYRIGHT_EDIT_NOTE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^copyright\s+sections?\s+were\s+added$").unwrap());

    COPYRIGHT_EDIT_NOTE_RE.is_match(line.trim())
}

fn run_final_variant_and_cleanup_repairs(
    raw_lines: &[&str],
    prepared_cache: &PreparedLines<'_>,
    copyrights: &mut Vec<CopyrightDetection>,
    holders: &mut Vec<HolderDetection>,
    seen: &mut SeenTextSets,
) {
    super::pattern_extract::drop_shadowed_bare_c_copyrights_same_span(copyrights);
    super::pattern_extract::drop_copyright_shadowed_by_bare_c_copyrights_same_span(copyrights);
    super::pattern_extract::drop_shadowed_copyright_c_years_only_prefixes(copyrights);
    super::pattern_extract::drop_non_copyright_like_copyrights(copyrights);

    super::postprocess_transforms::drop_wider_duplicate_holder_spans(holders);
    super::postprocess_transforms::drop_shadowed_multiline_prefix_copyrights(copyrights);
    super::postprocess_transforms::drop_shadowed_multiline_prefix_holders(holders);

    super::pattern_extract::drop_shadowed_prefix_copyrights(copyrights);
    super::postprocess_transforms::drop_combined_semicolon_shadowed_copyrights(copyrights);

    super::postprocess_transforms::drop_shadowed_for_clause_holders_with_email_copyrights(
        copyrights, holders,
    );

    super::postprocess_transforms::drop_shadowed_c_sign_variants(copyrights);
    super::postprocess_transforms::drop_shadowed_year_prefixed_holders(holders);

    super::postprocess_transforms::truncate_lonely_svox_baslerstr_address(copyrights, holders);
    let (new_c, new_h) = super::postprocess_transforms::add_short_svox_baslerstr_variants(
        &copyrights[..],
        &holders[..],
        seen,
    );
    copyrights.extend(new_c);
    holders.extend(new_h);

    super::postprocess_transforms::drop_shadowed_year_only_copyright_prefixes_same_start_line(
        copyrights,
    );
    super::postprocess_transforms::drop_year_only_copyrights_shadowed_by_previous_software_copyright_line(
        raw_lines,
        prepared_cache,
        copyrights,
    );

    let new_c =
        super::postprocess_transforms::add_embedded_copyright_clause_variants(&copyrights[..]);
    copyrights.extend(new_c);
    let (new_c, new_h) =
        super::postprocess_transforms::add_found_at_short_variants(&copyrights[..], &holders[..]);
    copyrights.extend(new_c);
    holders.extend(new_h);
    super::postprocess_transforms::drop_shadowed_linux_foundation_holder_copyrights_same_line(
        copyrights,
    );
    let new_c = super::postprocess_transforms::add_bare_email_variants_for_escaped_angle_lines(
        raw_lines,
        &copyrights[..],
    );
    copyrights.extend(new_c);
    super::postprocess_transforms::drop_comma_holders_shadowed_by_space_version_same_span(holders);
    super::postprocess_transforms::normalize_company_suffix_period_holder_variants(holders);
    let (new_c, new_h) = super::postprocess_transforms::add_confidential_short_variants_late(
        &copyrights[..],
        &holders[..],
    );
    copyrights.extend(new_c);
    holders.extend(new_h);
    let (new_c, new_h) = super::postprocess_transforms::add_karlsruhe_university_short_variants(
        &copyrights[..],
        &holders[..],
    );
    copyrights.extend(new_c);
    holders.extend(new_h);
    let new_c = super::postprocess_transforms::add_intel_and_sun_non_portions_variants(
        prepared_cache,
        &copyrights[..],
    );
    copyrights.extend(new_c);
    let new_c = super::postprocess_transforms::add_pipe_read_parenthetical_variants(
        prepared_cache,
        &copyrights[..],
    );
    copyrights.extend(new_c);
    let new_c = super::postprocess_transforms::add_from_url_parenthetical_copyright_variants(
        prepared_cache,
        &copyrights[..],
    );
    copyrights.extend(new_c);
    let (new_c, new_h) = super::postprocess_transforms::add_at_affiliation_short_variants(
        &copyrights[..],
        &holders[..],
    );
    copyrights.extend(new_c);
    holders.extend(new_h);
    let new_c = super::postprocess_transforms::add_but_suffix_short_variants(&copyrights[..]);
    copyrights.extend(new_c);
    let new_c = super::postprocess_transforms::add_missing_copyrights_for_holder_lines_with_emails(
        prepared_cache,
        &copyrights[..],
        &holders[..],
    );
    copyrights.extend(new_c);
    super::postprocess_transforms::extend_inline_obfuscated_angle_email_suffixes(
        prepared_cache,
        copyrights,
    );
    super::postprocess_transforms::strip_lone_obfuscated_angle_email_user_tokens(
        raw_lines, copyrights, holders,
    );
    let new_c = super::postprocess_transforms::add_at_domain_variants_for_short_net_angle_emails(
        prepared_cache,
        &copyrights[..],
    );
    copyrights.extend(new_c);

    super::postprocess_transforms::dedupe_exact_span_copyrights(copyrights);
    super::postprocess_transforms::dedupe_exact_span_holders(holders);

    super::postprocess_transforms::normalize_french_support_disclaimer_copyrights(
        copyrights, holders,
    );
    super::postprocess_transforms::drop_shadowed_inria_location_copyrights_same_span(copyrights);
    let extra_holders =
        super::postprocess_transforms::add_email_holders_from_leading_email_comma_holders(holders);
    holders.extend(extra_holders);
    super::postprocess_transforms::dedupe_exact_span_holders(holders);
    super::postprocess_transforms::drop_shadowed_email_comma_holders_same_span(holders);
    super::postprocess_transforms::drop_shadowed_plain_email_prefix_copyrights_same_span(
        copyrights,
    );
    super::postprocess_transforms::drop_single_line_copyrights_shadowed_by_multiline_same_start(
        copyrights,
    );
    super::postprocess_transforms::restore_url_slash_before_closing_paren_from_raw_lines(
        raw_lines, copyrights,
    );
    super::postprocess_transforms::add_missing_holders_from_preceding_name_lines(
        prepared_cache,
        copyrights,
        holders,
    );
    let new_c = super::postprocess_transforms::add_first_angle_email_only_variants(&copyrights[..]);
    copyrights.extend(new_c);
    super::postprocess_transforms::drop_shadowed_angle_email_prefix_copyrights_same_span(
        copyrights,
    );
    super::postprocess_transforms::drop_shadowed_quote_before_email_variants_same_span(copyrights);
    super::postprocess_transforms::drop_url_embedded_suffix_variants_same_span(copyrights, holders);
    if let Some(h) = super::postprocess_transforms::add_missing_holder_from_single_copyright(
        &copyrights[..],
        &holders[..],
    ) {
        holders.push(h);
    }

    super::postprocess_transforms::dedupe_exact_span_copyrights(copyrights);
    super::postprocess_transforms::dedupe_exact_span_holders(holders);

    super::postprocess_transforms::drop_shadowed_acronym_location_suffix_copyrights_same_span(
        copyrights,
    );
    super::postprocess_transforms::split_multiline_holder_lists_from_copyright_email_sequences(
        copyrights, holders,
    );
    super::postprocess_transforms::drop_json_description_metadata_copyrights_and_holders(
        raw_lines, copyrights, holders,
    );
    super::postprocess_transforms::drop_quoted_inline_notice_examples(
        raw_lines, copyrights, holders,
    );
    super::postprocess_transforms::drop_markup_declaration_and_versioninfo_copyrights_and_holders(
        raw_lines, copyrights, holders,
    );
    super::postprocess_transforms::drop_section_heading_multiline_copyrights_shadowed_by_final_line(
        raw_lines, copyrights,
    );
    super::postprocess_transforms::drop_copyright_like_holders(holders);
    drop_placeholder_and_code_junk_by_raw_line(raw_lines, copyrights, holders);
}

// Copyright postprocess phase entry point; the long argument list threads the shared detection-pipeline state.
#[allow(clippy::too_many_arguments)]
/// Runs the postprocess repairs, skipping the extraction steps once `deadline`
/// has passed.
///
/// The final cleanup runs either way. It is what removes the false positives the
/// earlier steps emit, so skipping it would leave output dirtier than a complete
/// run rather than merely smaller.
///
/// Bounds accumulated work between steps only. A single step that never returns
/// stays unbounded — a cooperative deadline cannot preempt one — so this is not
/// protection against a hang.
pub(in super::super) fn run_phase_postprocess(
    content: &str,
    raw_lines: &[&str],
    prepared_cache: &PreparedLines<'_>,
    did_expand_href: bool,
    copyrights: &mut Vec<CopyrightDetection>,
    holders: &mut Vec<HolderDetection>,
    authors: &mut Vec<AuthorDetection>,
    seen: &mut SeenTextSets,
    deadline: Option<Instant>,
) {
    run_initial_detection_repairs(content, prepared_cache, copyrights, holders, seen);

    if !super::postprocess_transforms::deadline_exceeded(deadline) {
        run_author_extraction_and_repairs(
            content,
            raw_lines,
            prepared_cache,
            copyrights,
            holders,
            authors,
            seen,
        );
    }

    if !super::postprocess_transforms::deadline_exceeded(deadline) {
        run_mid_pipeline_repairs(
            content,
            raw_lines,
            prepared_cache,
            did_expand_href,
            copyrights,
            holders,
            authors,
            seen,
        );
    }

    if !super::postprocess_transforms::deadline_exceeded(deadline) {
        run_late_pattern_extractions(content, prepared_cache, copyrights, holders, seen);
    }

    run_final_variant_and_cleanup_repairs(raw_lines, prepared_cache, copyrights, holders, seen);
}
