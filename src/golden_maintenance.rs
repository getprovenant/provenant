// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

fn prettier_parser_for_path(path: &Path) -> Option<&'static str> {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".expected"))
    {
        Some("json")
    } else {
        None
    }
}

pub fn find_files_with_extension(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if !dir.is_dir() {
        return Ok(paths);
    }

    fn recurse(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                recurse(&path, extension, out)?;
            } else if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == extension)
            {
                out.push(path);
            }
        }
        Ok(())
    }

    recurse(dir, extension, &mut paths)?;
    paths.sort();
    Ok(paths)
}

pub fn run_prettier(paths: &[PathBuf]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    let ignore_path = std::env::temp_dir().join("provenant-empty-prettierignore");
    if !ignore_path.exists() {
        fs::write(&ignore_path, "").context("failed to create empty prettier ignore file")?;
    }

    const CHUNK_SIZE: usize = 100;

    let mut default_paths = Vec::new();
    let mut json_paths = Vec::new();

    for path in paths {
        match prettier_parser_for_path(path) {
            Some("json") => json_paths.push(path.clone()),
            _ => default_paths.push(path.clone()),
        }
    }

    for (parser, parser_paths) in [(None, default_paths), (Some("json"), json_paths)] {
        for chunk in parser_paths.chunks(CHUNK_SIZE) {
            let mut cmd = Command::new("npm");
            cmd.args([
                "exec",
                "--",
                "prettier",
                "--write",
                "--ignore-path",
                ignore_path
                    .to_str()
                    .context("temporary prettier ignore path is not valid UTF-8")?,
            ]);
            if let Some(parser) = parser {
                cmd.args(["--parser", parser]);
            }
            for path in chunk {
                cmd.arg(path);
            }

            let output = cmd
                .output()
                .context("failed to run `npm exec -- prettier --write`")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "prettier formatting failed (status: {}): {}",
                    output.status,
                    stderr.trim()
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::prettier_parser_for_path;
    use std::path::Path;

    #[test]
    fn test_prettier_parser_for_expected_fixture_without_extension() {
        assert_eq!(
            prettier_parser_for_path(Path::new("testdata/maven-golden/basic/pom.xml.expected")),
            Some("json")
        );
    }

    #[test]
    fn test_prettier_parser_for_regular_json_extension_is_inferred() {
        assert_eq!(
            prettier_parser_for_path(Path::new("testdata/foo/bar.expected.json")),
            None
        );
    }

    #[test]
    fn test_prettier_parser_for_markdown_stays_default() {
        assert_eq!(
            prettier_parser_for_path(Path::new("docs/SUPPORTED_FORMATS.md")),
            None
        );
    }
}
