// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::path::{Path, PathBuf};

use crate::models::Package;

/// The directory whose subtree a RubyGems package owns. Registered as a
/// containment assigner in [`super::resource_assign`].
///
/// Handles extracted-gem layouts: a `metadata.gz-extract` marker roots the
/// package at its own directory, a `data.gz-extract` payload roots it at the
/// enclosing gem directory, and a bare `.gem` archive contributes no root.
pub(super) fn ruby_package_root(package: &Package) -> Option<PathBuf> {
    if package
        .datasource_ids
        .contains(&crate::models::DatasourceId::GemArchive)
    {
        return None;
    }

    for datafile_path in &package.datafile_paths {
        let path = Path::new(datafile_path);

        if path.file_name().and_then(|n| n.to_str()) == Some("metadata.gz-extract") {
            return path.parent().map(|p| p.to_path_buf());
        }

        if path
            .components()
            .any(|c| c.as_os_str() == "data.gz-extract")
        {
            let mut current = path;
            while let Some(parent) = current.parent() {
                if parent.file_name().and_then(|n| n.to_str()) == Some("data.gz-extract") {
                    return parent.parent().map(|p| p.to_path_buf());
                }
                current = parent;
            }
        }

        if let Some(parent) = path.parent() {
            return Some(parent.to_path_buf());
        }
    }

    None
}
