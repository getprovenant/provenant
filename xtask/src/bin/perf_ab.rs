// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! `perf-ab` builds two Provenant git refs in release mode and interleaved-A/B-times
//! a scan against the same target, reporting per-side medians and the head-vs-base
//! speedup. It is a self-comparison tool (does this code change make Provenant
//! faster?), distinct from `compare-outputs`/`benchmark-target` which measure
//! Provenant against ScanCode.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::Parser;
use provenant_xtask::common::{
    ScanProfile, derive_repo_name_from_url, now_run_id, project_root, realpath, resolve_scan_args,
    write_pretty_json,
};
use provenant_xtask::repo_cache::{
    cleanup_repo_worktree, ensure_repo_mirror, prepare_repo_worktree, repo_cache_path,
    resolve_repo_ref_to_sha,
};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;

const WARMUP_LABEL: &str = "warmup";

#[derive(Parser, Debug)]
#[command(
    name = "perf-ab",
    trailing_var_arg = true,
    about = "Interleaved A/B timing of a Provenant scan across two git refs"
)]
struct Args {
    /// Base git ref to build and time (the "before" side).
    #[arg(long, default_value = "origin/main")]
    base: String,
    /// Head git ref to build and time (the "after" side).
    #[arg(long, default_value = "HEAD")]
    head: String,
    /// Benchmark a remote repository URL via the shared repo cache.
    #[arg(long)]
    repo_url: Option<String>,
    /// Required with `--repo-url`; commit SHA, tag, or branch of the target repo.
    #[arg(long)]
    repo_ref: Option<String>,
    /// Benchmark an existing local directory in place.
    #[arg(long)]
    target_path: Option<PathBuf>,
    /// Number of interleaved timed rounds per side (after the discarded warmup).
    #[arg(long, default_value_t = 5)]
    rounds: usize,
    /// Use this prebuilt base binary instead of building the base ref.
    #[arg(long)]
    base_bin: Option<PathBuf>,
    /// Use this prebuilt head binary instead of building the head ref.
    #[arg(long)]
    head_bin: Option<PathBuf>,
    /// Scan against this target with both binaries to JSON and require
    /// byte-identical output after normalizing the volatile header and
    /// randomly-generated package UIDs.
    #[arg(long)]
    check_output: bool,
    /// Scan profile shorthand. Mutually exclusive with explicit scan flags after `--`.
    #[arg(long, value_enum)]
    profile: Option<ScanProfile>,
    /// Explicit scan flags forwarded to both binaries (after `--`).
    scan_args: Vec<String>,
}

/// One built side of the A/B comparison.
struct Side {
    label: &'static str,
    git_ref: String,
    binary: PathBuf,
    build_revision: Option<String>,
    /// Set when this side owns a transient worktree that must be cleaned up.
    cleanup: Option<(PathBuf, PathBuf)>,
}

impl Drop for Side {
    fn drop(&mut self) {
        if let Some((cache_dir, worktree_dir)) = &self.cleanup {
            let _ = cleanup_repo_worktree(cache_dir, worktree_dir);
        }
    }
}

#[derive(Serialize)]
struct SideManifest {
    label: String,
    git_ref: String,
    build_revision: Option<String>,
    binary: PathBuf,
    median_seconds: f64,
    round_seconds: Vec<f64>,
}

#[derive(Serialize)]
struct RunManifest {
    run_id: String,
    target_label: String,
    rounds: usize,
    scan_profile: Option<String>,
    scan_args: Vec<String>,
    check_output: bool,
    base: SideManifest,
    head: SideManifest,
    speedup_factor: Option<f64>,
    reduction_percent: Option<f64>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.rounds == 0 {
        bail!("--rounds must be at least 1");
    }
    let scan_args = resolve_scan_args(
        args.profile,
        args.scan_args.clone(),
        "pass --profile <common|common-with-compiled|licenses|packages> or scan flags after --",
    )?;
    let project_root = project_root();

    println!("==========================================");
    println!("Provenant perf-ab (self before/after A/B)");
    println!("==========================================\n");

    let (target_dir, target_label, _target_guard) = prepare_target(&project_root, &args)?;

