// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
#[path = "../author_heuristics_test.rs"]
mod tests;

mod cleanup;
mod extraction;
mod markdown_roster;
mod pod_sections;

pub(super) use cleanup::*;
pub(super) use extraction::*;
pub(super) use markdown_roster::extract_markdown_contributor_roster_authors;
pub(crate) use pod_sections::is_pod_author_heading;
pub(super) use pod_sections::{
    extract_pod_author_section_contact_authors, extract_pod_author_section_contactless_authors,
    extract_pod_author_section_narrative_credit_authors,
};
