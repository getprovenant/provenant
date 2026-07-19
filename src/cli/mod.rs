// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

mod run;

pub use run::run;

use clap::{ArgGroup, Args, Parser, Subcommand};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use std::ffi::OsString;
use std::fs;
#[cfg(test)]
use std::ops::Deref;
use std::path::{Path, PathBuf};
use yaml_serde::Value as YamlValue;

use crate::app::request::{InputMode, OutputTarget, ScanRequest};
use crate::license_detection::DEFAULT_LICENSEDB_URL_TEMPLATE;
use crate::output::OutputFormat;
use crate::scanner::MemoryMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessMode {
    Parallel(usize),
    SequentialWithTimeouts,
    SequentialWithoutTimeouts,
}

impl Default for ProcessMode {
    fn default() -> Self {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        if cpus > 1 {
            ProcessMode::Parallel(cpus - 1)
        } else {
            ProcessMode::Parallel(1)
        }
    }
}

impl ProcessMode {
    fn default_value() -> Self {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        if cpus > 1 {
            ProcessMode::Parallel(cpus - 1)
        } else {
            ProcessMode::Parallel(1)
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            ProcessMode::Parallel(n) => n as i32,
            ProcessMode::SequentialWithTimeouts => 0,
            ProcessMode::SequentialWithoutTimeouts => -1,
        }
    }
}

impl std::fmt::Display for ProcessMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_i32())
    }
}

fn parse_processes(value: &str) -> Result<ProcessMode, String> {
    let parsed: i32 = value
        .parse()
        .map_err(|e| format!("invalid integer for --processes: {e}"))?;
    if parsed > 0 {
        Ok(ProcessMode::Parallel(
            u32::try_from(parsed).unwrap() as usize
        ))
    } else if parsed == 0 {
        Ok(ProcessMode::SequentialWithTimeouts)
    } else {
        Ok(ProcessMode::SequentialWithoutTimeouts)
    }
}

const PDF_OXIDE_LOG_HELP: &str = "Troubleshooting PDF parser logs:\n  Provenant suppresses noisy pdf_oxide logs by default.\n  To inspect raw pdf_oxide logs for debugging, rerun with RUST_LOG=pdf_oxide=warn (or =error).";
const CLI_ABOUT: &str = "Rust scanner for ScanCode-compatible workflows. Not affiliated with, endorsed by, or sponsored by ScanCode Toolkit, AboutCode, or nexB Inc.";
const CLI_LONG_ABOUT: &str = "Rust scanner for ScanCode-compatible workflows.\n\nNot affiliated with, endorsed by, or sponsored by ScanCode Toolkit, AboutCode, or nexB Inc.";

fn parse_license_policy_arg(value: &str) -> Result<String, String> {
    let policy_path = Path::new(value);
    let metadata = fs::metadata(policy_path).map_err(|err| {
        format!(
            "Failed to read license policy file {:?}: {err}",
            policy_path
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "License policy path {:?} is not a regular file",
            policy_path
        ));
    }

    let policy_text = fs::read_to_string(policy_path).map_err(|err| {
        format!(
            "Failed to read license policy file {:?}: {err}",
            policy_path
        )
    })?;
    if policy_text.trim().is_empty() {
        return Err(format!("License policy file {:?} is empty", policy_path));
    }

    let policy_value: YamlValue = yaml_serde::from_str(&policy_text).map_err(|err| {
        format!(
            "Failed to parse license policy file {:?}: {err}",
            policy_path
        )
    })?;
    let has_license_policies = policy_value
        .as_mapping()
        .and_then(|mapping| mapping.get(YamlValue::String("license_policies".to_string())))
        .is_some();
    if !has_license_policies {
        return Err(format!(
            "License policy file {:?} is missing a 'license_policies' attribute",
            policy_path
        ));
    }

    Ok(value.to_string())
}

#[derive(Parser, Debug)]
#[command(
    author = "The Provenant contributors",
    version = crate::version::BUILD_VERSION,
    long_version = crate::version::build_long_version(),
    after_help = PDF_OXIDE_LOG_HELP,
    about = CLI_ABOUT,
    long_about = CLI_LONG_ABOUT,
    arg_required_else_help = true,
    subcommand_required = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Scan files or existing ScanCode-style JSON inputs.
    Scan(Box<ScanArgs>),
    /// Run the long-lived HTTP service.
    Serve(ServeArgs),
    /// Compare ScanCode and Provenant JSON outputs to review migration-confidence deltas.
    Compare(CompareArgs),
    /// Show attribution notices for embedded license detection data.
    ShowAttribution,
    /// Export the effective built-in license dataset to DIR and exit.
    ExportLicenseDataset(ExportLicenseDatasetArgs),
}

