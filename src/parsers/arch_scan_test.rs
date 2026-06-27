// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::scan_test_utils::{assert_dependency_present, scan_and_assemble};
    use crate::models::DatasourceId;

    #[test]
    fn test_arch_srcinfo_promotes_subpackages_to_top_level_owning_dependencies() {
        let (files, result) = scan_and_assemble(Path::new("testdata/arch/srcinfo/split"));

        // A split `.SRCINFO` promotes its subpackages to top-level `pkg:alpm`
        // packages, each owning its declared depends rather than orphaning them.
        assert!(!result.packages.is_empty());
        assert!(result.packages.iter().all(|p| {
            p.purl
                .as_deref()
                .is_some_and(|purl| purl.starts_with("pkg:alpm/"))
        }));
        assert_dependency_present(&result.dependencies, "pkg:alpm/arch/glibc", ".SRCINFO");
        assert_dependency_present(&result.dependencies, "pkg:alpm/arch/gcc-libs", ".SRCINFO");
        for name in ["glibc", "gcc-libs"] {
            let purl = format!("pkg:alpm/arch/{name}");
            let dep = result
                .dependencies
                .iter()
                .find(|dep| dep.purl.as_deref() == Some(purl.as_str()))
                .expect("dependency should be present");
            let owner = dep
                .for_package_uid
                .as_ref()
                .expect("dependency must be owned by a promoted package");
            assert!(
                result.packages.iter().any(|p| &p.package_uid == owner),
                "{name} owner must be one of the promoted packages"
            );
        }
        let srcinfo = files
            .iter()
            .find(|file| file.path.ends_with("/.SRCINFO"))
            .expect(".SRCINFO should be scanned");
        assert!(
            srcinfo
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::ArchSrcinfo))
        );
    }
}
