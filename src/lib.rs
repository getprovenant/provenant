// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! # Provenant
//!
//! `provenant` is the library crate behind the `provenant` CLI. It
//! provides ScanCode-compatible scanning, package parsing, and output-writing
//! building blocks for Rust applications.
//!
//! The main entry points are:
//!
//! - [`workflow::scan_path`] and [`workflow::scan_paths`] for the supported high-level embedding flow
//! - [`collect_paths`] to discover files in a directory tree when you need lower-level control
//! - [`process_collected`] to scan collected files in parallel when you are assembling the pipeline manually
//! - [`OutputFormat`], [`OutputWriter`], and [`write_output_file`] to serialize scan results
//! - [`parsers`] and [`models`] for lower-level package parsing and result inspection
//!
//! High-level crate organization:
//!
//! - [`scanner`] orchestrates traversal, filtering, and scan execution
//! - [`license_detection`] extracts license information from files
//! - [`parsers`] extracts package metadata from ecosystem-specific inputs
//! - [`copyright`] and [`finder`] extract text clues such as copyrights, emails, and URLs
//! - [`output`] renders ScanCode-compatible and SBOM-oriented output formats
//! - [`models`] defines the core scan result data structures
//!
//! User-facing installation, CLI usage, supported format coverage, and broader
//! architecture notes live in the repository documentation. The crate-level
//! rustdoc stays intentionally concise so fast-changing project details have a
//! single source of truth outside this file.

extern crate self as provenance;

pub(crate) mod app;
pub mod assembly;
pub mod cache;
pub mod cli;
pub(crate) mod compare;
#[doc(hidden)]
pub mod compare_driver_shared;
#[doc(hidden)]
pub mod compare_normalization;
pub mod copyright;
pub mod finder;
pub mod golden_maintenance;
pub mod license_detection;
pub mod models;
pub mod output;
pub mod output_schema;
pub mod parsers;
#[cfg(feature = "golden-tests")]
pub mod post_processing;
#[cfg(not(feature = "golden-tests"))]
pub(crate) mod post_processing;
pub mod progress;
pub(crate) mod scan_result_shaping;
pub mod scanner;
pub(crate) mod serve;
#[doc(hidden)]
pub mod serve_api;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod time;
pub mod utils;
pub mod version;
pub mod workflow;

pub use cli::ProcessMode;
pub use models::{ExtraData, FileInfo, FileType, Header, Output, SystemEnvironment};
pub use output::{
    OutputFormat, OutputWriteConfig, OutputWriter, write_output_file, writer_for_format,
};
pub use parsers::{NpmParser, PackageParser};
pub use progress::{ProgressMode, ScanProgress};
pub use scanner::{
    CollectedPaths, ProcessResult, TextDetectionOptions, collect_paths, process_collected,
};
