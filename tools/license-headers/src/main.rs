// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;
use glob::Pattern;
use serde::Deserialize;

const COPYRIGHT_LINE: &str = "SPDX-FileCopyrightText: Provenant contributors";
const LICENSE_LINE: &str = "SPDX-License-Identifier: Apache-2.0";
/// Upstream attribution carried by files derived from ScanCode Toolkit, in
/// addition to the Provenant copyright line.
const UPSTREAM_COPYRIGHT_LINE: &str = "SPDX-FileCopyrightText: nexB Inc. and others";
/// Trademark notice retained verbatim from upstream ScanCode source-file headers
/// (Apache-2.0 section 4(c)) for derived files.
const UPSTREAM_TRADEMARK_LINE: &str = "ScanCode is a trademark of nexB Inc.";
/// "Stating changes" notice (Apache-2.0 section 4(b)) for derived files. The
/// set of derived files is enumerated in `.license-headers.toml`; this line
/// stays uniform across them so the header remains idempotent.
const DERIVED_NOTE: &str = "Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.";
const SCOPE_CONFIG_PATH: &str = ".license-headers.toml";

#[derive(Parser, Debug)]
struct Args {
    /// Fail if any in-scope file lacks the expected header.
    #[arg(long, conflicts_with = "fix")]
    check: bool,

    /// Insert or normalize headers in in-scope files.
    #[arg(long, conflicts_with = "check")]
    fix: bool,

    /// Optional file paths to restrict processing; defaults to all in-scope files.
    paths: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
struct ScopePatterns {
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    /// In-scope paths whose Rust source is derived from ScanCode Toolkit and
    /// must carry upstream nexB attribution alongside the Provenant header.
    #[serde(default)]
    derived: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ScopeConfigFile {
    #[serde(default)]
    license_headers: ScopePatterns,
}

#[derive(Debug)]
struct CompiledScopePatterns {
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
    derived: Vec<Pattern>,
}

#[derive(Debug)]
struct ScopeConfig {
    patterns: CompiledScopePatterns,
    /// Raw `derived` glob strings, kept alongside the compiled patterns so
    /// validation errors can name the offending entry.
    derived_raw: Vec<String>,
}

impl ScopeConfig {
    fn load(repo_root: &Path) -> Result<Self> {
        let config_path = repo_root.join(SCOPE_CONFIG_PATH);
        let contents = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        let parsed: ScopeConfigFile = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", config_path.display()))?;

        let include = compile_patterns(&config_path, "include", parsed.license_headers.include)?;
        let exclude = compile_patterns(&config_path, "exclude", parsed.license_headers.exclude)?;
        let derived_raw = parsed.license_headers.derived.clone();
        let derived = compile_patterns(&config_path, "derived", parsed.license_headers.derived)?;

        anyhow::ensure!(
            !include.is_empty(),
            "{} must define at least one include pattern",
            config_path.display()
        );

        Ok(Self {
            patterns: CompiledScopePatterns {
                include,
                exclude,
                derived,
            },
            derived_raw,
        })
    }

    fn includes(&self, relative_path: &str) -> bool {
        let path = Path::new(relative_path);
        self.patterns
            .include
            .iter()
            .any(|pattern| pattern.matches_path(path))
            && !self
                .patterns
                .exclude
                .iter()
                .any(|pattern| pattern.matches_path(path))
    }

    fn is_derived(&self, relative_path: &str) -> bool {
        let path = Path::new(relative_path);
        self.patterns
            .derived
            .iter()
            .any(|pattern| pattern.matches_path(path))
    }

    /// Returns the `derived` entries that match no in-scope tracked file.
    ///
    /// A typo or stale path in the `derived` list would otherwise be silent: the
    /// real file would receive the two-line non-derived header and `--check`
    /// would still pass, dropping its required upstream attribution. Validating
    /// against the actual in-scope file set (rather than just include/exclude)
    /// catches typos, since a misspelled path still matches `src/**/*.rs`.
    fn unmatched_derived(&self, in_scope_paths: &[String]) -> Vec<String> {
        self.patterns
            .derived
            .iter()
            .zip(&self.derived_raw)
            .filter(|(pattern, _)| {
                !in_scope_paths
                    .iter()
                    .any(|relative| pattern.matches_path(Path::new(relative)))
            })
            .map(|(_, raw)| raw.clone())
            .collect()
    }
}

fn compile_patterns(
    config_path: &Path,
    kind: &'static str,
    patterns: Vec<String>,
) -> Result<Vec<Pattern>> {
    patterns
        .into_iter()
        .map(|pattern| {
            let normalized = pattern.trim().trim_start_matches('/').to_string();
            anyhow::ensure!(
                !normalized.is_empty(),
                "{} contains an empty {} pattern",
                config_path.display(),
                kind
            );
            Pattern::new(&normalized).with_context(|| {
                format!(
                    "invalid {} pattern {:?} in {}",
                    kind,
                    normalized,
                    config_path.display()
                )
            })
        })
        .collect()
}

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(args.check || args.fix, "pass either --check or --fix");

