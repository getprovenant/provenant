// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Generate the checked-in SBOM examples under `examples/sbom/`.
//!
//! For each verified target this bin checks the repository out at its pinned
//! commit, runs the current-build Provenant scanner with license, package, and
//! copyright detection, and writes SPDX tag-value plus CycloneDX JSON documents
//! next to a short provenance README. `--check` regenerates into a temporary
//! directory, normalizes every per-run volatile field (SPDX `Created`
//! timestamp, CycloneDX `serialNumber`/`metadata.timestamp`, the embedded
//! Provenant tool version, and any random package-UID suffix), and fails only
//! on real content drift. Version normalization keeps a routine release bump
//! from failing the check before the examples are regenerated.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use regex::Regex;
use serde_json::{Map, Value};

use provenant_xtask::common::{ensure_release_binary, project_root, shell_join};
use provenant_xtask::repo_cache::{
    cleanup_repo_worktree, ensure_repo_mirror, prepare_repo_worktree, repo_cache_path,
    resolve_repo_ref_to_sha,
};

/// One verified SBOM example target.
///
/// Each target has been compared against ScanCode with `compare-outputs` and
/// carries no medium-or-major regression; see the top-level
/// `examples/sbom/README.md` for the per-target verification verdicts.
struct SbomTarget {
    /// Directory name under `examples/sbom/` and SBOM document subject name.
    name: &'static str,
    /// Human-facing ecosystem label for the provenance README.
    ecosystem: &'static str,
    /// Upstream clone URL.
    repo_url: &'static str,
    /// Release tag the pinned SHA resolves to (documentation only).
    tag: &'static str,
    /// Full commit SHA the example is generated from.
    pinned_sha: &'static str,
}

const TARGETS: &[SbomTarget] = &[
    SbomTarget {
        name: "ripgrep",
        ecosystem: "Rust / Cargo",
        repo_url: "https://github.com/BurntSushi/ripgrep.git",
        tag: "15.2.0",
        pinned_sha: "e89fff89ac9af12e8d4ce9d5fd07beb408ca730f",
    },
    SbomTarget {
        name: "express",
        ecosystem: "JavaScript / npm",
        repo_url: "https://github.com/expressjs/express.git",
        tag: "v5.2.1",
        pinned_sha: "dbac741a49a5a64336b70c06e85c2e2706e36336",
    },
    SbomTarget {
        name: "flask",
        ecosystem: "Python / PyPI",
        repo_url: "https://github.com/pallets/flask.git",
        tag: "3.1.3",
        pinned_sha: "22d924701a6ae2e4cd01e9a15bbaf3946094af65",
    },
];

const SPDX_FILE: &str = "sbom.spdx";
const CYCLONEDX_FILE: &str = "sbom.cdx.json";
const README_FILE: &str = "README.md";

#[derive(Parser, Debug)]
#[command(name = "generate-sbom-examples")]
struct Args {
    /// Verify the checked-in examples instead of rewriting them.
    #[arg(long)]
    check: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let project_root = project_root();
    let version = workspace_package_version(&project_root)?;

    // Pin the embedded tool version so committed output is deterministic
    // regardless of a dirty working tree or `git describe` state. `--check`
    // still normalizes the version token, so a release bump does not fail CI
    // before the examples are regenerated.
    // SAFETY: set before any Provenant build or scan child process is spawned,
    // while the process is still single-threaded.
    unsafe {
        std::env::set_var("PROVENANT_BUILD_VERSION", &version);
    }

    let binary = project_root.join("target/release/provenant");
    ensure_release_binary(&project_root, &binary, "provenant")?;

    let examples_root = project_root.join("examples/sbom");
    let worktree_root = project_root.join(".provenant/sbom-worktrees");

    let mut generated: Vec<GeneratedExample> = Vec::with_capacity(TARGETS.len());
    for target in TARGETS {
        println!("== {} ({}) @ {}", target.name, target.ecosystem, target.tag);
        generated.push(generate_target(
            &project_root,
            &binary,
            &worktree_root,
            target,
        )?);
    }

    let top_readme = render_top_readme(&version);

