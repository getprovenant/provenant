// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;

/// Create `path` and any missing parents, restricting each newly created
/// directory to the owner (`0700`) on Unix.
///
/// Cache entries can contain license/copyright text and file paths from private
/// repositories, so the cache tree must not be group/world-readable on a
/// permissive-umask multi-user host. On non-Unix platforms this falls back to
/// the standard `create_dir_all` behavior.
pub fn create_dir_all_private(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;

        DirBuilder::new().recursive(true).mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(path)
    }
}

pub fn write_bytes_atomically(path: &Path, payload: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Atomic write path has no parent: {path:?}"),
        )
    })?;

    create_dir_all_private(parent)?;

    let temp_path = temp_atomic_path(path);
    let result =
        write_bytes_to_temp(&temp_path, payload).and_then(|_| replace_file(&temp_path, path));

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

fn write_bytes_to_temp(temp_path: &Path, payload: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    // Restrict cache files to the owner (`0600`) on Unix; they can hold
    // license/copyright text and file paths from private repositories.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut temp_file = options.open(temp_path)?;
    temp_file.write_all(payload)?;
    temp_file.sync_all()?;
    Ok(())
}

fn replace_file(temp_path: &Path, final_path: &Path) -> io::Result<()> {
    match fs::rename(temp_path, final_path) {
        Ok(()) => Ok(()),
        Err(err) if final_path.exists() => {
            fs::remove_file(final_path)?;
            fs::rename(temp_path, final_path).map_err(|rename_err| {
                io::Error::new(
                    rename_err.kind(),
                    format!("{err}; replace retry failed: {rename_err}"),
                )
            })
        }
        Err(err) => Err(err),
    }
}

fn temp_atomic_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snapshot");
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".tmp-{file_name}-{}", Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_write_bytes_atomically_round_trip() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("incremental").join("manifest.json");

        write_bytes_atomically(&path, b"hello world").expect("write bytes atomically");

        assert_eq!(fs::read(&path).expect("read bytes"), b"hello world");
    }

    #[cfg(unix)]
    #[test]
    fn test_write_bytes_atomically_restricts_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let dir = temp_dir.path().join("nested").join("cache");
        let path = dir.join("entry.bin");

        write_bytes_atomically(&path, b"secret").expect("write bytes atomically");

        let file_mode = fs::metadata(&path)
            .expect("file metadata")
            .permissions()
            .mode();
        assert_eq!(file_mode & 0o777, 0o600, "cache file must be owner-only");

        let dir_mode = fs::metadata(&dir)
            .expect("dir metadata")
            .permissions()
            .mode();
        assert_eq!(dir_mode & 0o777, 0o700, "created dir must be owner-only");
    }
}
