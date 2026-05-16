// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

/// Registered detection-surface metadata for auto-generating documentation.
///
/// This module provides the `ParserMetadata` type used by parser `metadata()`
/// trait methods and by `bin/generate_supported_formats.rs` to automatically
/// generate `docs/SUPPORTED_FORMATS.md`.
///
/// Fields are used by the xtask but not in library code,
/// so we allow dead_code warnings for library builds.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParserMetadata {
    /// Human-readable description (e.g., "npm package.json manifest")
    pub description: &'static str,
    /// File patterns this parser matches (e.g., ["**/package.json"])
    pub file_patterns: &'static [&'static str],
    /// Package type identifier (e.g., "npm", "pypi", "maven")
    pub package_type: &'static str,
    /// Primary programming language (e.g., "JavaScript", "Python")
    pub primary_language: &'static str,
    /// Optional documentation URL
    pub documentation_url: Option<&'static str>,
}
