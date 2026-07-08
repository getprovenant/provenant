// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::scan_test_utils::{
        assert_file_links_to_package, scan_and_assemble_with_stripped_root,
    };
    use crate::models::{DatasourceId, PackageType};

    #[test]
    fn test_vscode_extension_manifest_assembles_into_package() {
        let (files, result) = scan_and_assemble_with_stripped_root(Path::new(
            "testdata/vscode-extension-golden/basic",
        ));

        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("python"))
            .expect("VS Code extension package should be assembled");

        assert_eq!(package.package_type, Some(PackageType::VscodeExtension));
        assert_eq!(package.namespace.as_deref(), Some("ms-python"));
        assert_eq!(package.version.as_deref(), Some("2023.25.10292213"));
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:vscode-extension/ms-python/python@2023.25.10292213?platform=linux-x64")
        );
        assert_eq!(
            package.datafile_paths,
            vec!["extension.vsixmanifest".to_string()]
        );

        assert_file_links_to_package(
            &files,
            "extension.vsixmanifest",
            &package.package_uid,
            DatasourceId::VscodeExtensionVsixManifest,
        );
    }
}
