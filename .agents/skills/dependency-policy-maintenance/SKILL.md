---
name: dependency-policy-maintenance
description: Maintain Provenant's Rust dependency policy and Cargo dependency changes. Use for cargo add/remove/update, Cargo.toml, Cargo.lock, deny.toml, cargo-deny, cargo-machete, unused deps, dependency licenses, advisories, or crate-size policy.
---

# Dependency Policy Maintenance

Use this skill when adding, removing, updating, or reviewing Rust dependencies in the Provenant repository. This is about Provenant's own build dependencies, not package/dependency data extracted by scans.

## Best Fit

Use this skill when the task says:

- add, remove, update, or replace a Rust crate
- fix `cargo deny`, advisory, license, source, or bans failures
- fix `cargo machete` or unused dependency failures
- sort Cargo manifests
- investigate crate size changes caused by dependencies
- adjust `deny.toml` or dependency policy

## High-Signal Gotchas

- `cargo add`, `cargo remove`, and targeted `cargo update` are preferred; manual edits are a last resort.
- Cargo manifest sorting is checked separately from rustfmt.
- The crate-size check can fail even when `cargo deny` and `cargo machete` pass.
- Prefer feature minimization before accepting binary-size growth from a new dependency.
- `cargo machete` can reveal dependencies that are unused after feature or target changes.
- Do not weaken `deny.toml` to make a dependency pass unless the policy exception is intentional, justified, and reviewed.
- This skill is unrelated to package dependencies discovered by Provenant scans; use parser or CLI skills for scan output behavior.

## Source Documents

- `CONTRIBUTING.md` - dependency-change expectations
- `Cargo.toml` and workspace crate manifests - declared dependencies
- `Cargo.lock` - resolved dependency graph
- `deny.toml` - dependency policy
- `.github/workflows/check.yml` - dependency, size, sorting, and unused-dep checks
- `package.json` - local helper scripts
- `scripts/README.md` - script ownership and usage

## Workflow

### 1. Prefer Cargo commands over hand edits

Use Cargo tooling for dependency mutations:

```bash
cargo add <crate> [-p <package>]
cargo remove <crate> [-p <package>]
cargo update -p <crate> --precise <version>
```

Do not edit dependency versions or `Cargo.lock` by hand unless there is no Cargo-supported path and you can explain why.

### 2. Justify new dependencies

Before adding a crate, check whether existing dependencies or standard library code are sufficient. New dependencies need a clear maintenance, security, license, and binary-size justification.

Prefer small, maintained, well-scoped crates. Avoid adding broad framework dependencies for narrow parsing or utility tasks.

### 3. Run policy checks narrowly first

Common local checks:

```bash
npm run format:manifests:check
cargo deny check advisories bans licenses sources
./scripts/check_unused_deps.sh
./scripts/check_crate_size.sh
```

If manifest ordering is wrong, repair with:

```bash
npm run format:manifests
```

### 4. Fix the right layer

- Unused dependency: remove it or move it to the correct crate/target.
- Advisory: update the affected crate narrowly or replace the dependency if no safe version exists.
- License/source/bans issue: prefer a dependency choice that satisfies policy over weakening `deny.toml`.
- Crate-size regression: confirm the dependency is necessary and whether features can be reduced.

### 5. Validate affected code

Run the narrow tests or checks for code that uses the dependency, then the relevant policy checks. Do not substitute policy checks for behavior tests.

## Boundaries

- Parser ecosystem dependency extraction belongs to `add-parser`.
- CLI scan dependency output belongs to `provenant-cli`.
- CI symptom routing belongs to `ci-failure-triage`; this skill owns the dependency-policy fix once identified.
