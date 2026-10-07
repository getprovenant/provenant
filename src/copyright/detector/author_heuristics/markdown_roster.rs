// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Markdown contributor rosters: a contributor heading followed by a bullet
//! list of `[Name](contact)` links, one author per entry.

use std::sync::LazyLock;

use regex::Regex;

use crate::copyright::refiner::refine_author;
use crate::copyright::types::AuthorDetection;
use crate::models::LineNumber;

const MAX_ROSTER_ENTRIES: usize = 500;
const MAX_BLANK_LINES_AFTER_HEADING: usize = 2;

pub(in super::super) fn extract_markdown_contributor_roster_authors(
    raw_lines: &[&str],
) -> Vec<AuthorDetection> {
    static HEADING_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s{0,3}#{1,6}\s+.*\b(?:contributors?|authors|credits)\b")
            .expect("valid roster heading regex")
    });
    static LINK_DESTINATION_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\]\([^()]*\)").expect("valid link destination regex"));
    static ENTRY_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^\s*[*+-]\s+\[(?P<name>[^\[\]]{1,80})\]\((?P<contact>(?:https?://|mailto:)[^()\s]{1,200})\)[.,;]?\s*$",
        )
        .expect("valid roster entry regex")
    });
    static PERSON_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\p{Lu}[\p{L}'.-]*(?:\s+\p{Lu}[\p{L}'.-]*){1,3}$")
            .expect("valid person name regex")
    });

    let in_fence = fenced_code_lines(raw_lines);
    let mut authors = Vec::new();
    let mut idx = 0;
    while idx < raw_lines.len() {
        if in_fence[idx]
            || !raw_lines[idx].trim_start().starts_with('#')
            || !HEADING_RE.is_match(&LINK_DESTINATION_RE.replace_all(raw_lines[idx], "]"))
        {
            idx += 1;
            continue;
        }

        let mut cursor = idx + 1;
        let mut blanks = 0;
        while cursor < raw_lines.len() && raw_lines[cursor].trim().is_empty() {
            blanks += 1;
            cursor += 1;
        }
        if blanks > MAX_BLANK_LINES_AFTER_HEADING {
            idx = cursor;
            continue;
        }

        let mut entries = Vec::new();
        while cursor < raw_lines.len() && entries.len() < MAX_ROSTER_ENTRIES {
            let Some(cap) = ENTRY_RE.captures(raw_lines[cursor]) else {
                break;
            };
            let name = cap["name"].trim();
            if PERSON_NAME_RE.is_match(name) {
                entries.push((cursor, name.to_string(), cap["contact"].to_string()));
            }
            cursor += 1;
        }

        if entries.len() >= 2 {
            for (line_idx, name, contact) in entries {
                let candidate = format!("{name} ({contact})");
                let author = refine_author(&candidate).or_else(|| {
                    refine_author(&name).map(|refined| format!("{refined} ({contact})"))
                });
                if let Some(author) = author {
                    let line = LineNumber::from_0_indexed(line_idx);
                    authors.push(AuthorDetection {
                        author,
                        start_line: line,
                        end_line: line,
                    });
                }
            }
        }
        idx = cursor.max(idx + 1);
    }

    authors
}

fn fenced_code_lines(raw_lines: &[&str]) -> Vec<bool> {
    let mut open: Option<(char, usize)> = None;
    raw_lines
        .iter()
        .map(|line| {
            let trimmed = line.trim_start();
            let fence_char = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'));
            let fence_len =
                fence_char.map_or(0, |c| trimmed.chars().take_while(|&x| x == c).count());
            let is_fence = line.len() - trimmed.len() <= 3 && fence_len >= 3;
            match (open, fence_char) {
                (Some((c, len)), Some(fc)) if is_fence && fc == c && fence_len >= len => {
                    open = None;
                    true
                }
                (Some(_), _) => true,
                (None, Some(fc)) if is_fence => {
                    open = Some((fc, fence_len));
                    true
                }
                (None, _) => false,
            }
        })
        .collect()
}
