// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Type definitions for the copyright grammar: the matcher and rule types used
//! to express grammar rules as Rust data. Rules are applied bottom-up to a
//! sequence of POS-tagged tokens; each rule matches a pattern of tags/labels
//! and replaces the matched span with a new tree node.
//!
//! The grammar rules themselves — ported from the Python `GRAMMAR` string in
//! `reference/scancode-toolkit/src/cluecode/copyrights.py` — live in
//! `rules.rs`, which carries the corresponding upstream attribution.

use crate::copyright::types::{PosTag, TreeLabel};

/// A matcher for a single position in a grammar rule pattern.
#[derive(Debug, Clone)]
pub(crate) enum TagMatcher {
    /// Match a specific POS tag on a leaf token.
    Tag(PosTag),
    /// Match a specific tree label on a tree node.
    Label(TreeLabel),
    /// Match any of several POS tags.
    AnyTag(&'static [PosTag]),
    /// Match any of several tree labels.
    AnyLabel(&'static [TreeLabel]),
    /// Match any of several tags OR labels.
    AnyTagOrLabel(&'static [PosTag], &'static [TreeLabel]),
}

/// A grammar rule: matches a pattern and produces a tree node with the given label.
#[derive(Debug, Clone)]
pub(crate) struct GrammarRule {
    /// The label for the tree node produced by this rule.
    pub(crate) label: TreeLabel,
    /// The pattern to match (sequence of matchers).
    pub(crate) pattern: &'static [TagMatcher],
}