/// Requested output verbosity for a subcommand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// Warnings and errors only.
    Quiet,
    /// Progress and informational output (the default).
    #[default]
    Normal,
    /// Everything, including debug-level diagnostics.
    Verbose,
}

impl Verbosity {
    /// Default global log level for this verbosity. `RUST_LOG` still overrides.
    pub fn log_level(self) -> log::LevelFilter {
        match self {
            Self::Quiet => log::LevelFilter::Warn,
            Self::Normal => log::LevelFilter::Info,
            Self::Verbose => log::LevelFilter::Debug,
        }
    }
}

/// Reusable `-q/-v` verbosity flags, flattened into subcommands other than
/// `scan` (which has its own richer progress modes tied to its output header).
#[derive(Args, Debug, Clone, Default)]
pub struct VerbosityFlags {
    /// Suppress progress and informational output; show warnings and errors only.
    #[arg(short = 'q', long = "quiet", conflicts_with = "verbose")]
    pub quiet: bool,

    /// Emit verbose diagnostics, including debug-level logging.
    #[arg(short = 'v', long = "verbose", conflicts_with = "quiet")]
    pub verbose: bool,
}

impl VerbosityFlags {
    pub fn verbosity(&self) -> Verbosity {
        if self.quiet {
            Verbosity::Quiet
        } else if self.verbose {
            Verbosity::Verbose
        } else {
            Verbosity::Normal
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct CompareArgs {
    /// Path to an existing ScanCode JSON output file.
    #[arg(long = "scancode-json", value_name = "PATH")]
    pub scancode_json: PathBuf,

    /// Path to an existing Provenant JSON output file.
    #[arg(long = "provenant-json", value_name = "PATH")]
    pub provenant_json: PathBuf,

    /// Directory where comparison artifacts should be written. Defaults to a timestamped directory in the current working directory.
    #[arg(long = "artifact-dir", value_name = "DIR")]
    pub artifact_dir: Option<PathBuf>,

    #[command(flatten)]
    pub verbosity: VerbosityFlags,
}

#[derive(Args, Debug, Clone)]
pub struct ExportLicenseDatasetArgs {
    #[arg(value_name = "DIR")]
    pub dir: String,

    #[command(flatten)]
    pub verbosity: VerbosityFlags,
}

/// Minimum license-policy compliance severity that fails the build (`--fail-on`).
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailOn {
    Warning,
    Error,
}

impl FailOn {
    /// Lowest `ComplianceAlert` severity that should trip the gate.
    pub fn threshold(self) -> crate::models::ComplianceAlert {
        match self {
            Self::Warning => crate::models::ComplianceAlert::Warning,
            Self::Error => crate::models::ComplianceAlert::Error,
        }
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompatibilityMode {
    #[default]
    Native,
    Scancode,
}

impl CompatibilityMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Scancode => "scancode",
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// Bind the service shell to HOST:PORT.
    #[arg(long = "bind", value_name = "ADDR", default_value = "127.0.0.1:8080")]
    pub bind: String,

    /// Allow paths, URL, and repository inputs when bound beyond localhost.
    #[arg(long = "allow-privileged-inputs")]
    pub allow_privileged_inputs: bool,

    #[command(flatten)]
    pub verbosity: VerbosityFlags,
}

#[derive(Args, Debug, Clone)]
#[command(
    group(
        ArgGroup::new("output")
            .required(true)
            .multiple(true)
            .args([
                "output_json",
                "output_json_pp",
                "output_json_lines",
                "output_yaml",
                "output_debian",
                "output_html",
                "output_spdx_tv",
                "output_spdx_rdf",
                "output_cyclonedx",
                "output_cyclonedx_xml",
                "custom_output"
            ])
    ),
    after_help = PDF_OXIDE_LOG_HELP
)]
pub struct ScanArgs {
    /// File or directory paths to scan
    #[arg(required = false)]
    pub dir_path: Vec<String>,

    /// Write scan output as compact JSON to FILE
    #[arg(long = "json", value_name = "FILE", allow_hyphen_values = true)]
    pub output_json: Option<String>,

    /// Write scan output as pretty-printed JSON to FILE
    #[arg(long = "json-pp", value_name = "FILE", allow_hyphen_values = true)]
    pub output_json_pp: Option<String>,

    /// Write scan output as JSON Lines to FILE
    #[arg(long = "json-lines", value_name = "FILE", allow_hyphen_values = true)]
    pub output_json_lines: Option<String>,

    /// Write scan output as YAML to FILE
    #[arg(long = "yaml", value_name = "FILE", allow_hyphen_values = true)]
    pub output_yaml: Option<String>,

    /// Write scan output in machine-readable Debian copyright format to FILE (requires --license, --copyright, and --license-text)
    #[arg(
        long = "debian",
        value_name = "FILE",
        allow_hyphen_values = true,
        requires_all = ["copyright", "license", "license_text"]
    )]
    pub output_debian: Option<String>,

