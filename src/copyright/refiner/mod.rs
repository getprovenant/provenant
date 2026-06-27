// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Refinement and cleanup functions for detected copyright strings.
//!
//! After the parser produces raw detection text from parse tree nodes,
//! these functions clean up artifacts: strip junk prefixes/suffixes,
//! normalize whitespace, remove duplicate copyright words, strip
//! unbalanced parentheses, and filter out known junk patterns.
//!
//! This module is decomposed into cohesive submodules. Every submodule does
//! `use super::*;` and `mod.rs` re-globs each submodule back into this module's
//! namespace, so the historic flat namespace is preserved exactly: any item
//! defined in any submodule is reachable from every other submodule and from
//! `super::*` consumers such as `author.rs`, `utils.rs`, and `tests`.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::candidates::strip_balanced_edge_parens;
use super::prepare::prepare_text_line;

// Pattern-data tables (curated regex lists). Shared by junk detection and the
// constant sets, and re-globbed below so `use super::*;` resolves them.
mod authors_junk_patterns;
mod copyrights_junk_patterns;
mod holders_junk_patterns;

use authors_junk_patterns::AUTHORS_JUNK_PATTERNS;
use copyrights_junk_patterns::COPYRIGHTS_JUNK_PATTERNS;
use holders_junk_patterns::HOLDERS_JUNK_PATTERNS;

// Cohesive refinement submodules. The private glob re-imports below make every
// `pub(super)` item visible to every sibling's `use super::*;`, preserving the
// original single-module namespace.
mod constants;
mod copyright;
mod copyright_clauses;
mod holder;
mod junk;

use constants::*;
use copyright::*;
use copyright_clauses::*;
use holder::*;
use junk::*;

mod author;
mod utils;

pub(crate) use author::looks_like_name_with_parenthesized_url;
pub use author::refine_author;
pub use utils::{
    remove_dupe_copyright_words, remove_some_extra_words_and_punct, strip_all_unbalanced_parens,
    strip_prefixes, strip_solo_quotes, strip_some_punct, strip_suffixes, strip_trailing_period,
};

// Public refinement entry points, re-exported from their owning submodules so
// existing `crate::copyright::refiner::*` import paths keep resolving.
pub use copyright::refine_copyright;
pub use holder::{refine_holder, refine_holder_in_copyright_context};
pub use junk::is_junk_copyright;
pub(crate) use junk::{
    has_copyright_year, is_junk_holder, is_path_like_code_fragment, is_tokenizer_data_fragment,
    looks_like_bpe_merges_table, looks_like_hf_tokenizer_json, looks_like_source_code,
};

/// Compile a static regex literal.
///
/// The argument is always a compile-time string literal, so a failure here is a
/// programming error in this crate (a malformed literal), not a condition that
/// can be triggered by scanned input. The first access panics with the offending
/// pattern; the refiner test suite exercises every pattern, so a bad literal
/// fails fast in CI rather than in production.
fn compile_static_regex(pattern: &'static str) -> Regex {
    Regex::new(pattern)
        .unwrap_or_else(|err| panic!("invalid static refiner regex `{pattern}`: {err}"))
}

#[cfg(test)]
use self::utils::{strip_leading_numbers, strip_unbalanced_parens};

use self::author::{normalize_angle_bracket_comma_spacing, strip_trailing_company_co_ltd};

use self::utils::{
    normalize_comma_spacing, normalize_whitespace, refine_names, remove_dupe_holder,
    strip_repeated_leading_holder_prefix, strip_trailing_incomplete_as_represented_by,
    strip_trailing_url, strip_trailing_url_slash, truncate_long_words,
};

#[cfg(test)]
mod tests;
