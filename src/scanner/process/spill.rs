// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::models::FileInfo;
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use tempfile::TempDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryMode {
    CollectFirst,
    StreamUnlimited,
    Limit(usize),
}

impl std::fmt::Display for MemoryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryMode::CollectFirst => write!(f, "0"),
            MemoryMode::StreamUnlimited => write!(f, "-1"),
            MemoryMode::Limit(n) => write!(f, "{n}"),
        }
    }
}

pub(super) fn retain_or_spill_chunk(
    chunk: Vec<FileInfo>,
    retained_files: &mut Vec<FileInfo>,
    spill_store: &mut Option<FileInfoSpillStore>,
    memory_limit: usize,
) -> Result<()> {
    if memory_limit == 0 {
        spill_store_mut(spill_store)?.spill(chunk)?;
        return Ok(());
    }

    let remaining_capacity = memory_limit.saturating_sub(retained_files.len());
    if remaining_capacity >= chunk.len() && spill_store.is_none() {
        retained_files.extend(chunk);
        return Ok(());
    }

    let mut chunk_iter = chunk.into_iter();
    retained_files.extend(chunk_iter.by_ref().take(remaining_capacity));
    let overflow: Vec<FileInfo> = chunk_iter.collect();
    if !overflow.is_empty() {
        spill_store_mut(spill_store)?.spill(overflow)?;
    }

    Ok(())
}

fn spill_store_mut(
    spill_store: &mut Option<FileInfoSpillStore>,
) -> Result<&mut FileInfoSpillStore> {
    if spill_store.is_none() {
        *spill_store = Some(FileInfoSpillStore::new()?);
    }

    Ok(spill_store
        .as_mut()
        .expect("spill store is always initialized after creation"))
}

pub(super) struct FileInfoSpillStore {
    temp_dir: TempDir,
    batch_index: usize,
}

impl FileInfoSpillStore {
    fn new() -> Result<Self> {
        Ok(Self {
            temp_dir: TempDir::new().context("create scanner spill directory")?,
            batch_index: 0,
        })
    }

    fn spill(&mut self, files: Vec<FileInfo>) -> Result<()> {
        let path = self
            .temp_dir
            .path()
            .join(format!("batch-{:06}.postcard.zst", self.batch_index));
        self.batch_index += 1;

        let payload = postcard::to_allocvec(&files).context("encode scanner spill batch")?;
        let file = File::create(&path)
            .with_context(|| format!("create scanner spill batch file {}", path.display()))?;
        let mut encoder = zstd::Encoder::new(file, 3)
            .with_context(|| format!("create scanner spill encoder for {}", path.display()))?;
        encoder
            .write_all(&payload)
            .with_context(|| format!("write scanner spill batch {}", path.display()))?;
        encoder
            .finish()
            .with_context(|| format!("finish scanner spill encoder for {}", path.display()))?;

        Ok(())
    }

    pub(super) fn load_all(self) -> Result<Vec<FileInfo>> {
        let spill_dir = self.temp_dir.path();
        let mut paths = Vec::new();
        for entry in fs::read_dir(spill_dir)
            .with_context(|| format!("read scanner spill directory {}", spill_dir.display()))?
        {
            let entry = entry.with_context(|| {
                format!(
                    "read scanner spill directory entry from {}",
                    spill_dir.display()
                )
            })?;
            paths.push(entry.path());
        }
        paths.sort();

        let mut files = Vec::new();
        for path in paths {
            let file = File::open(&path)
                .with_context(|| format!("open scanner spill batch {}", path.display()))?;
            let mut decoder = zstd::Decoder::new(file)
                .with_context(|| format!("create scanner spill decoder for {}", path.display()))?;
            let mut payload = Vec::new();
            decoder
                .read_to_end(&mut payload)
                .with_context(|| format!("read scanner spill batch {}", path.display()))?;
            let mut batch: Vec<FileInfo> = postcard::from_bytes(&payload)
                .with_context(|| format!("decode scanner spill batch {}", path.display()))?;
            files.append(&mut batch);
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DiagnosticSeverity, FileType, ScanDiagnostic};

    #[test]
    fn spilled_files_preserve_scan_diagnostic_severity() -> Result<()> {
        let mut store = FileInfoSpillStore::new()?;
        let file = FileInfo::new(
            "custom.txt".to_string(),
            "custom".to_string(),
            ".txt".to_string(),
            "project/custom.txt".to_string(),
            FileType::File,
            None,
            None,
            10,
            None,
            None,
            None,
            None,
            None,
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![ScanDiagnostic::warning("custom recoverable warning")],
        );

        store.spill(vec![file])?;
        let loaded = store.load_all()?;

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].scan_diagnostics.len(), 1);
        assert_eq!(
            loaded[0].scan_diagnostics[0].severity,
            DiagnosticSeverity::Warning
        );
        Ok(())
    }

    #[test]
    fn corrupt_spill_batch_returns_error() -> Result<()> {
        let mut store = FileInfoSpillStore::new()?;
        let path = store.temp_dir.path().join("batch-000000.postcard.zst");
        fs::write(&path, b"not zstd")?;
        store.batch_index = 1;

        let error = store
            .load_all()
            .expect_err("corrupt spill batch should return an error");

        assert!(error.to_string().contains("scanner spill"));
        Ok(())
    }
}
