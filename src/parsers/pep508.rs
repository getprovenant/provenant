// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use crate::parser_warn as warn;
use crate::parsers::utils::{CappedIterExt, MAX_FIELD_LENGTH, truncate_field};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pep508Requirement {
    pub name: String,
    pub extras: Vec<String>,
    pub specifiers: Option<String>,
    pub marker: Option<String>,
    pub url: Option<String>,
    pub is_name_at_url: bool,
}

pub(crate) fn parse_pep508_requirement(input: &str) -> Option<Pep508Requirement> {
    if input.len() > MAX_FIELD_LENGTH {
        warn!(
            "pep508: input exceeds MAX_FIELD_LENGTH ({} bytes), skipping",
            input.len()
        );
        return None;
    }

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.splitn(2, ';');
    let requirement_part = parts.next().unwrap_or_default().trim();
    let marker = parts
        .next()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if requirement_part.is_empty() {
        return None;
    }

    if let Some((name_part, url)) = split_name_at_url(requirement_part) {
        let (name, extras, _rest) = parse_name_and_extras(&name_part)?;
        return Some(Pep508Requirement {
            name: truncate_field(name),
            extras,
            specifiers: None,
            marker: marker.map(truncate_field),
            url: Some(truncate_field(url)),
            is_name_at_url: true,
        });
    }

    let (name, extras, rest) = parse_name_and_extras(requirement_part)?;
    let specifiers = normalize_specifiers(rest);

    // Whatever follows the name must be a version specifier. Accepting it
    // unchecked meant any prose starting with a word-like token parsed as a
    // requirement, so `import os` became `pkg:pypi/import` and a line of licence
    // text became a dependency named after its first word.
    if specifiers
        .as_deref()
        .is_some_and(|specifiers| !is_valid_specifier_set(specifiers))
    {
        return None;
    }

    Some(Pep508Requirement {
        name: truncate_field(name),
        extras,
        specifiers: specifiers.map(truncate_field),
        marker: marker.map(truncate_field),
        url: None,
        is_name_at_url: false,
    })
}

fn split_name_at_url(input: &str) -> Option<(String, String)> {
    if let Some((left, right)) = input.split_once(" @ ") {
        let name = left.trim();
        let url = right.trim();
        if !name.is_empty() && !url.is_empty() {
            return Some((name.to_string(), url.to_string()));
        }
    }

    if let Some((left, right)) = input.split_once('@') {
        let name = left.trim();
        let url = right.trim();
        if !name.is_empty() && !url.is_empty() && (url.contains("://") || url.starts_with("file:"))
        {
            return Some((name.to_string(), url.to_string()));
        }
    }

    None
}

/// True for a name matching PEP 508's `identifier` grammar:
/// `letterOrDigit (letterOrDigit | '-' | '_' | '.')* letterOrDigit`, i.e. it must
/// start and end alphanumeric and contain only alphanumerics and `-_.` between.
///
/// Without this check any text before the first specifier character is taken as a
/// distribution name, so a line that is not a requirement at all — a
/// reStructuredText `::` literal-block marker, a table rule, prose — becomes a
/// package, and its name is then spliced straight into a PURL. Rejecting the name
/// here lets callers fall through to their link/URL handling or drop the line,
/// which is what pip's own parser does with an unparsable requirement.
pub(crate) fn is_valid_distribution_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    let Some(last) = name.chars().next_back() else {
        return false;
    };
    if !last.is_ascii_alphanumeric() {
        return false;
    }
    name.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn parse_name_and_extras(input: &str) -> Option<(String, Vec<String>, &str)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut name_end = trimmed.len();
    for (idx, ch) in trimmed.char_indices().capped("pep508 name characters") {
        if ch == '[' || ch.is_whitespace() || matches!(ch, '<' | '>' | '=' | '!' | '~' | ';') {
            name_end = idx;
            break;
        }
    }

    let name = trimmed[..name_end].trim();
    if !is_valid_distribution_name(name) {
        return None;
    }

    let mut extras = Vec::new();
    let mut rest = &trimmed[name_end..];

    let rest_trimmed = rest.trim_start();
    if rest_trimmed.starts_with('[')
        && let Some(close_idx) = rest_trimmed.find(']')
    {
        let extras_str = &rest_trimmed[1..close_idx];
        extras = extras_str
            .split(',')
            .capped("pep508 extras")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| truncate_field(value.to_string()))
            .collect();
        rest = &rest_trimmed[close_idx + 1..];
    }

    Some((name.to_string(), extras, rest))
}