    /// Write scan output as HTML report to FILE
    #[arg(long = "html", value_name = "FILE", allow_hyphen_values = true)]
    pub output_html: Option<String>,

    /// Write scan output as SPDX tag/value to FILE
    #[arg(long = "spdx-tv", value_name = "FILE", allow_hyphen_values = true)]
    pub output_spdx_tv: Option<String>,

    /// Write scan output as SPDX RDF/XML to FILE
    #[arg(long = "spdx-rdf", value_name = "FILE", allow_hyphen_values = true)]
    pub output_spdx_rdf: Option<String>,

    /// Write scan output as CycloneDX JSON to FILE
    #[arg(long = "cyclonedx", value_name = "FILE", allow_hyphen_values = true)]
    pub output_cyclonedx: Option<String>,

    /// Write scan output as CycloneDX XML to FILE
    #[arg(
        long = "cyclonedx-xml",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    pub output_cyclonedx_xml: Option<String>,

    /// Write license-policy violations as SARIF 2.1.0 to FILE (needs --license-policy)
    #[arg(long = "sarif", value_name = "FILE", allow_hyphen_values = true)]
    pub output_sarif: Option<String>,

    /// Write scan output to FILE formatted with the custom template
    #[arg(
        long = "custom-output",
        value_name = "FILE",
        requires = "custom_template",
        allow_hyphen_values = true
    )]
    pub custom_output: Option<String>,

    /// Use this template FILE with --custom-output
    #[arg(
        long = "custom-template",
        value_name = "FILE",
        requires = "custom_output"
    )]
    pub custom_template: Option<String>,

    /// Maximum recursion depth (0 means no depth limit)
    #[arg(short, long, default_value = "0")]
    pub max_depth: usize,

    #[arg(short = 'n', long, default_value_t = ProcessMode::default_value(), value_parser = parse_processes, allow_hyphen_values = true)]
    pub processes: ProcessMode,

    #[arg(long, default_value_t = 120.0)]
    pub timeout: f64,

    #[arg(short, long, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Emit verbose diagnostics, including the per-phase timing breakdown and, on a TTY, per-file scan detail.
    #[arg(short, long, conflicts_with = "quiet")]
    pub verbose: bool,

    #[arg(long, conflicts_with = "full_root")]
    pub strip_root: bool,

    #[arg(long, conflicts_with = "strip_root")]
    pub full_root: bool,

    /// Exclude patterns (ScanCode-compatible alias: --ignore)
    #[arg(long = "exclude", visible_alias = "ignore", value_delimiter = ',')]
    pub exclude: Vec<String>,

    /// Include files matching PATTERN. Use `**` when you want recursion across directories.
    #[arg(long, value_delimiter = ',')]
    pub include: Vec<String>,

    /// Read selected scan paths from FILE (or '-' for stdin), relative to the explicit scan root.
    #[arg(long = "paths-file", value_name = "FILE", allow_hyphen_values = true)]
    pub paths_file: Vec<String>,

    #[arg(long = "cache-dir", value_name = "PATH")]
    pub cache_dir: Option<String>,

    #[arg(long = "cache-clear")]
    pub cache_clear: bool,

    #[arg(long = "incremental")]
    pub incremental: bool,

    /// Trust size + mtime for incremental reuse, skipping the content re-hash of
    /// unchanged files. Speeds up warm incremental re-scans at the cost of
    /// missing the rare edit that keeps the same size and mtime tick. Such a miss
    /// is not permanent: the next scan without this flag re-hashes and detects it.
    /// Default off keeps the paranoid full-hash check so scans stay reproducible.
    #[arg(long = "cache-trust-mtime", requires = "incremental")]
    pub cache_trust_mtime: bool,

    /// Maximum number of file and directory scan details kept in memory during
    /// the in-scan file-processing window before the rest spill to disk.
    /// Use 0 for unlimited memory or -1 for disk-only spill during the scan.
    /// This bounds only the in-scan working set, NOT total or peak process
    /// memory: assembly, summary, and output reconstitute the full result set
    /// after the scan, so peak RSS is not bounded by this flag.
    #[arg(
        long = "max-in-memory",
        value_name = "INT",
        default_value_t = MemoryMode::Limit(10000),
        value_parser = parse_max_in_memory,
        allow_hyphen_values = true
    )]
    pub max_in_memory: MemoryMode,

    /// Collect file information such as checksums, type hints, and source/script flags.
    #[arg(short = 'i', long)]
    pub info: bool,

    /// Load one or more existing ScanCode-style JSON scans instead of rescanning inputs.
    #[arg(long)]
    pub from_json: bool,

    /// Scan input for application package and dependency manifests, lockfiles and related data
    #[arg(short = 'p', long)]
    pub package: bool,

    /// Select a compatibility bundle for intentional Provenant-vs-ScanCode behavior differences.
    #[arg(
        long = "compat-mode",
        visible_alias = "compat",
        value_enum,
        default_value_t = CompatibilityMode::Native
    )]
    pub compat_mode: CompatibilityMode,

    /// Scan input for installed system package databases (RPM, dpkg, apk, etc.)
    #[arg(long = "system-package")]
    pub system_package: bool,

    /// Scan supported compiled Go and Rust binaries for embedded package metadata.
    #[arg(long = "package-in-compiled")]
    pub package_in_compiled: bool,

    /// Scan for system and application package data and skip license/copyright detection and top-level package creation.
    #[arg(
        long = "package-only",
        conflicts_with_all = ["license", "summary", "package", "system_package"]
    )]
    pub package_only: bool,

    /// Disable package assembly (merging related manifest/lockfiles into packages)
    #[arg(long)]
    pub no_assemble: bool,

    /// Path to a custom license dataset root containing manifest.json, rules/, and licenses/.
    /// If not specified, uses the built-in embedded license index.
    #[arg(
        long = "license-dataset-path",
        value_name = "PATH",
        requires = "license"
    )]
    pub license_dataset_path: Option<String>,

    /// Force rebuild of the license index cache, ignoring any existing cache.
    #[arg(long)]
    pub reindex: bool,

    /// Build the license index in memory for this run without reading or writing persistent cache files.
    #[arg(long = "no-license-index-cache")]
    pub no_license_index_cache: bool,

    /// Include matched text in license detection output
    #[arg(long = "license-text", requires = "license")]
    pub license_text: bool,

    #[arg(long = "license-text-diagnostics", requires = "license_text")]
    pub license_text_diagnostics: bool,

    #[arg(long = "license-diagnostics", requires = "license")]
    pub license_diagnostics: bool,

    #[arg(long = "unknown-licenses", requires = "license")]
    pub unknown_licenses: bool,

    /// Disable approximate sequence matching during `--license` detection.
    #[arg(long = "no-sequence-matching", requires = "license")]
    pub no_sequence_matching: bool,

    #[arg(
        long = "license-score",
        default_value_t = 0,
        requires = "license",
        value_parser = clap::value_parser!(u8).range(0..=100)
    )]
    pub license_score: u8,

    #[arg(
        long = "license-url-template",
        default_value = DEFAULT_LICENSEDB_URL_TEMPLATE,
        requires = "license"
    )]
    pub license_url_template: String,

    #[arg(long)]
    pub filter_clues: bool,

    #[arg(
        long = "ignore-author",
        value_name = "PATTERN",
        help = "Ignore a file and all its findings if an author matches the regex PATTERN"
    )]
    pub ignore_author: Vec<String>,

    #[arg(
        long = "ignore-copyright-holder",
        value_name = "PATTERN",
        help = "Ignore a file and all its findings if a copyright holder matches the regex PATTERN"
    )]
    pub ignore_copyright_holder: Vec<String>,

    #[arg(long)]
    pub only_findings: bool,

    #[arg(long, requires = "info")]
    pub mark_source: bool,

    #[arg(long)]
    pub classify: bool,

    #[arg(long, requires = "classify")]
    pub summary: bool,

    #[arg(long = "license-clarity-score", requires = "classify")]
    pub license_clarity_score: bool,

    #[arg(long = "license-references", requires = "license")]
    pub license_references: bool,

    /// Evaluate file license detections against a YAML license policy file.
    #[arg(
        long = "license-policy",
        value_name = "FILE",
        value_parser = parse_license_policy_arg
    )]
    pub license_policy: Option<String>,

    /// Exit non-zero (code 3) when a scanned file matches a license policy whose
    /// compliance_alert is at or above this level. Requires --license-policy.
    #[arg(long = "fail-on", value_enum, requires = "license_policy")]
    pub fail_on: Option<FailOn>,

    #[arg(long)]
    pub tallies: bool,

    #[arg(long = "tallies-key-files", requires_all = ["tallies", "classify"])]
    pub tallies_key_files: bool,

    #[arg(long = "tallies-with-details")]
    pub tallies_with_details: bool,

    #[arg(long = "facet", value_name = "<facet>=<pattern>")]
    pub facet: Vec<String>,

    #[arg(long = "tallies-by-facet", requires_all = ["facet", "tallies"])]
    pub tallies_by_facet: bool,

    #[arg(long)]
    pub generated: bool,

    /// Scan input for licenses
    #[arg(short = 'l', long)]
    pub license: bool,

    #[arg(short = 'c', long)]
    pub copyright: bool,

    /// Scan input for email addresses
    #[arg(short = 'e', long)]
    pub email: bool,

    /// Report only up to INT emails found in a file. Use 0 for no limit.
    #[arg(long, default_value_t = 50, requires = "email")]
    pub max_email: usize,

    /// Scan input for URLs
    #[arg(short = 'u', long)]
    pub url: bool,

    /// Report only up to INT URLs found in a file. Use 0 for no limit.
    #[arg(long, default_value_t = 50, requires = "url")]
    pub max_url: usize,
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse_from(rewrite_args_for_default_scan(std::env::args_os()))
    }

    pub fn try_parse_from<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        <Self as Parser>::try_parse_from(rewrite_args_for_default_scan(itr))
    }

    pub(crate) fn scan_args(&self) -> Option<&ScanArgs> {
        match &self.command {
            Command::Scan(scan_args) => Some(scan_args.as_ref()),
            Command::Serve(_)
            | Command::Compare(_)
            | Command::ShowAttribution
            | Command::ExportLicenseDataset(_) => None,
        }
    }
}

