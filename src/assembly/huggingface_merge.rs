// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Cross-file assembly for Hugging Face model/dataset repositories.
//!
//! A checked-out Hugging Face repository describes a single logical model or
//! dataset across several metadata files in the same directory: a model-card
//! `README.md`, a Transformers `config.json`, and/or a Diffusers
//! `model_index.json`. The file-local parsers each emit their own
//! `PackageData`; this merger combines the ones that belong to the same
//! directory into one `Package` so that license, tags, architecture metadata,
//! and `base_model`/`dataset` dependencies are reported once for the model
//! rather than scattered across per-file packages.
//!
//! Only `PackageData` carrying a Hugging Face datasource id participates, so
//! generic `README.md`/`config.json` files (which the parsers decline to claim)
//! never trigger a merge.

use crate::models::{DatasourceId, FileInfo, Package, PackageData, TopLevelDependency};

type DirectoryMergeOutput = (Option<Package>, Vec<TopLevelDependency>, Vec<usize>);

struct HuggingfaceSource<'a> {
    file_index: usize,
    datafile_path: String,
    package_data: &'a PackageData,
}

fn is_huggingface_datasource(datasource_id: DatasourceId) -> bool {
    matches!(
        datasource_id,
        DatasourceId::HuggingfaceModelCard
            | DatasourceId::HuggingfaceConfigJson
            | DatasourceId::HuggingfaceModelIndexJson
    )
}

/// Merge all Hugging Face metadata files in a directory into one package.
///
/// Returns at most one directory-merge output. The anchor is the first source
/// that carries a proven `pkg:huggingface/<ns>/<name>` identity (preferring
/// `config.json`/`model_index.json`, whose `_name_or_path` is the most reliable
/// identity hint, over the model card). When no source proves an identity the
/// merge still produces one identity-less package so the model's license, tags,
/// architecture, and dependency facts are reported together — the honest
/// no-guess outcome.
pub fn assemble_huggingface_packages(
    files: &[FileInfo],
    file_indices: &[usize],
) -> Vec<DirectoryMergeOutput> {
    let mut sources: Vec<HuggingfaceSource> = Vec::new();
    for &file_index in file_indices {
        let file = &files[file_index];
        for package_data in &file.package_data {
            if package_data
                .datasource_id
                .is_some_and(is_huggingface_datasource)
            {
                sources.push(HuggingfaceSource {
                    file_index,
                    datafile_path: file.path.clone(),
                    package_data,
                });
            }
        }
    }

    if sources.is_empty() {
        return Vec::new();
    }

    let anchor_position = choose_anchor(&sources);
    let anchor = &sources[anchor_position];
    let mut package = Package::from_package_data(anchor.package_data, anchor.datafile_path.clone());

    let mut affected_indices = vec![anchor.file_index];
    let mut pending_dependencies = collect_dependencies(anchor.package_data, &anchor.datafile_path);

    for (position, source) in sources.iter().enumerate() {
        if position == anchor_position {
            continue;
        }
        package.update(source.package_data, source.datafile_path.clone());
        affected_indices.push(source.file_index);
        pending_dependencies.extend(collect_dependencies(
            source.package_data,
            &source.datafile_path,
        ));
    }

    affected_indices.sort_unstable();
    affected_indices.dedup();

    let for_package_uid = Some(package.package_uid.clone());
    let dependencies = pending_dependencies
        .into_iter()
        .map(|(dependency, datafile_path, datasource_id)| {
            TopLevelDependency::from_dependency(
                &dependency,
                datafile_path,
                datasource_id,
                for_package_uid.clone(),
            )
        })
        .collect();

    vec![(Some(package), dependencies, affected_indices)]
}

/// Pick the source whose identity should anchor the merged package. A source
/// with a proven PURL wins; among those, a `config.json`/`model_index.json`
/// (`_name_or_path`) is preferred over the model card (`model_name`) because
/// the config field is written by `save_pretrained` and is the more reliable
/// repository-id hint. Falls back to the first source when none has an
/// identity.
fn choose_anchor(sources: &[HuggingfaceSource]) -> usize {
    let config_with_identity = sources.iter().position(|source| {
        source.package_data.purl.is_some()
            && matches!(
                source.package_data.datasource_id,
                Some(DatasourceId::HuggingfaceConfigJson)
                    | Some(DatasourceId::HuggingfaceModelIndexJson)
            )
    });
    if let Some(position) = config_with_identity {
        return position;
    }

    sources
        .iter()
        .position(|source| source.package_data.purl.is_some())
        .unwrap_or(0)
}

fn collect_dependencies(
    package_data: &PackageData,
    datafile_path: &str,
) -> Vec<(crate::models::Dependency, String, DatasourceId)> {
    let Some(datasource_id) = package_data.datasource_id else {
        return Vec::new();
    };

    package_data
        .dependencies
        .iter()
        .filter(|dependency| {
            dependency.purl.is_some() || dependency.extracted_requirement.is_some()
        })
        .cloned()
        .map(|dependency| (dependency, datafile_path.to_string(), datasource_id))
        .collect()
}
