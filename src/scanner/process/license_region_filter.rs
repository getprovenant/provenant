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
//! substantial license-body matches are known. A copyright, holder, or author
//! finding is dropped only when its span lies entirely inside such a region and
//! it carries no real copyright-notice signal (a year). The year guard is what
//! keeps a genuine `Copyright (c) 2020 Acme, Inc.` notice that sits at the top
//! of (or even inside) a LICENSE file, since real notices carry a year while
//! license prose does not.

use crate::copyright::has_copyright_year;
use crate::models::FileInfo;

/// Minimum matched token count for a license match to count as a full
/// license-text region. Substantial license bodies match well above this
/// (BSD-new bodies match ~100+ tokens, Apache-2.0 grant headers ~70), while
/// short tags and `see LICENSE` references match only a handful of tokens and
/// must not gate copyright suppression.
const MIN_LICENSE_BODY_TOKENS: usize = 40;

/// Inclusive 1-based line range covered by a full-license-text match.
#[derive(Clone, Copy)]
struct LicenseRegion {
    start_line: usize,
    end_line: usize,
}

/// Drop copyright, holder, and author findings whose spans fall inside detected
/// full-license-text regions and that lack a real copyright-notice year.
pub(super) fn suppress_license_text_region_parties(file_info: &mut FileInfo) {
    let regions = collect_license_text_regions(file_info);
    if regions.is_empty() {
        return;
    }

    file_info.copyrights.retain(|c| {
        !is_license_prose_party(&regions, c.start_line.get(), c.end_line.get(), || {
            has_copyright_year(c.normalized_text()) || has_copyright_year(&c.copyright)
        })
    });
    file_info.holders.retain(|h| {
        !is_license_prose_party(&regions, h.start_line.get(), h.end_line.get(), || {
            has_copyright_year(&h.holder)
        })
    });
    file_info.authors.retain(|a| {
        !is_license_prose_party(&regions, a.start_line.get(), a.end_line.get(), || {
            has_copyright_year(&a.author)
        })
    });
}

fn collect_license_text_regions(file_info: &FileInfo) -> Vec<LicenseRegion> {
    file_info
        .license_detections
        .iter()
        .flat_map(|detection| detection.matches.iter())
        .filter(|m| m.matched_length.unwrap_or(0) >= MIN_LICENSE_BODY_TOKENS)
        .map(|m| LicenseRegion {
            start_line: m.start_line.get(),
            end_line: m.end_line.get(),
        })
        .collect()
}

/// Return true when `[start, end]` is fully contained in any license region and
/// the finding has no real copyright-notice signal. `has_notice_signal` is only
/// evaluated when the span is contained, so the (cheap) containment check short
/// circuits the (regex) year check for the common no-region case.
fn is_license_prose_party(
    regions: &[LicenseRegion],
    start: usize,
    end: usize,
    has_notice_signal: impl FnOnce() -> bool,
) -> bool {
    let contained = regions
        .iter()
        .any(|region| region.start_line <= start && end <= region.end_line);
    contained && !has_notice_signal()
}

#[cfg(test)]
#[path = "license_region_filter_test.rs"]
mod tests;