#[cfg(test)]
impl Deref for Cli {
    type Target = ScanArgs;

    fn deref(&self) -> &Self::Target {
        self.scan_args()
            .expect("scan arguments are only available for the scan command")
    }
}

fn rewrite_args_for_default_scan<I, T>(itr: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args: Vec<OsString> = itr.into_iter().map(Into::into).collect();
    if args.len() <= 1 {
        return args;
    }

    let first = args[1].to_string_lossy();
    if matches!(
        first.as_ref(),
        "scan"
            | "serve"
            | "compare"
            | "show-attribution"
            | "export-license-dataset"
            | "help"
            | "-h"
            | "--help"
            | "-V"
            | "--version"
    ) {
        return args;
    }

    if first.starts_with('-') || Path::new(first.as_ref()).exists() {
        args.insert(1, OsString::from("scan"));
    }

    args
}

fn parse_max_in_memory(value: &str) -> Result<MemoryMode, String> {
    let parsed = value
        .parse::<i64>()
        .map_err(|_| format!("invalid integer value: {value}"))?;
    if parsed < -1 {
        return Err("--max-in-memory must be -1, 0, or a positive integer".to_string());
    }
    match parsed {
        -1 => Ok(MemoryMode::StreamUnlimited),
        0 => Ok(MemoryMode::CollectFirst),
        n if n > 0 => Ok(MemoryMode::Limit(usize::try_from(n).unwrap_or(usize::MAX))),
        _ => Ok(MemoryMode::CollectFirst),
    }
}

