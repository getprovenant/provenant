// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::models::{DatasourceId, PackageType};
    use crate::parsers::scan_test_utils::scan_and_assemble;

    #[test]
    fn test_maven_repository_pom_scan_assembles_package_from_repo_style_filename() {
        let (files, result) = scan_and_assemble(Path::new(
            "testdata/summarycode-golden/tallies/packages/scan/aopalliance",
        ));

        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("aopalliance"))
            .expect("aopalliance package should be assembled");

        assert_eq!(package.package_type, Some(PackageType::Maven));
        assert_eq!(package.namespace.as_deref(), Some("aopalliance"));
        assert_eq!(package.version.as_deref(), Some("1.0"));
        assert_eq!(
            package.declared_license_expression.as_deref(),
            Some("public-domain")
        );

        let pom = files
            .iter()
            .find(|file| file.path.ends_with("/aopalliance-1.0.pom"))
            .expect("repository pom should be scanned");
        assert!(pom.for_packages.contains(&package.package_uid));
        assert!(
            pom.package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::MavenPom))
        );
    }

    #[test]
    fn test_maven_distinct_gav_poms_in_one_dir_assemble_as_separate_packages() {
        // A flat directory of standalone `.pom` fixtures, each with a distinct
        // GAV, must produce one top-level package per pom rather than collapsing
        // them all into a single sibling-merged package.
        let (files, result) = scan_and_assemble(Path::new("testdata/maven/distinct-gav-poms"));

        let mut purls: Vec<&str> = result
            .packages
            .iter()
            .filter_map(|package| package.purl.as_deref())
            .collect();
        purls.sort_unstable();
        assert_eq!(
            purls,
            vec![
                "pkg:maven/org.example.gadgets/gadget@2.5",
                "pkg:maven/org.example.widgets/widget@1.0",
            ],
            "each distinct-GAV pom should assemble into its own package: {:#?}",
            result.packages
        );

        for (file_name, purl) in [
            ("widget-1.0.pom", "pkg:maven/org.example.widgets/widget@1.0"),
            ("gadget-2.5.pom", "pkg:maven/org.example.gadgets/gadget@2.5"),
        ] {
            let package = result
                .packages
                .iter()
                .find(|package| package.purl.as_deref() == Some(purl))
                .unwrap_or_else(|| panic!("package {purl} should be assembled"));
            assert_eq!(package.datafile_paths.len(), 1);

            let pom = files
                .iter()
                .find(|file| file.path.ends_with(file_name))
                .unwrap_or_else(|| panic!("{file_name} should be scanned"));
            assert!(
                pom.for_packages.contains(&package.package_uid),
                "{file_name} should belong only to its own package"
            );
            assert_eq!(pom.for_packages.len(), 1);
        }
    }
}
