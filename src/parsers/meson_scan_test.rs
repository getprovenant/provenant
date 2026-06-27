// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::scan_test_utils::scan_and_assemble;
    use crate::models::DatasourceId;

    #[test]
    fn test_meson_scan_silently_falls_back_for_unsupported_multiline_strings() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let manifest_path = temp_dir.path().join("meson.build");
        fs::write(
            &manifest_path,
            r#"
project(
  'manual',
  version : files('.version'),
)

custom_target(
  'manual',
  command : [
    'bash',
    '''
      echo hello
    ''',
  ],
)
"#,
        )
        .expect("write meson.build");

        let (files, _result) = scan_and_assemble(temp_dir.path());

        let file = files
            .iter()
            .find(|file| file.path.ends_with("/meson.build"))
            .expect("meson.build should be scanned");

        assert!(
            file.scan_diagnostics.is_empty(),
            "{:?}",
            file.scan_diagnostics
        );
        assert!(
            file.package_data
                .iter()
                .any(|package| package.datasource_id == Some(DatasourceId::MesonBuild))
        );
    }

    #[test]
    fn test_meson_project_promotes_to_top_level_package_with_attached_deps() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(
            temp_dir.path().join("meson.build"),
            "project('demo', 'c', version: '1.2.3')\nzlib = dependency('zlib')\n",
        )
        .expect("write root meson.build");
        // A subdirectory build file with no project() declaration must not become
        // its own package.
        let sub = temp_dir.path().join("sub");
        fs::create_dir(&sub).expect("create sub dir");
        fs::write(sub.join("meson.build"), "executable('x', 'x.c')\n")
            .expect("write sub meson.build");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        let meson_pkgs: Vec<_> = result
            .packages
            .iter()
            .filter(|p| {
                p.purl
                    .as_deref()
                    .is_some_and(|purl| purl.starts_with("pkg:meson/"))
            })
            .collect();
        assert_eq!(
            meson_pkgs.len(),
            1,
            "only the project()-bearing meson.build should promote: {:?}",
            result.packages.iter().map(|p| &p.purl).collect::<Vec<_>>()
        );
        assert_eq!(meson_pkgs[0].purl.as_deref(), Some("pkg:meson/demo@1.2.3"));

        // The project's declared dependency is hoisted and owned by that package,
        // not left orphaned with a null for_package_uid.
        let zlib = result
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:generic/meson/zlib"))
            .expect("zlib dependency should be present");
        assert_eq!(
            zlib.for_package_uid.as_ref(),
            Some(&meson_pkgs[0].package_uid)
        );
    }
}
