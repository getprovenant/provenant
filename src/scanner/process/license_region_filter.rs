// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Region-aware suppression of copyright/holder/author false positives that
//! originate from the legal prose of a detected full license text.
//!
//! Copyright detection runs before license detection in the per-file pipeline,
//! so it has no view of which line ranges are license text and happily extracts
//! attribution-shaped fragments from license boilerplate (for example
//! `MAKE NO REPRESENTATIONS`, `name of the author`, `Redistributions in binary
//! form`, or `Software Foundation` out of "Licensed to the Apache Software
//! Foundation"). ScanCode suppresses these because it is license-text-region
//! aware; this pass restores that behavior without reordering the pipeline.
//!
//! Once both detections have populated the [`FileInfo`], the line spans of
//! substantial license-body matches are known. The pass works in two steps:
//!
//! 1. A copyright entry inside such a region is dropped only when it has neither
//!    a real copyright-notice signal: a year, or a leading copyright marker
//!    (`Copyright`, `(c)`, `©`). A genuine notice begins with such a marker
//!    (`Copyright (c) Mort Bay Consulting Pty. Ltd.`), while license prose that
//!    merely mentions the word mid-sentence (`at the Copyright Holders option
//!    either a return of any price paid or`) does not, so the marker/year guard
//!    keeps real notices that sit at the top of (or inside) a LICENSE file.
//! 2. Holders and authors are bare entity names with no notice marker of their
//!    own, so they cannot be classified directly. Instead they are anchored to
//!    the copyrights that survived step 1: a holder/author whose span overlaps a
//!    preserved copyright is a real attribution and is kept; one that falls in a
//!    license region with no co-located preserved copyright is license prose
//!    (`MAKE NO REPRESENTATIONS`, `...and its contributors`) and is dropped.

use crate::copyright::has_copyright_year;
use crate::models::FileInfo;

/// Minimum matched token count for a license match to count as a full
/// license-text region. Substantial license bodies match well above this
/// (BSD-new bodies match ~100+ tokens, Apache-2.0 grant headers ~70), while
/// short tags and `see LICENSE` references match only a handful of tokens and
/// must not gate copyright suppression.
const MIN_LICENSE_BODY_TOKENS: usize = 40;

/// Inclusive 1-based line range.
#[derive(Clone, Copy)]
struct LineRange {
    start_line: usize,
    end_line: usize,
}

/// Drop copyright/holder/author findings that originate from license prose.
///
/// Copyrights are filtered by their own notice year; holders and authors are
/// then kept or dropped based on whether they co-locate with a copyright that
/// survived (so a real holder, which carries no year, is preserved alongside
/// its year-bearing copyright).
pub(super) fn suppress_license_text_region_parties(file_info: &mut FileInfo) {
    let regions = collect_license_text_regions(file_info);
    if regions.is_empty() {
        return;
    }

    file_info.copyrights.retain(|c| {
        !is_year_free_party_in_region(&regions, c.start_line.get(), c.end_line.get(), || {
            is_copyright_notice(c.normalized_text()) || is_copyright_notice(&c.copyright)
        })
    });

    // Holders/authors are anchored to the copyrights that survived the year
    // filter above. A bare entity name co-located with a kept copyright is a
    // real attribution; one in a license region with no kept copyright nearby
    // is license prose.
    let preserved_copyright_ranges: Vec<LineRange> = file_info
        .copyrights
        .iter()
        .map(|c| LineRange {
            start_line: c.start_line.get(),
            end_line: c.end_line.get(),
        })
        .collect();

    file_info.holders.retain(|h| {
        !is_unanchored_party_in_region(
            &regions,
            &preserved_copyright_ranges,
            h.start_line.get(),
            h.end_line.get(),
        )
    });
    file_info.authors.retain(|a| {
        !is_unanchored_party_in_region(
            &regions,
            &preserved_copyright_ranges,
            a.start_line.get(),
            a.end_line.get(),
        )
    });
}

fn collect_license_text_regions(file_info: &FileInfo) -> Vec<LineRange> {
    file_info
        .license_detections
        .iter()
        .flat_map(|detection| detection.matches.iter())
        .filter(|m| m.matched_length.unwrap_or(0) >= MIN_LICENSE_BODY_TOKENS)
        .map(|m| LineRange {
            start_line: m.start_line.get(),
            end_line: m.end_line.get(),
        })
        .collect()
}

/// Return true when `[start, end]` is fully contained in any license region and
/// the finding has no real copyright-notice signal. `has_notice_signal` is only
/// evaluated when the span is contained, so the (cheap) containment check short
/// circuits the (regex) year check for the common no-region case.
fn is_year_free_party_in_region(
    regions: &[LineRange],
    start: usize,
    end: usize,
    has_notice_signal: impl FnOnce() -> bool,
) -> bool {
    span_contained_in_any(regions, start, end) && !has_notice_signal()
}

/// Return true when `[start, end]` is inside a license region but does not
/// overlap any preserved copyright, i.e. it is an attribution-shaped fragment of
/// license prose rather than a holder/author tied to a real notice.
fn is_unanchored_party_in_region(
    regions: &[LineRange],
    preserved_copyright_ranges: &[LineRange],
    start: usize,
    end: usize,
) -> bool {
    span_contained_in_any(regions, start, end)
        && !preserved_copyright_ranges
            .iter()
            .any(|range| ranges_overlap(range.start_line, range.end_line, start, end))
}

/// Return true when `text` reads as a genuine copyright notice rather than
/// license prose: it either carries a copyright year or begins with a copyright
/// marker (`Copyright`, `Copr.`, `(c)`, `©`). Real notices lead with the marker
/// (`Copyright (c) Mort Bay Consulting Pty. Ltd.`); prose that mentions the word
/// mid-sentence (`at the Copyright Holders option ...`) does not.
fn is_copyright_notice(text: &str) -> bool {
    has_copyright_year(text) || starts_with_copyright_marker(text)
}

fn starts_with_copyright_marker(text: &str) -> bool {
    let trimmed = text.trim_start();
    if trimmed.starts_with(['©', '\u{2117}']) {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("copyright") || lower.starts_with("copr.") || lower.starts_with("(c)")
}

fn span_contained_in_any(regions: &[LineRange], start: usize, end: usize) -> bool {
    regions
        .iter()
        .any(|region| region.start_line <= start && end <= region.end_line)
}

fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start <= b_end && b_start <= a_end
}

#[cfg(test)]
#[path = "license_region_filter_test.rs"]
mod tests;
