// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::super::scan_test_utils::scan_and_assemble;
    use crate::models::{DatasourceId, PackageType};

    #[test]
    fn test_cocoapods_scan_assembles_single_podspec_and_hoists_lockfile_dependencies() {
        let (files, result) = scan_and_assemble(Path::new(
            "testdata/cocoapods-golden/assemble/single-podspec",
        ));

        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("RxDataSources"))
            .expect("RxDataSources package should be assembled");

        assert_eq!(package.package_type, Some(PackageType::Cocoapods));
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:cocoapods/RxDataSources@4.0.1")
        );
        assert!(result.dependencies.iter().any(|dep| {
            dep.purl.as_deref() == Some("pkg:cocoapods/boost@1.76.0")
                && dep.for_package_uid.as_deref() == Some(package.package_uid.as_str())
                && dep.datafile_path.ends_with("Podfile.lock")
        }));

        let podfile = files
            .iter()
            .find(|file| file.path.ends_with("/Podfile"))
            .expect("Podfile should be scanned");
        let podfile_lock = files
            .iter()
            .find(|file| file.path.ends_with("/Podfile.lock"))
            .expect("Podfile.lock should be scanned");
        let podspec = files
            .iter()
            .find(|file| file.path.ends_with("/RxDataSources.podspec"))
            .expect("podspec should be scanned");

        assert!(
            podfile
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::CocoapodsPodfile))
        );
        assert!(podfile_lock.for_packages.contains(&package.package_uid));
        assert!(podspec.for_packages.contains(&package.package_uid));
    }

    #[test]
    fn test_cocoapods_scan_keeps_extra_sibling_podspecs_as_standalone_packages() {
        let (files, result) = scan_and_assemble(Path::new(
            "testdata/cocoapods-golden/assemble/multiple-podspec",
        ));

        assert_eq!(result.packages.len(), 2);

        let rx_data_sources = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("RxDataSources"))
            .expect("RxDataSources package should be assembled");
        let differentiator = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("Differentiator"))
            .expect("Differentiator package should remain standalone");

        assert_eq!(rx_data_sources.package_type, Some(PackageType::Cocoapods));
        assert_eq!(differentiator.package_type, Some(PackageType::Cocoapods));
        assert!(
            rx_data_sources
                .datafile_paths
                .iter()
                .any(|path| path.ends_with("/Podfile"))
        );
        assert!(
            rx_data_sources
                .datafile_paths
                .iter()
                .any(|path| path.ends_with("/Podfile.lock"))
        );
        assert_eq!(differentiator.datafile_paths.len(), 1);
        assert!(
            differentiator
                .datafile_paths
                .iter()
                .all(|path| !path.ends_with("/Podfile") && !path.ends_with("/Podfile.lock"))
        );

        assert!(result.dependencies.iter().all(|dependency| {
            dependency.for_package_uid.as_deref() != Some(differentiator.package_uid.as_str())
        }));
        assert!(result.dependencies.iter().any(|dependency| {
            dependency.for_package_uid.as_deref() == Some(rx_data_sources.package_uid.as_str())
                && dependency.datafile_path.ends_with("Podfile.lock")
        }));

        let podfile = files
            .iter()
            .find(|file| file.path.ends_with("/Podfile"))
            .expect("Podfile should be scanned");
        let podfile_lock = files
            .iter()
            .find(|file| file.path.ends_with("/Podfile.lock"))
            .expect("Podfile.lock should be scanned");
        let rx_data_sources_podspec = files
            .iter()
            .find(|file| file.path.ends_with("/RxDataSources.podspec"))
            .expect("RxDataSources podspec should be scanned");
        let differentiator_podspec = files
            .iter()
            .find(|file| file.path.ends_with("/Differentiator.podspec"))
            .expect("Differentiator podspec should be scanned");

        assert!(podfile.for_packages.contains(&rx_data_sources.package_uid));
        assert!(
            podfile_lock
                .for_packages
                .contains(&rx_data_sources.package_uid)
        );
        assert!(
            rx_data_sources_podspec
                .for_packages
                .contains(&rx_data_sources.package_uid)
        );
        assert!(
            differentiator_podspec
                .for_packages
                .contains(&differentiator.package_uid)
        );
        assert!(
            !differentiator_podspec
                .for_packages
                .contains(&rx_data_sources.package_uid)
        );
    }

    #[test]
    fn test_cocoapods_scan_skips_top_level_assembly_for_generic_nonliteral_identity_podspec() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let podspec_path = temp_dir.path().join("dynamic-identity.podspec");
        fs::write(
            &podspec_path,
            r#"Pod::Spec.new do |s|
  s.name     = pod_name
  s.version  = pod_version
  s.summary  = package['description']
  s.homepage = homepage_url
  s.license  = license_name
  s.author   = author_name
  s.source   = { :git => homepage_url, :tag => 'v#{pod_version}' }
  s.dependency 'React-Core'
end
"#,
        )
        .expect("fixture podspec should be written");

        let (files, result) = scan_and_assemble(temp_dir.path());

        assert!(
            result.packages.is_empty(),
            "packages: {:?}",
            result.packages
        );
        assert!(
            result.dependencies.is_empty(),
            "dependencies: {:?}",
            result.dependencies
        );

        let podspec = files
            .iter()
            .find(|file| file.path.ends_with("/dynamic-identity.podspec"))
            .expect("podspec should be scanned");
        assert_eq!(
            podspec.package_data.len(),
            1,
            "package_data: {:?}",
            podspec.package_data
        );
        assert_eq!(
            podspec.package_data[0].purl.as_deref(),
            Some("pkg:cocoapods/pod_name@pod_version")
        );
        assert_eq!(
            podspec.package_data[0]
                .extra_data
                .as_ref()
                .and_then(|data| data.get("dynamic_identity_placeholders"))
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(
            podspec.for_packages.is_empty(),
            "for_packages: {:?}",
            podspec.for_packages
        );
    }
}