type OutputTargetFieldAccessor = fn(&ScanArgs) -> &Option<String>;

/// Single source of truth mapping each plain-file `ScanArgs` output flag to
/// its [`OutputFormat`], in the order `output_targets` should emit them.
/// `--custom-output` is handled separately (see `output_targets`) since it
/// also needs `--custom-template` attached. Adding a new plain output flag
/// means adding its clap field above, one row here, and one row in
/// `output::OUTPUT_WRITERS` (`src/output/mod.rs`).
const OUTPUT_TARGET_FIELDS: &[(OutputFormat, OutputTargetFieldAccessor)] = &[
    (OutputFormat::Json, |args| &args.output_json),
    (OutputFormat::JsonPretty, |args| &args.output_json_pp),
    (OutputFormat::JsonLines, |args| &args.output_json_lines),
    (OutputFormat::Yaml, |args| &args.output_yaml),
    (OutputFormat::Debian, |args| &args.output_debian),
    (OutputFormat::Html, |args| &args.output_html),
    (OutputFormat::SpdxTv, |args| &args.output_spdx_tv),
    (OutputFormat::SpdxRdf, |args| &args.output_spdx_rdf),
    (OutputFormat::CycloneDxJson, |args| &args.output_cyclonedx),
    (OutputFormat::CycloneDxXml, |args| {
        &args.output_cyclonedx_xml
    }),
    (OutputFormat::Sarif, |args| &args.output_sarif),
];

