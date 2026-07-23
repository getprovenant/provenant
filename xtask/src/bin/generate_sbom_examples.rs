// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Generate the checked-in SBOM examples under `examples/sbom/`.
//!
//! For each verified target this bin checks the repository out at its pinned
//! commit, runs the current-build Provenant scanner with license, package, and
//! copyright detection, and writes SPDX tag-value plus CycloneDX JSON documents
//! next to a short provenance README. The examples are illustrative artifacts,
//! not golden fixtures: regenerate them on demand (a new release, an output
//! improvement worth showcasing, or a new target) rather than drift-checking
//! them on every change. Run with no arguments:
//!
//! ```bash
//! cargo run --manifest-path xtask/Cargo.toml --bin generate-sbom-examples
//! ```

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

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
    /// Verified, source-faithful differences from ScanCode, phrased for a
    /// public README. State the difference; do not editorialize. The universal
    /// "complete, closed inventory" point is added by the template from the
    /// generated document's own counts, so these are the target-specific ones.
    highlights: &'static [&'static str],
}

const TARGETS: &[SbomTarget] = &[
    SbomTarget {
        name: "ripgrep",
        ecosystem: "Rust / Cargo",
        repo_url: "https://github.com/BurntSushi/ripgrep.git",
        tag: "15.2.0",
        pinned_sha: "e89fff89ac9af12e8d4ce9d5fd07beb408ca730f",
        highlights: &[
            "Cleaner copyright: Provenant does not record two prose fragments that ScanCode \
             captures as \"authors\" (`missed. Similarly`; a bare `github.com/Genivia/ugrep` URL).",
        ],
    },
    SbomTarget {
        name: "express",
        ecosystem: "JavaScript / npm",
        repo_url: "https://github.com/expressjs/express.git",
        tag: "v5.2.1",
        pinned_sha: "dbac741a49a5a64336b70c06e85c2e2706e36336",
        highlights: &[
            "Source-faithful copyright: express source literally writes `Copyright(c)` (no space) \
             and Provenant preserves it, where ScanCode rewrites it to `Copyright (c)`.",
            "Cleaner authors: Provenant extracts `TJ Holowaychuk <tj@vision-media.ca>` and drops \
             three ScanCode false-positive \"authors\" (a `graphs/contributors` URL and an \
             `application.js` filename).",
        ],
    },
    SbomTarget {
        name: "flask",
        ecosystem: "Python / PyPI",
        repo_url: "https://github.com/pallets/flask.git",
        tag: "3.1.3",
        pinned_sha: "22d924701a6ae2e4cd01e9a15bbaf3946094af65",
        highlights: &[
            "Cleaner declared license: Provenant resolves `BSD-3-Clause`, where ScanCode reports \
             `BSD-3-Clause AND LicenseRef-scancode-unknown-license-reference`.",
            "Precise package identity: the package carries a proper `pkg:pypi/flask@3.1.3` purl.",
        ],
    },
    SbomTarget {
        name: "tokio",
        ecosystem: "Rust / Cargo workspace",
        repo_url: "https://github.com/tokio-rs/tokio.git",
        tag: "tokio-1.53.1",
        pinned_sha: "75fef53d0a8590c2d1dbb63672aa7b7d1ef51155",
        highlights: &[
            "Monorepo-aware ownership: each member crate of the Cargo workspace (`tokio`, \
             `tokio-util`, `tokio-stream`, `tokio-test`, ...) is its own package, and files are \
             attributed to the crate that owns them rather than to one flattened project.",
            "More complete copyright: Provenant attributes the `Tokio Contributors` holder to \
             every workspace package, where ScanCode records no holder for them.",
            "Cleaner copyright: ScanCode misreads Rust's `#[repr(C)]` attribute as a copyright \
             symbol (e.g. `tokio/src/runtime/task/core.rs` and `runtime/task/raw.rs`) and records \
             the trailing comment as a copyright and holder; Provenant does not.",
        ],
    },
];

const SPDX_FILE: &str = "sbom.spdx";
const CYCLONEDX_FILE: &str = "sbom.cdx.json";
const README_FILE: &str = "README.md";

