// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! SPDX package planning: map the shared SBOM inventory onto SPDX package
//! identities, file ownership, and document roles.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::output_schema::{Output, OutputFileInfo as FileInfo, OutputPackage};

use super::OutputWriteConfig;
use super::sbom::SbomInventory;

/// Role of an SPDX package in the document graph (ADR 0012).
#[derive(Clone, Copy)]
pub(crate) enum SpdxPackageRole {
    /// Scanned subject: `DESCRIBES` + `FilesAnalyzed: true`.
    DocumentSubject,
    /// Promoted resolved dependency: `DEPENDS_ON` only + `FilesAnalyzed: false`.
    PromotedDependency,
}

impl SpdxPackageRole {
    pub(crate) fn files_analyzed(self) -> bool {
        matches!(self, Self::DocumentSubject)
    }

    pub(crate) fn is_described_by_document(self) -> bool {
        matches!(self, Self::DocumentSubject)
    }
}

/// One SPDX Package to emit, with the files it owns.
pub(crate) struct SpdxPackagePlan<'a> {
    pub spdx_id: String,
    pub name: String,
    pub files: Vec<&'a FileInfo>,
    /// The assembled package this plan was built from, when there is one.
    /// `None` for the no-package synthetic plan and the unassigned-files
    /// fallback bucket, which have no package-level license data to draw on.
    pub package: Option<&'a OutputPackage>,
    pub role: SpdxPackageRole,
}

/// SPDX ids parallel to [`SbomInventory::entries`], matching the numbering in
/// [`plan_spdx_packages`] so shared edge resolution and emission agree.
pub(crate) fn inventory_spdx_ids(inventory: &SbomInventory<'_>) -> Vec<String> {
    let mut detected = 0usize;
    let mut promoted = 0usize;
    inventory
        .entries
        .iter()
        .map(|entry| {
            if entry.is_promoted() {
                promoted += 1;
                format!("SPDXRef-Package-Dependency-{promoted}")
            } else {
                detected += 1;
                format!("SPDXRef-Package-{detected}")
            }
        })
        .collect()
}

/// Plan SPDX packages: one per assembled package (with owned files), plus a
/// scan-root fallback package for files that no assembled package claims,
/// plus every promoted dependency from the shared inventory (ADR 0012).
/// When there are no assembled packages and no promotions, keep a single
/// synthetic package (`SPDXRef-001`) owning every file — the historic
/// no-package contract.
pub(crate) fn plan_spdx_packages<'a>(
    output: &'a Output,
    files: &[&'a FileInfo],
    config: &OutputWriteConfig,
    inventory: &'a SbomInventory<'a>,
) -> Vec<SpdxPackagePlan<'a>> {
    if inventory.is_empty() {
        return vec![SpdxPackagePlan {
            spdx_id: "SPDXRef-001".to_string(),
            name: primary_package_name(output, config),
            files: files.to_vec(),
            package: None,
            role: SpdxPackageRole::DocumentSubject,
        }];
    }

    let mut assigned_paths: HashSet<&str> = HashSet::new();
    let mut plans = Vec::with_capacity(inventory.entries.len() + 1);

    // Subject packages first (contiguous), then the unassigned-files bucket,
    // then promoted dependencies — matching inventory_spdx_ids numbering.
    for (detected_idx, entry) in inventory
        .entries
        .iter()
        .filter(|entry| !entry.is_promoted())
        .enumerate()
    {
        let package = entry.package();
        let owned: Vec<&FileInfo> = files
            .iter()
            .copied()
            .filter(|file| {
                file.for_packages
                    .iter()
                    .any(|uid| uid == &package.package_uid)
            })
            .collect();
        for file in &owned {
            assigned_paths.insert(file.path.as_str());
        }
        plans.push(SpdxPackagePlan {
            spdx_id: format!("SPDXRef-Package-{}", detected_idx + 1),
            name: spdx_assembled_package_name(package, detected_idx),
            files: owned,
            package: Some(package),
            role: SpdxPackageRole::DocumentSubject,
        });
    }

    let unassigned: Vec<&FileInfo> = files
        .iter()
        .copied()
        .filter(|file| !assigned_paths.contains(file.path.as_str()))
        .collect();
    if !unassigned.is_empty() {
        plans.push(SpdxPackagePlan {
            spdx_id: "SPDXRef-Package-unassigned".to_string(),
            name: primary_package_name(output, config),
            files: unassigned,
            package: None,
            role: SpdxPackageRole::DocumentSubject,
        });
    }

    for (promoted_idx, entry) in inventory
        .entries
        .iter()
        .filter(|entry| entry.is_promoted())
        .enumerate()
    {
        let package = entry.package();
        plans.push(SpdxPackagePlan {
            spdx_id: format!("SPDXRef-Package-Dependency-{}", promoted_idx + 1),
            name: spdx_assembled_package_name(package, promoted_idx),
            files: Vec::new(),
            package: Some(package),
            role: SpdxPackageRole::PromotedDependency,
        });
    }

    plans
}

pub(crate) fn primary_package_name(output: &Output, config: &OutputWriteConfig) -> String {
    if output.packages.len() == 1
        && let Some(name) = output.packages.first().and_then(|p| p.name.clone())
    {
        return sanitize_spdx_package_name(&name);
    }

    if let Some(scanned_path) = &config.scanned_path {
        let path = PathBuf::from(scanned_path);
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && !name.is_empty()
        {
            return sanitize_spdx_package_name(name);
        }
    }

    output
        .packages
        .first()
        .and_then(|p| p.name.clone())
        .map(|name| sanitize_spdx_package_name(&name))
        .unwrap_or_else(|| "provenant-analyzed-package".to_string())
}

pub(crate) fn sanitize_spdx_package_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "provenant-analyzed-package".to_string()
    } else {
        out
    }
}

fn spdx_assembled_package_name(package: &OutputPackage, idx: usize) -> String {
    package
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(sanitize_spdx_package_name)
        .unwrap_or_else(|| format!("package-{}", idx + 1))
}
