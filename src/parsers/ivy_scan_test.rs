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

    #[test]
    fn test_dependencies_properties_hoists_unowned_maven_dependencies() {
        let (files, result) = scan_and_assemble(Path::new("testdata/ivy-golden/dependencies"));

        assert!(
            result.packages.is_empty(),
            "dependency-list fixture should not create a root package"
        );
        assert_eq!(result.dependencies.len(), 4);
        assert!(result.dependencies.iter().any(|dependency| {
            dependency.purl.as_deref() == Some("pkg:maven/javax.ws.rs/javax.ws.rs-api@2.1")
                && dependency.datasource_id == DatasourceId::AntIvyDependenciesProperties
                && dependency.for_package_uid.is_none()
        }));
        assert!(result.dependencies.iter().any(|dependency| {
            dependency.purl.as_deref() == Some("pkg:maven/org.slf4j/slf4j-api@2.0.13")
                && dependency.datasource_id == DatasourceId::AntIvyDependenciesProperties
        }));
        assert!(result.dependencies.iter().any(|dependency| {
            dependency.purl.as_deref()
                == Some("pkg:maven/io.dropwizard.metrics5/metrics-core@5.0.0-rc16")
                && dependency.datasource_id == DatasourceId::AntIvyDependenciesProperties
        }));

        let dependencies_file = files
            .iter()
            .find(|file| file.path.ends_with("/dependencies.properties"))
            .expect("dependencies.properties should be scanned");
        assert!(dependencies_file.for_packages.is_empty());
        assert!(
            dependencies_file
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id
                    == Some(DatasourceId::AntIvyDependenciesProperties))
        );
    }

    #[test]
    fn test_dependencies_properties_attach_to_colocated_ivy_xml_package() {
        let (files, result) = scan_and_assemble(Path::new("testdata/ivy-golden/assembly"));

        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("assembly-demo"))
            .expect("ivy.xml should assemble into a package");

        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::AntIvyDependenciesProperties)
        );
        assert!(
            package
                .datafile_paths
                .iter()
                .any(|path| path.ends_with("/dependencies.properties"))
        );

        let dependencies_file = files
            .iter()
            .find(|file| file.path.ends_with("/dependencies.properties"))
            .expect("dependencies.properties should be scanned");
        assert!(
            dependencies_file
                .for_packages
                .contains(&package.package_uid)
        );

        let resolved_dep = result
            .dependencies
            .iter()
            .find(|dependency| {
                dependency.purl.as_deref() == Some("pkg:maven/org.slf4j/slf4j-api@2.0.13")
                    && dependency.datasource_id == DatasourceId::AntIvyDependenciesProperties
            })
            .expect("dependencies.properties resolved dependency should be visible");
        assert_eq!(
            resolved_dep.for_package_uid.as_ref(),
            Some(&package.package_uid)
        );
        assert_eq!(resolved_dep.is_pinned, Some(true));
        assert_eq!(resolved_dep.is_direct, Some(true));

        let second_resolved_dep = result
            .dependencies
            .iter()
            .find(|dependency| {
                dependency.purl.as_deref() == Some("pkg:maven/javax.ws.rs/javax.ws.rs-api@2.1")
                    && dependency.datasource_id == DatasourceId::AntIvyDependenciesProperties
            })
            .expect("second dependencies.properties resolved dependency should be visible");
        assert_eq!(
            second_resolved_dep.for_package_uid.as_ref(),
            Some(&package.package_uid)
        );

        assert!(
            result.dependencies.iter().any(|dependency| {
                dependency.purl.as_deref() == Some("pkg:ivy/commons-lang/commons-lang")
                    && dependency.for_package_uid.as_ref() == Some(&package.package_uid)
            }),
            "ivy.xml dependencies should remain assigned to the owning package"
        );
    }
}