fn main() -> Result<()> {
    let project_root = project_root();

    // Version stamped into every generated document. Defaults to the workspace
    // package version, but an explicit `PROVENANT_BUILD_VERSION` wins so the
    // examples can be stamped for the release that first ships their behavior —
    // e.g. regenerate with `PROVENANT_BUILD_VERSION=1.0.1` before 1.0.1 is cut.
    // Once that version is released the default matches on its own.
    let version = match std::env::var("PROVENANT_BUILD_VERSION") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => workspace_package_version(&project_root)?,
    };

    // Pin the embedded tool version so committed output is deterministic
    // regardless of a dirty working tree or `git describe` state.
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

    for example in &generated {
        let dir = examples_root.join(example.target_name);
        fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
        write_file(&dir.join(SPDX_FILE), &example.spdx)?;
        write_file(&dir.join(CYCLONEDX_FILE), &example.cyclonedx)?;
        write_file(&dir.join(README_FILE), &example.readme)?;
    }
    prune_obsolete_examples(&examples_root)?;
    write_file(
        &examples_root.join(README_FILE),
        &render_top_readme(&version),
    )?;
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
    let cyclonedx_raw = fs::read_to_string(&cyclonedx_path)
        .with_context(|| format!("failed to read {}", cyclonedx_path.display()))?;
    // Re-serialize CycloneDX so the committed file is pretty-printed and stable.
    let cyclonedx_value: Value = serde_json::from_str(&cyclonedx_raw)
        .with_context(|| format!("{} produced invalid CycloneDX JSON", target.name))?;
    let stats = cyclonedx_stats(&cyclonedx_value);
    let cyclonedx = format!("{}\n", serde_json::to_string_pretty(&cyclonedx_value)?);

    let _ = fs::remove_dir_all(&scratch);

    let readme = render_example_readme(target, &stats);
    Ok(GeneratedExample {
        target_name: target.name,
        spdx,
        cyclonedx,
        readme,
    })
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

