// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::scan_test_utils::scan_and_assemble;
    use crate::models::{DatasourceId, PackageType};
    use rpm::PackageBuilder;

    #[test]
    fn test_rpm_specfile_scan_assembles_package_and_dependencies() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        fs::copy(
            "testdata/rpm/specfile/cpio.spec",
            temp_dir.path().join("cpio.spec"),
        )
        .expect("copy cpio.spec fixture");

        let (files, result) = scan_and_assemble(temp_dir.path());

        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("cpio"))
            .expect("cpio package should be assembled");

        assert_eq!(package.package_type, Some(PackageType::Rpm));
        assert_eq!(package.version.as_deref(), Some("2.9"));
        assert_eq!(package.purl.as_deref(), Some("pkg:rpm/cpio@2.9"));
        assert!(result.dependencies.iter().any(|dep| {
            dep.purl.as_deref() == Some("pkg:rpm/texinfo")
                && dep.scope.as_deref() == Some("build")
                && dep.for_package_uid.as_deref() == Some(package.package_uid.as_str())
        }));
        assert!(result.dependencies.iter().any(|dep| {
            dep.purl.is_none()
                && dep.extracted_requirement.as_deref() == Some("/sbin/install-info")
                && dep.scope.as_deref() == Some("post")
                && dep.for_package_uid.as_deref() == Some(package.package_uid.as_str())
        }));

        let specfile = files
            .iter()
            .find(|file| file.path.ends_with("/cpio.spec"))
            .expect("cpio.spec should be scanned");
        assert!(specfile.for_packages.contains(&package.package_uid));
        assert!(
            specfile
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::RpmSpecfile))
        );
    }

    #[test]
    fn test_rpm_specfiles_in_same_directory_remain_separate_packages() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        fs::copy(
            "testdata/rpm/specfile/cpio.spec",
            temp_dir.path().join("cpio.spec"),
        )
        .expect("copy cpio.spec fixture");
        fs::copy(
            "testdata/rpm/specfile/openssl.spec",
            temp_dir.path().join("openssl.spec"),
        )
        .expect("copy openssl.spec fixture");

        let (files, result) = scan_and_assemble(temp_dir.path());

        let cpio = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("cpio"))
            .expect("cpio package should be assembled");
        let openssl = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("openssl"))
            .expect("openssl package should be assembled");

        assert_ne!(cpio.package_uid, openssl.package_uid);
        assert_eq!(cpio.datafile_paths.len(), 1);
        assert!(cpio.datafile_paths[0].ends_with("/cpio.spec"));
        assert_eq!(openssl.datafile_paths.len(), 1);
        assert!(openssl.datafile_paths[0].ends_with("/openssl.spec"));

        let cpio_spec = files
            .iter()
            .find(|file| file.path.ends_with("/cpio.spec"))
            .expect("cpio.spec should be scanned");
        let openssl_spec = files
            .iter()
            .find(|file| file.path.ends_with("/openssl.spec"))
            .expect("openssl.spec should be scanned");

        assert_eq!(cpio_spec.for_packages, vec![cpio.package_uid.clone()]);
        assert_eq!(openssl_spec.for_packages, vec![openssl.package_uid.clone()]);
    }

    #[test]
    fn test_rpm_archive_scan_assembles_top_level_package() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let rpm_path = temp_dir.path().join("demo-1.0-1.x86_64.rpm");
        PackageBuilder::new("demo", "1.0", "MIT", "x86_64", "Demo RPM package")
            .release("1")
            .build()
            .expect("build rpm fixture")
            .write_file(&rpm_path)
            .expect("write rpm fixture");

        let (files, result) = scan_and_assemble(temp_dir.path());

        let rpm_file = files
            .iter()
            .find(|file| file.path.ends_with("/demo-1.0-1.x86_64.rpm"))
            .expect("rpm archive should be scanned");
        assert_eq!(rpm_file.for_packages.len(), 1);
        assert!(
            rpm_file
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::RpmArchive))
        );

        let package = result
            .packages
            .iter()
            .find(|package| Some(&package.package_uid) == rpm_file.for_packages.first())
            .expect("rpm archive should assemble a top-level package");

        assert_eq!(package.package_type, Some(PackageType::Rpm));
        assert!(package.datasource_ids.contains(&DatasourceId::RpmArchive));
        assert_eq!(package.name.as_deref(), Some("demo"));
        assert_eq!(package.version.as_deref(), Some("1.0-1"));
        assert_eq!(package.datafile_paths.len(), 1);
        assert!(package.datafile_paths[0].ends_with("/demo-1.0-1.x86_64.rpm"));
        assert_eq!(rpm_file.for_packages, vec![package.package_uid.clone()]);
    }

    #[test]
    fn test_rpm_mariner_manifest_scan_assembles_each_manifest_row() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let manifest_dir = temp_dir.path().join("var/lib/rpmmanifest");
        fs::create_dir_all(&manifest_dir).expect("create rpmmanifest dir");
        fs::copy(
            "testdata/rpm/var/lib/rpmmanifest/container-manifest-2",
            manifest_dir.join("container-manifest-2"),
        )
        .expect("copy RPM Mariner manifest fixture");

        let (files, result) = scan_and_assemble(temp_dir.path());

        assert_eq!(result.packages.len(), 2, "packages: {:#?}", result.packages);
        let bash = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("bash"))
            .expect("bash package should be assembled");
        assert_eq!(bash.package_type, Some(PackageType::Rpm));
        assert_eq!(bash.version.as_deref(), Some("5.0.17"));
        assert_eq!(
            bash.purl.as_deref(),
            Some("pkg:rpm/mariner/bash@5.0.17?arch=x86_64")
        );
        assert!(
            bash.datasource_ids
                .contains(&DatasourceId::RpmMarinerManifest)
        );
        assert_eq!(bash.datafile_paths.len(), 1);
        assert!(bash.datafile_paths[0].ends_with("/var/lib/rpmmanifest/container-manifest-2"));

        let manifest = files
            .iter()
            .find(|file| {
                file.path
                    .ends_with("/var/lib/rpmmanifest/container-manifest-2")
            })
            .expect("manifest should be scanned");
        assert_eq!(manifest.for_packages.len(), 2);
        assert!(manifest.for_packages.contains(&bash.package_uid));
    }

    #[test]
    fn test_rpm_mariner_manifest_keeps_explicit_namespace_over_os_release() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let manifest_dir = temp_dir.path().join("var/lib/rpmmanifest");
        let os_release_dir = temp_dir.path().join("usr/lib");
        fs::create_dir_all(&manifest_dir).expect("create rpmmanifest dir");
        fs::create_dir_all(&os_release_dir).expect("create os-release dir");

        fs::copy(
            "testdata/rpm/var/lib/rpmmanifest/container-manifest-2",
            manifest_dir.join("container-manifest-2"),
        )
        .expect("copy RPM Mariner manifest fixture");
        fs::write(
            os_release_dir.join("os-release"),
            r#"NAME="Azure Linux"
ID=azurelinux
VERSION_ID="3"
PRETTY_NAME="Azure Linux 3.0"
"#,
        )
        .expect("write Azure Linux os-release fixture");

        let (files, result) = scan_and_assemble(temp_dir.path());

        let os_release = files
            .iter()
            .find(|file| file.path.ends_with("/usr/lib/os-release"))
            .expect("os-release should be scanned");
        assert!(os_release.package_data.iter().any(|pkg_data| {
            pkg_data.datasource_id == Some(DatasourceId::EtcOsRelease)
                && pkg_data.namespace.as_deref() == Some("azurelinux")
        }));

        let bash = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("bash"))
            .expect("bash package should be assembled");

        assert_eq!(bash.namespace.as_deref(), Some("mariner"));
        assert_eq!(
            bash.purl.as_deref(),
            Some("pkg:rpm/mariner/bash@5.0.17?arch=x86_64")
        );
    }

    #[test]
    fn test_rpm_mariner_manifest_scan_merges_package_license_file() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let manifest_dir = temp_dir.path().join("var/lib/rpmmanifest");
        let license_dir = temp_dir.path().join("usr/share/licenses/bash");
        fs::create_dir_all(&manifest_dir).expect("create rpmmanifest dir");
        fs::create_dir_all(&license_dir).expect("create license dir");

        let manifest_path = manifest_dir.join("container-manifest-2");
        let license_path = license_dir.join("LICENSE");
        fs::write(
            &manifest_path,
            "bash\t5.0.17\t1\t2\tMicrosoft\t3\t4\tx86_64\tsha256\tbash-5.0.17-1.cm2.x86_64.rpm\n",
        )
        .expect("write RPM Mariner manifest fixture");
        fs::write(&license_path, "GPL-3.0-or-later\n").expect("write license fixture");

        let (files, result) = scan_and_assemble(temp_dir.path());

        assert_eq!(result.packages.len(), 1, "packages: {:#?}", result.packages);
        let bash = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("bash"))
            .expect("bash package should be assembled");

        assert_eq!(bash.package_type, Some(PackageType::Rpm));
        assert!(
            bash.datasource_ids
                .contains(&DatasourceId::RpmMarinerManifest)
        );
        assert!(
            bash.datasource_ids
                .contains(&DatasourceId::RpmPackageLicenses)
        );
        let mut actual_datafile_paths = bash.datafile_paths.clone();
        actual_datafile_paths.sort();
        let mut expected_datafile_paths = vec![
            manifest_path.to_string_lossy().to_string(),
            license_path.to_string_lossy().to_string(),
        ];
        expected_datafile_paths.sort();
        assert_eq!(actual_datafile_paths, expected_datafile_paths);

        let license_file = files
            .iter()
            .find(|file| file.path == license_path.to_string_lossy())
            .expect("license file should be scanned");
        assert_eq!(license_file.for_packages, vec![bash.package_uid.clone()]);
    }
}