    if args.check {
        let mut drift: Vec<String> = Vec::new();
        for example in &generated {
            drift.extend(check_example(&examples_root, example)?);
        }
        drift.extend(check_file(
            &examples_root.join(README_FILE),
            &top_readme,
            normalize_markdown,
        )?);
        if !drift.is_empty() {
            bail!(
                "examples/sbom is out of date; run `cargo run --manifest-path \
                 xtask/Cargo.toml --bin generate-sbom-examples`:\n{}",
                drift.join("\n")
            );
        }
        println!("examples/sbom is up to date");
        return Ok(());
    }

    for example in &generated {
        let dir = examples_root.join(example.target_name);
        fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
        write_file(&dir.join(SPDX_FILE), &example.spdx)?;
        write_file(&dir.join(CYCLONEDX_FILE), &example.cyclonedx)?;
        write_file(&dir.join(README_FILE), &example.readme)?;
    }
    write_file(&examples_root.join(README_FILE), &top_readme)?;
    println!(
        "wrote {} SBOM examples to {}",
        generated.len(),
        examples_root.display()
    );
    Ok(())
}

struct GeneratedExample {
    target_name: &'static str,
    spdx: String,
    cyclonedx: String,
    readme: String,
}

fn generate_target(
    project_root: &Path,
    binary: &Path,
    worktree_root: &Path,
    target: &SbomTarget,
) -> Result<GeneratedExample> {
    let cache_dir = repo_cache_path(project_root, target.repo_url);
    ensure_repo_mirror(target.repo_url, target.pinned_sha, &cache_dir)?;
    let resolved = resolve_repo_ref_to_sha(&cache_dir, target.pinned_sha)?;
    if resolved != target.pinned_sha {
        bail!(
            "{} resolved to {resolved}, expected pinned {}",
            target.name,
            target.pinned_sha
        );
    }

    // Check out with a stable basename so the SPDX document namespace and
    // package-subject names stay deterministic across runs.
    let worktree_dir = worktree_root.join(target.name);
    prepare_repo_worktree(&cache_dir, &resolved, &worktree_dir)?;
    let result = scan_target(project_root, binary, target, &worktree_dir);
    let _ = cleanup_repo_worktree(&cache_dir, &worktree_dir);
    result
}

