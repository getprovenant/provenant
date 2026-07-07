// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Weekly ScanCode -> Provenant issue triage, driven by a GitHub Models LLM.
//!
//! The tool is deliberately thin: it fetches recent ScanCode issues, exposes a
//! `run_provenant` tool (real scans against the built binary) plus a
//! `list_parsers` tool, and lets the model do all the judgment -- filtering the
//! "New license request:" noise, classifying, deciding what to reproduce, and
//! writing the verdict table.
//!
//! Runs on GitHub Models inference, authenticated with a GitHub token that
//! carries `models:read`. In GitHub Actions that is the built-in GITHUB_TOKEN
//! (with `permissions: models: read`); locally it is `gh auth token`. The model
//! must be supplied via `--model` or the `SCANCODE_TRIAGE_MODEL` env var
//! (typically a repository variable); there is no built-in default.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use serde_json::{Value, json};

const SCANCODE_REPO: &str = "aboutcode-org/scancode-toolkit";
const TRIAGE_LABEL: &str = "scancode-triage";
const MODELS_ENDPOINT: &str = "https://models.github.ai/inference/chat/completions";
const THROTTLE: Duration = Duration::from_secs(6); // space requests within rate limits
const MAX_TURNS: usize = 6; // tool-loop backstop per issue
const TOOL_RESULT_CHARS: usize = 2500; // cap tool output fed back to the model
const ISSUE_BODY_CHARS: usize = 1800; // keep each request within model input limits
const MAX_FETCH_BYTES: u64 = 2 * 1024 * 1024; // 2 MiB is plenty for a fixture

// Every value the model passes to a tool is untrusted (it is steered by
// attacker-controllable issue text), so constrain what it can reach: only fetch
// fixtures from GitHub, and only allow read-only detection flags on the scanner.
const ALLOWED_FETCH_HOSTS: &[&str] = &[
    "raw.githubusercontent.com",
    "github.com",
    "gist.githubusercontent.com",
];
const ALLOWED_CLI_FLAGS: &[&str] = &[
    "--copyright",
    "--license",
    "--package",
    "--package-only",
    "--email",
    "--url",
    "--info",
];

const SYSTEM_PROMPT: &str = "\
You triage ONE issue from the upstream ScanCode Toolkit for relevance to \
Provenant, a Rust reimplementation of ScanCode's scanning. Provenant ports \
ScanCode's license/copyright detection and package parsers, but its goal is the \
BEST practical scan result, not blind parity -- ScanCode is a reference, not an \
unquestioned source of truth. A ScanCode bug only matters to Provenant if \
Provenant REPRODUCES it.

Procedure:
1. Classify the issue: copyright/license/author DETECTION bug, PACKAGE PARSER \
feature/bug, or OTHER (website/infra/process).
2. If it is a DETECTION bug and the issue links a fetchable source file (a github \
blob/raw URL) or shows inline offending text, CALL run_provenant to reproduce it \
and compare Provenant's output to the issue's expected result.
3. If it is a PARSER issue, CALL list_parsers to check whether Provenant already \
covers that ecosystem/manifest.
4. Prefer real tool evidence over guessing. Make at most a few tool calls.

