// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use crate::models::{FileInfo, Package};

use super::PackageIx;
use super::output_indexes::OutputIndexes;
use super::summary_helpers::unique;

pub(super) fn promote_package_metadata_from_key_files(
    files: &[FileInfo],
    packages: &mut [Package],
    indexes: &OutputIndexes,
) {
    for (idx, package) in packages.iter_mut().enumerate() {
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
