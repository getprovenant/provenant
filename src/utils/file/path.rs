// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Path-oriented helpers: filesystem metadata, glob exclusion, and
//! extension/filename predicates used across the file-classification utilities.

use std::fs;
use std::path::Path;

use chrono::{TimeZone, Utc};
use glob::Pattern;

pub(super) const PLAIN_TEXT_EXTENSIONS: &[&str] = &[
    "rst", "rest", "md", "txt", "log", "json", "xml", "yaml", "yml", "toml", "ini",
];

/// Get the last modified date of a file as a `YYYY-MM-DD` string.
pub fn get_creation_date(metadata: &fs::Metadata) -> Option<String> {
    let time = metadata.modified().ok()?;
    let seconds_since_epoch = time.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;

    Some(
        Utc.timestamp_opt(seconds_since_epoch, 0)
            .single()
            .unwrap_or_else(Utc::now)
            .format("%Y-%m-%d")
            .to_string(),
    )
}

/// Check if a path should be excluded based on a list of glob patterns.
pub fn is_path_excluded(path: &Path, exclude_patterns: &[Pattern]) -> bool {
    let path_str = path.to_string_lossy();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();

    for pattern in exclude_patterns {
        // Match against full path
        if pattern.matches(&path_str) {
            return true;
        }

        // Match against just the file/directory name
        if pattern.matches(&file_name) {
            return true;
        }
    }

    false
}

pub(super) fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|ext| ext.to_str())
}

pub(super) fn lower_extension(path: &Path) -> Option<String> {
    extension(path).map(|ext| ext.to_ascii_lowercase())
}

pub(super) fn lower_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default()
}

pub(super) fn is_plain_text(path: &Path) -> bool {
    lower_extension(path)
        .as_deref()
        .is_some_and(|ext| PLAIN_TEXT_EXTENSIONS.contains(&ext))
}

pub(super) fn is_makefile(path: &Path) -> bool {
    matches!(lower_file_name(path).as_str(), "makefile" | "makefile.inc")
}

pub(super) fn is_source_map(path: &Path) -> bool {
    let path_lower = path.to_string_lossy().to_ascii_lowercase();
    path_lower.ends_with(".js.map") || path_lower.ends_with(".css.map")
}

pub(super) fn is_c_like_source(path: &Path) -> bool {
    lower_extension(path).as_deref().is_some_and(|ext| {
        matches!(
            ext,
            "c" | "cc"
                | "cp"
                | "cpp"
                | "cxx"
                | "c++"
                | "h"
                | "hh"
                | "hpp"
                | "hxx"
                | "h++"
                | "i"
                | "ii"
                | "m"
                | "s"
                | "asm"
        )
    })
}

pub(super) fn is_java_like_source(path: &Path) -> bool {
    lower_extension(path)
        .as_deref()
        .is_some_and(|ext| matches!(ext, "java" | "aj" | "jad" | "ajt"))
}
