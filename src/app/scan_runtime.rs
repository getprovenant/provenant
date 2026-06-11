// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};

use crate::app::request::ScanRequest;
use crate::cache::{CACHE_DIR_ENV_VAR, CacheConfig};
use crate::license_detection::LicenseDetectionEngine;
use crate::license_detection::license_cache::LicenseCacheConfig;
use crate::scan_result_shaping::{
    SelectedPath, resolve_native_scan_inputs, resolve_paths_file_entries,
};
use crate::scanner::CollectionFrontier;

#[derive(Debug)]
pub(crate) struct NativeScanSelection {
    pub(crate) scan_path: String,
    pub(crate) selected_paths: Vec<SelectedPath>,
    pub(crate) collection_frontier: Vec<CollectionFrontier>,
    pub(crate) missing_entries: Vec<String>,
}

pub(crate) fn resolve_native_scan_selection(request: &ScanRequest) -> Result<NativeScanSelection> {
    if request.paths_files.is_empty() {
        let (scan_path, selected_paths) = resolve_native_scan_inputs(&request.input_paths)?;
        return Ok(NativeScanSelection {
            scan_path,
            selected_paths,
            collection_frontier: Vec::new(),
            missing_entries: Vec::new(),
        });
    }

    let scan_path = request
        .input_paths
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("--paths-file requires one positional scan root"))?;
    let path_file_entries = load_paths_file_entries(&request.paths_files)?;
    let resolved = resolve_paths_file_entries(Path::new(&scan_path), &path_file_entries)?;
    if resolved.selections.is_empty() {
        return Err(anyhow!(
            "--paths-file did not resolve to any existing files or directories under {:?}",
            Path::new(&scan_path)
        ));
    }

    Ok(NativeScanSelection {
        scan_path,
        selected_paths: resolved.selections,
        collection_frontier: resolved.frontier,
        missing_entries: resolved.missing_entries,
    })
}

pub(crate) fn build_paths_file_warning_messages(missing_entries: &[String]) -> Vec<String> {
    missing_entries
        .iter()
        .map(|entry| format!("Skipping missing --paths-file entry: {entry}"))
        .collect()
}

pub(crate) fn prepare_cache_config(
    scan_root: Option<&Path>,
    request: &ScanRequest,
) -> Result<CacheConfig> {
    let env_cache_dir = if request.respect_process_cache_env {
        env::var_os(CACHE_DIR_ENV_VAR).map(PathBuf::from)
    } else {
        None
    };
    let config = CacheConfig::from_overrides(
        scan_root,
        request.cache_dir.as_deref().map(Path::new),
        env_cache_dir.as_deref(),
        request.incremental,
        request.cache_trust_mtime,
    );

    if request.cache_clear {
        crate::cache::locking::with_exclusive_cache_lock(config.root_dir(), || {
            config.clear_contents()
        })?;
    }

    if config.incremental_enabled() {
        config.ensure_dirs()?;
    }

    Ok(config)
}

pub(crate) fn build_license_cache_config(
    cache_root: &CacheConfig,
    request: &ScanRequest,
) -> LicenseCacheConfig {
    LicenseCacheConfig::new(
        cache_root.root_dir().to_path_buf(),
        request.reindex,
        !request.no_license_index_cache,
    )
}

pub(crate) fn init_license_engine(
    cache_root: &CacheConfig,
    request: &ScanRequest,
) -> Result<Arc<LicenseDetectionEngine>> {
    let cache_config = build_license_cache_config(cache_root, request);

    match &request.license_dataset_path {
        Some(p) => {
            let path = PathBuf::from(p);
            if !path.exists() {
                return Err(anyhow!("License dataset path does not exist: {:?}", path));
            }
            let engine = LicenseDetectionEngine::from_directory_with_cache(&path, &cache_config)?;
            Ok(Arc::new(engine))
        }
        None => {
            let engine = LicenseDetectionEngine::from_embedded_with_cache(&cache_config)?;
            Ok(Arc::new(engine))
        }
    }
}

pub(crate) fn describe_license_engine_source(
    engine: &LicenseDetectionEngine,
    rules_path: Option<&str>,
) -> String {
    match rules_path {
        Some(path) => format!(
            "License detection engine initialized with {} rules from custom dataset {}",
            engine.index().rules_by_rid.len(),
            path
        ),
        None => format!(
            "License detection engine initialized with {} rules from embedded artifact",
            engine.index().rules_by_rid.len()
        ),
    }
}

fn load_paths_file_entries(paths_files: &[String]) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    for paths_file in paths_files {
        let content = read_paths_file_content(paths_file)?;
        entries.extend(content.lines().map(ToOwned::to_owned));
    }
    Ok(entries)
}

fn read_paths_file_content(paths_file: &str) -> Result<String> {
    if paths_file == "-" {
        let mut content = String::new();
        std::io::stdin()
            .read_to_string(&mut content)
            .map_err(|err| anyhow!("Failed to read --paths-file from stdin: {err}"))?;
        return Ok(content);
    }

    fs::read_to_string(paths_file)
        .map_err(|err| anyhow!("Failed to read --paths-file {:?}: {err}", paths_file))
}
