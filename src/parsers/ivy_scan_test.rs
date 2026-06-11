// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::models::{DatasourceId, PackageType};
    use crate::parsers::scan_test_utils::scan_and_assemble;

    #[test]
    fn test_ivy_xml_assembles_package_and_hoists_dependencies() {
        let (files, result) = scan_and_assemble(Path::new("testdata/ivy-golden/basic"));

        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("example-core"))
            .expect("ivy.xml should assemble into a package");

        assert_eq!(package.package_type, Some(PackageType::Ivy));
        assert_eq!(package.namespace.as_deref(), Some("org.apache.example"));
        assert_eq!(package.version.as_deref(), Some("4.5.6"));

        let ivy_file = files
            .iter()
            .find(|file| file.path.ends_with("/ivy.xml"))
            .expect("ivy.xml should be scanned");
        assert!(ivy_file.for_packages.contains(&package.package_uid));
        assert!(
            ivy_file
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::AntIvyXml))
        );

        // Direct dependencies declared in ivy.xml are hoisted to top-level deps.
        assert!(
            result
                .dependencies
                .iter()
                .any(|dep| dep.purl.as_deref() == Some("pkg:ivy/commons-lang/commons-lang"))
        );
    }
}
