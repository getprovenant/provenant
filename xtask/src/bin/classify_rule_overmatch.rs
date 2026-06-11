// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Data-driven classifier for upstream license-rule overmatch risk.
//!
//! The bundled rule overlays in `index_build_policy.toml` were each added
//! reactively after a specific benchmark target tripped on one rule. They
//! cluster into a small number of *systematic* root causes. This tool scores
//! every upstream rule against the same risk signals those overlays encode and
//! reports the rules that share a root cause but are not yet covered by an
//! overlay, so curation can move from case-by-case to data-driven.
//!
//! Risk classes (mirroring the existing overlays):
//!
//! - `BareWeakWord`: bare/weak GPL-family shorthand mapped to an unversioned
//!   bucket; too short to assert a concrete license, so it should be clue-only
//!   (e.g. `gpl_bare_word_only`, `gpl-1.0-plus_351`, `agpl-3.0-plus_101`).
//! - `VersionMismatch`: a short notice whose expression asserts a *specific
//!   elevated* GPL/LGPL/AGPL version but whose text lacks the matching version
//!   digit anchor, so it overmatches a different version.
//! - `BareReferencedFilename`: a short notice that asserts a license via a bare
//!   referenced filename (COPYING/LICENSE) with no independent version anchor,
//!   so the filename can be inherited by text that never names it
//!   (e.g. `gpl-2.0-plus_239`).
//! - `BsdEndorsement`: a short BSD-3-Clause (`bsd-new`) text rule that is
//!   neither continuous nor full-coverage, so BSD-2-style headers lacking the
//!   endorsement clause partially overmatch it (e.g. `bsd-new_99`).
//!
//! This is a reporting tool only: it never edits the dataset or the policy.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use provenant::license_detection::SCANCODE_LICENSES_RULES_PATH;
use provenant::license_detection::build_policy::default_index_build_policy;
use provenant::license_detection::models::{LoadedRule, RuleKind};
use provenant::license_detection::rules::load_loaded_rules_from_directory;

#[derive(Parser, Debug)]
#[command(
    name = "classify-rule-overmatch",
    about = "Classify upstream license rules by overmatch-risk class and rank un-covered candidates"
)]
struct Args {
    #[arg(
        long,
        help = "Rules directory (defaults to the bundled ScanCode corpus)"
    )]
    rules: Option<PathBuf>,

    #[arg(long, help = "Emit JSON instead of a text report")]
    json: bool,

    #[arg(
        long,
        default_value_t = 40,
        help = "Maximum candidates to print per risk class in text mode"
    )]
    top: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RiskClass {
    BareWeakWord,
    VersionMismatch,
    BareReferencedFilename,
    BsdEndorsement,
}

impl RiskClass {
    fn label(self) -> &'static str {
        match self {
            RiskClass::BareWeakWord => "BareWeakWord",
            RiskClass::VersionMismatch => "VersionMismatch",
            RiskClass::BareReferencedFilename => "BareReferencedFilename",
            RiskClass::BsdEndorsement => "BsdEndorsement",
        }
    }

    fn treatment(self) -> &'static str {
        match self {
            RiskClass::BareWeakWord => "demote to clue-only (is_license_clue)",
            RiskClass::VersionMismatch => "add version-anchor required phrase or reclassify",
            RiskClass::BareReferencedFilename => "require the referenced-filename phrase",
            RiskClass::BsdEndorsement => "require continuous / full coverage",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct Candidate {
    identifier: String,
    class: &'static str,
    score: u32,
    license_expression: String,
    rule_kind: String,
    relevance: u8,
    token_len: usize,
    already_covered: bool,
    reason: String,
}

fn word_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric() && c != '+')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