    println!("[build] base ref: {}", args.base);
    let base = build_side(&project_root, "base", &args.base, args.base_bin.as_deref())?;
    println!("[build] head ref: {}", args.head);
    let head = build_side(&project_root, "head", &args.head, args.head_bin.as_deref())?;

    println!("\nConfiguration:");
    println!("  Target:    {target_label}");
    println!("  Work dir:  {}", target_dir.display());
    if let Some(name) = args.profile.map(|p| p.display_name()) {
        println!("  Profile:   {name}");
    }
    println!("  Scan args: {}", scan_args.join(" "));
    println!("  Rounds:    {}", args.rounds);
    println!("  base:      {} ({})", base.git_ref, describe_rev(&base));
    println!("  head:      {} ({})", head.git_ref, describe_rev(&head));
    println!();

    if args.check_output {
        run_correctness_gate(&base, &head, &target_dir, &scan_args)?;
    }

    let run_id = now_run_id("perf-ab");
    let run_dir = project_root.join(".provenant/perf-ab").join(&run_id);
    fs::create_dir_all(&run_dir)
        .with_context(|| format!("failed to create {}", run_dir.display()))?;

    let timings = run_interleaved(&base, &head, &target_dir, &scan_args, args.rounds, &run_dir)?;

    let base_median = median(&timings.base).context("no base timings recorded")?;
    let head_median = median(&timings.head).context("no head timings recorded")?;
    let factor = speedup_factor(base_median, head_median);
    let reduction = reduction_percent(base_median, head_median);

    report(&timings, base_median, head_median, factor, reduction);

    let manifest = RunManifest {
        run_id: run_id.clone(),
        target_label,
        rounds: args.rounds,
        scan_profile: args.profile.map(|p| p.display_name().to_string()),
        scan_args,
        check_output: args.check_output,
        base: side_manifest(&base, base_median, &timings.base),
        head: side_manifest(&head, head_median, &timings.head),
        speedup_factor: factor,
        reduction_percent: reduction,
    };
    let manifest_path = run_dir.join("run-manifest.json");
    write_pretty_json(&manifest_path, &manifest)?;
    println!("\nArtifacts:\n  {}", manifest_path.display());

    Ok(())
}

fn describe_rev(side: &Side) -> String {
    match &side.build_revision {
        Some(rev) if rev.len() >= 8 => rev[..8].to_string(),
        Some(rev) => rev.clone(),
        None => "prebuilt".to_string(),
    }
}

/// Resolve the scan target, returning its directory, a display label, and an
/// optional cleanup guard for transient remote checkouts.
fn prepare_target(
    project_root: &Path,
    args: &Args,
) -> Result<(PathBuf, String, Option<TargetGuard>)> {
    match (&args.target_path, &args.repo_url) {
        (Some(_), Some(_)) => bail!("specify exactly one of --target-path or --repo-url"),
        (None, None) => bail!("specify a target with --target-path or --repo-url"),
        (Some(path), None) => {
            if args.repo_ref.is_some() {
                bail!("--repo-ref can only be used with --repo-url");
            }
            let resolved = realpath(path)?;
            let label = resolved.display().to_string();
            Ok((resolved, label, None))
        }
        (None, Some(repo_url)) => {
            let repo_ref = args
                .repo_ref
                .as_deref()
                .context("--repo-url requires --repo-ref (commit SHA, tag, or branch)")?;
            let cache_dir = repo_cache_path(project_root, repo_url);
            println!("[target] updating repo cache: {}", cache_dir.display());
            ensure_repo_mirror(repo_url, repo_ref, &cache_dir)?;
            let resolved_sha = resolve_repo_ref_to_sha(&cache_dir, repo_ref)?;
            let worktree_dir = project_root
                .join(".provenant/perf-ab-targets")
                .join(derive_repo_name_from_url(repo_url, "perf-ab-target"));
            println!("[target] checkout {repo_ref} -> {}", &resolved_sha[..8]);
            prepare_repo_worktree(&cache_dir, &resolved_sha, &worktree_dir)?;
            let label = format!("{repo_url}@{resolved_sha}");
            let guard = TargetGuard {
                cache_dir,
                worktree_dir: worktree_dir.clone(),
            };
            Ok((worktree_dir, label, Some(guard)))
        }
    }
}

