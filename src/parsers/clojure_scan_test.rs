// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::models::DatasourceId;

    use super::super::scan_test_utils::scan_and_assemble;

    #[test]
    fn test_leiningen_project_promotes_to_top_level_package_with_attached_deps() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(
            temp_dir.path().join("project.clj"),
            r#"(defproject org.example/sample "1.0.0"
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [ring/ring-core "1.9.6"]])
"#,
        )
        .expect("write project.clj");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        // The defproject becomes a top-level package that owns its declared
        // dependencies, rather than dropping the identity and orphaning the deps.
        let package = result
            .packages
            .iter()
            .find(|p| p.name.as_deref() == Some("sample"))
            .expect("project.clj should promote to a top-level package");
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:maven/org.example/sample@1.0.0")
        );

        let clojure_dep = result
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:maven/org.clojure/clojure@1.11.1"))
            .expect("declared dependency should be present");
        assert_eq!(
            clojure_dep.for_package_uid.as_ref(),
            Some(&package.package_uid)
        );
    }

    #[test]
    fn test_deps_edn_attach_to_colocated_project_clj_package() {
        let (files, result) =
            scan_and_assemble(std::path::Path::new("testdata/clojure-golden/assembly"));

        let package = result
            .packages
            .iter()
            .find(|p| p.name.as_deref() == Some("assembly-demo"))
            .expect("project.clj should promote to a top-level package");
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:maven/org.example/assembly-demo@1.0.0")
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::ClojureDepsEdn)
        );
        assert!(
            package
                .datafile_paths
                .iter()
                .any(|path| path.ends_with("/deps.edn"))
        );

        let deps_file = files
            .iter()
            .find(|file| file.path.ends_with("/deps.edn"))
            .expect("deps.edn should be scanned");
        assert!(deps_file.for_packages.contains(&package.package_uid));

        for (purl, scope, is_runtime, is_optional) in [
            (
                "pkg:maven/com.cognitect/transit-clj@1.0.333",
                None,
                Some(true),
                Some(false),
            ),
            (
                "pkg:maven/org.clojure/tools.logging@1.3.0",
                None,
                Some(true),
                Some(false),
            ),
            (
                "pkg:maven/io.github.cognitect-labs/test-runner@0.5.1",
                Some("test"),
                Some(false),
                Some(true),
            ),
        ] {
            let dependency = result
                .dependencies
                .iter()
                .find(|dependency| {
                    dependency.purl.as_deref() == Some(purl)
                        && dependency.datasource_id == DatasourceId::ClojureDepsEdn
                })
                .unwrap_or_else(|| panic!("deps.edn dependency {purl} should be visible"));
            assert_eq!(
                dependency.for_package_uid.as_ref(),
                Some(&package.package_uid)
            );
            assert_eq!(dependency.scope.as_deref(), scope);
            assert_eq!(dependency.is_runtime, is_runtime);
            assert_eq!(dependency.is_optional, is_optional);
            assert_eq!(dependency.is_direct, Some(true));
        }

        assert!(
            result.dependencies.iter().any(|dependency| {
                dependency.purl.as_deref() == Some("pkg:maven/org.clojure/clojure@1.11.1")
                    && dependency.datasource_id == DatasourceId::ClojureProjectClj
                    && dependency.for_package_uid.as_ref() == Some(&package.package_uid)
            }),
            "project.clj dependencies should remain assigned to the owning package"
        );
    }

    #[test]
    fn test_dynamic_defproject_promotes_and_attaches_colocated_deps_edn() {
        // A `project.clj` whose version is a runtime `(or …)` form and whose
        // dependencies use `~`-unquoted `def` bindings must still promote to a
        // package, which in turn lets the co-located `deps.edn` attach its deps.
        let (files, result) = scan_and_assemble(std::path::Path::new(
            "testdata/clojure-golden/assembly-dynamic",
        ));

        let package = result
            .packages
            .iter()
            .find(|p| p.name.as_deref() == Some("dynamic-demo"))
            .expect("dynamic-version project.clj should still promote to a package");
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:maven/org.example/dynamic-demo@2.0.0")
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::ClojureDepsEdn),
            "co-located deps.edn should attach once the package promotes"
        );

        // The `~netty-version` dependency resolves from the file-local `def`.
        let netty = result
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:maven/io.netty/netty-transport@4.1.135.Final"))
            .expect("~def-resolved project.clj dependency should be present");
        assert_eq!(netty.datasource_id, DatasourceId::ClojureProjectClj);
        assert_eq!(netty.for_package_uid.as_ref(), Some(&package.package_uid));

        // The co-located deps.edn dependency is now owned rather than orphaned.
        let deps_file = files
            .iter()
            .find(|file| file.path.ends_with("/deps.edn"))
            .expect("deps.edn should be scanned");
        assert!(deps_file.for_packages.contains(&package.package_uid));

        let transit = result
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:maven/com.cognitect/transit-clj@1.0.333"))
            .expect("deps.edn dependency should be visible");
        assert_eq!(transit.datasource_id, DatasourceId::ClojureDepsEdn);
        assert_eq!(transit.for_package_uid.as_ref(), Some(&package.package_uid));
    }
}