When done, respond with ONLY a single GitHub-flavored markdown TABLE ROW, exactly:
| [#N](https://github.com/aboutcode-org/scancode-toolkit/issues/N) | <type> | <verdict> | <evidence> |
where <verdict> is one of: `reproduces - worth fixing`, `already fixed/better in \
PV`, `not relevant`, `needs manual check`; <evidence> is a terse note (what the \
scan showed, which parser already exists, or why not relevant). No other text.";

#[derive(Parser)]
#[command(about = "Weekly ScanCode -> Provenant issue triage via GitHub Models")]
struct Args {
    /// YYYY-MM-DD lower bound on issue creation date (default: --days ago)
    #[arg(long)]
    since: Option<String>,
    /// Window size in days when --since is omitted (must be >= 1)
    #[arg(long, default_value_t = 8)]
    days: u32,
    /// Path to the provenant binary used for reproduction scans
    #[arg(long, default_value = "target/release/provenant")]
    binary: String,
    /// Provenant repo root (for list_parsers)
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    /// GitHub Models model id (env: SCANCODE_TRIAGE_MODEL)
    #[arg(long)]
    model: Option<String>,
    /// Cap candidates triaged (0 = no cap)
    #[arg(long = "max-issues", default_value_t = 0)]
    max_issues: usize,
    /// Write the run summary markdown to this file (it is always printed to stdout too)
    #[arg(long)]
    out: Option<PathBuf>,
    /// Provenant repo (owner/name) to open per-finding issues in (deduped, create-if-absent)
    #[arg(long = "post-to")]
    post_to: Option<String>,
}

#[derive(Clone)]
struct Issue {
    number: i64,
    title: String,
    body: String,
    state: String,
    author: String,
    labels: Vec<String>,
}

/// An actionable triage result: a ScanCode issue the model judged
/// `reproduces - worth fixing` against the current Provenant build.
struct Finding {
    scancode_number: i64,
    scancode_title: String,
    kind: String,
    evidence: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.days == 0 {
        bail!(
            "--days must be >= 1 (a non-positive window yields a future date and an empty report)"
        );
    }
    let model = args
        .model
        .clone()
        .or_else(|| {
            std::env::var("SCANCODE_TRIAGE_MODEL")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .ok_or_else(|| {
            anyhow!(
                "no model configured: set the SCANCODE_TRIAGE_MODEL repository variable \
                 (Settings -> Variables) or pass --model"
            )
        })?;
    let since = match &args.since {
        Some(s) => s.clone(),
        None => default_since(args.days)?,
    };

    let (report, findings) = run(&args, &model, &since)?;

    // The full run summary is not an issue; it lives in the run output. Print it
    // to stdout (captured in CI logs) and, when running under Actions, append it
    // to the job's step summary so it renders on the run page.
    println!("{report}");
    if let Ok(summary_path) = std::env::var("GITHUB_STEP_SUMMARY") {
        let _ = std::fs::write(summary_path, &report);
    }
    if let Some(out) = &args.out {
        std::fs::write(out, &report).with_context(|| format!("writing {}", out.display()))?;
        eprintln!("Wrote report to {}", out.display());
    }

    // Only the actionable findings become issues; a re-run never duplicates one.
    if let Some(repo) = &args.post_to {
        create_finding_issues(repo, &findings)?;
    } else if !findings.is_empty() {
        eprintln!(
            "{} actionable finding(s); pass --post-to <owner>/<repo> to open issues",
            findings.len()
        );
    }
    Ok(())
}

fn run(args: &Args, model: &str, since: &str) -> Result<(String, Vec<Finding>)> {
    let token = gh_token()?;
    let client = reqwest::blocking::Client::builder()
        .user_agent("provenant-triage")
        .build()?;

    let issues = fetch_issues(since)?;
    let (license_requests, mut candidates): (Vec<_>, Vec<_>) = issues
        .iter()
        .cloned()
        .partition(|i| is_license_request(&i.title));
    if args.max_issues > 0 {
        candidates.truncate(args.max_issues);
    }
    eprintln!(
        "Fetched {} issues since {since}: {} license-requests (skipped), {} candidates",
        issues.len(),
        license_requests.len(),
        candidates.len()
    );

    let mut rows = Vec::new();
    let mut findings = Vec::new();
    for (i, issue) in candidates.iter().enumerate() {
        eprintln!(
            "  [{}/{}] triaging #{}: {}",
            i + 1,
            candidates.len(),
            issue.number,
            truncate(&issue.title, 60)
        );
        let row = triage_one_issue(&client, &token, model, args, issue);
        if row.to_lowercase().contains("worth fixing") {
            // Row shape: '' | Issue | Type | Verdict | Evidence | ''
            let cells: Vec<&str> = row.split('|').map(str::trim).collect();
            findings.push(Finding {
                scancode_number: issue.number,
                scancode_title: issue.title.clone(),
                kind: cells.get(2).copied().unwrap_or("").to_string(),
                evidence: cells.get(4).copied().unwrap_or("").to_string(),
            });
        }
        rows.push(row);
        sleep(THROTTLE);
    }

    let report = assemble_report(
        since,
        issues.len(),
        license_requests.len(),
        &candidates,
        &rows,
    );
    Ok((report, findings))
}

fn assemble_report(
    since: &str,
    total: usize,
    license_requests: usize,
    candidates: &[Issue],
    rows: &[String],
) -> String {
    let mut lines = vec![
        "## ScanCode -> Provenant weekly triage".to_string(),
        String::new(),
        format!(
            "Window: issues created since **{since}**. {total} total · {license_requests} \
             \"New license request\" (skipped) · {} candidates triaged.",
            candidates.len()
        ),
        String::new(),
        "| Issue | Type | Verdict | Evidence |".to_string(),
        "| --- | --- | --- | --- |".to_string(),
    ];
    lines.extend(rows.iter().cloned());
    lines.push(String::new());

    let worth: Vec<&String> = rows
        .iter()
        .filter(|r| r.to_lowercase().contains("worth fixing"))
        .collect();
    if worth.is_empty() {
        lines.push("_No `reproduces - worth fixing` items this week._".to_string());
    } else {
        lines.push("### Recommended next actions".to_string());
        for r in worth {
            // A well-formed row is: '' | Issue | Type | Verdict | Evidence | ''
            let cells: Vec<&str> = r.split('|').map(str::trim).collect();
            if cells.len() >= 5 {
                lines.push(format!("- {} — {}", cells[1], cells[4]));
            } else {
                lines.push(format!("- {}", r.trim()));
            }
        }
    }
    lines.join("\n")
}

/// Bounded per-issue tool-loop. Returns a single markdown table row.
fn triage_one_issue(
    client: &reqwest::blocking::Client,
    token: &str,
    model: &str,
    args: &Args,
    issue: &Issue,
) -> String {
    let n = issue.number;
    let fallback = |note: &str| {
        format!(
            "| [#{n}](https://github.com/{SCANCODE_REPO}/issues/{n}) | ? | needs manual check | {note} |"
        )
    };
    let user = format!(
        "Issue #{n} [{}] by {} (labels: {})\nTITLE: {}\nBODY:\n{}",
        issue.state,
        issue.author,
        if issue.labels.is_empty() {
            "none".to_string()
        } else {
            issue.labels.join(", ")
        },
        issue.title,
        truncate(&issue.body, ISSUE_BODY_CHARS),
    );
    let mut messages = vec![
        json!({"role": "system", "content": SYSTEM_PROMPT}),
        json!({"role": "user", "content": user}),
    ];

    for _turn in 0..MAX_TURNS {
        let resp = match call_model(client, token, model, &messages) {
            Ok(v) => v,
            Err(e) => return fallback(&format!("LLM error: {}", truncate(&e.to_string(), 80))),
        };
        let msg = &resp["choices"][0]["message"];
        let tool_calls = msg.get("tool_calls").cloned().filter(|v| !v.is_null());

        // Re-append a clean assistant message (drop provider-specific noise).
        let mut assistant = json!({"role": "assistant", "content": msg.get("content").cloned().unwrap_or(Value::Null)});
        if let Some(tc) = &tool_calls {
            assistant["tool_calls"] = tc.clone();
        }
        messages.push(assistant);

        let Some(tool_calls) = tool_calls.as_ref().and_then(Value::as_array) else {
            // No tool calls: the content is the final row.
            let content = msg.get("content").and_then(Value::as_str).unwrap_or("");
            return content
                .lines()
                .find(|l| l.trim_start().starts_with('|'))
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| fallback("no table row in model output"));
        };

        for tc in tool_calls {
            let name = tc["function"]["name"].as_str().unwrap_or("");
            let fargs: Value = tc["function"]["arguments"]
                .as_str()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| json!({}));
            let result = match name {
                "run_provenant" => {
                    eprintln!(
                        "    #{n}: run_provenant({}, {})",
                        fargs["cli_args"].as_str().unwrap_or(""),
                        fargs["url"]
                            .as_str()
                            .or(fargs["filename"].as_str())
                            .unwrap_or("inline")
                    );
                    run_provenant_tool(&args.binary, &fargs)
                }
                "list_parsers" => {
                    eprintln!("    #{n}: list_parsers()");
                    json!({ "parsers": list_parsers(&args.repo_root) })
                }
                other => json!({ "error": format!("unknown tool {other}") }),
            };
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc["id"].as_str().unwrap_or(""),
                "content": truncate(&result.to_string(), TOOL_RESULT_CHARS),
            }));
        }
        sleep(THROTTLE);
    }
    fallback("did not converge")
}