struct TargetGuard {
    cache_dir: PathBuf,
    worktree_dir: PathBuf,
}

impl Drop for TargetGuard {
    fn drop(&mut self) {
        let _ = cleanup_repo_worktree(&self.cache_dir, &self.worktree_dir);
    }
}

/// Build (or reuse) a Provenant release binary for one git ref into a stable
/// per-ref location and return its built revision.
fn build_side(
    project_root: &Path,
    label: &'static str,
    git_ref: &str,
    prebuilt: Option<&Path>,
) -> Result<Side> {
    if let Some(prebuilt) = prebuilt {
        let binary = realpath(prebuilt)?;
        if !binary.is_file() {
            bail!("{label} binary not found at {}", binary.display());
        }
        return Ok(Side {
            label,
            git_ref: git_ref.to_string(),
            binary,
            build_revision: None,
            cleanup: None,
        });
    }

    let cache_dir = self_repo_cache(project_root);
    let resolved = resolve_self_ref(project_root, git_ref)
        .with_context(|| format!("failed to resolve {label} ref '{git_ref}'"))?;
    let worktree_dir = project_root
        .join(".provenant/perf-ab-build")
        .join(label)
        .join(&resolved[..12.min(resolved.len())]);
    prepare_self_worktree(project_root, &resolved, &worktree_dir)
        .with_context(|| format!("failed to materialize {label} worktree for {resolved}"))?;

    // Arm cleanup before building: once the worktree exists, the owning `Side`
    // holds the cleanup handle so its `Drop` removes the worktree even if the
    // build below fails or the binary is missing. Returning the error after
    // constructing `side` lets that `Drop` fire, avoiding a build-artifact leak
    // under `.provenant/perf-ab-build/`.
    let binary = worktree_dir.join("target/release/provenant");
    let side = Side {
        label,
        git_ref: git_ref.to_string(),
        binary,
        build_revision: Some(resolved),
        cleanup: Some((cache_dir, worktree_dir.clone())),
    };

    build_release(&worktree_dir, label)?;
    if !side.binary.is_file() {
        bail!("{label} binary not found at {}", side.binary.display());
    }

    Ok(side)
}

fn self_repo_cache(project_root: &Path) -> PathBuf {
    // A bare repo whose object store lets us add detached worktrees for arbitrary
    // local refs without disturbing the developer's checkout.
    project_root.join(".provenant/perf-ab-self.git")
}