impl ScanArgs {
    pub(crate) fn output_targets(&self) -> Vec<OutputTarget> {
        let mut targets: Vec<OutputTarget> = OUTPUT_TARGET_FIELDS
            .iter()
            .filter_map(|(format, field)| {
                field(self).as_ref().map(|file| OutputTarget {
                    format: *format,
                    file: file.clone(),
                    custom_template: None,
                })
            })
            .collect();

        if let Some(file) = &self.custom_output {
            targets.push(OutputTarget {
                format: OutputFormat::CustomTemplate,
                file: file.clone(),
                custom_template: self.custom_template.clone(),
            });
        }

        targets
    }

    pub(crate) fn output_header_options(&self) -> JsonMap<String, JsonValue> {
        let mut options = JsonMap::new();
        if !self.dir_path.is_empty() {
            options.insert(
                "input".to_string(),
                JsonValue::Array(
                    self.dir_path
                        .iter()
                        .cloned()
                        .map(JsonValue::String)
                        .collect(),
                ),
            );
        }

        let mut flags = Vec::new();

        push_string_option(&mut flags, "--cache-dir", self.cache_dir.as_ref());
        push_bool_option(&mut flags, "--cache-clear", self.cache_clear);
        push_bool_option(&mut flags, "--cache-trust-mtime", self.cache_trust_mtime);
        push_bool_option(&mut flags, "--classify", self.classify);
        push_string_option(&mut flags, "--custom-output", self.custom_output.as_ref());
        push_string_option(
            &mut flags,
            "--custom-template",
            self.custom_template.as_ref(),
        );
        push_bool_option(&mut flags, "--copyright", self.copyright);
        if self.compat_mode != CompatibilityMode::Native {
            flags.push((
                "--compat-mode".to_string(),
                JsonValue::String(self.compat_mode.as_str().to_string()),
            ));
        }
        push_string_option(&mut flags, "--cyclonedx", self.output_cyclonedx.as_ref());
        push_string_option(
            &mut flags,
            "--cyclonedx-xml",
            self.output_cyclonedx_xml.as_ref(),
        );
        push_string_option(&mut flags, "--debian", self.output_debian.as_ref());
        push_bool_option(&mut flags, "--email", self.email);
        push_array_option(&mut flags, "--facet", &self.facet);
        push_bool_option(&mut flags, "--filter-clues", self.filter_clues);
        push_bool_option(&mut flags, "--from-json", self.from_json);
        push_bool_option(&mut flags, "--full-root", self.full_root);
        push_bool_option(&mut flags, "--generated", self.generated);
        push_string_option(&mut flags, "--html", self.output_html.as_ref());
        push_array_option(&mut flags, "--ignore", &self.exclude);
        push_array_option(&mut flags, "--ignore-author", &self.ignore_author);
        push_array_option(
            &mut flags,
            "--ignore-copyright-holder",
            &self.ignore_copyright_holder,
        );
        push_bool_option(&mut flags, "--incremental", self.incremental);
        push_array_option(&mut flags, "--include", &self.include);
        push_bool_option(&mut flags, "--info", self.info);
        push_string_option(&mut flags, "--json", self.output_json.as_ref());
        push_string_option(&mut flags, "--json-lines", self.output_json_lines.as_ref());
        push_string_option(&mut flags, "--json-pp", self.output_json_pp.as_ref());
        push_bool_option(&mut flags, "--license", self.license);
        push_bool_option(
            &mut flags,
            "--license-clarity-score",
            self.license_clarity_score,
        );
        push_bool_option(
            &mut flags,
            "--license-diagnostics",
            self.license_diagnostics,
        );
        push_string_option(
            &mut flags,
            "--license-dataset-path",
            self.license_dataset_path.as_ref(),
        );
        push_string_option(&mut flags, "--license-policy", self.license_policy.as_ref());
        push_bool_option(
            &mut flags,
            "--no-license-index-cache",
            self.no_license_index_cache,
        );
        push_bool_option(&mut flags, "--license-references", self.license_references);
        push_bool_option(&mut flags, "--reindex", self.reindex);
        push_non_default_u8_option(&mut flags, "--license-score", self.license_score, 0);
        push_bool_option(&mut flags, "--license-text", self.license_text);
        push_bool_option(
            &mut flags,
            "--license-text-diagnostics",
            self.license_text_diagnostics,
        );
        push_non_default_string_option(
            &mut flags,
            "--license-url-template",
            &self.license_url_template,
            DEFAULT_LICENSEDB_URL_TEMPLATE,
        );
        push_non_default_usize_option(&mut flags, "--max-depth", self.max_depth, 0);
        match self.max_in_memory {
            MemoryMode::Limit(10000) => {}
            MemoryMode::CollectFirst => {
                flags.push(("--max-in-memory".to_string(), JsonValue::Number(0.into())));
            }
            MemoryMode::StreamUnlimited => {
                flags.push((
                    "--max-in-memory".to_string(),
                    JsonValue::Number((-1i64).into()),
                ));
            }
            MemoryMode::Limit(n) => {
                flags.push(("--max-in-memory".to_string(), JsonValue::Number(n.into())));
            }
        }
        if self.email {
            push_non_default_usize_option(&mut flags, "--max-email", self.max_email, 50);
        }
        if self.url {
            push_non_default_usize_option(&mut flags, "--max-url", self.max_url, 50);
        }
        push_bool_option(&mut flags, "--mark-source", self.mark_source);
        push_bool_option(&mut flags, "--no-assemble", self.no_assemble);
        push_bool_option(&mut flags, "--only-findings", self.only_findings);
        push_bool_option(&mut flags, "--package", self.package);
        push_bool_option(
            &mut flags,
            "--package-in-compiled",
            self.package_in_compiled,
        );
        push_bool_option(&mut flags, "--package-only", self.package_only);
        push_array_option(&mut flags, "--paths-file", &self.paths_file);
        push_non_default_process_mode_option(
            &mut flags,
            "--processes",
            self.processes,
            ProcessMode::default_value(),
        );
        push_bool_option(&mut flags, "--quiet", self.quiet);
        push_string_option(&mut flags, "--spdx-rdf", self.output_spdx_rdf.as_ref());
        push_string_option(&mut flags, "--spdx-tv", self.output_spdx_tv.as_ref());
        push_bool_option(&mut flags, "--strip-root", self.strip_root);
        push_bool_option(&mut flags, "--summary", self.summary);
        push_bool_option(&mut flags, "--system-package", self.system_package);
        push_bool_option(&mut flags, "--tallies", self.tallies);
        push_bool_option(&mut flags, "--tallies-by-facet", self.tallies_by_facet);
        push_bool_option(&mut flags, "--tallies-key-files", self.tallies_key_files);
        push_bool_option(
            &mut flags,
            "--tallies-with-details",
            self.tallies_with_details,
        );
        push_non_default_f64_option(&mut flags, "--timeout", self.timeout, 120.0);
        push_bool_option(&mut flags, "--unknown-licenses", self.unknown_licenses);
        push_bool_option(
            &mut flags,
            "--no-sequence-matching",
            self.no_sequence_matching,
        );
        push_bool_option(&mut flags, "--url", self.url);
        push_bool_option(&mut flags, "--verbose", self.verbose);
        push_string_option(&mut flags, "--yaml", self.output_yaml.as_ref());

        flags.sort_by(|left, right| left.0.cmp(&right.0));
        for (key, value) in flags {
            options.insert(key, value);
        }

        options
    }
}

