// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::models::{DatasourceId, PackageType};
    use crate::parsers::scan_test_utils::scan_and_assemble;

    #[test]
    fn test_jar_archive_assembles_one_package_per_archive() {
        let (files, result) = scan_and_assemble(Path::new("testdata/jvm-archive-golden"));

        let jar = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("demo-lib"))
            .expect("demo-lib jar should be assembled into a package");

        assert_eq!(jar.package_type, Some(PackageType::Maven));
        assert_eq!(jar.namespace.as_deref(), Some("org.example"));
        assert_eq!(jar.version.as_deref(), Some("1.2.3"));

        // The .jar datafile is owned by the assembled package.
        let jar_file = files
            .iter()
            .find(|file| file.path.ends_with("demo-lib-1.2.3.jar"))
            .expect("jar should be scanned");
        assert!(jar_file.for_packages.contains(&jar.package_uid));
        assert!(
            jar_file
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::JavaJar))
        );
    }

    #[test]
    fn test_war_and_aar_archives_assemble() {
        let (_files, result) = scan_and_assemble(Path::new("testdata/jvm-archive-golden"));

        let war = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("web-app"))
            .expect("war should be assembled");
        assert_eq!(war.namespace.as_deref(), Some("com.example.web"));

        let aar = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("ui-lib"))
            .expect("aar should be assembled");
        assert_eq!(aar.namespace.as_deref(), Some("com.example.android"));
    }
}
