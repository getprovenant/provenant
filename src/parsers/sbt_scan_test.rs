// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::scan_test_utils::scan_and_assemble;

    #[test]
    fn test_sbt_build_promotes_to_top_level_package_with_attached_deps() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(
            temp_dir.path().join("build.sbt"),
            r#"ThisBuild / organization := "com.example"
ThisBuild / version := "1.2.3"
name := "demo-app"
libraryDependencies += "org.typelevel" %% "cats-core" % "2.10.0"
"#,
        )
        .expect("write build.sbt");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        // The build.sbt project becomes a top-level package that owns its
        // libraryDependencies rather than dropping the identity and orphaning
        // the dependencies.
        let package = result
            .packages
            .iter()
            .find(|p| p.purl.as_deref() == Some("pkg:maven/com.example/demo-app@1.2.3"))
            .expect("build.sbt should promote to a top-level package");

        let dep = result
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:maven/org.typelevel/cats-core@2.10.0"))
            .expect("libraryDependencies entry should be present");
        assert_eq!(dep.for_package_uid.as_ref(), Some(&package.package_uid));
    }
}