/// Resolve a ref against the current Provenant repository to a full commit SHA.
fn resolve_self_ref(project_root: &Path, git_ref: &str) -> Result<String> {
    let output = Command::new("git")
        .current_dir(project_root)
        .args(["rev-parse", "--verify", &format!("{git_ref}^{{commit}}")])
        .output()
        .context("failed to run git rev-parse")?;
    if !output.status.success() {
        bail!(
            "git rev-parse failed for '{git_ref}': {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Materialize a detached worktree of the current repo at `resolved_sha` under
/// `worktree_dir`, reusing a dedicated bare clone so the build is isolated from
/// the developer's working tree (avoiding the worktree-churn timing corruption
/// the skill warns about).
fn prepare_self_worktree(
    project_root: &Path,
    resolved_sha: &str,
    worktree_dir: &Path,
) -> Result<()> {
    let cache_dir = self_repo_cache(project_root);
    if !cache_dir.exists() {
        if let Some(parent) = cache_dir.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        run_git(
            Command::new("git")
                .args(["clone", "--bare", "--local"])
                .arg(project_root)
                .arg(&cache_dir),
            "failed to create perf-ab self bare clone",
        )?;
    }
    // Refresh objects so newly created local commits (e.g. HEAD) are present.
    run_git(
        Command::new("git")
            .arg(format!("--git-dir={}", cache_dir.display()))
            .args(["fetch", "--force", "--tags"])
            .arg(project_root)
            .arg("+refs/heads/*:refs/heads/*"),
        "failed to refresh perf-ab self bare clone",
    )?;
    // Also fetch the exact commit in case it is not on any local branch.
    let _ = run_git(
        Command::new("git")
            .arg(format!("--git-dir={}", cache_dir.display()))
            .args(["fetch", "--force"])
            .arg(project_root)
            .arg(resolved_sha),
        "failed to fetch target commit into perf-ab self bare clone",
    );
    prepare_repo_worktree(&cache_dir, resolved_sha, worktree_dir)
}

fn build_release(worktree_dir: &Path, label: &str) -> Result<()> {
    println!("  building {label} in {} ...", worktree_dir.display());
    let output = Command::new("cargo")
        .current_dir(worktree_dir)
        .args(["build", "--release", "--bin", "provenant"])
        .output()
        .with_context(|| format!("failed to build {label}"))?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for line in combined
        .lines()
        .filter(|line| line.contains("Compiling provenant") || line.contains("Finished"))
    {
        println!("    {line}");
    }
    if !output.status.success() {
        bail!(
            "cargo build --release failed for {label}:\n{}",
            combined.trim()
        );
    }
    Ok(())
}

struct Timings {
    base: Vec<f64>,
    head: Vec<f64>,
}

/// Warm up each binary once (discarded), then run `rounds` interleaved timed
/// scans (base, head, base, head, ...). Interleaving spreads any machine-wide
/// thermal/scheduling drift evenly across both sides.
fn run_interleaved(
    base: &Side,
    head: &Side,
    target_dir: &Path,
    scan_args: &[String],
    rounds: usize,
    run_dir: &Path,
) -> Result<Timings> {
    println!("[warmup] discarding one cold run per side");
    timed_scan(base, target_dir, scan_args, WARMUP_LABEL, run_dir)?;
    timed_scan(head, target_dir, scan_args, WARMUP_LABEL, run_dir)?;

    let mut timings = Timings {
        base: Vec::with_capacity(rounds),
        head: Vec::with_capacity(rounds),
    };
    println!("\n[timed] {rounds} interleaved rounds");
    println!("  round | base (s) | head (s)");
    println!("  ------+----------+---------");
    for round in 1..=rounds {
        let base_secs = timed_scan(base, target_dir, scan_args, "timed", run_dir)?;
        let head_secs = timed_scan(head, target_dir, scan_args, "timed", run_dir)?;
        timings.base.push(base_secs);
        timings.head.push(head_secs);
        println!("  {round:>5} | {base_secs:>8.3} | {head_secs:>7.3}");
    }
    Ok(timings)
}

/// Run one scan and return its wall-clock seconds. Output goes to a discarded
/// per-side scratch file so the disk write is part of the measured work but the
/// artifacts do not accumulate.
fn timed_scan(
    side: &Side,
    target_dir: &Path,
    scan_args: &[String],
    phase: &str,
    run_dir: &Path,
) -> Result<f64> {
    let out_file = run_dir.join(format!("{}-{}-scan.json", side.label, phase));
    let mut command = Command::new(&side.binary);
    command
        .current_dir(target_dir)
        .arg("--json")
        .arg(&out_file)
        .arg("--no-license-index-cache")
        .args(scan_args)
        .arg(".");
    let start = Instant::now();
    let output = command
        .output()
        .with_context(|| format!("failed to run {} scan", side.label))?;
    let elapsed = start.elapsed().as_secs_f64();
    if !output.status.success() {
        bail!(
            "{} scan failed:\n{}",
            side.label,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(elapsed)
}

/// Scan with both binaries to JSON and require byte-identical output after
/// normalizing the volatile `headers` section (version, timestamps, duration).
fn run_correctness_gate(
    base: &Side,
    head: &Side,
    target_dir: &Path,
    scan_args: &[String],
) -> Result<()> {
    println!("[check-output] verifying byte-identical output (normalized header)");
    let tmp = std::env::temp_dir().join(now_run_id("perf-ab-check"));
    fs::create_dir_all(&tmp).with_context(|| format!("failed to create {}", tmp.display()))?;
    let base_json = scan_to_json(base, target_dir, scan_args, &tmp)?;
    let head_json = scan_to_json(head, target_dir, scan_args, &tmp)?;
    let base_norm = normalize_scan_json(&base_json)?;
    let head_norm = normalize_scan_json(&head_json)?;
    let _ = fs::remove_dir_all(&tmp);
    if base_norm != head_norm {
        bail!(
            "check-output FAILED: base and head produced different scan output after header normalization.\n\
             A pure-refactor perf change must be byte-identical. Fix the code, not the comparison."
        );
    }
    println!("  check-output OK: outputs are byte-identical after header normalization");
    Ok(())
}

fn scan_to_json(
    side: &Side,
    target_dir: &Path,
    scan_args: &[String],
    tmp: &Path,
) -> Result<String> {
    let out_file = tmp.join(format!("{}-check.json", side.label));
    let output = Command::new(&side.binary)
        .current_dir(target_dir)
        .arg("--json")
        .arg(&out_file)
        .arg("--no-license-index-cache")
        .args(scan_args)
        .arg(".")
        .output()
        .with_context(|| format!("failed to run {} check scan", side.label))?;
    if !output.status.success() {
        bail!(
            "{} check scan failed:\n{}",
            side.label,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    fs::read_to_string(&out_file).with_context(|| format!("failed to read {}", out_file.display()))
}

/// Normalize the volatile parts of scan output so that only behavioral diffs
/// remain:
///
/// - the top-level `headers` array (version, timestamps, duration), and
/// - randomly-generated package UIDs (`uuid=<uuid>` in `package_uid`,
///   `dependency_uid`, `for_package_uid`, and the PURLs that reference them),
///   which differ on every scan run and would otherwise make `--check-output`
///   fail on any `--package` profile regardless of the code change.
fn normalize_scan_json(raw: &str) -> Result<String> {
    let mut value: Value = serde_json::from_str(raw).context("scan output was not valid JSON")?;
    if let Some(object) = value.as_object_mut()
        && object.contains_key("headers")
    {
        object.insert(
            "headers".to_string(),
            Value::String("<normalized>".to_string()),
        );
    }
    let serialized =
        serde_json::to_string(&value).context("failed to re-serialize normalized scan output")?;
    Ok(normalize_package_uids(&serialized))
}

/// Replace each random `uuid=<uuid>` (as emitted in package UID fields and the
/// PURLs that reference them) with a stable placeholder. Matches the canonical
/// 8-4-4-4-12 hex UUID form so non-UID content is left untouched.
fn normalize_package_uids(input: &str) -> String {
    static UID_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = UID_RE.get_or_init(|| {
        // Case-insensitive so uppercase-hex UUIDs (some `Display` impls) are also
        // normalized rather than slipping through and causing spurious failures.
        Regex::new(r"(?i)uuid=[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
            .expect("static UID regex is valid")
    });
    re.replace_all(input, "uuid=<normalized>").into_owned()
}

fn side_manifest(side: &Side, median_seconds: f64, rounds: &[f64]) -> SideManifest {
    SideManifest {
        label: side.label.to_string(),
        git_ref: side.git_ref.clone(),
        build_revision: side.build_revision.clone(),
        binary: side.binary.clone(),
        median_seconds,
        round_seconds: rounds.to_vec(),
    }
}

fn report(
    timings: &Timings,
    base_median: f64,
    head_median: f64,
    factor: Option<f64>,
    reduction: Option<f64>,
) {
    println!("\n==========================================");
    println!("Results");
    println!("==========================================");
    println!(
        "  base median: {base_median:.3}s  (rounds: {})",
        fmt_rounds(&timings.base)
    );
    println!(
        "  head median: {head_median:.3}s  (rounds: {})",
        fmt_rounds(&timings.head)
    );
    match (factor, reduction) {
        (Some(factor), Some(reduction)) => {
            if reduction >= 0.0 {
                println!("  head is {factor:.3}x the speed of base ({reduction:.2}% faster)");
            } else {
                println!(
                    "  head is {factor:.3}x the speed of base ({:.2}% SLOWER \u{2014} regression)",
                    reduction.abs()
                );
            }
        }
        _ => println!("  unable to compute speedup (zero or missing median)"),
    }
}

fn fmt_rounds(rounds: &[f64]) -> String {
    rounds
        .iter()
        .map(|value| format!("{value:.3}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn run_git(command: &mut Command, context_message: &str) -> Result<()> {
    let output = command
        .output()
        .with_context(|| context_message.to_string())?;
    if !output.status.success() {
        bail!(
            "{}: {}",
            context_message,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Median of a sample. Returns `None` for an empty slice. For an even count it
/// averages the two central values.
fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        Some(sorted[mid])
    } else {
        Some((sorted[mid - 1] + sorted[mid]) / 2.0)
    }
}

/// Speedup as a factor: how many times the head throughput is relative to base.
/// `> 1.0` means head is faster. `None` when head median is zero.
fn speedup_factor(base_median: f64, head_median: f64) -> Option<f64> {
    (head_median != 0.0).then_some(base_median / head_median)
}

/// Percent reduction in time from base to head. Positive means head is faster.
/// `None` when base median is zero.
fn reduction_percent(base_median: f64, head_median: f64) -> Option<f64> {
    (base_median != 0.0).then_some((base_median - head_median) / base_median * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Order in which `run_interleaved` emits scans: base/head alternating per
    /// round so machine-wide drift is shared evenly across both sides. This
    /// mirrors the loop in `run_interleaved` and locks the contract that timed
    /// rounds are interleaved rather than all-base-then-all-head.
    fn interleave_order(rounds: usize) -> Vec<&'static str> {
        let mut order = Vec::with_capacity(rounds * 2);
        for _ in 0..rounds {
            order.push("base");
            order.push("head");
        }
        order
    }

    #[test]
    fn median_of_odd_sample_is_middle_value() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
    }

    #[test]
    fn median_of_even_sample_averages_central_pair() {
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
    }

    #[test]
    fn median_of_empty_sample_is_none() {
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn median_ignores_input_order() {
        let unsorted = [10.0, 2.0, 8.0, 4.0, 6.0];
        assert_eq!(median(&unsorted), Some(6.0));
    }

    #[test]
    fn speedup_factor_reports_head_relative_to_base() {
        // base 2.0s, head 1.0s -> head runs at 2x the speed.
        assert_eq!(speedup_factor(2.0, 1.0), Some(2.0));
    }

    #[test]
    fn speedup_factor_below_one_signals_regression() {
        // base 1.0s, head 2.0s -> head is half the speed.
        assert_eq!(speedup_factor(1.0, 2.0), Some(0.5));
    }

    #[test]
    fn speedup_factor_is_none_for_zero_head_median() {
        assert_eq!(speedup_factor(1.0, 0.0), None);
    }

    #[test]
    fn reduction_percent_is_positive_when_head_is_faster() {
        // 4.0 -> 3.0 is a 25% reduction.
        assert_eq!(reduction_percent(4.0, 3.0), Some(25.0));
    }

    #[test]
    fn reduction_percent_is_negative_for_regression() {
        // 2.0 -> 3.0 is a -50% reduction (slower).
        assert_eq!(reduction_percent(2.0, 3.0), Some(-50.0));
    }

    #[test]
    fn reduction_percent_is_none_for_zero_base_median() {
        assert_eq!(reduction_percent(0.0, 1.0), None);
    }

    #[test]
    fn interleave_order_alternates_base_then_head_per_round() {
        assert_eq!(
            interleave_order(3),
            ["base", "head", "base", "head", "base", "head"]
        );
        assert!(interleave_order(0).is_empty());
    }

    #[test]
    fn normalize_scan_json_replaces_header_block() {
        let a = r#"{"headers":[{"tool_version":"1.0","duration":1.5}],"files":[{"path":"a"}]}"#;
        let b = r#"{"headers":[{"tool_version":"2.0","duration":9.9}],"files":[{"path":"a"}]}"#;
        assert_eq!(
            normalize_scan_json(a).unwrap(),
            normalize_scan_json(b).unwrap()
        );
    }

    #[test]
    fn normalize_scan_json_detects_real_diffs() {
        let a = r#"{"headers":[{"tool_version":"1.0"}],"files":[{"path":"a"}]}"#;
        let b = r#"{"headers":[{"tool_version":"1.0"}],"files":[{"path":"b"}]}"#;
        assert_ne!(
            normalize_scan_json(a).unwrap(),
            normalize_scan_json(b).unwrap()
        );
    }

    #[test]
    fn normalize_scan_json_ignores_random_package_uids() {
        // Two runs of the same code differ only by randomly-generated package UIDs.
        let a = r#"{"headers":[],"packages":[{"package_uid":"pkg:golang/x@1?uuid=7d2f562e-87fa-4423-9951-aac2ca7f521f"}]}"#;
        let b = r#"{"headers":[],"packages":[{"package_uid":"pkg:golang/x@1?uuid=6ce89029-256e-4642-8bbd-8a45b15a771d"}]}"#;
        assert_eq!(
            normalize_scan_json(a).unwrap(),
            normalize_scan_json(b).unwrap()
        );
    }

    #[test]
    fn normalize_scan_json_normalizes_uppercase_uuids() {
        // Lowercase vs uppercase hex for the same logical (random) UID must both
        // normalize to the placeholder, so case never causes a spurious failure.
        let a = r#"{"headers":[],"packages":[{"package_uid":"pkg:x@1?uuid=7d2f562e-87fa-4423-9951-aac2ca7f521f"}]}"#;
        let b = r#"{"headers":[],"packages":[{"package_uid":"pkg:x@1?uuid=6CE89029-256E-4642-8BBD-8A45B15A771D"}]}"#;
        assert_eq!(
            normalize_scan_json(a).unwrap(),
            normalize_scan_json(b).unwrap()
        );
    }

    #[test]
    fn normalize_scan_json_keeps_real_diffs_under_uid_normalization() {
        // Same UID shape, but a genuine content difference (the PURL itself) must survive.
        let a = r#"{"headers":[],"packages":[{"package_uid":"pkg:golang/x@1?uuid=7d2f562e-87fa-4423-9951-aac2ca7f521f"}]}"#;
        let b = r#"{"headers":[],"packages":[{"package_uid":"pkg:golang/y@2?uuid=6ce89029-256e-4642-8bbd-8a45b15a771d"}]}"#;
        assert_ne!(
            normalize_scan_json(a).unwrap(),
            normalize_scan_json(b).unwrap()
        );
    }

    fn git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A `Side` armed with a `cleanup` handle removes its worktree on drop, so
    /// the build path is leak-proof no matter which error fires after the
    /// worktree is materialized (build failure, missing binary, etc.).
    #[test]
    fn side_drop_removes_armed_worktree() {
        let temp = tempfile::TempDir::new().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        git(&source, &["init"]);
        git(&source, &["config", "user.name", "Test User"]);
        git(&source, &["config", "user.email", "test@example.com"]);
        git(&source, &["config", "commit.gpgsign", "false"]);
        fs::write(source.join("tracked.txt"), "hello\n").unwrap();
        git(&source, &["add", "tracked.txt"]);
        git(&source, &["commit", "-m", "init"]);

        let cache_dir = temp.path().join("cache.git");
        run_git(
            Command::new("git")
                .args(["clone", "--bare", "--local"])
                .arg(&source)
                .arg(&cache_dir),
            "failed to create bare clone",
        )
        .unwrap();
        let resolved = String::from_utf8_lossy(
            &Command::new("git")
                .arg(format!("--git-dir={}", cache_dir.display()))
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .trim()
        .to_string();

        let worktree_dir = temp.path().join("worktree");
        prepare_repo_worktree(&cache_dir, &resolved, &worktree_dir).unwrap();
        assert!(worktree_dir.exists());

        let side = Side {
            label: "base",
            git_ref: "HEAD".to_string(),
            binary: worktree_dir.join("target/release/provenant"),
            build_revision: Some(resolved),
            cleanup: Some((cache_dir, worktree_dir.clone())),
        };
        drop(side);

        assert!(
            !worktree_dir.exists(),
            "armed Side drop should remove the worktree"
        );
    }
}
