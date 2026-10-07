// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::pep508::marker_conjunct_comparisons;

fn python_version(marker: &str) -> Vec<String> {
    marker_conjunct_comparisons(marker, "python_version")
}

#[test]
fn test_marker_comparisons_accept_every_pep508_operator() {
    for op in ["<", "<=", "!=", "==", ">=", ">", "~=", "==="] {
        assert_eq!(
            python_version(&format!("python_version {op} \"3.8\"")),
            vec![format!("{op} 3.8")]
        );
    }
}

#[test]
fn test_marker_comparisons_find_clause_after_other_comparisons() {
    assert_eq!(
        python_version(r#"extra == "dev" and python_version < "3.10""#),
        vec!["< 3.10"]
    );
    assert_eq!(
        marker_conjunct_comparisons(
            r#"python_version < "3.10" and sys_platform == "win32""#,
            "sys_platform"
        ),
        vec!["== win32"]
    );
}

#[test]
fn test_marker_comparisons_normalize_reversed_operands() {
    assert_eq!(python_version(r#""3.8" <= python_version"#), vec![">= 3.8"]);
    assert_eq!(python_version(r#"'3.12' > python_version"#), vec!["< 3.12"]);
    assert!(python_version(r#""3.8" ~= python_version"#).is_empty());
}

#[test]
fn test_marker_comparisons_collect_every_conjunct_in_order() {
    assert_eq!(
        python_version(r#"python_version >= "3.8" and (extra == "a" and python_version < "3.12")"#),
        vec![">= 3.8", "< 3.12"]
    );
}

#[test]
fn test_marker_comparisons_keep_quoted_whitespace_and_dedupe() {
    assert_eq!(
        marker_conjunct_comparisons(r#"sys_platform != " win32 ""#, "sys_platform"),
        vec!["!=  win32 "]
    );
    assert_eq!(
        python_version(r#"python_version < "3.10" and python_version < "3.10""#),
        vec!["< 3.10"]
    );
}

#[test]
fn test_marker_comparisons_handle_long_flat_markers() {
    let marker = (0..20_000)
        .map(|minor| format!("python_version != \"3.{minor}\""))
        .collect::<Vec<_>>()
        .join(" and ");
    let clauses = python_version(&marker);
    assert_eq!(clauses.len(), 20_000);
    assert_eq!(clauses[19_999], "!= 3.19999");
}

#[test]
fn test_marker_comparisons_skip_disjunctions_and_membership() {
    assert!(python_version(r#"python_version < "3" or python_version >= "3.8""#).is_empty());
    assert_eq!(
        python_version(r#"(extra == "a" or python_version < "3") and python_version != "3.9""#),
        vec!["!= 3.9"]
    );
    assert!(python_version(r#"python_version in "2.7 3.4""#).is_empty());
    assert!(python_version(r#"python_version not in "2.7 3.4""#).is_empty());
}

#[test]
fn test_marker_comparisons_reject_malformed_markers() {
    assert!(python_version(r#"python_version < "3.10" and"#).is_empty());
    assert!(python_version(r#"(python_version < "3.10""#).is_empty());
    assert!(python_version(r#"python_version < "3.10"#).is_empty());
    let deep = format!("{}python_version < \"3\"{}", "(".repeat(64), ")".repeat(64));
    assert!(python_version(&deep).is_empty());
}