fn tools() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "run_provenant",
                "description": "Run a Provenant scan on a source file and return its \
                    license/copyright/author findings. Provide the file via a github \
                    blob/raw URL or inline text.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "cli_args": {"type": "string", "description": "detection flags, e.g. '--copyright' or '--license'"},
                        "url": {"type": "string", "description": "raw or blob URL of the fixture"},
                        "text": {"type": "string", "description": "inline file content (alternative to url)"},
                        "filename": {"type": "string", "description": "filename incl. extension (drives parser dispatch)"}
                    },
                    "required": ["cli_args"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_parsers",
                "description": "List Provenant's existing package parsers (by module name) to \
                    check whether an ecosystem/manifest is already supported.",
                "parameters": {"type": "object", "properties": {}}
            }
        }
    ])
}

fn call_model(
    client: &reqwest::blocking::Client,
    token: &str,
    model: &str,
    messages: &[Value],
) -> Result<Value> {
    let payload = json!({
        "model": model,
        "messages": messages,
        "tools": tools(),
        "temperature": 0.1,
    });
    let body = serde_json::to_vec(&payload)?;
    for attempt in 0..5u64 {
        let resp = client
            .post(MODELS_ENDPOINT)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send();
        match resp {
            Ok(r) if r.status().as_u16() == 429 => {
                let wait = 15 * (attempt + 1);
                eprintln!("  [rate limited, waiting {wait}s]");
                sleep(Duration::from_secs(wait));
            }
            Ok(r) => {
                let r = r.error_for_status()?;
                return Ok(serde_json::from_slice(&r.bytes()?)?);
            }
            Err(e) => return Err(anyhow!("model request failed: {e}")),
        }
    }
    bail!("exhausted retries (429)")
}