impl From<&ScanArgs> for ScanRequest {
    fn from(cli: &ScanArgs) -> Self {
        Self {
            input_paths: cli.dir_path.clone(),
            input_mode: if cli.from_json {
                InputMode::FromJson
            } else {
                InputMode::Native
            },
            output_targets: cli.output_targets(),
            output_header_options: cli.output_header_options(),
            progress_mode: if cli.quiet {
                crate::progress::ProgressMode::Quiet
            } else if cli.verbose {
                crate::progress::ProgressMode::Verbose
            } else {
                crate::progress::ProgressMode::Default
            },
            process_mode: cli.processes,
            timeout_seconds: cli.timeout,
            quiet: cli.quiet,
            verbose: cli.verbose,
            strip_root: cli.strip_root,
            full_root: cli.full_root,
            exclude: cli.exclude.clone(),
            include: cli.include.clone(),
            paths_files: cli.paths_file.clone(),
            respect_process_cache_env: true,
            cache_dir: cli.cache_dir.clone(),
            cache_clear: cli.cache_clear,
            incremental: cli.incremental,
            cache_trust_mtime: cli.cache_trust_mtime,
            max_depth: cli.max_depth,
            max_in_memory: cli.max_in_memory,
            info: cli.info,
            package: cli.package,
            system_package: cli.system_package,
            package_in_compiled: cli.package_in_compiled,
            package_only: cli.package_only,
            no_assemble: cli.no_assemble,
            license_dataset_path: cli.license_dataset_path.clone(),
            reindex: cli.reindex,
            no_license_index_cache: cli.no_license_index_cache,
            license_text: cli.license_text,
            license_text_diagnostics: cli.license_text_diagnostics,
            license_diagnostics: cli.license_diagnostics,
            unknown_licenses: cli.unknown_licenses,
            no_sequence_matching: cli.no_sequence_matching,
            license_score: cli.license_score,
            license_url_template: cli.license_url_template.clone(),
            filter_clues: cli.filter_clues,
            ignore_author: cli.ignore_author.clone(),
            ignore_copyright_holder: cli.ignore_copyright_holder.clone(),
            only_findings: cli.only_findings,
            mark_source: cli.mark_source,
            classify: cli.classify,
            summary: cli.summary,
            license_clarity_score: cli.license_clarity_score,
            license_references: cli.license_references,
            license_policy: cli.license_policy.clone(),
            fail_on: cli.fail_on.map(FailOn::threshold),
            tallies: cli.tallies,
            tallies_key_files: cli.tallies_key_files,
            tallies_with_details: cli.tallies_with_details,
            facet: cli.facet.clone(),
            tallies_by_facet: cli.tallies_by_facet,
            generated: cli.generated,
            license: cli.license,
            copyright: cli.copyright,
            email: cli.email,
            max_email: cli.max_email,
            url: cli.url,
            max_url: cli.max_url,
            scan_bounds: crate::app::request::ScanBounds::default(),
        }
    }
}