/// True for a PEP 440 specifier set: comma-separated clauses, each an operator
/// followed by a version. PEP 508 also permits the whole set to be parenthesised
/// (`foo (>=1.0)`), which `pyproject.toml` dependency strings do use.
///
/// Input arrives whitespace-stripped from [`normalize_specifiers`].
fn is_valid_specifier_set(specifiers: &str) -> bool {
    const OPERATORS: [&str; 8] = ["===", "==", "!=", "<=", ">=", "~=", "<", ">"];

    let specifiers = specifiers
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(specifiers);

    if specifiers.is_empty() {
        return false;
    }

    specifiers.split(',').all(|clause| {
        OPERATORS.iter().any(|operator| {
            clause.strip_prefix(operator).is_some_and(|version| {
                // `===` is PEP 440 arbitrary-string equality, so its operand is
                // deliberately unconstrained. Every other operator takes a
                // version, and accepting arbitrary text there let `hello==world`
                // through as `pkg:pypi/hello@world`.
                if *operator == "===" {
                    !version.is_empty()
                } else {
                    is_valid_version(version)
                }
            })
        })
    })
}

/// True for a PEP 440 version as it appears in a specifier: an optional `v`
/// prefix, then a digit, then only characters the grammar can produce — digits
/// and letters for pre/post/dev segments, `.` separators, `!` for an epoch, `+`
/// for a local version, `-`/`_` for the normalising forms, and `*` for the
/// `==1.4.*` prefix match.
fn is_valid_version(version: &str) -> bool {
    let version = version.strip_prefix(['v', 'V']).unwrap_or(version);
    version.starts_with(|ch: char| ch.is_ascii_digit())
        && version
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '!' | '+' | '-' | '_' | '*'))
}

