// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use crate::models::Package;

/// The directory of a Cargo package's `Cargo.toml`, whose subtree the package
/// owns. Registered as a containment assigner in [`super::resource_assign`].
pub(super) fn cargo_package_root(package: &Package) -> Option<PathBuf> {
    package
        .datafile_paths
        .iter()
        .find(|path| {
            Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("cargo.toml"))
        })
        .and_then(|path| Path::new(path).parent())
        .map(Path::to_path_buf)
}
