// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::scan_test_utils::{
        assert_dependency_present, assert_file_links_to_package, scan_and_assemble,
    };
    use crate::models::{DatasourceId, PackageType};

    #[test]
    fn cargo_target_dependencies_survive_assembly() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"[package]
name = "demo"
version = "1.0.0"
[target.'cfg(windows)'.dependencies]
shared = "1"
[target.x86_64-unknown-linux-gnu.dev-dependencies]
shared = "2"
[target.'cfg(unix)'.build-dependencies]
builder = "3"
"#,
        )
        .unwrap();
        let (_, result) = scan_and_assemble(dir.path());
        assert_eq!(result.dependencies.len(), 3);
        for (purl, scope, target, runtime) in [
            ("pkg:cargo/shared", "dependencies", "cfg(windows)", true),
            (
                "pkg:cargo/shared",
                "dev-dependencies",
                "x86_64-unknown-linux-gnu",
                false,
            ),
            (
                "pkg:cargo/builder",
                "build-dependencies",
                "cfg(unix)",
                false,
            ),
        ] {
            assert!(result.dependencies.iter().any(|dep| {
                dep.purl.as_deref() == Some(purl)
                    && dep.scope.as_deref() == Some(scope)
                    && dep.is_runtime == Some(runtime)
                    && dep
                        .extra_data
                        .as_ref()
                        .and_then(|data| data.get("target"))
                        .and_then(|value| value.as_str())
                        == Some(target)
            }));
        }
    }

    #[test]
    fn test_cargo_basic_scan_assembles_manifest_and_lockfile() {
        let (files, result) = scan_and_assemble(Path::new("testdata/assembly-golden/cargo-basic"));

        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("test-crate"))
            .expect("cargo package should be assembled");

        assert_eq!(package.package_type, Some(PackageType::Cargo));
        assert_eq!(package.version.as_deref(), Some("0.1.0"));
        assert_eq!(package.purl.as_deref(), Some("pkg:cargo/test-crate@0.1.0"));
        assert_dependency_present(&result.dependencies, "pkg:cargo/serde", "Cargo.toml");
        assert_dependency_present(
            &result.dependencies,
            "pkg:cargo/serde@1.0.195",
            "Cargo.lock",
        );
        assert_file_links_to_package(
            &files,
            "/Cargo.toml",
            &package.package_uid,
            DatasourceId::CargoToml,
        );
        assert_file_links_to_package(
            &files,
            "/Cargo.lock",
            &package.package_uid,
            DatasourceId::CargoLock,
        );
    }

    #[test]
    fn test_cargo_scan_assigns_workspace_resources_to_package() {
        let (files, result) = scan_and_assemble(Path::new("testdata/assembly-golden/cargo-basic"));

        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("test-crate"))
            .expect("cargo package should be assembled");

        let readme = files
            .iter()
            .find(|file| file.path.ends_with("/README.md"))
            .expect("README should be scanned");
        let lib_rs = files
            .iter()
            .find(|file| file.path.ends_with("/src/lib.rs"))
            .expect("src/lib.rs should be scanned");

        assert!(readme.for_packages.contains(&package.package_uid));
        assert!(lib_rs.for_packages.contains(&package.package_uid));
    }
}