fn normalize_specifiers(rest: &str) -> Option<String> {
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized: String = trimmed.chars().filter(|ch| !ch.is_whitespace()).collect();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

const MAX_MARKER_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkerToken {
    Open,
    Close,
    And,
    Or,
    Op(String),
    Var(String),
    Str(String),
}

enum MarkerExpr {
    And(Vec<MarkerExpr>),
    Or,
    Compare(MarkerToken, String, MarkerToken),
}

/// Comparisons on `variable` that hold whenever the PEP 508 `marker` holds,
/// rendered as `"<op> <value>"` with the variable on the left. Clauses under an
/// `or` are not unconditional and are skipped; an unparseable marker yields none.
pub(crate) fn marker_conjunct_comparisons(marker: &str, variable: &str) -> Vec<String> {
    if marker.len() > MAX_FIELD_LENGTH {
        return Vec::new();
    }
    let Some(tokens) = tokenize_marker(marker) else {
        return Vec::new();
    };
    let mut position = 0;
    let Some(expr) = parse_marker_or(&tokens, &mut position, 0) else {
        return Vec::new();
    };
    if position != tokens.len() {
        return Vec::new();
    }

    let mut clauses = Vec::new();
    collect_conjunct_comparisons(&expr, variable, &mut clauses);
    let mut seen = HashSet::new();
    clauses.retain(|clause| seen.insert(clause.clone()));
    clauses
}

fn tokenize_marker(marker: &str) -> Option<Vec<MarkerToken>> {
    let mut tokens = Vec::new();
    let mut chars = marker.char_indices().peekable();
    while let Some(&(start, ch)) = chars.peek() {
        match ch {
            c if c.is_whitespace() => {
                chars.next();
            }
            '(' => {
                chars.next();
                tokens.push(MarkerToken::Open);
            }
            ')' => {
                chars.next();
                tokens.push(MarkerToken::Close);
            }
            '\'' | '"' => {
                chars.next();
                let end = marker[start + 1..].find(ch)? + start + 1;
                tokens.push(MarkerToken::Str(marker[start + 1..end].to_string()));
                while chars.peek().is_some_and(|&(index, _)| index <= end) {
                    chars.next();
                }
            }
            '<' | '>' | '=' | '!' | '~' => {
                let rest = &marker[start..];
                let op = ["===", "~=", "==", "!=", "<=", ">=", "<", ">"]
                    .into_iter()
                    .find(|op| rest.starts_with(op))?;
                for _ in 0..op.len() {
                    chars.next();
                }
                tokens.push(MarkerToken::Op(op.to_string()));
            }
            c if c.is_ascii_alphanumeric() || c == '_' || c == '.' => {
                let mut end = start;
                while let Some(&(index, next)) = chars.peek() {
                    if !(next.is_ascii_alphanumeric() || next == '_' || next == '.') {
                        break;
                    }
                    end = index + next.len_utf8();
                    chars.next();
                }
                let word = &marker[start..end];
                let token = match word {
                    "and" => MarkerToken::And,
                    "or" => MarkerToken::Or,
                    "in" | "not" => MarkerToken::Op(word.to_string()),
                    _ => MarkerToken::Var(word.to_string()),
                };
                tokens.push(token);
            }
            _ => return None,
        }
    }
    Some(tokens)
}

fn parse_marker_or(
    tokens: &[MarkerToken],
    position: &mut usize,
    depth: usize,
) -> Option<MarkerExpr> {
    let first = parse_marker_and(tokens, position, depth)?;
    let mut is_disjunction = false;
    while tokens.get(*position) == Some(&MarkerToken::Or) {
        *position += 1;
        parse_marker_and(tokens, position, depth)?;
        is_disjunction = true;
    }
    Some(if is_disjunction {
        MarkerExpr::Or
    } else {
        first
    })
}

fn parse_marker_and(
    tokens: &[MarkerToken],
    position: &mut usize,
    depth: usize,
) -> Option<MarkerExpr> {
    let mut operands = vec![parse_marker_atom(tokens, position, depth)?];
    while tokens.get(*position) == Some(&MarkerToken::And) {
        *position += 1;
        operands.push(parse_marker_atom(tokens, position, depth)?);
    }
    Some(if operands.len() == 1 {
        operands.remove(0)
    } else {
        MarkerExpr::And(operands)
    })
}

fn parse_marker_atom(
    tokens: &[MarkerToken],
    position: &mut usize,
    depth: usize,
) -> Option<MarkerExpr> {
    if tokens.get(*position) == Some(&MarkerToken::Open) {
        if depth >= MAX_MARKER_DEPTH {
            return None;
        }
        *position += 1;
        let expr = parse_marker_or(tokens, position, depth + 1)?;
        if tokens.get(*position) != Some(&MarkerToken::Close) {
            return None;
        }
        *position += 1;
        return Some(expr);
    }

    let lhs = marker_operand(tokens.get(*position)?)?;
    *position += 1;
    let op = match tokens.get(*position)? {
        MarkerToken::Op(op) if op == "not" => {
            *position += 1;
            match tokens.get(*position)? {
                MarkerToken::Op(next) if next == "in" => "not in".to_string(),
                _ => return None,
            }
        }
        MarkerToken::Op(op) => op.clone(),
        _ => return None,
    };
    *position += 1;
    let rhs = marker_operand(tokens.get(*position)?)?;
    *position += 1;
    Some(MarkerExpr::Compare(lhs, op, rhs))
}

fn marker_operand(token: &MarkerToken) -> Option<MarkerToken> {
    matches!(token, MarkerToken::Var(_) | MarkerToken::Str(_)).then(|| token.clone())
}

fn collect_conjunct_comparisons(expr: &MarkerExpr, variable: &str, clauses: &mut Vec<String>) {
    match expr {
        MarkerExpr::And(operands) => {
            for operand in operands {
                collect_conjunct_comparisons(operand, variable, clauses);
            }
        }
        MarkerExpr::Or => {}
        MarkerExpr::Compare(lhs, op, rhs) => {
            let clause = match (lhs, rhs) {
                (MarkerToken::Var(name), MarkerToken::Str(value)) if name == variable => {
                    Some((op.as_str(), value))
                }
                (MarkerToken::Str(value), MarkerToken::Var(name)) if name == variable => {
                    reversed_marker_op(op).map(|op| (op, value))
                }
                _ => None,
            };
            if let Some((op, value)) = clause.filter(|(op, _)| !matches!(*op, "in" | "not in")) {
                clauses.push(format!("{op} {value}"));
            }
        }
    }
}

fn reversed_marker_op(op: &str) -> Option<&'static str> {
    match op {
        "<" => Some(">"),
        "<=" => Some(">="),
        ">" => Some("<"),
        ">=" => Some("<="),
        "==" => Some("=="),
        "!=" => Some("!="),
        "===" => Some("==="),
        _ => None,
    }
}