fn scan_target(
    project_root: &Path,
    binary: &Path,
    target: &SbomTarget,
    worktree_dir: &Path,
) -> Result<GeneratedExample> {
    let scratch = project_root
        .join(".provenant/sbom-scratch")
        .join(target.name);
    if scratch.exists() {
        fs::remove_dir_all(&scratch)
            .with_context(|| format!("failed to clear {}", scratch.display()))?;
    }
    fs::create_dir_all(&scratch)
        .with_context(|| format!("failed to create {}", scratch.display()))?;
    let spdx_path = scratch.join(SPDX_FILE);
    let cyclonedx_path = scratch.join(CYCLONEDX_FILE);

    let args: Vec<String> = vec![
        "scan".to_string(),
        worktree_dir.display().to_string(),
        "--license".to_string(),
        "--package".to_string(),
        "--copyright".to_string(),
        "--strip-root".to_string(),
        "--quiet".to_string(),
        "--spdx-tv".to_string(),
        spdx_path.display().to_string(),
        "--cyclonedx".to_string(),
        cyclonedx_path.display().to_string(),
    ];

    println!(
        "  scanning: {}",
        shell_join(
            &std::iter::once(binary.display().to_string())
                .chain(args.iter().cloned())
                .collect::<Vec<_>>()
        )
    );
    let output = Command::new(binary)
        .current_dir(project_root)
        .args(&args)
        .output()
        .with_context(|| format!("failed to run provenant scan for {}", target.name))?;
    if !output.status.success() {
        bail!(
            "provenant scan failed for {}: {}",
            target.name,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let spdx = fs::read_to_string(&spdx_path)
        .with_context(|| format!("failed to read {}", spdx_path.display()))?;
    let cyclonedx = fs::read_to_string(&cyclonedx_path)
        .with_context(|| format!("failed to read {}", cyclonedx_path.display()))?;
    // Re-serialize CycloneDX so the committed file is pretty-printed and stable.
    let cyclonedx_value: Value = serde_json::from_str(&cyclonedx)
        .with_context(|| format!("{} produced invalid CycloneDX JSON", target.name))?;
    let cyclonedx = format!("{}\n", serde_json::to_string_pretty(&cyclonedx_value)?);

    let _ = fs::remove_dir_all(&scratch);

    let readme = render_example_readme(target);
    Ok(GeneratedExample {
        target_name: target.name,
        spdx,
        cyclonedx,
        readme,
    })
}

fn check_example(examples_root: &Path, example: &GeneratedExample) -> Result<Vec<String>> {
    let dir = examples_root.join(example.target_name);
    let mut drift = Vec::new();
    drift.extend(check_file(
        &dir.join(SPDX_FILE),
        &example.spdx,
        normalize_spdx,
    )?);
    drift.extend(check_file(
        &dir.join(CYCLONEDX_FILE),
        &example.cyclonedx,
        normalize_cyclonedx,
    )?);
    drift.extend(check_file(
        &dir.join(README_FILE),
        &example.readme,
        normalize_markdown,
    )?);
    Ok(drift)
}

fn check_file(
    path: &Path,
    expected: &str,
    normalize: fn(&str) -> Result<String>,
) -> Result<Vec<String>> {
    let existing = match fs::read_to_string(path) {
        Ok(existing) => existing,
        Err(_) => return Ok(vec![format!("  missing: {}", path.display())]),
    };
    if normalize(&existing)? != normalize(expected)? {
        return Ok(vec![format!("  drift: {}", path.display())]);
    }
    Ok(Vec::new())
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

const NORMALIZED: &str = "<NORMALIZED>";

/// Regex matching the embedded Provenant tool version token in text output
/// (`Provenant-1.0.0`, `Provenant 1.2.3-5-gabcd`, etc.). Only the version that
/// trails a literal `Provenant` marker is rewritten, so real package versions
/// in the surrounding document are left untouched.
fn provenant_version_regex() -> Regex {
    Regex::new(r"Provenant([ -])v?\d+\.\d+\.\d+[0-9A-Za-z.\-]*").expect("valid version regex")
}

/// Strip the random `?uuid=<uuid>` suffix that Provenant appends to a package
/// UID when a CycloneDX `bom-ref` or dependency reference falls back to it.
fn package_uid_regex() -> Regex {
    Regex::new(r"\?uuid=[0-9a-fA-F-]{36}").expect("valid uuid suffix regex")
}

fn normalize_spdx(text: &str) -> Result<String> {
    let version = provenant_version_regex();
    let uid = package_uid_regex();
    let normalized = text
        .lines()
        .map(|line| {
            if let Some(rest) = line.strip_prefix("Created:") {
                let _ = rest;
                format!("Created: {NORMALIZED}")
            } else {
                let line = version.replace_all(line, format!("Provenant${{1}}{NORMALIZED}"));
                uid.replace_all(&line, format!("?uuid={NORMALIZED}"))
                    .into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(normalized)
}

fn normalize_cyclonedx(text: &str) -> Result<String> {
    let mut value: Value =
        serde_json::from_str(text).context("committed CycloneDX example is not valid JSON")?;
    if let Some(obj) = value.as_object_mut() {
        if obj.contains_key("serialNumber") {
            obj.insert(
                "serialNumber".to_string(),
                Value::String(NORMALIZED.to_string()),
            );
        }
        if let Some(metadata) = obj.get_mut("metadata").and_then(Value::as_object_mut) {
            normalize_cyclonedx_metadata(metadata);
        }
    }
    // Blank the random package-UID suffix anywhere it survives in bom-refs or
    // dependency references.
    let uid = package_uid_regex();
    let serialized = serde_json::to_string_pretty(&value)?;
    Ok(uid
        .replace_all(&serialized, format!("?uuid={NORMALIZED}"))
        .into_owned())
}

fn normalize_cyclonedx_metadata(metadata: &mut Map<String, Value>) {
    if metadata.contains_key("timestamp") {
        metadata.insert(
            "timestamp".to_string(),
            Value::String(NORMALIZED.to_string()),
        );
    }
    if let Some(tools) = metadata.get_mut("tools").and_then(Value::as_array_mut) {
        for tool in tools {
            if let Some(tool_obj) = tool.as_object_mut()
                && tool_obj.contains_key("version")
            {
                tool_obj.insert("version".to_string(), Value::String(NORMALIZED.to_string()));
            }
        }
    }
}

fn normalize_markdown(text: &str) -> Result<String> {
    let version = provenant_version_regex();
    Ok(version
        .replace_all(text, format!("Provenant${{1}}{NORMALIZED}"))
        .into_owned())
}

fn workspace_package_version(project_root: &Path) -> Result<String> {
    let manifest_path = project_root.join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let mut in_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package && let Some(rest) = trimmed.strip_prefix("version") {
            let value = rest.trim_start_matches([' ', '=']).trim().trim_matches('"');
            if !value.is_empty() {
                return Ok(value.to_string());
            }
        }
    }
    Err(anyhow!(
        "could not find [package] version in {}",
        manifest_path.display()
    ))
}

fn render_example_readme(target: &SbomTarget) -> String {
    format!(
        "# `{name}` SBOM example\n\
         \n\
         SPDX tag-value ([`{spdx}`]({spdx})) and CycloneDX JSON \
         ([`{cyclonedx}`]({cyclonedx})) software bills of materials produced by \
         [Provenant](https://github.com/getprovenant/provenant) for \
         [{name}]({repo_web}) ({ecosystem}).\n\
         \n\
         ## Provenance\n\
         \n\
         - Source: `{repo}` at tag `{tag}`\n\
         - Pinned commit: `{sha}`\n\
         - Generated by: Provenant {version}\n\
         - Command: `provenant scan <checkout> --license --package --copyright \
           --strip-root --spdx-tv {spdx} --cyclonedx {cyclonedx}`\n\
         \n\
         Verified against ScanCode with `compare-outputs` before publication: no \
         medium-or-major license, copyright, or package regressions. See \
         [`../README.md`](../README.md) for the regeneration command and the \
         per-target verification verdict.\n",
        name = target.name,
        ecosystem = target.ecosystem,
        repo = target.repo_url,
        repo_web = repo_web_url(target.repo_url),
        tag = target.tag,
        sha = target.pinned_sha,
        spdx = SPDX_FILE,
        cyclonedx = CYCLONEDX_FILE,
        version = current_forced_version(),
    )
}

fn render_top_readme(_version: &str) -> String {
    let mut rows = String::new();
    for target in TARGETS {
        rows.push_str(&format!(
            "| [`{name}`]({name}/) | {ecosystem} | [`{tag}`]({repo_web}/releases/tag/{tag}) | `{sha}` |\n",
            name = target.name,
            ecosystem = target.ecosystem,
            tag = target.tag,
            repo_web = repo_web_url(target.repo_url),
            sha = target.pinned_sha,
        ));
    }
    format!(
        "# SBOM examples\n\
         \n\
         Real software bills of materials produced by \
         [Provenant](https://github.com/getprovenant/provenant), one directory \
         per target. Each target directory contains an SPDX tag-value document \
         ([`{spdx}`]({first}/{spdx})) and a CycloneDX JSON document \
         ([`{cyclonedx}`]({first}/{cyclonedx})), plus a short provenance README.\n\
         \n\
         These examples are generated by Provenant {version} (the current \
         build) scanning each project at a pinned commit with license, package, \
         and copyright detection enabled.\n\
         \n\
         ## Targets\n\
         \n\
         | Target | Ecosystem | Release | Pinned commit |\n\
         | ------ | --------- | ------- | ------------- |\n\
         {rows}\n\
         ## Verification\n\
         \n\
         Every target was compared against ScanCode with the `compare-outputs` \
         xtask before being included here. None carry a medium-or-major \
         regression (a real missing license or copyright, garbled detections, \
         scan errors, or materially wrong package inventory); the remaining \
         differences are benign (ScanCode noise, unknown-license-reference \
         placeholders, or Provenant's cleaner source-faithful rendering).\n\
         \n\
         ## Regeneration\n\
         \n\
         These files are generated and drift-checked in CI. Regenerate them with \
         a single command:\n\
         \n\
         ```bash\n\
         cargo run --manifest-path xtask/Cargo.toml --bin generate-sbom-examples\n\
         ```\n\
         \n\
         Verify they are current (as CI does) with:\n\
         \n\
         ```bash\n\
         cargo run --manifest-path xtask/Cargo.toml --bin generate-sbom-examples -- --check\n\
         ```\n",
        spdx = SPDX_FILE,
        cyclonedx = CYCLONEDX_FILE,
        first = TARGETS[0].name,
        rows = rows,
        version = current_forced_version(),
    )
}

/// The Provenant version embedded in the generated documents. Sourced from the
/// forced `PROVENANT_BUILD_VERSION`, so READMEs and SBOMs agree, and normalized
/// the same way in `--check`.
fn current_forced_version() -> String {
    std::env::var("PROVENANT_BUILD_VERSION").unwrap_or_else(|_| "unknown".to_string())
}

fn repo_web_url(repo_url: &str) -> String {
    repo_url.trim_end_matches(".git").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spdx_normalization_blanks_created_and_version() {
        let raw = "SPDXVersion: SPDX-2.2\nCreator: Tool: Provenant-1.0.0\nCreated: 2026-07-23T17:13:48Z\nPackageName: demo\n";
        let normalized = normalize_spdx(raw).unwrap();
        assert!(normalized.contains(&format!("Provenant-{NORMALIZED}")));
        assert!(normalized.contains(&format!("Created: {NORMALIZED}")));
        assert!(normalized.contains("PackageName: demo"));
        assert!(!normalized.contains("1.0.0"));
    }

    #[test]
    fn spdx_normalization_is_version_agnostic() {
        let a = "Creator: Tool: Provenant-1.0.0\nCreated: 2026-01-01T00:00:00Z\n";
        let b = "Creator: Tool: Provenant-1.1.0-3-gdeadbee\nCreated: 2027-02-02T02:02:02Z\n";
        assert_eq!(normalize_spdx(a).unwrap(), normalize_spdx(b).unwrap());
    }

    #[test]
    fn spdx_normalization_keeps_real_package_versions() {
        let raw = "PackageName: demo\nPackageVersion: 2.3.4\nCreated: x\n";
        assert!(normalize_spdx(raw).unwrap().contains("2.3.4"));
    }

    #[test]
    fn cyclonedx_normalization_blanks_volatile_fields_only() {
        let a = r#"{"bomFormat":"CycloneDX","serialNumber":"urn:uuid:6989640f-5e12-4358-a157-2f5106dd223e","version":1,"metadata":{"timestamp":"2026-07-23T17:13:48Z","tools":[{"name":"Provenant","version":"1.0.0"}]},"components":[{"bom-ref":"pkg:npm/demo@2.3.4","version":"2.3.4"}]}"#;
        let b = r#"{"bomFormat":"CycloneDX","serialNumber":"urn:uuid:11111111-2222-3333-4444-555555555555","version":1,"metadata":{"timestamp":"2027-02-02T02:02:02Z","tools":[{"name":"Provenant","version":"1.9.0-dirty"}]},"components":[{"bom-ref":"pkg:npm/demo@2.3.4","version":"2.3.4"}]}"#;
        let na = normalize_cyclonedx(a).unwrap();
        assert_eq!(na, normalize_cyclonedx(b).unwrap());
        // Real component version and BOM `version: 1` survive.
        assert!(na.contains("2.3.4"));
        assert!(na.contains("\"version\": 1"));
        assert!(na.contains(NORMALIZED));
    }

    #[test]
    fn cyclonedx_normalization_blanks_package_uid_suffix() {
        let doc = r#"{"dependencies":[{"ref":"pkg:npm/a@1.0.0?uuid=6989640f-5e12-4358-a157-2f5106dd223e"}]}"#;
        let normalized = normalize_cyclonedx(doc).unwrap();
        assert!(normalized.contains(&format!("?uuid={NORMALIZED}")));
        assert!(!normalized.contains("6989640f"));
    }

    #[test]
    fn markdown_normalization_is_version_agnostic() {
        let a = "Generated by: Provenant 1.0.0\n";
        let b = "Generated by: Provenant 1.4.2-9-gabc\n";
        assert_eq!(
            normalize_markdown(a).unwrap(),
            normalize_markdown(b).unwrap()
        );
    }

    #[test]
    fn workspace_version_parses_package_section() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\".\"]\n\n[package]\nname = \"provenant-cli\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        assert_eq!(workspace_package_version(dir.path()).unwrap(), "1.0.0");
    }
}
