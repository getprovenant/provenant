// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use crate::models::{DatasourceId, FileInfo, Package};

use super::PackageIx;
use super::output_indexes::OutputIndexes;
use super::summary_helpers::unique;

/// Packages synthesized purely from a dependency lockfile's resolved entries
/// (currently Swift `Package.resolved`) describe remote third-party packages, not a
/// package anchored by a local manifest. A co-located `LICENSE`/`README` therefore
/// belongs to the enclosing repository, not to these dependency records, so key-file
/// copyright/holder must not be promoted onto them (matching ScanCode, which leaves
/// them unset). The primary package retains a manifest datasource alongside the
/// resolved one, so it is not affected by this guard.
fn is_resolved_dependency_record(package: &Package) -> bool {
    !package.datasource_ids.is_empty()
        && package
            .datasource_ids
            .iter()
            .all(|id| matches!(id, DatasourceId::SwiftPackageResolved))
}

pub(super) fn promote_package_metadata_from_key_files(
    files: &[FileInfo],
    packages: &mut [Package],
    indexes: &OutputIndexes,
) {
    for (idx, package) in packages.iter_mut().enumerate() {
        if is_resolved_dependency_record(package) {
            continue;
        }

        let Some(key_file_indices) = indexes.key_file_indices_for_package(PackageIx(idx)) else {
            continue;
        };

        if package.copyright.is_none() {
            package.copyright = key_file_indices
                .iter()
                .filter_map(|index| files.get(index.0))
                .flat_map(|file| file.copyrights.iter())
                .map(|copyright| copyright.normalized_text().to_string())
                .next();
        }

        if package.holder.is_none() {
            let promoted_holders = unique(
                &key_file_indices
                    .iter()
                    .filter_map(|index| files.get(index.0))
                    .flat_map(|file| file.holders.iter())
                    .map(|holder| holder.holder.clone())
                    .collect::<Vec<_>>(),
            );
            if promoted_holders.len() == 1 {
                package.holder = promoted_holders.into_iter().next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PackageData;

    fn package_with_datasources(datasources: &[DatasourceId]) -> Package {
        let mut package =
            Package::from_package_data(&PackageData::default(), "Package.resolved".to_string());
        package.datasource_ids = datasources.to_vec();
        package
    }

    #[test]
    fn resolved_dependency_record_detected_only_when_resolved_only() {
        // A package built purely from `Package.resolved` is a dependency record.
        assert!(is_resolved_dependency_record(&package_with_datasources(&[
            DatasourceId::SwiftPackageResolved
        ])));
        // The primary package retains its manifest datasource, so it still receives
        // co-located key-file metadata.
        assert!(!is_resolved_dependency_record(&package_with_datasources(
            &[
                DatasourceId::SwiftPackageManifestJson,
                DatasourceId::SwiftPackageResolved,
            ]
        )));
        assert!(!is_resolved_dependency_record(&package_with_datasources(
            &[DatasourceId::SwiftPackageManifestJson]
        )));
        // No datasource at all must not be treated as a dependency record.
        assert!(!is_resolved_dependency_record(&package_with_datasources(
            &[]
        )));
    }
}