fn push_bool_option(options: &mut Vec<(String, JsonValue)>, key: &str, enabled: bool) {
    if enabled {
        options.push((key.to_string(), JsonValue::Bool(true)));
    }
}

fn push_string_option(options: &mut Vec<(String, JsonValue)>, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        options.push((key.to_string(), JsonValue::String(value.clone())));
    }
}

fn push_non_default_string_option(
    options: &mut Vec<(String, JsonValue)>,
    key: &str,
    value: &str,
    default: &str,
) {
    if value != default {
        options.push((key.to_string(), JsonValue::String(value.to_string())));
    }
}

fn push_array_option(options: &mut Vec<(String, JsonValue)>, key: &str, values: &[String]) {
    if !values.is_empty() {
        options.push((
            key.to_string(),
            JsonValue::Array(values.iter().cloned().map(JsonValue::String).collect()),
        ));
    }
}

fn push_non_default_usize_option(
    options: &mut Vec<(String, JsonValue)>,
    key: &str,
    value: usize,
    default: usize,
) {
    if value != default {
        options.push((key.to_string(), JsonValue::Number(value.into())));
    }
}

fn push_non_default_u8_option(
    options: &mut Vec<(String, JsonValue)>,
    key: &str,
    value: u8,
    default: u8,
) {
    if value != default {
        options.push((key.to_string(), JsonValue::Number(value.into())));
    }
}

fn push_non_default_process_mode_option(
    options: &mut Vec<(String, JsonValue)>,
    key: &str,
    value: ProcessMode,
    default: ProcessMode,
) {
    if value != default {
        options.push((key.to_string(), JsonValue::Number(value.to_i32().into())));
    }
}

fn push_non_default_f64_option(
    options: &mut Vec<(String, JsonValue)>,
    key: &str,
    value: f64,
    default: f64,
) {
    if (value - default).abs() > f64::EPSILON
        && let Some(number) = JsonNumber::from_f64(value)
    {
        options.push((key.to_string(), JsonValue::Number(number)));
    }
}

#[cfg(test)]
mod tests;