/// GPL-family license keys carry an explicit version in the expression, so a
/// rule text that lacks that version digit is a version-mismatch candidate.
///
/// Operates on the leading license term so compound expressions
/// (`gpl-1.0-plus OR lgpl-2.0-plus`) are judged by their primary GPL term.
fn declared_version_digit(expr: &str) -> Option<char> {
    // expressions look like gpl-1.0-plus, lgpl-2.1, agpl-3.0-plus, gpl-2.0-only
    let lower = leading_term(expr);
    let stem = lower
        .strip_prefix("agpl-")
        .or_else(|| lower.strip_prefix("lgpl-"))
        .or_else(|| lower.strip_prefix("gpl-"))?;
    stem.chars().next().filter(|c| c.is_ascii_digit())
}

/// The first license key in a (possibly compound) expression, lowercased and
/// with any leading paren stripped, e.g. `(gpl-2.0-plus OR mit)` -> `gpl-2.0-plus`.
fn leading_term(expr: &str) -> String {
    expr.to_ascii_lowercase()
        .trim_start_matches('(')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

fn is_gpl_family(expr: &str) -> bool {
    let lower = leading_term(expr);
    lower.starts_with("gpl-")
        || lower.starts_with("lgpl-")
        || lower.starts_with("agpl-")
        || lower == "gpl"
        || lower == "lgpl"
        || lower == "agpl"
}

/// True when the expression is the lowest/unspecified-version bucket for its
/// family. ScanCode deliberately maps a *bare* "GPL"/"LGPL"/"AGPL" mention with
/// no stated version to these keys (GPL -> gpl-1.0-plus, LGPL -> lgpl-2.0-plus,
/// AGPL -> agpl-3.0-plus, the only AGPL version). A short notice using one of
/// these keys without a digit is therefore the *intended* unversioned reading,
/// not a version-mismatch overmatch. Such cases belong to the clue-only
/// `BareWeakWord` treatment, not to `VersionMismatch`.
fn is_unversioned_bucket(expr: &str) -> bool {
    let lower = leading_term(expr);
    matches!(
        lower.as_str(),
        "gpl"
            | "lgpl"
            | "agpl"
            | "gpl-1.0"
            | "gpl-1.0-plus"
            | "lgpl-2.0"
            | "lgpl-2.0-plus"
            | "agpl-3.0"
            | "agpl-3.0-plus"
    )
}

/// True when the text contains a version anchor for the declared digit.
///
/// Covers the many shapes a version can take in a GPL-family notice:
/// - separated:  "version 2", "v 2"
/// - glued to a number word: "v2", "gplv2", "version2"
/// - glued to the license acronym: "gpl2", "agpl3", "lgpl21"
/// - a standalone numeric token: "2", "2.0", "3+"
/// - the spelled-out ordinal: "two", "three"
fn text_has_version_anchor(tokens: &[String], digit: char) -> bool {
    let dstr = digit.to_string();
    let ordinal = match digit {
        '1' => "one",
        '2' => "two",
        '3' => "three",
        _ => "",
    };
    for (i, tok) in tokens.iter().enumerate() {
        if (tok == "version" || tok == "v")
            && tokens
                .get(i + 1)
                .is_some_and(|n| n.starts_with(digit) || (!ordinal.is_empty() && n == ordinal))
        {
            return true;
        }
        // glued forms: any alphabetic prefix immediately followed by the digit,
        // e.g. v2, gplv2, version2, gpl2, agpl3, lgpl21.
        if tok.contains(&format!("v{dstr}"))
            || tok.contains(&format!("version{dstr}"))
            || tok.contains(&format!("gpl{dstr}"))
        {
            return true;
        }
        // a standalone numeric token that begins with the digit (e.g. "2", "2.0").
        if tok.starts_with(digit) && tok.chars().all(|c| c.is_ascii_digit() || c == '+') {
            return true;
        }
    }
    false
}

fn classify(rule: &LoadedRule) -> Option<(RiskClass, u32, String)> {
    // False positives, clue rules, and deprecated rules are already harmless or
    // out of scope; skip them. Rules whose text already carries inline required
    // phrases (`{{...}}`) are likewise already guarded against partial overmatch,
    // so they are not curation candidates.
    if rule.is_false_positive
        || rule.rule_kind == RuleKind::Clue
        || rule.is_deprecated
        || rule.text.contains("{{")
    {
        return None;
    }

    let expr = &rule.license_expression;
    let tokens = word_tokens(&rule.text);
    let token_len = tokens.len();
    let relevance = rule.relevance.unwrap_or(100);
    let lower_text = rule.text.to_ascii_lowercase();

    // Class 1: bare/weak GPL-family shorthand mapped to an unversioned-bucket
    // expression. Short, GPL-family, asserts a concrete license as
    // text/notice/reference/tag with high relevance, and the text carries no
    // version digit at all. These are the cases the existing
    // `gpl_bare_word_only` / `agpl-3.0-plus_101` overlays demote to clue-only.
    // We allow up to 7 tokens here so short phrasal mentions ("AGPL-licensed
    // open source project") fold into the same root cause rather than masquerade
    // as a version mismatch.
    if is_gpl_family(expr)
        && is_unversioned_bucket(expr)
        && token_len <= 7
        && matches!(
            rule.rule_kind,
            RuleKind::Reference | RuleKind::Tag | RuleKind::Notice | RuleKind::Text
        )
        && relevance >= 50
    {
        let has_any_version = tokens.iter().any(|t| t.chars().any(|c| c.is_ascii_digit()));
        if !has_any_version {
            let score = 100u32.saturating_sub(token_len as u32 * 8) + u32::from(relevance);
            return Some((
                RiskClass::BareWeakWord,
                score,
                format!("bare GPL-family shorthand ({token_len} tokens, no version anchor)"),
            ));
        }
    }

    // Class 2: a *short* versioned GPL-family notice whose text lacks the
    // matching version digit anchor. Restricting to short fragments is what
    // makes this an overmatch risk: a long notice that omits the digit usually
    // carries enough other version-specific wording to disambiguate, while a
    // short fragment ("under the GNU GPL") can be claimed by any version. We
    // skip anything already guarded by a required phrase or stored minimum
    // coverage (deprecated rules are already filtered at the top of `classify`).
    if is_gpl_family(expr)
        && !is_unversioned_bucket(expr)
        && (5..=20).contains(&token_len)
        && rule.rule_kind == RuleKind::Notice
        && !rule.is_required_phrase
        && rule.minimum_coverage.is_none()
        && let Some(digit) = declared_version_digit(expr)
        && !text_has_version_anchor(&tokens, digit)
    {
        let mentions_license = lower_text.contains("general public license")
            || lower_text.contains("gpl")
            || lower_text.contains("lesser");
        if mentions_license {
            // Shorter fragments are riskier (less disambiguating context).
            let brevity = 30u32.saturating_sub(token_len as u32);
            let score = 40 + brevity + u32::from(relevance) / 10;
            return Some((
                RiskClass::VersionMismatch,
                score,
                format!(
                    "'{expr}' asserts version {digit} but the short text has no matching version anchor"
                ),
            ));
        }
    }

    // Class 3: license asserted through a bare referenced filename with no
    // required-phrase / continuous guard. The filename gets inherited by text
    // that never names it.
    if let Some(refs) = &rule.referenced_filenames {
        let bare_license_filename = refs.iter().any(|f| {
            let lf = f.to_ascii_lowercase();
            let base = lf.rsplit(['/', '\\']).next().unwrap_or(&lf);
            matches!(
                base,
                "copying" | "license" | "licence" | "copying.lib" | "notice"
            )
        });
        if bare_license_filename
            && rule.rule_kind == RuleKind::Notice
            && !rule.is_required_phrase
            && !rule.is_continuous
            && (5..=25).contains(&token_len)
        {
            // The risk is real only when the filename actually appears in the text
            // (so a partial match that drops it would inherit it) AND the notice has
            // no independent version anchor of its own. A short notice that already
            // states "version 2" cannot be claimed by the wrong version even if the
            // referenced filename is dropped, so it is not an overmatch candidate.
            let names_file = refs.iter().any(|f| {
                let base = f
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(f)
                    .to_ascii_lowercase();
                lower_text.contains(&base)
            });
            let has_self_version =
                declared_version_digit(expr).is_some_and(|d| text_has_version_anchor(&tokens, d));
            if names_file && !has_self_version {
                let brevity = 30u32.saturating_sub(token_len as u32);
                let score = 30 + brevity + u32::from(relevance) / 10;
                return Some((
                    RiskClass::BareReferencedFilename,
                    score,
                    "short notice asserts license via bare COPYING/LICENSE reference with no independent version anchor".to_string(),
                ));
            }
        }
    }

    // Class 4: a *short* BSD-3-Clause (`bsd-new`) rule that contains the
    // endorsement clause but is neither continuous nor full coverage. The full
    // BSD-3 license text (~190-220 tokens) is a correct, complete match and is
    // NOT an overmatch risk, so we only flag header-sized rules: those short
    // enough that a BSD-2 header lacking the endorsement clause could partially
    // overmatch them on the shared redistribution wording.
    if expr.eq_ignore_ascii_case("bsd-new")
        && rule.rule_kind == RuleKind::Text
        && (10..=90).contains(&token_len)
        && lower_text.contains("redistribution")
        && lower_text.contains("endorse")
        && !rule.is_continuous
        && rule.minimum_coverage != Some(100)
    {
        let brevity = 100u32.saturating_sub(token_len as u32);
        let score = 20 + brevity / 2 + u32::from(relevance) / 10;
        return Some((
            RiskClass::BsdEndorsement,
            score,
            "short BSD-3 endorsement-clause rule lacks continuous/full-coverage guard".to_string(),
        ));
    }

    None
}

fn covered_identifiers() -> HashSet<String> {
    let policy = default_index_build_policy();
    policy
        .overlay_reasons
        .rules
        .keys()
        .map(|k| k.trim_end_matches(".RULE").to_string())
        .chain(
            policy
                .ignored_rules
                .iter()
                .map(|k| k.trim_end_matches(".RULE").to_string()),
        )
        .collect()
}

fn main() -> Result<()> {
    let args = Args::parse();

    let rules_dir = args
        .rules
        .unwrap_or_else(|| PathBuf::from(SCANCODE_LICENSES_RULES_PATH));

    let covered = covered_identifiers();

    let rules = load_loaded_rules_from_directory(&rules_dir)
        .with_context(|| format!("loading rules from {}", rules_dir.display()))?;

    let mut candidates: Vec<Candidate> = Vec::new();
    for rule in &rules {
        if let Some((class, score, reason)) = classify(rule) {
            let stem = rule.identifier.trim_end_matches(".RULE").to_string();
            candidates.push(Candidate {
                already_covered: covered.contains(&stem),
                identifier: rule.identifier.clone(),
                class: class.label(),
                score,
                license_expression: rule.license_expression.clone(),
                rule_kind: format!("{:?}", rule.rule_kind),
                relevance: rule.relevance.unwrap_or(100),
                token_len: word_tokens(&rule.text).len(),
                reason,
            });
        }
    }

    candidates.sort_by(|a, b| b.score.cmp(&a.score).then(a.identifier.cmp(&b.identifier)));

    if args.json {
        println!("{}", serde_json::to_string_pretty(&candidates)?);
        return Ok(());
    }

    let total = candidates.len();
    let covered_count = candidates.iter().filter(|c| c.already_covered).count();
    let uncovered_count = total - covered_count;

    println!("# License rule overmatch classifier");
    println!();
    println!("Scanned {} upstream rules.", rules.len());
    println!(
        "Flagged {total} rules across the systematic risk classes ({covered_count} already covered by an overlay, {uncovered_count} not yet covered).",
    );
    println!();

    let mut by_class: BTreeMap<&str, Vec<&Candidate>> = BTreeMap::new();
    for c in &candidates {
        by_class.entry(c.class).or_default().push(c);
    }

    for class in [
        RiskClass::BareWeakWord,
        RiskClass::VersionMismatch,
        RiskClass::BareReferencedFilename,
        RiskClass::BsdEndorsement,
    ] {
        let label = class.label();
        let empty = Vec::new();
        let entries = by_class.get(label).unwrap_or(&empty);
        let uncovered: Vec<&&Candidate> = entries.iter().filter(|c| !c.already_covered).collect();
        let covered_in_class = entries.len() - uncovered.len();
        println!(
            "## {label}  ({} total, {} not covered)",
            entries.len(),
            uncovered.len()
        );
        println!("Suggested treatment: {}", class.treatment());
        println!(
            "Existing overlays already cover {covered_in_class} of these (validates the signal)."
        );
        println!();
        for c in uncovered.iter().take(args.top) {
            println!(
                "  [{:>3}] {:<40} {:<14} rel={:<3} tok={:<3}  {}",
                c.score, c.identifier, c.license_expression, c.relevance, c.token_len, c.reason
            );
        }
        if uncovered.len() > args.top {
            println!("  ... and {} more", uncovered.len() - args.top);
        }
        println!();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(expr: &str, text: &str) -> LoadedRule {
        LoadedRule {
            identifier: "test.RULE".to_string(),
            license_expression: expr.to_string(),
            text: text.to_string(),
            rule_kind: RuleKind::Notice,
            is_false_positive: false,
            is_required_phrase: false,
            skip_for_required_phrase_generation: false,
            relevance: Some(100),
            minimum_coverage: None,
            has_stored_minimum_coverage: false,
            is_continuous: false,
            referenced_filenames: None,
            ignorable_urls: None,
            ignorable_emails: None,
            ignorable_copyrights: None,
            ignorable_holders: None,
            ignorable_authors: None,
            language: None,
            notes: None,
            is_deprecated: false,
            replaced_by: vec![],
        }
    }

    #[test]
    fn detects_glued_version_forms() {
        assert!(text_has_version_anchor(
            &word_tokens("under the agpl3"),
            '3'
        ));
        assert!(text_has_version_anchor(&word_tokens("gplv2 only"), '2'));
        assert!(text_has_version_anchor(&word_tokens("gpl-3 license"), '3'));
        assert!(text_has_version_anchor(
            &word_tokens("version 2 or later"),
            '2'
        ));
        assert!(!text_has_version_anchor(&word_tokens("under the gpl"), '2'));
    }

    #[test]
    fn unversioned_bucket_uses_leading_term() {
        assert!(is_unversioned_bucket("gpl-1.0-plus"));
        assert!(is_unversioned_bucket("agpl-3.0-plus"));
        assert!(is_unversioned_bucket("gpl-1.0-plus OR lgpl-2.0-plus"));
        assert!(!is_unversioned_bucket("gpl-2.0-plus"));
        assert!(!is_unversioned_bucket("lgpl-2.1-plus"));
    }

    #[test]
    fn bare_elevated_gpl_notice_is_version_mismatch() {
        // "freely available under the GPL" mapped to gpl-2.0-plus: bare wording,
        // elevated version, no anchor -> a version-mismatch candidate.
        let rule = notice("gpl-2.0-plus", "It is freely available under the GPL");
        let (class, _, _) = classify(&rule).expect("should be flagged");
        assert_eq!(class, RiskClass::VersionMismatch);
    }

    #[test]
    fn bare_unversioned_gpl_notice_is_bare_weak_word_not_mismatch() {
        // Same bare wording, but mapped to the unversioned bucket: this is the
        // intended reading, classified as clue-only BareWeakWord, not a mismatch.
        let rule = notice("gpl-1.0-plus", "available under the GPL");
        let (class, _, _) = classify(&rule).expect("should be flagged");
        assert_eq!(class, RiskClass::BareWeakWord);
    }

    #[test]
    fn versioned_gpl_notice_is_not_flagged() {
        // A notice that names its version is correct and must not be flagged.
        let rule = notice(
            "gpl-2.0-plus",
            "licensed under the GNU GPL version 2 or later",
        );
        assert!(classify(&rule).is_none());
    }

    #[test]
    fn inline_required_phrase_is_already_guarded() {
        let rule = notice("gpl-2.0-plus", "available under the {{GNU GPL}}");
        assert!(classify(&rule).is_none());
    }
}
