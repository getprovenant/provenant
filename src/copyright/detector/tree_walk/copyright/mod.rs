// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Tree-walk copyright/holder/author extraction.
//!
//! This module is split by tree-walk concern:
//!
//! - [`prefix`]: detection of copyright/portions/`not`-copyrighted prefix tokens
//!   that precede a copyright clause.
//! - [`node_classify`]: pure predicates that classify parse nodes (orphan
//!   boundaries, continuations, year-only clauses, nearby-signal lookahead).
//! - [`absorb`]: forward absorption of trailing orphan tokens and following
//!   copyright/year clauses onto a copyright node.
//! - [`merge`]: cross-node merges (copyright with a following author, year-only
//!   clause with a preceding `copyrighted by` clause).
//! - [`tree_nodes`]: the primary node-by-node tree walk driver.
//! - [`spans`]: flat-leaf span extraction over the whole token stream.
//! - [`bare`]: bare copyright and `holder is name` extraction passes.

// `absorb` is crate-visible so the tree-walk tests can reach
// `should_start_absorbing` / `collect_trailing_orphan_tokens` by path. Those two
// are not re-exported below because they have no non-test caller in the lib, and
// an unused re-export would trip `unused_imports`.
pub(crate) mod absorb;
mod bare;
mod merge;
mod node_classify;
mod prefix;
mod spans;
mod tree_nodes;

pub use bare::{extract_bare_copyrights, extract_holder_is_name};
pub use spans::{extract_copyrights_from_spans, extract_from_spans};
pub use tree_nodes::extract_from_tree_nodes;
