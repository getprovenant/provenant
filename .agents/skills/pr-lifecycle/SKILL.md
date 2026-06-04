---
name: pr-lifecycle
description: Open a Provenant PR with the repo template and drive it to merge-ready — wait for CI and Greptile/human review, address comments by amending + force-pushing, resolve review threads, and iterate until zero unresolved. Use for opening a PR, handling review/Greptile comments, resolving review threads, or PR CI signals.
---

# PR Lifecycle

Use this skill to open a Provenant pull request and drive it to merge-ready: open with the repo template, wait for CI and review, address findings, resolve threads, and iterate until nothing is unresolved.

This skill owns only the operational loop and the non-obvious `gh`/GraphQL mechanics. It does not restate policy that other docs own.

## Source Documents

- `AGENTS.md` — Conventional Commits, DCO sign-off, PR scope discipline, `--template` vs `--body` rule.
- `CONTRIBUTING.md` — full contributor workflow, DCO, commit/PR conventions, validation defaults.
- `.github/pull_request_template.md` — required PR body structure.
- `ci-failure-triage` skill — map any red CI check to its owning surface and local reproduction.

The repo is `mstykow/provenant`; use that owner/name in every `gh api` and GraphQL call below.

## 1. Open the PR

- Conventional Commit title; commit with `git commit -s` (DCO); base `main`. Keep scope disciplined (one ecosystem family per PR per `AGENTS.md`).
- Render `.github/pull_request_template.md` MANUALLY into `--body`. Per `AGENTS.md`, do NOT combine `--template` with `--body`/`--body-file`. Omit sections that do not apply.
- **Keep the description reviewer-facing.** Do NOT restate validation CI already runs (build, clippy, fmt, unit/golden/integration suites) — CI is the source of truth for those. The "How to verify" section is a _test plan for a human reviewer_: the manual steps to exercise the change beyond CI (commands to run, what to observe, edge cases to poke). If CI fully covers it and there is nothing extra to try, say so in one line rather than listing the CI checks.
- End the body with the Claude Code footer line.

```bash
gh pr create --base main --head <branch> --title "<conventional title>" --body "<rendered template>"
```

## 2. Wait for Signals

Opening a PR does **not** obligate you to block on CI or review — it is fine to open one and move on rather than sit through a ~12-minute CI run. What matters is that a PR is not "done" at opened: **any time you return to a PR — later this turn or in a future session — resume the loop from Step 3 against the current head**, fetching fresh CI and review signals and addressing them. Treat an opened-but-unreviewed PR as unfinished work to come back to, not a finished task.

- CI: `gh pr checks <num> --watch` (or `gh run watch`). Map any failure to the `ci-failure-triage` skill.
- Review: Greptile posts a summary as an issue comment plus inline review comments; humans may comment too.

## 3. Fetch Comments

```bash
gh api repos/mstykow/provenant/issues/<num>/comments   # summary + human top-level
gh api repos/mstykow/provenant/pulls/<num>/comments    # inline review comments
gh api repos/mstykow/provenant/pulls/<num>/reviews     # review bodies/state
```

**CRITICAL — distinguish FRESH from STALE.** The inline-comments API returns ALL comments ever made, including ones from earlier reviews that GitHub carried forward onto the new commit. A comment is current only if it targets the PR head; treat the rest as stale.

```bash
head=$(gh api repos/mstykow/provenant/pulls/<num> --jq '.head.sha')
# Only comments on the PR head are current; the rest were carried forward and are stale.
# `gh api --jq` uses gojq (no `--arg`), so inject the value via the environment (env.HEAD).
HEAD="$head" gh api repos/mstykow/provenant/pulls/<num>/comments \
  --jq '.[] | select(.commit_id == env.HEAD) | {id, path, line, commit: .commit_id, body}'
```

Also treat `.outdated == true` or a null `.position` as outdated signals. Always verify against the actual current file content before acting.

## 4. Address Findings

- VERIFY each finding before fixing. The bot can be wrong, or even contradict its own earlier suggestion; confirm against the code/docs. If a finding is wrong, reply to the thread explaining why instead of "fixing" it.
- PREFER amending the existing commit and force-pushing over fixup commits:

```bash
git commit -s --amend --no-edit   # or drop --no-edit to revise the message
git push --force-with-lease origin <branch>
```

- Re-run the narrowest validation that proves the fix (focused `cargo build`/`clippy`/targeted tests, or docs/lint checks per `CONTRIBUTING.md`) before pushing.
- **Out-of-scope findings.** If a finding is valid but outside this PR's scope — an enhancement, an unrelated refactor, or work that belongs to another ecosystem/module per `AGENTS.md` scope discipline — do **not** silently expand the PR. Reply on the thread acknowledging it and say where it will live: a follow-up issue/PR, or an explicit "out of scope, declining" with the reason. Then open the follow-up if warranted. Decide explicitly rather than defaulting to fix-in-place; a finding being correct is not by itself a reason to widen this PR.

## 5. Reply and Resolve Threads

- **Always reply to every actionable thread — Greptile's included — with a short comment** saying what you changed (cite the commit) or, if you disagree, why. The reply is the transparency record that the feedback was considered; never address feedback silently. Post a threaded reply:

```bash
gh api repos/mstykow/provenant/pulls/<num>/comments \
  -f body='Addressed in <sha>: <what changed>.' -F in_reply_to=<comment_id>
```

- **Who resolves:**
  - **Greptile's own threads — leave resolution to Greptile.** It auto-resolves them on re-review once a force-push makes it detect the fix. Replying does **not** prevent this (a reply adds to the thread without reopening it), so reply freely. Only resolve a Greptile thread by hand if it fails to auto-resolve (it didn't re-review, or it left a fixed thread open).
  - **Human / non-Greptile-agent threads — resolve them yourself after replying** (and only after the feedback is actually addressed or rebutted).
- To resolve manually, use GraphQL. List threads:

```bash
gh api graphql -f query='query($num:Int!){repository(owner:"mstykow",name:"provenant"){pullRequest(number:$num){reviewThreads(first:100){nodes{id isResolved isOutdated comments(first:1){nodes{author{login} path}}} pageInfo{hasNextPage}}}}}' -F num=<num>
```

`first:100` covers virtually every PR; if `pageInfo.hasNextPage` is `true`, paginate with `after:<endCursor>` (add `endCursor` to the selection) rather than trusting a single page — otherwise the unresolved-thread count below can read falsely clean.

Resolve one thread by id:

```bash
gh api graphql -f query='mutation($t:ID!){resolveReviewThread(input:{threadId:$t}){thread{isResolved}}}' -F t=<threadId>
```

## 6. Iterate

Each force-push re-triggers CI + Greptile. Loop fetch → verify → fix → push → resolve until all of these hold:

- CI green.
- Zero unresolved review threads.
- An acceptable Greptile confidence.

Confirm zero unresolved threads with the list query in Step 5 (count nodes where `isResolved == false`). Do not block indefinitely waiting for review; report state when a pass finds nothing actionable.
