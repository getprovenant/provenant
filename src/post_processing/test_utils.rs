// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::assembly;
use crate::cache::{DEFAULT_CACHE_DIR_NAME, build_collection_exclude_patterns};
use crate::models::{DatasourceId, FileInfo, FileType, Package, PackageType, PackageUid};
use crate::progress::{ProgressMode, ScanProgress};
use crate::scanner::{LicenseScanOptions, TextDetectionOptions, collect_paths, process_collected};

pub(super) fn file(path: &str) -> FileInfo {
    FileInfo::new(
        Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
        Path::new(path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
        Path::new(path)
            .extension()
            .and_then(|n| n.to_str())
            .map(|ext| format!(".{ext}"))
            .unwrap_or_default(),
        path.to_string(),
        FileType::File,
        None,
        None,
        1,
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
        Vec::new(),
    )
}

pub(super) fn dir(path: &str) -> FileInfo {
    FileInfo::new(
        Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
        Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
        String::new(),
        path.to_string(),
        FileType::Directory,
        None,
        None,
        0,
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
        Vec::new(),
    )
}

pub(super) fn package(uid: &str, path: &str) -> Package {
    Package {
        package_type: Some(PackageType::Gem),
        namespace: None,
        name: Some("inspec-bin".to_string()),
        version: Some("6.8.2".to_string()),
        qualifiers: None,
        subpath: None,
        primary_language: Some("Ruby".to_string()),
        description: None,
        release_date: None,
        parties: vec![],
        keywords: vec![],
        homepage_url: None,
        download_url: None,
        size: None,
        sha1: None,
        md5: None,
        sha256: None,
        sha512: None,
        bug_tracking_url: None,
        code_view_url: None,
        vcs_url: None,
        copyright: None,
        holder: None,
        declared_license_expression: None,
        declared_license_expression_spdx: None,
        license_detections: vec![],
        other_license_expression: None,
        other_license_expression_spdx: None,
        other_license_detections: vec![],
        extracted_license_statement: None,
        notice_text: None,
        source_packages: vec![],
        is_private: false,
        is_virtual: false,
        extra_data: None,
        repository_homepage_url: None,
        repository_download_url: None,
        api_data_url: None,
        datasource_ids: vec![DatasourceId::GemArchiveExtracted],
        purl: Some("pkg:gem/inspec-bin@6.8.2".to_string()),
        package_uid: PackageUid::from_raw(uid.to_string()),
        datafile_paths: vec![path.to_string()],
    }
}

pub(super) fn scan_and_assemble_with_keyfiles(
    path: &Path,
) -> (Vec<FileInfo>, assembly::AssemblyResult) {
    let progress = Arc::new(ScanProgress::new(ProgressMode::Quiet));
    let collected = collect_paths(
        path,
        0,
        &build_collection_exclude_patterns(path, &path.join(DEFAULT_CACHE_DIR_NAME)),
    );
    let result = process_collected(
        &collected,
        progress,
        None,
        LicenseScanOptions::default(),
        &TextDetectionOptions {
            collect_info: true,
            detect_packages: true,
            detect_application_packages: true,
            detect_system_packages: true,
            detect_packages_in_compiled: false,
            ..TextDetectionOptions::default()
        },
    );

    let mut files = result.files;
    let assembly_result = assembly::assemble(&mut files);
    classify_key_files(&mut files, &assembly_result.packages);
    (files, assembly_result)
}