// ---- tools -----------------------------------------------------------------

fn run_provenant_tool(binary: &str, fargs: &Value) -> Value {
    let cli_args = fargs["cli_args"].as_str().unwrap_or("");
    let args = match validate_cli_args(cli_args) {
        Ok(a) => a,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    // A model-supplied filename must not escape the temp dir.
    let safe_name = fargs["filename"].as_str().and_then(|f| {
        Path::new(f)
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from)
    });

    let dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => return json!({ "error": format!("tempdir: {e}") }),
    };
    let target = if let Some(url) = fargs["url"].as_str() {
        let raw = to_raw_url(url);
        let name = safe_name.unwrap_or_else(|| {
            raw.split('?')
                .next()
                .and_then(|u| u.rsplit('/').next())
                .filter(|s| !s.is_empty())
                .unwrap_or("input.txt")
                .to_string()
        });
        let path = dir.path().join(name);
        match safe_fetch(&raw) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&path, bytes) {
                    return json!({ "error": format!("write: {e}") });
                }
            }
            Err(e) => return json!({ "error": format!("failed to fetch {raw}: {e}") }),
        }
        path
    } else if let Some(text) = fargs["text"].as_str() {
        let path = dir
            .path()
            .join(safe_name.unwrap_or_else(|| "input.txt".to_string()));
        if let Err(e) = std::fs::write(&path, text) {
            return json!({ "error": format!("write: {e}") });
        }
        path
    } else {
        return json!({ "error": "provide either url or text" });
    };

    let output = Command::new(binary)
        .args(&args)
        .args(["--json-pp", "-"])
        .arg(&target)
        .output();
    let output = match output {
        Ok(o) => o,
        Err(e) => return json!({ "error": format!("scan failed: {e}") }),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return json!({ "error": format!("exit {:?}", output.status.code()), "stderr": tail(&stderr, 500) });
    }
    let data: Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return json!({ "error": "non-JSON output", "stdout": tail(&stdout, 500) });
        }
    };
    let empty = Value::Array(vec![]);
    let f = data["files"].get(0).cloned().unwrap_or(Value::Null);
    let detections: Vec<Value> = f["license_detections"]
        .as_array()
        .unwrap_or(empty.as_array().unwrap())
        .iter()
        .map(|d| {
            json!({
                "license_expression": d["license_expression"],
                "matches": d["matches"].as_array().unwrap_or(empty.as_array().unwrap()).iter().map(|m| {
                    json!({"license_expression": m["license_expression"], "score": m["score"]})
                }).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "path": f["path"],
        "detected_license_expression": f["detected_license_expression"],
        "copyrights": f["copyrights"],
        "holders": f["holders"],
        "authors": f["authors"],
        "license_detections": detections,
    })
}

fn validate_cli_args(cli_args: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    for a in cli_args.split_whitespace() {
        if !ALLOWED_CLI_FLAGS.contains(&a) {
            bail!("disallowed scan argument: {a:?}");
        }
        args.push(a.to_string());
    }
    Ok(args)
}

fn to_raw_url(url: &str) -> String {
    if url.contains("github.com") && url.contains("/blob/") {
        url.replacen("github.com", "raw.githubusercontent.com", 1)
            .replacen("/blob/", "/", 1)
    } else {
        url.to_string()
    }
}

/// Fetch a fixture, but only from allowlisted GitHub hosts, with no cross-host
/// redirects and a hard size cap.
fn safe_fetch(url: &str) -> Result<Vec<u8>> {
    let parsed = url::Url::parse(url).with_context(|| format!("parsing {url}"))?;
    if parsed.scheme() != "https" {
        bail!("only https is allowed, got {:?}", parsed.scheme());
    }
    match parsed.host_str() {
        Some(h) if ALLOWED_FETCH_HOSTS.contains(&h) => {}
        other => bail!("host not allowlisted: {other:?}"),
    }
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        let host = attempt.url().host_str().unwrap_or("").to_string();
        if ALLOWED_FETCH_HOSTS.contains(&host.as_str()) {
            attempt.follow()
        } else {
            attempt.error(format!("redirect to non-allowlisted host: {host:?}"))
        }
    });
    let client = reqwest::blocking::Client::builder()
        .user_agent("provenant-triage")
        .redirect(policy)
        .build()?;
    let resp = client.get(url).send()?.error_for_status()?;
    let mut buf = Vec::new();
    resp.take(MAX_FETCH_BYTES + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_FETCH_BYTES {
        bail!("fixture exceeds {MAX_FETCH_BYTES} bytes");
    }
    Ok(buf)
}