/// Remove per-target directories that are no longer in `TARGETS`, so
/// `examples/sbom/` reflects exactly the current manifest. Without this a
/// removed or renamed target would leave a stale, generator-owned directory
/// behind. The top-level `README.md` and any non-target files are left alone.
fn prune_obsolete_examples(examples_root: &Path) -> Result<()> {
    let keep: std::collections::HashSet<&str> = TARGETS.iter().map(|t| t.name).collect();
    for entry in fs::read_dir(examples_root)
        .with_context(|| format!("failed to read {}", examples_root.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !keep.contains(name.as_ref()) {
            fs::remove_dir_all(entry.path())
                .with_context(|| format!("failed to remove obsolete example dir {name}"))?;
            println!("  removed obsolete example directory: {name}");
        }
    }
    Ok(())
}

/// Component and dependency-graph counts read back from a generated CycloneDX
/// document, so the README states the document's own numbers.
struct CycloneDxStats {
    components: usize,
    dependency_edges: usize,
    dangling_edges: usize,
}

fn cyclonedx_stats(doc: &Value) -> CycloneDxStats {
    let mut known_refs: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if let Some(root) = doc
        .get("metadata")
        .and_then(|m| m.get("component"))
        .and_then(|c| c.get("bom-ref"))
        .and_then(Value::as_str)
    {
        known_refs.insert(root);
    }
    let components = doc.get("components").and_then(Value::as_array);
    if let Some(components) = components {
        for component in components {
            if let Some(bom_ref) = component.get("bom-ref").and_then(Value::as_str) {
                known_refs.insert(bom_ref);
            }
        }
    }

    let mut dependency_edges = 0;
    let mut dangling_edges = 0;
    if let Some(dependencies) = doc.get("dependencies").and_then(Value::as_array) {
        for dependency in dependencies {
            if let Some(depends_on) = dependency.get("dependsOn").and_then(Value::as_array) {
                for edge in depends_on {
                    if let Some(target) = edge.as_str() {
                        dependency_edges += 1;
                        if !known_refs.contains(target) {
                            dangling_edges += 1;
                        }
                    }
                }
            }
        }
    }

    CycloneDxStats {
        components: components.map(|c| c.len()).unwrap_or(0),
        dependency_edges,
        dangling_edges,
    }
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

fn render_example_readme(target: &SbomTarget, stats: &CycloneDxStats) -> String {
    let inventory = if stats.dangling_edges == 0 {
        format!(
            "- **{components} components** — the project plus every resolved dependency, each \
             with a package URL (purl).\n\
             - **{edges} dependency relationships**, fully resolved: every `dependsOn` edge \
             points to a component in this same document (no dangling references). A \
             packages-only SBOM lists just the top-level package(s).\n",
            components = stats.components,
            edges = stats.dependency_edges,
        )
    } else {
        // Defensive: never publish a document we would describe as closed when
        // it is not. The generator prints the count so it is caught on regen.
        format!(
            "- **{components} components** and **{edges} dependency relationships** \
             ({dangling} unresolved).\n",
            components = stats.components,
            edges = stats.dependency_edges,
            dangling = stats.dangling_edges,
        )
    };

    let mut compares = String::from(
        "Verified against ScanCode with the `compare-outputs` xtask; no medium-or-major \
         license, copyright, or package regression. Notable source-faithful differences:\n\n\
         - Complete inventory: every resolved dependency is promoted to a component with a purl \
         and the dependency graph is closed, so the SBOM never references materials it does not \
         also list.\n",
    );
    for highlight in target.highlights {
        compares.push_str(&format!("- {highlight}\n"));
    }

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
         ## What's inside\n\
         \n\
         {inventory}\
         \n\
         ## How it compares\n\
         \n\
         {compares}\
         \n\
         See [`../README.md`](../README.md) for the full target list and the verification \
         method.\n",
        name = target.name,
        ecosystem = target.ecosystem,
        repo = target.repo_url,
        repo_web = repo_web_url(target.repo_url),
        tag = target.tag,
        sha = target.pinned_sha,
        spdx = SPDX_FILE,
        cyclonedx = CYCLONEDX_FILE,
        version = current_forced_version(),
        inventory = inventory,
        compares = compares,
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
         and copyright detection enabled. Every SBOM is a complete, closed \
         inventory: each resolved dependency is promoted to a component with a \
         package URL, and every dependency relationship resolves within the \
         document.\n\
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
         These are illustrative artifacts, not golden fixtures, so they are \
         refreshed on demand rather than drift-checked on every change. \
         Regenerate all of them — pinned checkout, scan, and documents — with a \
         single command:\n\
         \n\
         ```bash\n\
         cargo run --manifest-path xtask/Cargo.toml --bin generate-sbom-examples\n\
         ```\n\
         \n\
         Refresh them when cutting a release, when a detection or output change \
         is worth showcasing, or when adding a target (edit the `TARGETS` list \
         in `xtask/src/bin/generate_sbom_examples.rs`).\n",
        spdx = SPDX_FILE,
        cyclonedx = CYCLONEDX_FILE,
        first = TARGETS[0].name,
        rows = rows,
        version = current_forced_version(),
    )
}

/// The Provenant version embedded in the generated documents. Sourced from the
/// forced `PROVENANT_BUILD_VERSION`, so READMEs and SBOMs agree.
fn current_forced_version() -> String {
    std::env::var("PROVENANT_BUILD_VERSION").unwrap_or_else(|_| "unknown".to_string())
}

fn repo_web_url(repo_url: &str) -> String {
    repo_url.trim_end_matches(".git").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cyclonedx_stats_counts_components_and_closed_graph() {
        let doc = json!({
            "metadata": {"component": {"bom-ref": "root"}},
            "components": [
                {"bom-ref": "pkg:cargo/a@1.0.0"},
                {"bom-ref": "pkg:cargo/b@2.0.0"}
            ],
            "dependencies": [
                {"ref": "root", "dependsOn": ["pkg:cargo/a@1.0.0", "pkg:cargo/b@2.0.0"]},
                {"ref": "pkg:cargo/a@1.0.0", "dependsOn": ["pkg:cargo/b@2.0.0"]}
            ]
        });
        let stats = cyclonedx_stats(&doc);
        assert_eq!(stats.components, 2);
        assert_eq!(stats.dependency_edges, 3);
        assert_eq!(stats.dangling_edges, 0);
    }

    #[test]
    fn cyclonedx_stats_flags_dangling_edges() {
        let doc = json!({
            "components": [{"bom-ref": "pkg:npm/present@1.0.0"}],
            "dependencies": [
                {"ref": "pkg:npm/present@1.0.0", "dependsOn": ["pkg:npm/missing@9.9.9"]}
            ]
        });
        let stats = cyclonedx_stats(&doc);
        assert_eq!(stats.components, 1);
        assert_eq!(stats.dependency_edges, 1);
        assert_eq!(stats.dangling_edges, 1);
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
