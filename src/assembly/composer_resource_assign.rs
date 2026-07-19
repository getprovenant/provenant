// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use crate::models::Package;

/// The directory of a Composer package's manifest, whose subtree the package
/// owns. Registered as a containment assigner in [`super::resource_assign`].
pub(super) fn composer_package_root(package: &Package) -> Option<PathBuf> {
    package
        .datafile_paths
        .iter()
        .find(|path| {
            Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_composer_manifest_filename)
        })
        .and_then(|path| Path::new(path).parent())
        .map(Path::to_path_buf)
}

fn is_composer_manifest_filename(name: &str) -> bool {
    name == "composer.json"
        || name.ends_with(".composer.json")
        || (name.starts_with("composer.") && name.ends_with(".json"))
}
