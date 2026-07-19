// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared lexical path identity for assembly.
//!
//! A CLI scan root of `.` keeps a `./` prefix on [`crate::models::FileInfo`]
//! paths through the scanner pipeline. Declared workspace/member paths and
//! lexically resolved candidates omit that prefix, so raw string/`Path`
//! equality and `starts_with` checks silently miss. These helpers put both
//! shapes on the same identity.
//!
//! Assembly assumes POSIX `/` separators on `FileInfo.path` (the scanner
//! already normalizes via `to_posix_string`). This module does not treat `\`
//! as a path separator.

use std::path::{Path, PathBuf};

/// Lexically resolve `.` and `..` components in a path without touching the
/// filesystem, so a declared module path such as `./module-a` or
/// `../sibling-module` compares equal to the scanned path's own normalized
/// form.
///
/// Unresolved parent components are kept rather than discarded. Dropping them
/// would let an over-escaped module like `../../../module-a` collapse onto an
/// unrelated in-scan `module-a/` and incorrectly accept it as a reactor member.
pub(super) fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => match normalized.components().next_back() {
                Some(std::path::Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(std::path::Component::ParentDir) | None => {
                    normalized.push(std::path::Component::ParentDir.as_os_str());
                }
                // Never escape a filesystem root / Windows prefix.
                Some(std::path::Component::RootDir) | Some(std::path::Component::Prefix(_)) => {}
                // CurDir is skipped above, so it never accumulates here.
                Some(std::path::Component::CurDir) => unreachable!(),
            },
            other => normalized.push(other.as_os_str()),
        }
    }

    normalized
}

/// Lexically normalized form of a scanned [`crate::models::FileInfo`] path.
pub(super) fn scanned_path(path: &str) -> PathBuf {
    normalize_lexical_path(Path::new(path))
}

/// Parent directory of a scanned file path, with `.`/`..` lexically resolved.
///
/// A scan whose CLI input is `.` emits paths like `./module-a/pom.xml`; without
/// normalization those parents stay as `./module-a` and fail to match lexically
/// resolved member candidates such as `module-a`.
pub(super) fn scanned_file_dir(path: &str) -> Option<PathBuf> {
    Path::new(path).parent().map(normalize_lexical_path)
}

/// Trim whitespace and strip one leading `./` from a declared workspace/member
/// pattern so glob and exact matches compare against relative scan paths.
pub(super) fn strip_declared_dot_slash(pattern: &str) -> &str {
    let trimmed = pattern.trim();
    trimmed.strip_prefix("./").unwrap_or(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lexical_path_strips_curdir_keeps_unresolved_parent() {
        assert_eq!(
            normalize_lexical_path(Path::new(".")),
            PathBuf::new(),
            "`.` collapses to the empty relative root"
        );
        assert_eq!(
            normalize_lexical_path(Path::new("./module-a")),
            PathBuf::from("module-a")
        );
        assert_eq!(
            normalize_lexical_path(Path::new("../../../module-a")),
            PathBuf::from("../../../module-a"),
            "unresolved `..` must be preserved so over-escaped modules do not collapse"
        );
    }

    #[test]
    fn scanned_path_matches_dot_prefixed_fileinfo() {
        assert_eq!(
            scanned_path("./module-a/pom.xml"),
            PathBuf::from("module-a/pom.xml")
        );
        assert_eq!(
            scanned_path("module-a/pom.xml"),
            PathBuf::from("module-a/pom.xml")
        );
    }

    #[test]
    fn scanned_file_dir_handles_dot_root_and_empty_parent() {
        assert_eq!(
            scanned_file_dir("./pom.xml").as_deref(),
            Some(Path::new("")),
            "parent of `./pom.xml` is `.`, which normalizes to empty"
        );
        assert_eq!(
            scanned_file_dir("pom.xml").as_deref(),
            Some(Path::new("")),
            "parent of a root-level file is already empty"
        );
        assert_eq!(
            scanned_file_dir("./module-a/src/Foo.java").as_deref(),
            Some(Path::new("module-a/src"))
        );
    }

    #[test]
    fn strip_declared_dot_slash_trims_and_strips_one_prefix() {
        assert_eq!(strip_declared_dot_slash("  ./packages/*  "), "packages/*");
        assert_eq!(strip_declared_dot_slash("crates/foo"), "crates/foo");
        assert_eq!(
            strip_declared_dot_slash("././nested"),
            "./nested",
            "only one leading `./` is stripped"
        );
    }
}
