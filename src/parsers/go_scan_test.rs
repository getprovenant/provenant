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
    fn test_go_basic_scan_assembles_module_and_sum() {
        let (files, result) = scan_and_assemble(Path::new("testdata/assembly-golden/go-basic"));

        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("test-module"))
            .expect("go module should be assembled");

        assert_eq!(package.package_type, Some(PackageType::Golang));
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:golang/example.com/test-module")
        );
        assert_dependency_present(
            &result.dependencies,
            "pkg:golang/github.com/gin-gonic/gin@v1.9.0",
            "go.sum",
        );
        assert_file_links_to_package(&files, "/go.mod", &package.package_uid, DatasourceId::GoMod);
        assert_file_links_to_package(&files, "/go.sum", &package.package_uid, DatasourceId::GoSum);
    }

    #[test]
    fn test_go_mod_graph_deplock_scan_skips_go_and_toolchain_nodes() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::write(
            temp_dir.path().join("go.mod"),
            "module example.com/main\n\ngo 1.22.0\n\nrequire github.com/a/b v1.0.0\n",
        )
        .expect("write go.mod");
        std::fs::write(
            temp_dir.path().join("go-mod-graph.deplock"),
            "example.com/main go@1.22.0\nexample.com/main toolchain@go1.22.3\n\
             example.com/main github.com/a/b@v1.0.0\ngo@1.22.0 toolchain@go1.22.3\n\
             github.com/a/b@v1.0.0 github.com/c/d@v0.1.0\ngithub.com/a/b@v1.0.0 go@1.20\n",
        )
        .expect("write go-mod-graph.deplock");

        let (files, result) = scan_and_assemble(temp_dir.path());
        let package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("main"))
            .expect("go module should be assembled");
        assert_file_links_to_package(
            &files,
            "/go-mod-graph.deplock",
            &package.package_uid,
            DatasourceId::GoModGraph,
        );

        let graph_deps: Vec<_> = result
            .dependencies
            .iter()
            .filter(|dep| dep.datasource_id == DatasourceId::GoModGraph)
            .collect();
        let mut purls: Vec<_> = graph_deps
            .iter()
            .filter_map(|dep| dep.purl.as_deref())
            .collect();
        purls.sort_unstable();
        assert_eq!(
            purls,
            vec![
                "pkg:golang/github.com/a/b@v1.0.0",
                "pkg:golang/github.com/c/d@v0.1.0"
            ]
        );
        assert!(graph_deps.iter().all(|dep| dep.is_runtime.is_none()
            && dep.is_optional.is_none()
            && dep.for_package_uid.as_deref() == Some(package.package_uid.as_str())));
    }
}
