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

    fn uberjar_purls(result: &crate::assembly::AssemblyResult) -> Vec<&str> {
        let mut purls: Vec<&str> = result
            .packages
            .iter()
            .filter_map(|package| package.purl.as_deref())
            .collect();
        purls.sort_unstable();
        purls
    }

    const UBERJAR_PURLS: [&str; 3] = [
        "pkg:maven/com.fasterxml.jackson.core/jackson-core@2.4.0",
        "pkg:maven/commons-logging/commons-logging@1.1.1",
        "pkg:maven/org.apache.htrace/htrace-core4@4.0.0-incubating",
    ];

    #[test]
    fn test_maven_uberjar_manifest_merges_into_matching_pom_only() {
        let (files, result) = scan_and_assemble(Path::new(
            "testdata/assembly-golden/maven-uberjar-manifest-match",
        ));

        assert_eq!(uberjar_purls(&result), UBERJAR_PURLS);

        let htrace = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("htrace-core4"))
            .expect("htrace package should be assembled");
        assert!(
            htrace
                .datafile_paths
                .iter()
                .any(|path| path.ends_with("META-INF/MANIFEST.MF"))
        );

        let manifest = files
            .iter()
            .find(|file| file.path.ends_with("META-INF/MANIFEST.MF"))
            .expect("manifest should be scanned");
        assert_eq!(manifest.for_packages, vec![htrace.package_uid.clone()]);

        for shaded in ["jackson-core", "commons-logging"] {
            let package = result
                .packages
                .iter()
                .find(|package| package.name.as_deref() == Some(shaded))
                .unwrap_or_else(|| panic!("{shaded} should stay a separate package"));
            assert!(
                package
                    .datafile_paths
                    .iter()
                    .all(|path| path.contains(&format!("/{shaded}/"))),
                "{shaded} must only own its own POM files: {:?}",
                package.datafile_paths
            );
        }
    }

    #[test]
    fn test_maven_uberjar_non_matching_manifest_stays_unowned() {
        let (files, result) = scan_and_assemble(Path::new(
            "testdata/assembly-golden/maven-uberjar-manifest-mismatch",
        ));

        assert_eq!(uberjar_purls(&result), UBERJAR_PURLS);

        let manifest = files
            .iter()
            .find(|file| file.path.ends_with("META-INF/MANIFEST.MF"))
            .expect("manifest should be scanned");
        assert!(manifest.for_packages.is_empty());
        assert!(result.packages.iter().all(|package| {
            !package
                .datafile_paths
                .iter()
                .any(|path| path.ends_with("META-INF/MANIFEST.MF"))
        }));
    }

    #[test]
    fn test_maven_single_pom_jar_still_merges_manifest() {
        let (files, result) =
            scan_and_assemble(Path::new("testdata/assembly-golden/maven-meta-inf-basic"));

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:maven/com.example/nested-lib@1.0.0")
        );
        for file in files.iter().filter(|file| !file.package_data.is_empty()) {
            assert_eq!(file.for_packages, vec![package.package_uid.clone()]);
        }
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