fn list_parsers(repo_root: &Path) -> Vec<String> {
    let pdir = repo_root.join("src").join("parsers");
    let Ok(entries) = std::fs::read_dir(&pdir) else {
        return Vec::new();
    };
    let mut names = std::collections::BTreeSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let mut stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            for suffix in ["_test", "_golden_test", "_scan_test"] {
                if let Some(base) = stem.strip_suffix(suffix) {
                    stem = base.to_string();
                }
            }
            if stem != "mod" && !stem.is_empty() {
                names.insert(stem);
            }
        } else if path.is_dir()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            names.insert(name.to_string());
        }
    }
    names.into_iter().collect()
}

// ---- GitHub (via gh CLI) ---------------------------------------------------

fn gh_token() -> Result<String> {
    for var in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(v) = std::env::var(var)
            && !v.is_empty()
        {
            return Ok(v);
        }
    }
    let out = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .context("running `gh auth token`")?;
    if !out.status.success() {
        bail!("no GitHub token (GITHUB_TOKEN/GH_TOKEN unset and `gh auth token` failed)");
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn fetch_issues(since: &str) -> Result<Vec<Issue>> {
    let out = Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            SCANCODE_REPO,
            "--state",
            "all",
            "--limit",
            "150",
            "--search",
            &format!("created:>={since}"),
            "--json",
            "number,title,body,labels,author,state",
        ])
        .output()
        .context("running `gh issue list`")?;
    if !out.status.success() {
        bail!(
            "`gh issue list` failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let raw: Value = serde_json::from_slice(&out.stdout)?;
    let mut issues = Vec::new();
    for it in raw.as_array().unwrap_or(&vec![]).iter() {
        issues.push(Issue {
            number: it["number"].as_i64().unwrap_or(0),
            title: it["title"].as_str().unwrap_or("").to_string(),
            body: it["body"].as_str().unwrap_or("").to_string(),
            state: it["state"].as_str().unwrap_or("").to_string(),
            author: it["author"]["login"].as_str().unwrap_or("").to_string(),
            labels: it["labels"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|l| l["name"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        });
    }
    Ok(issues)
}

/// Open one Provenant issue per actionable finding, create-if-absent.
///
/// Findings are deduplicated by the ScanCode issue number embedded in the title
/// prefix `[ScanCode #N]`, so re-runs never open a duplicate for a finding that
/// still reproduces. Stale findings are left untouched — the bot only creates,
/// never edits or closes.
fn create_finding_issues(repo: &str, findings: &[Finding]) -> Result<()> {
    if findings.is_empty() {
        eprintln!("No actionable findings; no issues opened.");
        return Ok(());
    }

    // Ensure the label exists (idempotent).
    let _ = Command::new("gh")
        .args([
            "label",
            "create",
            TRIAGE_LABEL,
            "--repo",
            repo,
            "--color",
            "5319E7",
            "--description",
            "ScanCode issue that reproduces in Provenant (weekly triage)",
            "--force",
        ])
        .output();

    let out = Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "all",
            "--label",
            TRIAGE_LABEL,
            "--limit",
            "200",
            "--json",
            "title",
        ])
        .output()
        .context("listing existing finding issues")?;
    let listed: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    let existing_titles: Vec<String> = listed
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|i| i["title"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    for f in findings {
        let marker = format!("[ScanCode #{}]", f.scancode_number);
        if existing_titles.iter().any(|t| t.starts_with(&marker)) {
            eprintln!(
                "  ScanCode #{} already tracked; skipping",
                f.scancode_number
            );
            continue;
        }
        let title = format!(
            "{marker} {} — reproduces in Provenant",
            truncate(&f.scancode_title, 90)
        );
        let body = format!(
            "Surfaced by the weekly ScanCode → Provenant triage: this upstream issue \
             reproduces against the current Provenant build.\n\n\
             - **Upstream:** https://github.com/{SCANCODE_REPO}/issues/{}\n\
             - **Type:** {}\n\
             - **Evidence:** {}\n\n\
             Reproduce locally with the `scancode-triage` xtask tool (see \
             [`xtask/README.md`](../blob/main/xtask/README.md)) or scan the fixture the \
             upstream issue links.\n\n\
             <!-- scancode-triage:SC-{} -->",
            f.scancode_number, f.kind, f.evidence, f.scancode_number
        );
        run_gh(&[
            "issue",
            "create",
            "--repo",
            repo,
            "--title",
            &title,
            "--label",
            TRIAGE_LABEL,
            "--body",
            &body,
        ])?;
        eprintln!("  Opened issue for ScanCode #{}", f.scancode_number);
    }
    Ok(())
}

fn run_gh(args: &[&str]) -> Result<()> {
    let out = Command::new("gh")
        .args(args)
        .output()
        .context("running gh")?;
    if !out.status.success() {
        bail!(
            "gh {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

// ---- helpers ---------------------------------------------------------------

fn is_license_request(title: &str) -> bool {
    title
        .trim()
        .to_lowercase()
        .starts_with("new license request")
}

fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((idx, _)) => s[..idx].to_string(),
        None => s.to_string(),
    }
}

fn tail(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        s.chars().skip(n - max).collect()
    }
}

/// Today minus `days`, as YYYY-MM-DD (UTC), without a date-library dependency.
fn default_since(days: u32) -> Result<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let day_number = (now.as_secs() / 86_400) as i64 - days as i64;
    Ok(civil_from_days(day_number))
}

/// Convert a count of days since the Unix epoch to a YYYY-MM-DD civil date.
/// Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(z: i64) -> String {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
