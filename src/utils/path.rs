// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::{MAIN_SEPARATOR, Path};

/// Render a filesystem path as a POSIX-style string with `/` separators.
///
/// Scan output must use `/` on every platform (ScanCode does, downstream tooling
/// expects it, and the internal path helpers here split on `/`). On Unix this is
/// just `to_string_lossy`; on Windows it rewrites the `\` separators the OS uses.
pub(crate) fn to_posix_string(path: &Path) -> String {
    path.to_string_lossy().replace(MAIN_SEPARATOR, "/")
}

pub(crate) fn parent_dir(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

pub(crate) fn parent_dir_for_lookup(path: &str) -> Option<&str> {
    if path.is_empty() {
        return None;
    }

    path.rsplit_once('/').map(|(parent, _)| parent).or(Some(""))
}

#[cfg(test)]
mod tests {
    use super::{parent_dir, parent_dir_for_lookup, to_posix_string};

    #[test]
    fn to_posix_string_uses_forward_slashes() {
        // Built from components so it uses the platform separator (\ on Windows,
        // / on Unix); the result must be POSIX on both.
        let path: std::path::PathBuf = ["packages", "app", "index.js"].iter().collect();
        assert_eq!(to_posix_string(&path), "packages/app/index.js");
    }

    #[test]
    fn parent_dir_handles_top_level_paths() {
        assert_eq!(parent_dir("package.json"), "");
        assert_eq!(parent_dir("packages/app/package.json"), "packages/app");
    }

    #[test]
    fn parent_dir_for_lookup_walks_up_to_empty_root() {
        assert_eq!(parent_dir_for_lookup("packages/app"), Some("packages"));
        assert_eq!(parent_dir_for_lookup("packages"), Some(""));
        assert_eq!(parent_dir_for_lookup(""), None);
    }
}