    let repo_root = find_repo_root()?;
    let scope = ScopeConfig::load(&repo_root)?;

    // Validate the `derived` list against the actual in-scope tree (regardless
    // of which paths this invocation processes) so a typo or stale entry fails
    // loudly instead of silently dropping a file's upstream attribution.
    let all_candidates = collect_all_candidates(&repo_root, &scope)?;
    let in_scope_paths = all_candidates
        .iter()
        .map(|path| rel_path(&repo_root, path))
        .collect::<Result<Vec<_>>>()?;
    let unmatched = scope.unmatched_derived(&in_scope_paths);
    anyhow::ensure!(
        unmatched.is_empty(),
        "{SCOPE_CONFIG_PATH} `derived` lists path(s) that match no in-scope tracked file \
         (typo or stale entry?): {}",
        unmatched.join(", ")
    );

    let candidates = if args.paths.is_empty() {
        all_candidates
    } else {
        collect_requested_candidates(&repo_root, &scope, &args.paths)?
    };

    if args.fix {
        let mut updated = Vec::new();
        for path in candidates {
            let original = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let derived = scope.is_derived(&rel_path(&repo_root, &path)?);
            let rewritten = rewrite_with_header(&path, &original, derived)?;
            if rewritten != original {
                fs::write(&path, rewritten)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                updated.push(rel_path(&repo_root, &path)?);
            }
        }

        if updated.is_empty() {
            println!("All in-scope files already have the expected license header.");
        } else {
            println!("Updated license headers:");
            for path in updated {
                println!("  {path}");
            }
        }
        return Ok(());
    }

    let mut missing = Vec::new();
    for path in candidates {
        let original = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let derived = scope.is_derived(&rel_path(&repo_root, &path)?);
        let rewritten = rewrite_with_header(&path, &original, derived)?;
        if rewritten != original {
            missing.push(rel_path(&repo_root, &path)?);
        }
    }

    if missing.is_empty() {
        println!("All in-scope files have the expected license header.");
        return Ok(());
    }

    eprintln!("Files missing the expected license header:");
    for path in missing {
        eprintln!("  {path}");
    }
    eprintln!();
    eprintln!("Scope rules live in {SCOPE_CONFIG_PATH}.");
    eprintln!(
        "Fix them with: cargo run --quiet --locked --manifest-path tools/license-headers/Cargo.toml -- --fix"
    );
    anyhow::bail!("license header check failed");
}

fn find_repo_root() -> Result<PathBuf> {
    let mut current = std::env::current_dir()
        .context("failed to resolve current working directory")?
        .canonicalize()
        .context("failed to canonicalize current working directory")?;

    loop {
        if current.join(SCOPE_CONFIG_PATH).is_file() {
            return Ok(current);
        }

        anyhow::ensure!(
            current.pop(),
            "failed to locate {SCOPE_CONFIG_PATH} from current working directory or any parent"
        );
    }
}

fn collect_all_candidates(repo_root: &Path, scope: &ScopeConfig) -> Result<Vec<PathBuf>> {
    let mut candidates = BTreeSet::new();
    for path in git_tracked_files(repo_root)? {
        if !path.is_file() {
            continue;
        }
        if is_in_scope(repo_root, scope, &path)? {
            candidates.insert(path);
        }
    }
    Ok(candidates.into_iter().collect())
}

fn collect_requested_candidates(
    repo_root: &Path,
    scope: &ScopeConfig,
    raw_paths: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let mut candidates = BTreeSet::new();
    for raw_path in raw_paths {
        let path = normalize_requested_path(raw_path)?;
        if !path.is_file() {
            continue;
        }
        if !path.starts_with(repo_root) {
            continue;
        }
        if is_in_scope(repo_root, scope, &path)? {
            candidates.insert(path);
        }
    }
    Ok(candidates.into_iter().collect())
}

fn git_tracked_files(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("ls-files")
        .arg("-z")
        .output()
        .with_context(|| {
            format!(
                "failed to enumerate tracked files in {}",
                repo_root.display()
            )
        })?;

    anyhow::ensure!(
        output.status.success(),
        "git ls-files failed for {}",
        repo_root.display()
    );

    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|bytes| !bytes.is_empty())
        .map(|bytes| repo_root.join(String::from_utf8_lossy(bytes).into_owned()))
        .collect())
}

fn normalize_requested_path(raw_path: &Path) -> Result<PathBuf> {
    if raw_path.is_absolute() {
        return Ok(raw_path.to_path_buf());
    }
    Ok(std::env::current_dir()
        .context("failed to resolve current working directory")?
        .join(raw_path))
}

fn rel_path(repo_root: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(repo_root)
        .with_context(|| format!("{} is outside {}", path.display(), repo_root.display()))?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn is_in_scope(repo_root: &Path, scope: &ScopeConfig, path: &Path) -> Result<bool> {
    let relative = rel_path(repo_root, path)?;
    Ok(scope.includes(&relative))
}

fn comment_prefix(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|value| value.to_str()) {
        Some("rs") => Some("//"),
        Some("sh") | Some("yml") | Some("yaml") => Some("#"),
        _ if path.file_name().and_then(|value| value.to_str()) == Some("build.rs") => Some("//"),
        _ => None,
    }
}

