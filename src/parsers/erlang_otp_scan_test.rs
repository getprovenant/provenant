// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::scan_test_utils::{assert_dependency_present, scan_and_assemble};
    use crate::models::DatasourceId;

    #[test]
    fn test_erlang_otp_scan_hoists_rebar_manifest_and_lock_dependencies() {
        let (files, result) =
            scan_and_assemble(Path::new("testdata/assembly-golden/erlang-otp-basic"));

        assert!(result.packages.is_empty());
        assert_eq!(result.dependencies.len(), 8);
        assert!(
            result
                .dependencies
                .iter()
                .all(|dependency| dependency.for_package_uid.is_none())
        );

        assert_dependency_present(
            &result.dependencies,
            "pkg:hex/cowboy@2.10.0",
            "rebar.config",
        );
        assert_dependency_present(&result.dependencies, "pkg:hex/jiffy@1.1.1", "rebar.config");
        assert_dependency_present(&result.dependencies, "pkg:hex/proper@1.4.0", "rebar.config");
        assert_dependency_present(&result.dependencies, "pkg:hex/cowboy@2.10.0", "rebar.lock");
        assert_dependency_present(&result.dependencies, "pkg:hex/cowlib@2.12.1", "rebar.lock");
        assert_dependency_present(
            &result.dependencies,
            "pkg:hex/jiffy@abc123def456",
            "rebar.lock",
        );

        let rebar_config = files
            .iter()
            .find(|file| file.path.ends_with("/rebar.config"))
            .expect("rebar.config should be scanned");
        let rebar_lock = files
            .iter()
            .find(|file| file.path.ends_with("/rebar.lock"))
            .expect("rebar.lock should be scanned");

        assert!(rebar_config.for_packages.is_empty());
        assert!(rebar_lock.for_packages.is_empty());

        assert!(
            rebar_config.package_data.iter().any(|package_data| {
                package_data.datasource_id == Some(DatasourceId::RebarConfig)
            })
        );
        assert!(
            rebar_lock.package_data.iter().any(|package_data| {
                package_data.datasource_id == Some(DatasourceId::RebarLock)
            })
        );
    }

    #[test]
    fn test_erlang_app_src_promotes_to_top_level_package_owning_applications() {
        // `src/<app>.app.src` carries the app name/version, so it is the app's
        // identity source and promotes to a top-level package owning its
        // `applications` dependencies.
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let src = temp_dir.path().join("src");
        std::fs::create_dir(&src).expect("create src dir");
        std::fs::write(
            src.join("demo.app.src"),
            "{application, demo,\n [{vsn, \"1.0.0\"},\n  {applications, [kernel, stdlib, cowboy]}]}.\n",
        )
        .expect("write app.src");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        let package = result
            .packages
            .iter()
            .find(|p| p.purl.as_deref() == Some("pkg:hex/demo@1.0.0"))
            .expect(".app.src should promote to a top-level package");
        let dep = result
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:hex/cowboy"))
            .expect("applications dependency should be present");
        assert_eq!(dep.for_package_uid.as_ref(), Some(&package.package_uid));
    }
}