fn expected_header(prefix: &str, derived: bool) -> Vec<String> {
    let mut header = Vec::new();
    if derived {
        header.push(format!("{prefix} {UPSTREAM_COPYRIGHT_LINE}"));
        header.push(format!("{prefix} {UPSTREAM_TRADEMARK_LINE}"));
    }
    header.push(format!("{prefix} {COPYRIGHT_LINE}"));
    header.push(format!("{prefix} {LICENSE_LINE}"));
    if derived {
        header.push(format!("{prefix} {DERIVED_NOTE}"));
    }
    header
}

fn rewrite_with_header(path: &Path, original: &str, derived: bool) -> Result<String> {
    let prefix = comment_prefix(path)
        .with_context(|| format!("no comment prefix configured for {}", path.display()))?;
    let expected = expected_header(prefix, derived);
    let trademark = format!("{prefix} {UPSTREAM_TRADEMARK_LINE}");
    let derived_note = format!("{prefix} {DERIVED_NOTE}");
    let mut lines: Vec<&str> = original.lines().collect();

    let mut output = Vec::new();
    let mut index = 0;
    if lines.first().is_some_and(|line| line.starts_with("#!")) {
        output.push(lines[0].to_string());
        index = 1;
    }

    while lines.get(index).is_some_and(|line| line.trim().is_empty()) {
        index += 1;
    }

    // Consume any existing header comment lines — SPDX tags, the upstream
    // trademark notice, and the derived-note line — in whatever order they
    // appear, so the header stays idempotent and reclassifying a file
    // (derived <-> non-derived) cleanly strips the upstream-only lines. The
    // trademark notice is not an SPDX tag, so it must be matched explicitly.
    while lines.get(index).is_some_and(|line| {
        let trimmed = line.trim();
        trimmed.contains("SPDX-") || trimmed == trademark.trim() || trimmed == derived_note.trim()
    }) {
        index += 1;
    }

    while lines.get(index).is_some_and(|line| line.trim().is_empty()) {
        index += 1;
    }

    output.extend(expected);
    if index < lines.len() {
        output.push(String::new());
        output.extend(lines.drain(index..).map(ToOwned::to_owned));
    }

    Ok(output.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewrite(body: &str, derived: bool) -> String {
        rewrite_with_header(Path::new("x.rs"), body, derived).expect("rewrite")
    }

    #[test]
    fn non_derived_gets_provenant_header_only() {
        assert_eq!(
            rewrite("mod a;\n", false),
            "// SPDX-FileCopyrightText: Provenant contributors\n\
             // SPDX-License-Identifier: Apache-2.0\n\
             \n\
             mod a;\n"
        );
    }

    #[test]
    fn derived_gets_upstream_attribution_and_change_note() {
        assert_eq!(
            rewrite("mod a;\n", true),
            "// SPDX-FileCopyrightText: nexB Inc. and others\n\
             // ScanCode is a trademark of nexB Inc.\n\
             // SPDX-FileCopyrightText: Provenant contributors\n\
             // SPDX-License-Identifier: Apache-2.0\n\
             // Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.\n\
             \n\
             mod a;\n"
        );
    }

    #[test]
    fn derived_header_is_idempotent() {
        let once = rewrite("mod a;\n", true);
        let twice = rewrite_with_header(Path::new("x.rs"), &once, true).expect("rewrite");
        assert_eq!(once, twice);
    }

    #[test]
    fn reclassifying_back_to_non_derived_strips_upstream_lines() {
        let derived = rewrite("mod a;\n", true);
        let reverted = rewrite_with_header(Path::new("x.rs"), &derived, false).expect("rewrite");
        assert_eq!(reverted, rewrite("mod a;\n", false));
    }

    fn scope_with_derived(derived: &[&str]) -> ScopeConfig {
        ScopeConfig {
            patterns: CompiledScopePatterns {
                include: vec![Pattern::new("src/**/*.rs").unwrap()],
                exclude: vec![],
                derived: derived.iter().map(|d| Pattern::new(d).unwrap()).collect(),
            },
            derived_raw: derived.iter().map(|d| (*d).to_string()).collect(),
        }
    }

    #[test]
    fn unmatched_derived_flags_typos_and_stale_entries() {
        let scope = scope_with_derived(&["src/finder/emails.rs", "src/finder/emals.rs"]);
        let in_scope = vec!["src/finder/emails.rs".to_string()];
        // The typo still matches the `src/**/*.rs` include glob, so only
        // validating against real tracked files catches it.
        assert_eq!(
            scope.unmatched_derived(&in_scope),
            vec!["src/finder/emals.rs".to_string()]
        );
    }

    #[test]
    fn unmatched_derived_empty_when_all_present() {
        let scope = scope_with_derived(&["src/finder/emails.rs"]);
        let in_scope = vec![
            "src/finder/emails.rs".to_string(),
            "src/finder/urls.rs".to_string(),
        ];
        assert!(scope.unmatched_derived(&in_scope).is_empty());
    }
}
