// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use super::super::assemble;
    use crate::models::{
        DatasourceId, Dependency, FileInfo, FileType, Package, PackageData, PackageType,
        PackageUid, Sha256Digest,
    };
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_file_info(
        path: &str,
        datasource_id: DatasourceId,
        purl: Option<&str>,
        name: Option<&str>,
        version: Option<&str>,
        dependencies: Vec<Dependency>,
    ) -> FileInfo {
        let path_parts: Vec<&str> = path.split('/').collect();
        let file_name = path_parts.last().unwrap_or(&"");
        let extension = file_name.split('.').next_back().unwrap_or("");

        FileInfo {
            name: file_name.to_string(),
            base_name: file_name.to_string(),
            extension: extension.to_string(),
            path: path.to_string(),
            file_type: FileType::File,
            mime_type: Some("application/json".to_string()),
            file_type_label: None,
            size: 100,
            date: None,
            sha1: None,
            md5: None,
            sha256: None,
            programming_language: None,
            package_data: vec![PackageData {
                datasource_id: Some(datasource_id),
                purl: purl.map(|s| s.to_string()),
                name: name.map(|s| s.to_string()),
                version: version.map(|s| s.to_string()),
                dependencies,
                ..Default::default()
            }],
            detected_license_expression: None,
            detected_license_expression_spdx: None,
            license_detections: vec![],
            license_clues: vec![],
            percentage_of_license_text: None,
            copyrights: vec![],
            holders: vec![],
            authors: vec![],
            emails: vec![],
            urls: vec![],
            for_packages: vec![],
            scan_diagnostics: vec![],
            license_policy: None,
            is_source: None,
            files_count: None,
            dirs_count: None,
            size_count: None,
            source_count: None,
            is_legal: false,
            is_manifest: false,
            is_readme: false,
            is_top_level: false,
            is_key_file: false,
            is_referenced: false,
            is_community: false,
            is_generated: None,
            sha1_git: None,
            is_binary: None,
            is_text: None,
            is_archive: None,
            is_media: None,
            is_script: None,
            facets: vec![],
            tallies: None,
        }
    }

    #[test]
    fn test_windows_update_assembly_prefers_update_mum_as_primary_identity() {
        let mut hidden_pkg = create_test_file_info(
            "windows/package_41_for_kb5050109~31bf3856ad364e35~amd64~~14393.7692.1.1.mum",
            DatasourceId::MicrosoftUpdateManifestMum,
            None,
            Some("Package_41_for_KB5050109"),
            Some("14393.7692.1.1"),
            vec![],
        );
        hidden_pkg.package_data[0].package_type = Some(PackageType::WindowsUpdate);

        let mut update_root = create_test_file_info(
            "windows/update.mum",
            DatasourceId::MicrosoftUpdateManifestMum,
            None,
            Some("Package_for_KB5050109"),
            Some("14393.7692.1.1"),
            vec![],
        );
        update_root.package_data[0].package_type = Some(PackageType::WindowsUpdate);

        let mut files = vec![hidden_pkg, update_root];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1, "packages: {:#?}", result.packages);
        let package = &result.packages[0];
        assert_eq!(package.name.as_deref(), Some("Package_for_KB5050109"));
        assert_eq!(package.version.as_deref(), Some("14393.7692.1.1"));
        assert!(
            package
                .datafile_paths
                .iter()
                .any(|path| path == "windows/update.mum"),
            "datafile_paths: {:?}",
            package.datafile_paths
        );
    }

    fn create_maven_pom_file_info(
        path: &str,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> FileInfo {
        let purl = format!("pkg:maven/{namespace}/{name}@{version}");
        let mut file = create_test_file_info(
            path,
            DatasourceId::MavenPom,
            Some(&purl),
            Some(name),
            Some(version),
            vec![],
        );
        file.package_data[0].namespace = Some(namespace.to_string());
        file.package_data[0].package_type = Some(PackageType::Maven);
        file
    }

    fn create_maven_reactor_root_file_info(
        path: &str,
        namespace: &str,
        name: &str,
        version: &str,
        modules: &[&str],
    ) -> FileInfo {
        let mut file = create_maven_pom_file_info(path, namespace, name, version);
        let mut extra_data = HashMap::new();
        extra_data.insert(
            "modules".to_string(),
            json!(modules.iter().collect::<Vec<_>>()),
        );
        file.package_data[0].extra_data = Some(extra_data);
        file
    }

    fn create_plain_source_file_info(path: &str) -> FileInfo {
        let mut file =
            create_test_file_info(path, DatasourceId::MavenPom, None, None, None, vec![]);
        file.package_data.clear();
        file
    }

    #[test]
    fn test_maven_reactor_assigns_module_source_files_to_module_package() {
        // A root pom.xml declaring <modules>module-a, module-b</modules> plus two
        // real module poms. Source files nested under each module must attach to
        // that module's own package (not the root, and not left orphaned), a
        // root-level file with no manifest attaches to the root package, and
        // Maven build output (`target/`) is excluded from ownership entirely.
        let mut files = vec![
            create_maven_reactor_root_file_info(
                "reactor/pom.xml",
                "org.example",
                "parent",
                "1.0",
                &["module-a", "module-b"],
            ),
            create_plain_source_file_info("reactor/README.md"),
            create_maven_pom_file_info(
                "reactor/module-a/pom.xml",
                "org.example",
                "module-a",
                "1.0",
            ),
            create_plain_source_file_info("reactor/module-a/src/main/java/com/example/Foo.java"),
            create_plain_source_file_info("reactor/module-a/target/classes/com/example/Foo.class"),
            create_plain_source_file_info(
                "reactor/module-a/src/main/java/com/example/target/Foo.java",
            ),
            create_maven_pom_file_info(
                "reactor/module-b/pom.xml",
                "org.example",
                "module-b",
                "1.0",
            ),
            create_plain_source_file_info("reactor/module-b/src/main/java/com/example/Bar.java"),
        ];

        let result = assemble(&mut files);

        let mut purls: Vec<&str> = result
            .packages
            .iter()
            .filter_map(|pkg| pkg.purl.as_deref())
            .collect();
        purls.sort_unstable();
        assert_eq!(
            purls,
            vec![
                "pkg:maven/org.example/module-a@1.0",
                "pkg:maven/org.example/module-b@1.0",
                "pkg:maven/org.example/parent@1.0",
            ],
            "root and both modules must each assemble into their own package: {:#?}",
            result.packages
        );

        let for_packages_purls = |path: &str, files: &[FileInfo]| -> Vec<String> {
            let file = files.iter().find(|f| f.path == path).unwrap_or_else(|| {
                panic!("file {path} should exist in scan results");
            });
            file.for_packages
                .iter()
                .map(|uid| {
                    result
                        .packages
                        .iter()
                        .find(|pkg| pkg.package_uid == *uid)
                        .and_then(|pkg| pkg.purl.clone())
                        .unwrap_or_default()
                })
                .collect()
        };

        assert_eq!(
            for_packages_purls("reactor/README.md", &files),
            vec!["pkg:maven/org.example/parent@1.0"],
            "a root-level file with no manifest must attach to the reactor root package"
        );
        assert_eq!(
            for_packages_purls(
                "reactor/module-a/src/main/java/com/example/Foo.java",
                &files
            ),
            vec!["pkg:maven/org.example/module-a@1.0"],
            "a source file nested under module-a must attach to module-a, not the root"
        );
        assert_eq!(
            for_packages_purls(
                "reactor/module-b/src/main/java/com/example/Bar.java",
                &files
            ),
            vec!["pkg:maven/org.example/module-b@1.0"],
            "a source file nested under module-b must attach to module-b, not the root"
        );
        assert!(
            for_packages_purls(
                "reactor/module-a/target/classes/com/example/Foo.class",
                &files
            )
            .is_empty(),
            "Maven build output under target/ must stay unowned by the source package"
        );
        assert_eq!(
            for_packages_purls(
                "reactor/module-a/src/main/java/com/example/target/Foo.java",
                &files
            ),
            vec!["pkg:maven/org.example/module-a@1.0"],
            "a source file merely nested under a directory named `target` deeper in the \
             tree (not the module's immediate target/ build-output dir) must still attach \
             to module-a"
        );
    }

    #[test]
    fn test_maven_reactor_nested_module_wins_over_outer_root() {
        // A module that itself declares further <modules> (a nested reactor)
        // contributes its own, more specific anchor. Files under the nested
        // module must resolve to it, not to the outer root, even though both
        // anchors' scope roots contain the file.
        let mut files = vec![
            create_maven_reactor_root_file_info(
                "reactor/pom.xml",
                "org.example",
                "parent",
                "1.0",
                &["module-a"],
            ),
            create_maven_reactor_root_file_info(
                "reactor/module-a/pom.xml",
                "org.example",
                "module-a",
                "1.0",
                &["submodule"],
            ),
            create_maven_pom_file_info(
                "reactor/module-a/submodule/pom.xml",
                "org.example",
                "submodule",
                "1.0",
            ),
            create_plain_source_file_info(
                "reactor/module-a/submodule/src/main/java/com/example/Baz.java",
            ),
        ];

        let result = assemble(&mut files);

        let submodule_file = files
            .iter()
            .find(|f| f.path == "reactor/module-a/submodule/src/main/java/com/example/Baz.java")
            .expect("submodule source file should exist");
        let owning_purls: Vec<String> = submodule_file
            .for_packages
            .iter()
            .map(|uid| {
                result
                    .packages
                    .iter()
                    .find(|pkg| pkg.package_uid == *uid)
                    .and_then(|pkg| pkg.purl.clone())
                    .unwrap_or_default()
            })
            .collect();

        assert_eq!(
            owning_purls,
            vec!["pkg:maven/org.example/submodule@1.0"],
            "a file under a nested module must attach to the nested module, not any outer reactor root"
        );
    }

    #[test]
    fn test_maven_reactor_resolves_relative_module_paths() {
        // Declared <module> strings are allowed to carry `.`/`..` components
        // (e.g. `./module-a`, `../sibling-module`). Raw path joining without
        // lexical normalization would keep those components and fail to match
        // the scanned pom.xml path, silently dropping the member from the
        // reactor. Both a same-directory `./` spelling and a `../` escape to a
        // sibling directory must still resolve to their real module package.
        let mut files = vec![
            create_maven_reactor_root_file_info(
                "reactor/parent/pom.xml",
                "org.example",
                "parent",
                "1.0",
                &["./module-a", "../sibling-module"],
            ),
            create_maven_pom_file_info(
                "reactor/parent/module-a/pom.xml",
                "org.example",
                "module-a",
                "1.0",
            ),
            create_plain_source_file_info(
                "reactor/parent/module-a/src/main/java/com/example/Foo.java",
            ),
            create_maven_pom_file_info(
                "reactor/sibling-module/pom.xml",
                "org.example",
                "sibling-module",
                "1.0",
            ),
            create_plain_source_file_info(
                "reactor/sibling-module/src/main/java/com/example/Bar.java",
            ),
        ];

        let result = assemble(&mut files);

        let mut purls: Vec<&str> = result
            .packages
            .iter()
            .filter_map(|pkg| pkg.purl.as_deref())
            .collect();
        purls.sort_unstable();
        assert_eq!(
            purls,
            vec![
                "pkg:maven/org.example/module-a@1.0",
                "pkg:maven/org.example/parent@1.0",
                "pkg:maven/org.example/sibling-module@1.0",
            ],
            "the relatively-spelled modules must still each assemble into their own package: {:#?}",
            result.packages
        );

        let owning_purls = |path: &str, files: &[FileInfo]| -> Vec<String> {
            let file = files.iter().find(|f| f.path == path).unwrap_or_else(|| {
                panic!("file {path} should exist in scan results");
            });
            file.for_packages
                .iter()
                .map(|uid| {
                    result
                        .packages
                        .iter()
                        .find(|pkg| pkg.package_uid == *uid)
                        .and_then(|pkg| pkg.purl.clone())
                        .unwrap_or_default()
                })
                .collect()
        };

        assert_eq!(
            owning_purls(
                "reactor/parent/module-a/src/main/java/com/example/Foo.java",
                &files
            ),
            vec!["pkg:maven/org.example/module-a@1.0"],
            "a `./module-a` declared module must still resolve and own its nested source file"
        );
        assert_eq!(
            owning_purls(
                "reactor/sibling-module/src/main/java/com/example/Bar.java",
                &files
            ),
            vec!["pkg:maven/org.example/sibling-module@1.0"],
            "a `../sibling-module` declared module must resolve outside the parent's own directory"
        );
    }

    #[test]
    fn test_maven_reactor_rejects_over_escaped_module_paths() {
        // An over-escaped `<module>` path such as `../../../module-a` must not
        // collapse onto an unrelated in-scan `module-a/` just because lexical
        // `..` popping ran out of parents. The declared path stays unresolved
        // and the reactor must not attribute that unrelated module's files.
        let mut files = vec![
            create_maven_reactor_root_file_info(
                "reactor/parent/pom.xml",
                "org.example",
                "parent",
                "1.0",
                &["../../../module-a"],
            ),
            create_maven_pom_file_info("module-a/pom.xml", "org.example", "module-a", "1.0"),
            create_plain_source_file_info("module-a/src/main/java/com/example/Foo.java"),
        ];

        let result = assemble(&mut files);

        assert!(
            files
                .iter()
                .find(|f| f.path == "module-a/src/main/java/com/example/Foo.java")
                .expect("source file should exist")
                .for_packages
                .is_empty(),
            "over-escaped reactor module paths must not claim unrelated in-scan modules: {:#?}",
            result.packages
        );
    }

    #[test]
    fn test_gradle_multi_project_owns_nested_sources_and_skips_build_output() {
        let mut settings = create_test_file_info(
            "repo/settings.gradle",
            DatasourceId::GradleSettings,
            None,
            None,
            None,
            vec![],
        );
        settings.package_data[0].package_type = Some(PackageType::Maven);
        settings.package_data[0].extra_data = Some(HashMap::from([
            (
                "projects".to_string(),
                json!(["./modules/app", "../../../escaped"]),
            ),
            ("root_project_name".to_string(), json!("gradle-root")),
        ]));

        let mut root_build = create_test_file_info(
            "repo/build.gradle",
            DatasourceId::BuildGradle,
            None,
            None,
            None,
            vec![],
        );
        root_build.package_data[0].package_type = Some(PackageType::Maven);
        root_build.package_data[0].extra_data = Some(HashMap::from([
            ("group".to_string(), json!("org.example")),
            ("version".to_string(), json!("1.0")),
        ]));

        let mut member_build = create_test_file_info(
            "repo/modules/app/build.gradle.kts",
            DatasourceId::BuildGradle,
            None,
            None,
            None,
            vec![],
        );
        member_build.package_data[0].package_type = Some(PackageType::Maven);
        member_build.package_data[0].extra_data = Some(HashMap::from([
            ("group".to_string(), json!("org.example")),
            ("version".to_string(), json!("1.0")),
        ]));

        let mut escaped_build = create_test_file_info(
            "escaped/build.gradle",
            DatasourceId::BuildGradle,
            None,
            None,
            None,
            vec![],
        );
        escaped_build.package_data[0].package_type = Some(PackageType::Maven);

        let mut files = vec![
            settings,
            root_build,
            create_plain_source_file_info("repo/README.md"),
            member_build,
            create_plain_source_file_info("repo/modules/app/src/main/java/App.java"),
            create_plain_source_file_info("repo/modules/app/build/classes/App.class"),
            escaped_build,
            create_plain_source_file_info("escaped/src/Escaped.java"),
        ];

        let result = assemble(&mut files);
        let root = result
            .packages
            .iter()
            .find(|package| {
                package.purl.as_deref() == Some("pkg:maven/org.example/gradle-root@1.0")
            })
            .expect("root Gradle package");
        let member = result
            .packages
            .iter()
            .find(|package| package.purl.as_deref() == Some("pkg:maven/org.example/app@1.0"))
            .expect("member Gradle package");

        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/README.md")
                .expect("root source")
                .for_packages
                .contains(&root.package_uid)
        );
        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/modules/app/src/main/java/App.java")
                .expect("member source")
                .for_packages
                .contains(&member.package_uid)
        );
        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/modules/app/build/classes/App.class")
                .expect("build output")
                .for_packages
                .is_empty()
        );
        assert!(
            files
                .iter()
                .find(|file| file.path == "escaped/src/Escaped.java")
                .expect("escaped source")
                .for_packages
                .is_empty(),
            "an over-escaped declared project must not claim an unrelated directory"
        );
    }

    #[test]
    fn test_gradle_multi_project_resolves_project_dir_remap() {
        // `include ':libs:core'` declares the default directory `libs/core`, but
        // a literal `project(':libs:core').projectDir = file('vendor/core')`
        // relocates it. Topology must resolve the member at its remapped
        // directory and own its nested sources there.
        let mut settings = create_test_file_info(
            "repo/settings.gradle",
            DatasourceId::GradleSettings,
            None,
            None,
            None,
            vec![],
        );
        settings.package_data[0].package_type = Some(PackageType::Maven);
        settings.package_data[0].extra_data = Some(HashMap::from([
            ("projects".to_string(), json!(["libs/core"])),
            ("root_project_name".to_string(), json!("gradle-root")),
            (
                "project_dir_overrides".to_string(),
                json!({ "libs/core": "vendor/core" }),
            ),
        ]));

        let mut root_build = create_test_file_info(
            "repo/build.gradle",
            DatasourceId::BuildGradle,
            None,
            None,
            None,
            vec![],
        );
        root_build.package_data[0].package_type = Some(PackageType::Maven);
        root_build.package_data[0].extra_data = Some(HashMap::from([
            ("group".to_string(), json!("org.example")),
            ("version".to_string(), json!("1.0")),
        ]));

        let mut member_build = create_test_file_info(
            "repo/vendor/core/build.gradle",
            DatasourceId::BuildGradle,
            None,
            None,
            None,
            vec![],
        );
        member_build.package_data[0].package_type = Some(PackageType::Maven);
        member_build.package_data[0].extra_data = Some(HashMap::from([
            ("group".to_string(), json!("org.example")),
            ("version".to_string(), json!("1.0")),
        ]));

        let mut files = vec![
            settings,
            root_build,
            member_build,
            create_plain_source_file_info("repo/vendor/core/src/main/java/Core.java"),
        ];

        let result = assemble(&mut files);
        let member = result
            .packages
            .iter()
            .find(|package| package.purl.as_deref() == Some("pkg:maven/org.example/core@1.0"))
            .expect("remapped member Gradle package");

        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/vendor/core/src/main/java/Core.java")
                .expect("remapped member source")
                .for_packages
                .contains(&member.package_uid)
        );
    }

    #[test]
    fn test_uv_workspace_owns_only_resolved_non_excluded_members() {
        let mut root = create_test_file_info(
            "repo/pyproject.toml",
            DatasourceId::PypiPyprojectToml,
            Some("pkg:pypi/workspace-root@1.0.0"),
            Some("workspace-root"),
            Some("1.0.0"),
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Pypi);
        root.package_data[0].extra_data = Some(HashMap::from([
            (
                "workspace_members".to_string(),
                json!(["./packages/*", "../../../escaped"]),
            ),
            ("workspace_exclude".to_string(), json!(["packages/ignored"])),
        ]));

        let mut member = create_test_file_info(
            "repo/packages/core/pyproject.toml",
            DatasourceId::PypiPyprojectToml,
            Some("pkg:pypi/core@0.1.0"),
            Some("core"),
            Some("0.1.0"),
            vec![],
        );
        member.package_data[0].package_type = Some(PackageType::Pypi);
        let mut ignored = create_test_file_info(
            "repo/packages/ignored/pyproject.toml",
            DatasourceId::PypiPyprojectToml,
            Some("pkg:pypi/ignored@0.1.0"),
            Some("ignored"),
            Some("0.1.0"),
            vec![],
        );
        ignored.package_data[0].package_type = Some(PackageType::Pypi);

        let mut files = vec![
            root,
            member,
            create_plain_source_file_info("repo/packages/core/src/core.py"),
            ignored,
            create_plain_source_file_info("repo/packages/ignored/src/ignored.py"),
            create_plain_source_file_info("escaped/src/escaped.py"),
        ];
        let result = assemble(&mut files);
        let core = result
            .packages
            .iter()
            .find(|package| package.purl.as_deref() == Some("pkg:pypi/core@0.1.0"))
            .expect("uv member package");

        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/packages/core/src/core.py")
                .expect("member source")
                .for_packages
                .contains(&core.package_uid)
        );
        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/packages/ignored/src/ignored.py")
                .expect("excluded source")
                .for_packages
                .is_empty()
        );
        assert!(
            files
                .iter()
                .find(|file| file.path == "escaped/src/escaped.py")
                .expect("escaped source")
                .for_packages
                .is_empty()
        );
    }

    #[test]
    fn test_uv_workspace_attributes_shared_lock_to_declaring_members() {
        // A uv workspace shares one root `uv.lock`. A locked entry that a
        // member's own `pyproject.toml` declares directly is attributed to that
        // member; a transitive-only entry that no member declares stays hoisted.
        let mut root = create_test_file_info(
            "repo/pyproject.toml",
            DatasourceId::PypiPyprojectToml,
            Some("pkg:pypi/workspace-root@1.0.0"),
            Some("workspace-root"),
            Some("1.0.0"),
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Pypi);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace_members".to_string(),
            json!(["packages/core"]),
        )]));

        let direct_requests = Dependency {
            purl: Some("pkg:pypi/requests".to_string()),
            extracted_requirement: Some(">=2.0".to_string()),
            scope: Some("dependencies".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(false),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        };
        let mut member = create_test_file_info(
            "repo/packages/core/pyproject.toml",
            DatasourceId::PypiPyprojectToml,
            Some("pkg:pypi/core@0.1.0"),
            Some("core"),
            Some("0.1.0"),
            vec![direct_requests],
        );
        member.package_data[0].package_type = Some(PackageType::Pypi);

        let mut lock = create_test_file_info(
            "repo/uv.lock",
            DatasourceId::PypiUvLock,
            None,
            None,
            None,
            vec![
                Dependency {
                    purl: Some("pkg:pypi/requests@2.31.0".to_string()),
                    extracted_requirement: None,
                    scope: None,
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(false),
                    resolved_package: None,
                    extra_data: None,
                },
                Dependency {
                    purl: Some("pkg:pypi/urllib3@2.2.0".to_string()),
                    extracted_requirement: None,
                    scope: None,
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(false),
                    resolved_package: None,
                    extra_data: None,
                },
            ],
        );
        lock.package_data[0].package_type = Some(PackageType::Pypi);

        let mut files = vec![
            root,
            lock,
            member,
            create_plain_source_file_info("repo/packages/core/src/core.py"),
        ];
        let result = assemble(&mut files);

        let core = result
            .packages
            .iter()
            .find(|package| package.purl.as_deref() == Some("pkg:pypi/core@0.1.0"))
            .expect("uv member package");

        // The directly-declared entry is attributed to the member that declares it.
        assert!(result.dependencies.iter().any(|dependency| {
            dependency.purl.as_deref() == Some("pkg:pypi/requests@2.31.0")
                && dependency.for_package_uid.as_ref() == Some(&core.package_uid)
        }));
        // The transitive-only entry no member declares stays hoisted.
        assert!(result.dependencies.iter().any(|dependency| {
            dependency.purl.as_deref() == Some("pkg:pypi/urllib3@2.2.0")
                && dependency.for_package_uid.is_none()
        }));
        // No shared-lock entry remains hoisted onto the root as a stale thin
        // attribution when a member declares it.
        assert!(!result.dependencies.iter().any(|dependency| {
            dependency.purl.as_deref() == Some("pkg:pypi/requests@2.31.0")
                && dependency.datafile_path == "repo/uv.lock"
                && dependency.for_package_uid.as_ref() != Some(&core.package_uid)
        }));
    }

    #[test]
    fn test_dart_workspace_claims_members_and_attributes_shared_lock_honestly() {
        let mut root = create_test_file_info(
            "repo/pubspec.yaml",
            DatasourceId::PubspecYaml,
            Some("pkg:pub/workspace_root@1.0.0"),
            Some("workspace_root"),
            Some("1.0.0"),
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Pub);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace_members".to_string(),
            json!(["./packages/app", "../../../escaped"]),
        )]));

        let direct_http = Dependency {
            purl: Some("pkg:pub/http".to_string()),
            extracted_requirement: Some("^1.0.0".to_string()),
            scope: Some("dependencies".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(false),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        };
        let mut member = create_test_file_info(
            "repo/packages/app/pubspec.yaml",
            DatasourceId::PubspecYaml,
            Some("pkg:pub/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![direct_http],
        );
        member.package_data[0].package_type = Some(PackageType::Pub);

        let mut lock = create_test_file_info(
            "repo/pubspec.lock",
            DatasourceId::PubspecLock,
            None,
            None,
            None,
            vec![
                Dependency {
                    purl: Some("pkg:pub/http@1.2.0".to_string()),
                    extracted_requirement: Some("1.2.0".to_string()),
                    scope: Some("direct main".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                },
                Dependency {
                    purl: Some("pkg:pub/collection@1.19.0".to_string()),
                    extracted_requirement: Some("1.19.0".to_string()),
                    scope: Some("transitive".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(false),
                    resolved_package: None,
                    extra_data: None,
                },
            ],
        );
        lock.package_data[0].package_type = Some(PackageType::Pub);

        let mut files = vec![
            root,
            lock,
            member,
            create_plain_source_file_info("repo/packages/app/lib/app.dart"),
            create_plain_source_file_info("escaped/lib/escaped.dart"),
        ];
        let result = assemble(&mut files);
        let app = result
            .packages
            .iter()
            .find(|package| package.purl.as_deref() == Some("pkg:pub/app@0.1.0"))
            .expect("Dart member package");

        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/packages/app/lib/app.dart")
                .expect("Dart member source")
                .for_packages
                .contains(&app.package_uid)
        );
        assert!(result.dependencies.iter().any(|dependency| {
            dependency.purl.as_deref() == Some("pkg:pub/http@1.2.0")
                && dependency.for_package_uid.as_ref() == Some(&app.package_uid)
        }));
        assert!(result.dependencies.iter().any(|dependency| {
            dependency.purl.as_deref() == Some("pkg:pub/collection@1.19.0")
                && dependency.for_package_uid.is_none()
        }));
        assert!(
            files
                .iter()
                .find(|file| file.path == "escaped/lib/escaped.dart")
                .expect("escaped source")
                .for_packages
                .is_empty()
        );
    }

    #[test]
    fn test_uv_workspace_exclude_rejects_descendants_of_excluded_dir() {
        // A recursive `members = ["packages/**"]` glob would otherwise pull back
        // a package nested under an excluded directory; a literal `exclude` must
        // drop the whole excluded subtree, not only the exact excluded directory.
        let mut root = create_test_file_info(
            "repo/pyproject.toml",
            DatasourceId::PypiPyprojectToml,
            Some("pkg:pypi/workspace-root@1.0.0"),
            Some("workspace-root"),
            Some("1.0.0"),
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Pypi);
        root.package_data[0].extra_data = Some(HashMap::from([
            ("workspace_members".to_string(), json!(["packages/**"])),
            ("workspace_exclude".to_string(), json!(["packages/ignored"])),
        ]));

        let mut included = create_test_file_info(
            "repo/packages/core/pyproject.toml",
            DatasourceId::PypiPyprojectToml,
            Some("pkg:pypi/core@0.1.0"),
            Some("core"),
            Some("0.1.0"),
            vec![],
        );
        included.package_data[0].package_type = Some(PackageType::Pypi);

        let mut nested = create_test_file_info(
            "repo/packages/ignored/sub/pyproject.toml",
            DatasourceId::PypiPyprojectToml,
            Some("pkg:pypi/nested@0.1.0"),
            Some("nested"),
            Some("0.1.0"),
            vec![],
        );
        nested.package_data[0].package_type = Some(PackageType::Pypi);

        let mut files = vec![
            root,
            included,
            create_plain_source_file_info("repo/packages/core/src/core.py"),
            nested,
            create_plain_source_file_info("repo/packages/ignored/sub/src/nested.py"),
        ];
        let result = assemble(&mut files);

        let core = result
            .packages
            .iter()
            .find(|package| package.purl.as_deref() == Some("pkg:pypi/core@0.1.0"))
            .expect("included uv member");
        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/packages/core/src/core.py")
                .expect("included member source")
                .for_packages
                .contains(&core.package_uid)
        );
        // The package under the excluded subtree is not adopted as a workspace
        // member, so the topology never attributes its nested sources.
        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/packages/ignored/sub/src/nested.py")
                .expect("excluded nested source")
                .for_packages
                .is_empty()
        );
    }

    #[test]
    fn test_dart_workspace_only_root_leaves_root_files_unowned() {
        // A workspace-only root pubspec (no package identity, e.g. a
        // `publish_to: none` root with no name) must not push its root-level
        // files into every member package.
        let mut root = create_test_file_info(
            "repo/pubspec.yaml",
            DatasourceId::PubspecYaml,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Pub);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace_members".to_string(),
            json!(["packages/app"]),
        )]));

        let mut member = create_test_file_info(
            "repo/packages/app/pubspec.yaml",
            DatasourceId::PubspecYaml,
            Some("pkg:pub/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![],
        );
        member.package_data[0].package_type = Some(PackageType::Pub);

        let mut files = vec![
            root,
            member,
            create_plain_source_file_info("repo/README.md"),
            create_plain_source_file_info("repo/packages/app/lib/app.dart"),
        ];
        let result = assemble(&mut files);

        let app = result
            .packages
            .iter()
            .find(|package| package.purl.as_deref() == Some("pkg:pub/app@0.1.0"))
            .expect("Dart member package");
        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/packages/app/lib/app.dart")
                .expect("member source")
                .for_packages
                .contains(&app.package_uid)
        );
        // The workspace-only root's README stays unowned rather than being
        // attributed to every member.
        assert!(
            files
                .iter()
                .find(|file| file.path == "repo/README.md")
                .expect("root readme")
                .for_packages
                .is_empty()
        );
    }

    #[test]
    fn test_maven_distinct_gav_poms_in_one_dir_stay_separate_packages() {
        // A directory of standalone `.pom` fixtures, each with a distinct GAV,
        // must NOT collapse into one top-level package.
        let mut files = vec![
            create_maven_pom_file_info("fixtures/m2/alpha-1.0.pom", "org.example", "alpha", "1.0"),
            create_maven_pom_file_info("fixtures/m2/beta-2.0.pom", "org.example", "beta", "2.0"),
            create_maven_pom_file_info("fixtures/m2/gamma-3.0.pom", "org.other", "gamma", "3.0"),
        ];

        let result = assemble(&mut files);

        let mut purls: Vec<&str> = result
            .packages
            .iter()
            .filter_map(|pkg| pkg.purl.as_deref())
            .collect();
        purls.sort_unstable();
        assert_eq!(
            purls,
            vec![
                "pkg:maven/org.example/alpha@1.0",
                "pkg:maven/org.example/beta@2.0",
                "pkg:maven/org.other/gamma@3.0",
            ],
            "each distinct-GAV pom must be its own package: {:#?}",
            result.packages
        );

        // Each package owns exactly its own datafile.
        for pkg in &result.packages {
            assert_eq!(
                pkg.datafile_paths.len(),
                1,
                "package {:?} should own a single datafile",
                pkg.purl
            );
        }
    }

    #[test]
    fn test_maven_same_gav_siblings_still_merge_into_one_package() {
        // A single real module: pom.xml plus a supplementary purl-less
        // MANIFEST.MF describing the SAME package must merge into one package.
        let pom =
            create_maven_pom_file_info("module/pom.xml", "com.example", "test-library", "1.0.0");

        let mut manifest = create_test_file_info(
            "module/META-INF/MANIFEST.MF",
            DatasourceId::JavaJarManifest,
            None,
            Some("Test Library"),
            Some("1.0.0"),
            vec![],
        );
        manifest.package_data[0].package_type = Some(PackageType::Maven);

        let mut files = vec![pom, manifest];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            1,
            "same-GAV module siblings must merge: {:#?}",
            result.packages
        );
        let package = &result.packages[0];
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:maven/com.example/test-library@1.0.0")
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::JavaJarManifest),
            "supplementary MANIFEST.MF should still merge into the module package: {:?}",
            package.datasource_ids
        );
    }

    #[test]
    fn test_maven_duplicate_gav_poms_merge_without_orphaning_files() {
        // Two `.pom` files sharing the SAME GAV alongside a third distinct GAV
        // still triggers the multi-GAV path (two distinct purls). The duplicate
        // must merge into one package owning both datafiles, never leaving a file
        // orphaned (associated with no package).
        let mut files = vec![
            create_maven_pom_file_info("dir/alpha-a.pom", "org.example", "alpha", "1.0"),
            create_maven_pom_file_info("dir/alpha-b.pom", "org.example", "alpha", "1.0"),
            create_maven_pom_file_info("dir/beta-2.0.pom", "org.example", "beta", "2.0"),
        ];

        let result = assemble(&mut files);

        let mut purls: Vec<&str> = result
            .packages
            .iter()
            .filter_map(|pkg| pkg.purl.as_deref())
            .collect();
        purls.sort_unstable();
        assert_eq!(
            purls,
            vec![
                "pkg:maven/org.example/alpha@1.0",
                "pkg:maven/org.example/beta@2.0"
            ],
            "duplicate-GAV poms must merge into one package: {:#?}",
            result.packages
        );

        let alpha = result
            .packages
            .iter()
            .find(|pkg| pkg.purl.as_deref() == Some("pkg:maven/org.example/alpha@1.0"))
            .expect("alpha package should exist");
        let mut alpha_datafiles = alpha.datafile_paths.clone();
        alpha_datafiles.sort();
        assert_eq!(
            alpha_datafiles,
            vec!["dir/alpha-a.pom".to_string(), "dir/alpha-b.pom".to_string()],
            "both duplicate-GAV datafiles must attach to the merged package"
        );

        // No scanned `.pom` is left orphaned.
        for file in &files {
            assert!(
                !file.for_packages.is_empty(),
                "file {} should belong to a package",
                file.path
            );
        }
    }

    fn create_test_dependency(
        purl: &str,
        extracted_requirement: Option<&str>,
        extra_data: Option<HashMap<String, serde_json::Value>>,
    ) -> Dependency {
        Dependency {
            purl: Some(purl.to_string()),
            extracted_requirement: extracted_requirement.map(str::to_string),
            scope: None,
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(false),
            is_direct: Some(true),
            resolved_package: None,
            extra_data,
        }
    }

    fn create_test_central_dependency(
        purl: &str,
        extracted_requirement: Option<&str>,
        extra_data: Option<HashMap<String, serde_json::Value>>,
    ) -> Dependency {
        let mut dependency = create_test_dependency(purl, extracted_requirement, extra_data);
        dependency.scope = Some("package_version".to_string());
        dependency
    }

    #[test]
    fn test_assemble_prunes_orphan_bazel_packages_after_npm_assignment() {
        let mut npm_file = create_test_file_info(
            "repo/package.json",
            DatasourceId::NpmPackageJson,
            Some("pkg:npm/demo@1.0.0"),
            Some("demo"),
            Some("1.0.0"),
            vec![],
        );
        npm_file.package_data[0].package_type = Some(PackageType::Npm);

        let mut bazel_file = create_test_file_info(
            "repo/closure/BUILD",
            DatasourceId::BazelBuild,
            Some("pkg:bazel/abstractspellchecker"),
            Some("abstractspellchecker"),
            None,
            vec![],
        );
        bazel_file.package_data[0].package_type = Some(PackageType::Bazel);

        let mut files = vec![npm_file, bazel_file];
        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.package_type, Some(PackageType::Npm));
        assert_eq!(package.name.as_deref(), Some("demo"));
        assert_eq!(files[0].for_packages, vec![package.package_uid.clone()]);
        assert_eq!(files[1].for_packages, vec![package.package_uid.clone()]);
    }

    #[test]
    fn test_assemble_keeps_bazel_package_when_it_owns_files() {
        let mut bazel_file = create_test_file_info(
            "repo/protobuf/BUILD",
            DatasourceId::BazelBuild,
            Some("pkg:bazel/parent_proto"),
            Some("parent_proto"),
            None,
            vec![],
        );
        bazel_file.package_data[0].package_type = Some(PackageType::Bazel);

        let mut files = vec![bazel_file];
        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.package_type, Some(PackageType::Bazel));
        assert_eq!(package.name.as_deref(), Some("parent_proto"));
        assert_eq!(files[0].for_packages, vec![package.package_uid.clone()]);
    }

    #[test]
    fn test_assemble_nuget_cpm_backfills_versionless_reference_from_nearest_ancestor() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("13.0.3")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_prefers_nearest_ancestor_props_file() {
        let mut root_props = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("12.0.1"),
                None,
            )],
        );
        root_props.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut nested_props = create_test_file_info(
            "repo/src/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        nested_props.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            root_props,
            nested_props,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("13.0.3")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_merges_imported_parent_props() {
        let mut parent_props = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        parent_props.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut child_props = create_test_file_info(
            "repo/src/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![],
        );
        child_props.package_data[0].extra_data = Some(HashMap::from([(
            "import_projects".to_string(),
            json!([
                "$([MSBuild]::GetPathOfFileAbove(Directory.Packages.props, $(MSBuildThisFileDirectory)..))"
            ]),
        )]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            parent_props,
            child_props,
        ];

        let result = assemble(&mut files);
        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("13.0.3")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_prefers_child_update_over_imported_parent() {
        let mut parent_props = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("12.0.1"),
                None,
            )],
        );
        parent_props.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut child_props = create_test_file_info(
            "repo/src/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        child_props.package_data[0].extra_data = Some(HashMap::from([
            ("manage_package_versions_centrally".to_string(), json!(true)),
            (
                "import_projects".to_string(),
                json!(["repo/Directory.Packages.props"]),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            parent_props,
            child_props,
        ];

        let result = assemble(&mut files);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("13.0.3")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_ignores_non_directory_packages_imports() {
        let mut non_cpm_import = create_test_file_info(
            "repo/Directory.Build.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        non_cpm_import.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut child_props = create_test_file_info(
            "repo/src/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![],
        );
        child_props.package_data[0].extra_data = Some(HashMap::from([
            ("manage_package_versions_centrally".to_string(), json!(true)),
            (
                "import_projects".to_string(),
                json!(["../Directory.Build.props"]),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            non_cpm_import,
            child_props,
        ];

        let result = assemble(&mut files);
        assert!(result.dependencies[0].extracted_requirement.is_none());
    }

    #[test]
    fn test_assemble_nuget_cpm_resolves_property_backed_version_override() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut project_file = create_test_file_info(
            "repo/src/Contoso.Utility.csproj",
            DatasourceId::NugetCsproj,
            Some("pkg:nuget/Contoso.Utility@1.0.0"),
            Some("Contoso.Utility"),
            Some("1.0.0"),
            vec![create_test_dependency(
                "pkg:nuget/Newtonsoft.Json",
                None,
                Some(HashMap::from([
                    (
                        "version_override".to_string(),
                        json!("$(NewtonsoftJsonVersion)"),
                    ),
                    ("version_override_resolved".to_string(), json!("14.0.1")),
                ])),
            )],
        );
        project_file.package_data[0].extra_data = Some(HashMap::from([(
            "central_package_version_override_enabled".to_string(),
            json!(true),
        )]));

        let mut files = vec![project_file, props_file];
        let result = assemble(&mut files);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("14.0.1")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_uses_directory_build_props_for_central_versions() {
        let mut build_props = create_test_file_info(
            "repo/src/Directory.Build.props",
            DatasourceId::NugetDirectoryBuildProps,
            None,
            None,
            None,
            vec![],
        );
        build_props.package_data[0].extra_data = Some(HashMap::from([(
            "property_values".to_string(),
            json!({
                "ManageVersions": "true",
                "NewtonsoftJsonVersion": "13.0.3"
            }),
        )]));

        let mut props_file = create_test_file_info(
            "repo/src/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            (
                "property_values".to_string(),
                json!({
                    "ManagePackageVersionsCentrally": "$(ManageVersions)"
                }),
            ),
            (
                "package_versions".to_string(),
                json!([
                    {
                        "name": "Newtonsoft.Json",
                        "version": "$(NewtonsoftJsonVersion)",
                        "condition": null
                    }
                ]),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            build_props,
            props_file,
        ];

        let result = assemble(&mut files);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("13.0.3")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_uses_composed_central_versions() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            (
                "property_values".to_string(),
                json!({
                    "ManagePackageVersionsCentrally": "true",
                    "VersionPrefix": "1.4.0",
                    "VersionSuffix": "preview.3"
                }),
            ),
            (
                "package_versions".to_string(),
                json!([
                    {
                        "name": "Newtonsoft.Json",
                        "version": "$(VersionPrefix)-$(VersionSuffix)",
                        "condition": null
                    }
                ]),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("1.4.0-preview.3")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_uses_optional_suffix_composed_central_versions() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            (
                "property_values".to_string(),
                json!({
                    "ManagePackageVersionsCentrally": "true",
                    "RegorusPackageVersion": "0.9.1",
                    "RegorusPackageVersionSuffix": "-$(VersionSuffix)"
                }),
            ),
            (
                "package_versions".to_string(),
                json!([
                    {
                        "name": "Microsoft.Regorus",
                        "version": "$(RegorusPackageVersion)$(RegorusPackageVersionSuffix)",
                        "condition": null
                    }
                ]),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Microsoft.Regorus",
                    None,
                    None,
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("0.9.1")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_leaves_composed_version_unresolved_when_all_properties_missing() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            (
                "property_values".to_string(),
                json!({
                    "ManagePackageVersionsCentrally": "true"
                }),
            ),
            (
                "package_versions".to_string(),
                json!([
                    {
                        "name": "Newtonsoft.Json",
                        "version": "$(VersionPrefix)-$(VersionSuffix)",
                        "condition": null
                    }
                ]),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);
        assert!(result.dependencies[0].extracted_requirement.is_none());
    }

    #[test]
    fn test_assemble_nuget_cpm_leaves_partially_unresolved_composed_versions_empty() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            (
                "property_values".to_string(),
                json!({
                    "ManagePackageVersionsCentrally": "true",
                    "VersionPrefix": "1.4.0"
                }),
            ),
            (
                "package_versions".to_string(),
                json!([
                    {
                        "name": "Newtonsoft.Json",
                        "version": "$(VersionPrefix)-$(VersionSuffix)",
                        "condition": null
                    }
                ]),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);
        assert!(result.dependencies[0].extracted_requirement.is_none());
    }

    #[test]
    fn test_assemble_nuget_cpm_preserves_imported_versions_when_local_raw_versions_exist() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Microsoft.Regorus",
                Some("0.9.1"),
                Some(HashMap::from([(
                    "version_expression".to_string(),
                    json!("$(RegorusPackageVersion)$(RegorusPackageVersionSuffix)"),
                )])),
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            (
                "property_values".to_string(),
                json!({
                    "ManagePackageVersionsCentrally": "true",
                    "VersionPrefix": "1.4.0",
                    "VersionSuffix": "preview.3"
                }),
            ),
            (
                "package_versions".to_string(),
                json!([
                    {
                        "name": "Newtonsoft.Json",
                        "version": "$(VersionPrefix)-$(VersionSuffix)",
                        "condition": null
                    }
                ]),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![
                    create_test_dependency("pkg:nuget/Newtonsoft.Json", None, None),
                    create_test_dependency("pkg:nuget/Microsoft.Regorus", None, None),
                ],
            ),
            props_file,
        ];

        let result = assemble(&mut files);
        assert_eq!(result.dependencies.len(), 2);

        let newtonsoft = result
            .dependencies
            .iter()
            .find(|dependency| dependency.purl.as_deref() == Some("pkg:nuget/Newtonsoft.Json"))
            .expect("missing Newtonsoft.Json dependency");
        assert_eq!(
            newtonsoft.extracted_requirement.as_deref(),
            Some("1.4.0-preview.3")
        );

        let regorus = result
            .dependencies
            .iter()
            .find(|dependency| dependency.purl.as_deref() == Some("pkg:nuget/Microsoft.Regorus"))
            .expect("missing Microsoft.Regorus dependency");
        assert_eq!(regorus.extracted_requirement.as_deref(), Some("0.9.1"));
    }

    #[test]
    fn test_assemble_nuget_cpm_uses_directory_build_props_for_version_override() {
        let mut build_props = create_test_file_info(
            "repo/src/Directory.Build.props",
            DatasourceId::NugetDirectoryBuildProps,
            None,
            None,
            None,
            vec![],
        );
        build_props.package_data[0].extra_data = Some(HashMap::from([(
            "property_values".to_string(),
            json!({
                "CentralOverridesEnabled": "true",
                "NewtonsoftJsonVersion": "14.0.1"
            }),
        )]));

        let mut props_file = create_test_file_info(
            "repo/src/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut project_file = create_test_file_info(
            "repo/src/app/Contoso.Utility.csproj",
            DatasourceId::NugetCsproj,
            Some("pkg:nuget/Contoso.Utility@1.0.0"),
            Some("Contoso.Utility"),
            Some("1.0.0"),
            vec![create_test_dependency(
                "pkg:nuget/Newtonsoft.Json",
                None,
                Some(HashMap::from([(
                    "version_override".to_string(),
                    json!("$(NewtonsoftJsonVersion)"),
                )])),
            )],
        );
        project_file.package_data[0].extra_data = Some(HashMap::from([(
            "central_package_version_override_enabled_raw".to_string(),
            json!("$(CentralOverridesEnabled)"),
        )]));

        let mut files = vec![project_file, build_props, props_file];
        let result = assemble(&mut files);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("14.0.1")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_ignores_conditioned_directory_build_props_data() {
        let mut build_props = create_test_file_info(
            "repo/src/Directory.Build.props",
            DatasourceId::NugetDirectoryBuildProps,
            None,
            None,
            None,
            vec![],
        );
        build_props.package_data[0].extra_data = Some(HashMap::from([
            (
                "property_values".to_string(),
                json!({
                    "ManageVersions": "true"
                }),
            ),
            ("import_projects".to_string(), json!([])),
        ]));

        let mut props_file = create_test_file_info(
            "repo/src/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            (
                "property_values".to_string(),
                json!({
                    "ManagePackageVersionsCentrally": "$(ManageVersions)"
                }),
            ),
            (
                "package_versions".to_string(),
                json!([
                    {
                        "name": "Newtonsoft.Json",
                        "version": "$(NewtonsoftJsonVersion)",
                        "condition": null
                    }
                ]),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/app/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            build_props,
            props_file,
        ];

        let result = assemble(&mut files);
        assert!(result.dependencies[0].extracted_requirement.is_none());
    }

    #[test]
    fn test_assemble_nuget_cpm_does_not_override_explicit_project_version() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    Some("12.0.1"),
                    None,
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("12.0.1")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_requires_matching_condition() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                Some(HashMap::from([(
                    "condition".to_string(),
                    json!("'$(TargetFramework)' == 'net472'"),
                )])),
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    Some(HashMap::from([(
                        "condition".to_string(),
                        json!("'$(TargetFramework)' == 'net8.0'"),
                    )])),
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert!(result.dependencies[0].extracted_requirement.is_none());
    }

    #[test]
    fn test_assemble_nuget_cpm_applies_exact_matching_condition() {
        let condition = "'$(TargetFramework)' == 'net8.0'";
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                Some(HashMap::from([("condition".to_string(), json!(condition))])),
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    Some(HashMap::from([("condition".to_string(), json!(condition))])),
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("13.0.3")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_requires_manage_package_versions_centrally_true() {
        let props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert!(result.dependencies[0].extracted_requirement.is_none());
    }

    #[test]
    fn test_assemble_hoists_unowned_standalone_pip_requirements_dependencies() {
        let mut files = vec![create_test_file_info(
            "docs/min_requirements.txt",
            DatasourceId::PipRequirements,
            None,
            None,
            None,
            vec![create_test_dependency(
                "pkg:pypi/sphinx@3.4.3",
                Some("==3.4.3"),
                None,
            )],
        )];

        let result = assemble(&mut files);

        assert!(result.packages.is_empty());
        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].purl.as_deref(),
            Some("pkg:pypi/sphinx@3.4.3")
        );
        assert_eq!(result.dependencies[0].for_package_uid, None);
        assert_eq!(
            result.dependencies[0].datafile_path,
            "docs/min_requirements.txt"
        );
        assert!(files[0].for_packages.is_empty());
    }

    #[test]
    fn test_assemble_does_not_hoist_unowned_nuget_cpm_metadata_dependencies() {
        let mut files = vec![create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        )];

        let result = assemble(&mut files);

        assert!(result.packages.is_empty());
        assert!(result.dependencies.is_empty());
        assert!(files[0].for_packages.is_empty());
    }

    #[test]
    fn test_assemble_creates_package_for_buck_metadata_without_package_type() {
        let mut files = vec![create_test_file_info(
            "repo/METADATA.bzl",
            DatasourceId::BuckMetadata,
            None,
            Some("example"),
            Some("0.0.1"),
            vec![],
        )];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.package_type, None);
        assert_eq!(package.name.as_deref(), Some("example"));
        assert_eq!(package.version.as_deref(), Some("0.0.1"));
        assert!(!package.package_uid.is_empty());
        assert!(
            package
                .package_uid
                .starts_with("generated-package:buck_metadata/example@0.0.1?uuid=")
        );
        assert!(package.datasource_ids.contains(&DatasourceId::BuckMetadata));
        assert_eq!(files[0].for_packages, vec![package.package_uid.clone()]);
    }

    #[test]
    fn test_assemble_nuget_cpm_prefers_version_override_when_enabled() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            ("manage_package_versions_centrally".to_string(), json!(true)),
            (
                "central_package_version_override_enabled".to_string(),
                json!(true),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    Some(HashMap::from([(
                        "version_override".to_string(),
                        json!("14.0.1"),
                    )])),
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("14.0.1")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_ignores_version_override_when_not_enabled() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    Some(HashMap::from([(
                        "version_override".to_string(),
                        json!("14.0.1"),
                    )])),
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("13.0.3")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_ignores_version_override_without_matching_central_entry() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Serilog",
                Some("3.1.1"),
                None,
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            ("manage_package_versions_centrally".to_string(), json!(true)),
            (
                "central_package_version_override_enabled".to_string(),
                json!(true),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    Some(HashMap::from([(
                        "version_override".to_string(),
                        json!("14.0.1"),
                    )])),
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert!(result.dependencies[0].extracted_requirement.is_none());
    }

    #[test]
    fn test_assemble_nuget_cpm_ignores_non_literal_version_override() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![create_test_central_dependency(
                "pkg:nuget/Newtonsoft.Json",
                Some("13.0.3"),
                None,
            )],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([
            ("manage_package_versions_centrally".to_string(), json!(true)),
            (
                "central_package_version_override_enabled".to_string(),
                json!(true),
            ),
        ]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    Some(HashMap::from([(
                        "version_override".to_string(),
                        json!("$(NewtonsoftJsonVersion)"),
                    )])),
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].extracted_requirement.as_deref(),
            Some("13.0.3")
        );
    }

    #[test]
    fn test_assemble_nuget_cpm_leaves_dependency_unresolved_when_matches_are_ambiguous() {
        let mut props_file = create_test_file_info(
            "repo/Directory.Packages.props",
            DatasourceId::NugetDirectoryPackagesProps,
            None,
            None,
            None,
            vec![
                create_test_central_dependency("pkg:nuget/Newtonsoft.Json", Some("13.0.3"), None),
                create_test_central_dependency("pkg:nuget/Newtonsoft.Json", Some("13.0.4"), None),
            ],
        );
        props_file.package_data[0].extra_data = Some(HashMap::from([(
            "manage_package_versions_centrally".to_string(),
            json!(true),
        )]));

        let mut files = vec![
            create_test_file_info(
                "repo/src/Contoso.Utility.csproj",
                DatasourceId::NugetCsproj,
                Some("pkg:nuget/Contoso.Utility@1.0.0"),
                Some("Contoso.Utility"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:nuget/Newtonsoft.Json",
                    None,
                    None,
                )],
            ),
            props_file,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.dependencies.len(), 1);
        assert!(result.dependencies[0].extracted_requirement.is_none());
    }

    #[test]
    fn test_assemble_npm_package_json_with_lockfile() {
        let dep = Dependency {
            purl: Some("pkg:npm/express@4.18.0".to_string()),
            extracted_requirement: Some("^4.18.0".to_string()),
            scope: Some("dependencies".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(false),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        };

        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/my-app@1.0.0"),
                Some("my-app"),
                Some("1.0.0"),
                vec![dep],
            ),
            create_test_file_info(
                "project/package-lock.json",
                DatasourceId::NpmPackageLockJson,
                Some("pkg:npm/my-app@1.0.0"),
                Some("my-app"),
                Some("1.0.0"),
                vec![],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1, "Expected exactly one package");
        let package = &result.packages[0];
        assert_eq!(package.name, Some("my-app".to_string()));
        assert!(
            package.package_uid.contains("uuid="),
            "Expected package_uid to contain uuid qualifier"
        );
        assert_eq!(
            package.datafile_paths.len(),
            2,
            "Expected both files in datafile_paths"
        );
        assert!(
            package
                .datafile_paths
                .contains(&"project/package.json".to_string())
        );
        assert!(
            package
                .datafile_paths
                .contains(&"project/package-lock.json".to_string())
        );
        assert_eq!(
            package.datasource_ids.len(),
            2,
            "Expected both datasource IDs"
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::NpmPackageJson)
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::NpmPackageLockJson)
        );

        assert_eq!(result.dependencies.len(), 1, "Expected one dependency");
        let dep = &result.dependencies[0];
        assert_eq!(dep.purl, Some("pkg:npm/express@4.18.0".to_string()));
        assert_eq!(dep.datafile_path, "project/package.json");
        assert_eq!(dep.datasource_id, DatasourceId::NpmPackageJson);
        assert!(
            dep.for_package_uid.is_some(),
            "Expected for_package_uid to be set"
        );
        assert!(
            dep.for_package_uid
                .as_ref()
                .expect("for_package_uid should be Some")
                .contains("uuid="),
            "Expected for_package_uid to contain uuid"
        );

        assert_eq!(
            files[0].for_packages.len(),
            1,
            "Expected package.json to have for_packages populated"
        );
        assert_eq!(
            files[1].for_packages.len(),
            1,
            "Expected package-lock.json to have for_packages populated"
        );
    }

    #[test]
    fn test_assemble_npm_package_json_with_yarn_pnp() {
        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/root-app@1.0.0"),
                Some("root-app"),
                Some("1.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/.pnp.cjs",
                DatasourceId::YarnPnpCjs,
                None,
                None,
                None,
                vec![Dependency {
                    purl: Some("pkg:npm/left-pad@1.3.0".to_string()),
                    extracted_requirement: Some("npm:1.3.0".to_string()),
                    scope: Some("dependencies".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].purl.as_deref(),
            Some("pkg:npm/left-pad@1.3.0")
        );
        assert_eq!(
            result.dependencies[0].datasource_id,
            DatasourceId::YarnPnpCjs
        );
        assert_eq!(result.dependencies[0].datafile_path, "project/.pnp.cjs");
        assert!(
            files[1]
                .for_packages
                .iter()
                .any(|uid| uid == &result.packages[0].package_uid)
        );
        assert!(
            result.packages[0]
                .datasource_ids
                .contains(&DatasourceId::YarnPnpCjs)
        );
    }

    #[test]
    fn test_assemble_npm_package_json_with_yarn_lock() {
        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/my-app@1.0.0"),
                Some("my-app"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:npm/rimraf",
                    Some("~2.5.4"),
                    None,
                )],
            ),
            create_test_file_info(
                "project/yarn.lock",
                DatasourceId::YarnLockV1,
                None,
                None,
                None,
                vec![create_test_dependency(
                    "pkg:npm/rimraf@2.5.4",
                    Some("2.5.4"),
                    None,
                )],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.name.as_deref(), Some("my-app"));
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::NpmPackageJson)
        );
        assert!(package.datasource_ids.contains(&DatasourceId::YarnLockV1));
        assert!(
            package
                .datafile_paths
                .contains(&"project/package.json".to_string())
        );
        assert!(
            package
                .datafile_paths
                .contains(&"project/yarn.lock".to_string())
        );

        assert_eq!(result.dependencies.len(), 2);
        assert!(result.dependencies.iter().any(|dep| {
            dep.purl.as_deref() == Some("pkg:npm/rimraf")
                && dep.datasource_id == DatasourceId::NpmPackageJson
                && dep.datafile_path == "project/package.json"
        }));
        assert!(result.dependencies.iter().any(|dep| {
            dep.purl.as_deref() == Some("pkg:npm/rimraf@2.5.4")
                && dep.datasource_id == DatasourceId::YarnLockV1
                && dep.datafile_path == "project/yarn.lock"
        }));
    }

    #[test]
    fn test_assemble_npm_package_json_skips_mismatched_lockfile() {
        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/my-app@1.0.0"),
                Some("my-app"),
                Some("1.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/package-lock.json",
                DatasourceId::NpmPackageLockJson,
                Some("pkg:npm/other-app@2.0.0"),
                Some("other-app"),
                Some("2.0.0"),
                vec![Dependency {
                    purl: Some("pkg:npm/left-pad@1.3.0".to_string()),
                    extracted_requirement: Some("1.3.0".to_string()),
                    scope: Some("dependencies".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(false),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            1,
            "Expected only the manifest package"
        );
        let package = &result.packages[0];
        assert_eq!(package.name, Some("my-app".to_string()));
        assert_eq!(
            package.datafile_paths,
            vec!["project/package.json".to_string()]
        );
        assert!(
            result.dependencies.is_empty(),
            "Mismatched lockfile deps should not merge"
        );
        assert_eq!(files[0].for_packages.len(), 1);
        assert!(
            files[1].for_packages.is_empty(),
            "Mismatched lockfile should remain unassigned"
        );
    }

    #[test]
    fn test_assemble_nix_flake_merges_lockfile_while_default_nix_stays_standalone() {
        let mut files = vec![
            create_test_file_info(
                "repo/flake.nix",
                DatasourceId::NixFlakeNix,
                Some("pkg:nix/demo-flake"),
                Some("demo-flake"),
                None,
                vec![],
            ),
            create_test_file_info(
                "repo/flake.lock",
                DatasourceId::NixFlakeLock,
                Some("pkg:nix/demo-flake"),
                Some("demo-flake"),
                None,
                vec![create_test_dependency(
                    "pkg:nix/nixpkgs@abc123",
                    Some("github:NixOS/nixpkgs"),
                    None,
                )],
            ),
            create_test_file_info(
                "repo/default.nix",
                DatasourceId::NixDefaultNix,
                Some("pkg:nix/demo-derivation@1.0.0"),
                Some("demo-derivation"),
                Some("1.0.0"),
                vec![],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 2);

        let flake_package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("demo-flake"))
            .expect("missing flake package");
        assert_eq!(flake_package.datafile_paths.len(), 2);
        assert!(
            flake_package
                .datafile_paths
                .contains(&"repo/flake.nix".to_string())
        );
        assert!(
            flake_package
                .datafile_paths
                .contains(&"repo/flake.lock".to_string())
        );
        assert!(
            flake_package
                .datasource_ids
                .contains(&DatasourceId::NixFlakeNix)
        );
        assert!(
            flake_package
                .datasource_ids
                .contains(&DatasourceId::NixFlakeLock)
        );

        let default_package = result
            .packages
            .iter()
            .find(|package| package.name.as_deref() == Some("demo-derivation"))
            .expect("missing default.nix package");
        assert_eq!(default_package.datafile_paths, vec!["repo/default.nix"]);
        assert_eq!(
            default_package.datasource_ids,
            vec![DatasourceId::NixDefaultNix]
        );

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(result.dependencies[0].datafile_path, "repo/flake.lock");
        assert_eq!(
            result.dependencies[0].for_package_uid.as_deref(),
            Some(flake_package.package_uid.as_str())
        );

        assert_eq!(files[0].for_packages.len(), 1);
        assert_eq!(files[1].for_packages.len(), 1);
        assert_eq!(files[2].for_packages.len(), 1);
        assert_eq!(files[0].for_packages[0], flake_package.package_uid);
        assert_eq!(files[1].for_packages[0], flake_package.package_uid);
        assert_eq!(files[2].for_packages[0], default_package.package_uid);
    }

    #[test]
    fn test_assemble_nix_flake_attaches_anonymous_flake_compat_default_nix() {
        let mut default_info = create_test_file_info(
            "repo/default.nix",
            DatasourceId::NixDefaultNix,
            None,
            None,
            None,
            vec![],
        );
        default_info.package_data[0].extra_data = Some(std::collections::HashMap::from([(
            "nix_wrapper_kind".to_string(),
            serde_json::Value::String("flake_compat".to_string()),
        )]));

        let mut files = vec![
            create_test_file_info(
                "repo/flake.nix",
                DatasourceId::NixFlakeNix,
                Some("pkg:nix/demo-flake"),
                Some("demo-flake"),
                None,
                vec![],
            ),
            create_test_file_info(
                "repo/flake.lock",
                DatasourceId::NixFlakeLock,
                Some("pkg:nix/demo-flake"),
                Some("demo-flake"),
                None,
                vec![],
            ),
            default_info,
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let flake_package = &result.packages[0];
        assert!(
            flake_package
                .datafile_paths
                .contains(&"repo/default.nix".to_string())
        );
        assert!(
            flake_package
                .datasource_ids
                .contains(&DatasourceId::NixDefaultNix)
        );
        assert_eq!(
            files[2].for_packages,
            vec![flake_package.package_uid.clone()]
        );
    }

    #[test]
    fn test_assemble_npm_package_json_skips_lockfile_with_same_name_different_version() {
        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/my-app@1.0.0"),
                Some("my-app"),
                Some("1.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/package-lock.json",
                DatasourceId::NpmPackageLockJson,
                Some("pkg:npm/my-app@2.0.0"),
                Some("my-app"),
                Some("2.0.0"),
                vec![Dependency {
                    purl: Some("pkg:npm/left-pad@1.3.0".to_string()),
                    extracted_requirement: Some("1.3.0".to_string()),
                    scope: Some("dependencies".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(false),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].name, Some("my-app".to_string()));
        assert_eq!(result.packages[0].version, Some("1.0.0".to_string()));
        assert_eq!(
            result.packages[0].datafile_paths,
            vec!["project/package.json".to_string()]
        );
        assert!(result.dependencies.is_empty());
        assert!(files[1].for_packages.is_empty());
    }

    #[test]
    fn test_assemble_npm_package_json_skips_lockfile_with_same_version_different_name() {
        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/my-app@1.0.0"),
                Some("my-app"),
                Some("1.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/package-lock.json",
                DatasourceId::NpmPackageLockJson,
                Some("pkg:npm/other-app@1.0.0"),
                Some("other-app"),
                Some("1.0.0"),
                vec![Dependency {
                    purl: Some("pkg:npm/left-pad@1.3.0".to_string()),
                    extracted_requirement: Some("1.3.0".to_string()),
                    scope: Some("dependencies".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(false),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].name, Some("my-app".to_string()));
        assert_eq!(result.packages[0].version, Some("1.0.0".to_string()));
        assert_eq!(
            result.packages[0].datafile_paths,
            vec!["project/package.json".to_string()]
        );
        assert!(result.dependencies.is_empty());
        assert!(files[1].for_packages.is_empty());
    }

    #[test]
    fn test_assemble_npm_package_json_merges_lockfile_with_missing_version_when_name_matches() {
        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/my-app@1.0.0"),
                Some("my-app"),
                Some("1.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/package-lock.json",
                DatasourceId::NpmPackageLockJson,
                None,
                Some("my-app"),
                None,
                vec![Dependency {
                    purl: Some("pkg:npm/left-pad@1.3.0".to_string()),
                    extracted_requirement: Some("1.3.0".to_string()),
                    scope: Some("dependencies".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(false),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].name, Some("my-app".to_string()));
        assert_eq!(result.packages[0].version, Some("1.0.0".to_string()));
        assert_eq!(
            result.packages[0].datafile_paths,
            vec![
                "project/package-lock.json".to_string(),
                "project/package.json".to_string()
            ]
        );
        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].purl.as_deref(),
            Some("pkg:npm/left-pad@1.3.0")
        );
        assert_eq!(
            result.dependencies[0].datafile_path,
            "project/package-lock.json"
        );
        assert_eq!(files[0].for_packages.len(), 1);
        assert_eq!(files[1].for_packages.len(), 1);
    }

    #[test]
    fn test_assemble_npm_package_json_and_lockfile_merge_when_both_omit_version() {
        let mut manifest = create_test_file_info(
            "project/package.json",
            DatasourceId::NpmPackageJson,
            None,
            Some("my-app"),
            None,
            vec![],
        );
        manifest.package_data[0].package_type = Some(PackageType::Npm);

        let mut lockfile = create_test_file_info(
            "project/package-lock.json",
            DatasourceId::NpmPackageLockJson,
            None,
            Some("my-app"),
            None,
            vec![Dependency {
                purl: Some("pkg:npm/left-pad@1.3.0".to_string()),
                extracted_requirement: Some("1.3.0".to_string()),
                scope: Some("dependencies".to_string()),
                is_runtime: Some(true),
                is_optional: Some(false),
                is_pinned: Some(true),
                is_direct: Some(false),
                resolved_package: None,
                extra_data: None,
            }],
        );
        lockfile.package_data[0].package_type = Some(PackageType::Npm);

        let mut files = vec![manifest, lockfile];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].name, Some("my-app".to_string()));
        assert_eq!(result.packages[0].version, None);
        assert_eq!(
            result.packages[0].datafile_paths,
            vec![
                "project/package-lock.json".to_string(),
                "project/package.json".to_string()
            ]
        );
        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].purl.as_deref(),
            Some("pkg:npm/left-pad@1.3.0")
        );
        assert!(result.dependencies[0].for_package_uid.is_some());
        assert_eq!(files[0].for_packages.len(), 1);
        assert_eq!(files[1].for_packages.len(), 1);
    }

    #[test]
    fn test_assemble_npm_package_json_skips_mismatched_bun_lock() {
        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/my-app@1.0.0"),
                Some("my-app"),
                Some("1.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/bun.lock",
                DatasourceId::BunLock,
                Some("pkg:npm/other-app@2.0.0"),
                Some("other-app"),
                Some("2.0.0"),
                vec![Dependency {
                    purl: Some("pkg:npm/left-pad@1.3.0".to_string()),
                    extracted_requirement: Some("1.3.0".to_string()),
                    scope: Some("dependencies".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].name.as_deref(), Some("my-app"));
        assert_eq!(
            result.packages[0].datafile_paths,
            vec!["project/package.json".to_string()]
        );
        assert!(result.dependencies.is_empty());
        assert!(files[1].for_packages.is_empty());
    }

    #[test]
    fn test_assemble_npm_package_json_skips_mismatched_bun_lockb() {
        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/my-app@1.0.0"),
                Some("my-app"),
                Some("1.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/bun.lockb",
                DatasourceId::BunLockb,
                Some("pkg:npm/other-app"),
                Some("other-app"),
                None,
                vec![Dependency {
                    purl: Some("pkg:npm/left-pad@1.3.0".to_string()),
                    extracted_requirement: Some("1.3.0".to_string()),
                    scope: Some("dependencies".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].name.as_deref(), Some("my-app"));
        assert_eq!(
            result.packages[0].datafile_paths,
            vec!["project/package.json".to_string()]
        );
        assert!(result.dependencies.is_empty());
        assert!(files[1].for_packages.is_empty());
    }

    #[test]
    fn test_assemble_cargo_toml_with_lock() {
        let mut files = vec![
            create_test_file_info(
                "project/Cargo.toml",
                DatasourceId::CargoToml,
                Some("pkg:cargo/my-crate@0.1.0"),
                Some("my-crate"),
                Some("0.1.0"),
                vec![],
            ),
            create_test_file_info(
                "project/Cargo.lock",
                DatasourceId::CargoLock,
                Some("pkg:cargo/my-crate@0.1.0"),
                Some("my-crate"),
                Some("0.1.0"),
                vec![],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1, "Expected exactly one package");
        let package = &result.packages[0];
        assert_eq!(package.name, Some("my-crate".to_string()));
        assert_eq!(package.version, Some("0.1.0".to_string()));
        assert!(
            package.package_uid.contains("uuid="),
            "Expected package_uid to contain uuid qualifier"
        );
        assert_eq!(
            package.datafile_paths.len(),
            2,
            "Expected both files in datafile_paths"
        );
        assert!(
            package
                .datafile_paths
                .contains(&"project/Cargo.toml".to_string())
        );
        assert!(
            package
                .datafile_paths
                .contains(&"project/Cargo.lock".to_string())
        );
        assert_eq!(
            package.datasource_ids.len(),
            2,
            "Expected both datasource IDs"
        );
        assert!(package.datasource_ids.contains(&DatasourceId::CargoToml));
        assert!(package.datasource_ids.contains(&DatasourceId::CargoLock));
    }

    #[test]
    fn test_assemble_cargo_workspace_preserves_member_lock_dependencies() {
        let mut root = create_test_file_info(
            "workspace/Cargo.toml",
            DatasourceId::CargoToml,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Cargo);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace".to_string(),
            json!({ "members": ["crates/app"] }),
        )]));

        let mut member_manifest = create_test_file_info(
            "workspace/crates/app/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![create_test_dependency("pkg:cargo/log", Some("0.4"), None)],
        );
        member_manifest.package_data[0].package_type = Some(PackageType::Cargo);

        let mut member_lock = create_test_file_info(
            "workspace/crates/app/Cargo.lock",
            DatasourceId::CargoLock,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![create_test_dependency(
                "pkg:cargo/log@0.4.22",
                Some("0.4.22"),
                None,
            )],
        );
        member_lock.package_data[0].package_type = Some(PackageType::Cargo);

        let mut files = vec![root, member_manifest, member_lock];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.purl.as_deref(), Some("pkg:cargo/app@0.1.0"));
        assert!(
            package
                .datafile_paths
                .contains(&"workspace/crates/app/Cargo.toml".to_string())
        );
        assert!(
            package
                .datafile_paths
                .contains(&"workspace/crates/app/Cargo.lock".to_string())
        );

        assert_eq!(result.dependencies.len(), 2);
        assert!(result.dependencies.iter().any(|dep| {
            dep.datafile_path == "workspace/crates/app/Cargo.toml"
                && dep.purl.as_deref() == Some("pkg:cargo/log")
                && dep.for_package_uid.as_ref() == Some(&package.package_uid)
        }));
        assert!(result.dependencies.iter().any(|dep| {
            dep.datafile_path == "workspace/crates/app/Cargo.lock"
                && dep.purl.as_deref() == Some("pkg:cargo/log@0.4.22")
                && dep.for_package_uid.as_ref() == Some(&package.package_uid)
        }));
    }

    #[test]
    fn test_assemble_cargo_workspace_preserves_member_lock_dependencies_when_lock_root_differs() {
        let mut root = create_test_file_info(
            "workspace/Cargo.toml",
            DatasourceId::CargoToml,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Cargo);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace".to_string(),
            json!({ "members": ["crates/app"] }),
        )]));

        let mut member_manifest = create_test_file_info(
            "workspace/crates/app/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![create_test_dependency("pkg:cargo/log", Some("0.4"), None)],
        );
        member_manifest.package_data[0].package_type = Some(PackageType::Cargo);

        let mut member_lock = create_test_file_info(
            "workspace/crates/app/Cargo.lock",
            DatasourceId::CargoLock,
            Some("pkg:cargo/workspace-helper@0.1.0"),
            Some("workspace-helper"),
            Some("0.1.0"),
            vec![create_test_dependency(
                "pkg:cargo/log@0.4.22",
                Some("0.4.22"),
                None,
            )],
        );
        member_lock.package_data[0].package_type = Some(PackageType::Cargo);

        let mut files = vec![root, member_manifest, member_lock];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.purl.as_deref(), Some("pkg:cargo/app@0.1.0"));

        assert_eq!(result.dependencies.len(), 2);
        assert!(result.dependencies.iter().any(|dep| {
            dep.datafile_path == "workspace/crates/app/Cargo.toml"
                && dep.purl.as_deref() == Some("pkg:cargo/log")
                && dep.for_package_uid.as_ref() == Some(&package.package_uid)
        }));
        assert!(result.dependencies.iter().any(|dep| {
            dep.datafile_path == "workspace/crates/app/Cargo.lock"
                && dep.purl.as_deref() == Some("pkg:cargo/log@0.4.22")
                && dep.for_package_uid.as_ref() == Some(&package.package_uid)
        }));
    }

    #[test]
    fn test_assemble_cargo_workspace_hoists_root_lock_dependencies() {
        let mut root = create_test_file_info(
            "workspace/Cargo.toml",
            DatasourceId::CargoToml,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Cargo);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace".to_string(),
            json!({ "members": ["crates/app"] }),
        )]));

        let mut root_lock = create_test_file_info(
            "workspace/Cargo.lock",
            DatasourceId::CargoLock,
            Some("pkg:cargo/workspace-root@0.0.0"),
            Some("workspace-root"),
            Some("0.0.0"),
            vec![create_test_dependency(
                "pkg:cargo/serde@1.0.215",
                Some("1.0.215"),
                None,
            )],
        );
        root_lock.package_data[0].package_type = Some(PackageType::Cargo);

        let mut member_manifest = create_test_file_info(
            "workspace/crates/app/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![],
        );
        member_manifest.package_data[0].package_type = Some(PackageType::Cargo);

        let mut files = vec![root, root_lock, member_manifest];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.dependencies.len(), 1);

        let dep = &result.dependencies[0];
        assert_eq!(dep.datafile_path, "workspace/Cargo.lock");
        assert_eq!(dep.purl.as_deref(), Some("pkg:cargo/serde@1.0.215"));
        assert!(dep.for_package_uid.is_none());
    }

    #[test]
    fn test_assemble_cargo_workspace_resolves_member_inherited_license() {
        let mut root = create_test_file_info(
            "workspace/Cargo.toml",
            DatasourceId::CargoToml,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Cargo);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace".to_string(),
            json!({
                "members": ["crates/app"],
                "package": { "license": "MIT OR Apache-2.0" },
            }),
        )]));

        let mut member_manifest = create_test_file_info(
            "workspace/crates/app/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![],
        );
        member_manifest.package_data[0].package_type = Some(PackageType::Cargo);
        // Member declares `license.workspace = true`, recorded as a marker by the parser.
        member_manifest.package_data[0].extra_data =
            Some(HashMap::from([("license".to_string(), json!("workspace"))]));

        let mut files = vec![root, member_manifest];
        let result = assemble(&mut files);

        let member = result
            .packages
            .iter()
            .find(|pkg| pkg.purl.as_deref() == Some("pkg:cargo/app@0.1.0"))
            .expect("member package should be assembled");
        // Inherited license is both captured as a statement and normalized into the
        // declared expression / SPDX fields with a backing detection.
        assert_eq!(
            member.extracted_license_statement.as_deref(),
            Some("MIT OR Apache-2.0")
        );
        assert_eq!(
            member.declared_license_expression.as_deref(),
            Some("apache-2.0 OR mit")
        );
        assert_eq!(
            member.declared_license_expression_spdx.as_deref(),
            Some("Apache-2.0 OR MIT")
        );
        assert!(!member.license_detections.is_empty());

        // The inherited license must also land on the member's file-level package
        // data, so the later license resync from file data does not clear it.
        let member_file_pkg = &files[1].package_data[0];
        assert_eq!(
            member_file_pkg.declared_license_expression.as_deref(),
            Some("apache-2.0 OR mit")
        );
        assert_eq!(
            member_file_pkg
                .extra_data
                .as_ref()
                .and_then(|extra| extra.get("license")),
            None,
            "workspace license marker should be consumed once resolved"
        );
        // File-level detections must carry the manifest path as `from_file`; the inherited
        // detection is cloned in with `from_file: None` and backfilled during assembly.
        let file_match = &member_file_pkg.license_detections[0].matches[0];
        assert_eq!(
            file_match.from_file.as_deref(),
            Some("workspace/crates/app/Cargo.toml")
        );
    }

    #[test]
    fn test_assemble_cargo_workspace_resolves_member_inherited_authors() {
        let mut root = create_test_file_info(
            "workspace/Cargo.toml",
            DatasourceId::CargoToml,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Cargo);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace".to_string(),
            json!({
                "members": ["crates/app"],
                "package": { "authors": ["uv", "Jane Doe <jane@example.com>"] },
            }),
        )]));

        let mut member_manifest = create_test_file_info(
            "workspace/crates/app/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![],
        );
        member_manifest.package_data[0].package_type = Some(PackageType::Cargo);
        // Member declares `authors.workspace = true`, recorded as a marker by the parser.
        member_manifest.package_data[0].extra_data =
            Some(HashMap::from([("authors".to_string(), json!("workspace"))]));

        let mut files = vec![root, member_manifest];
        let result = assemble(&mut files);

        let member = result
            .packages
            .iter()
            .find(|pkg| pkg.purl.as_deref() == Some("pkg:cargo/app@0.1.0"))
            .expect("member package should be assembled");
        let parties: Vec<(Option<&str>, Option<&str>)> = member
            .parties
            .iter()
            .map(|party| (party.name.as_deref(), party.email.as_deref()))
            .collect();
        assert_eq!(
            parties,
            vec![
                (Some("uv"), None),
                (Some("Jane Doe"), Some("jane@example.com")),
            ],
            "workspace authors should be resolved into parties, never the literal token"
        );
        // The marker must be consumed so the literal "workspace" never surfaces.
        assert_eq!(
            member
                .extra_data
                .as_ref()
                .and_then(|extra| extra.get("authors")),
            None,
            "workspace authors marker should be consumed once resolved"
        );
    }

    #[test]
    fn test_assemble_cargo_workspace_omits_inherited_authors_when_root_undeclared() {
        // Member inherits authors, but the workspace root never declares them.
        // The marker must still be consumed rather than leaked as an author named
        // "workspace".
        let mut root = create_test_file_info(
            "workspace/Cargo.toml",
            DatasourceId::CargoToml,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Cargo);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace".to_string(),
            json!({
                "members": ["crates/app"],
                "package": { "license": "MIT" },
            }),
        )]));

        let mut member_manifest = create_test_file_info(
            "workspace/crates/app/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![],
        );
        member_manifest.package_data[0].package_type = Some(PackageType::Cargo);
        member_manifest.package_data[0].extra_data =
            Some(HashMap::from([("authors".to_string(), json!("workspace"))]));

        let mut files = vec![root, member_manifest];
        let result = assemble(&mut files);

        let member = result
            .packages
            .iter()
            .find(|pkg| pkg.purl.as_deref() == Some("pkg:cargo/app@0.1.0"))
            .expect("member package should be assembled");
        assert!(
            member.parties.is_empty(),
            "unresolvable inherited authors must be omitted, not emitted as \"workspace\""
        );
        assert_eq!(
            member
                .extra_data
                .as_ref()
                .and_then(|extra| extra.get("authors")),
            None,
            "stale workspace authors marker must be removed even when unresolvable"
        );
        // The file-level package data must also be clean of the leaked marker.
        let member_file_pkg = &files[1].package_data[0];
        assert_eq!(
            member_file_pkg
                .extra_data
                .as_ref()
                .and_then(|extra| extra.get("authors")),
            None,
            "file-level workspace authors marker must be removed even when unresolvable"
        );
    }

    #[test]
    fn test_assemble_cargo_workspace_resolves_member_inherited_keywords_and_description() {
        let mut root = create_test_file_info(
            "workspace/Cargo.toml",
            DatasourceId::CargoToml,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Cargo);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace".to_string(),
            json!({
                "members": ["crates/app"],
                "package": {
                    "keywords": ["cli", "tool"],
                    "description": "shared description",
                },
            }),
        )]));

        let mut member_manifest = create_test_file_info(
            "workspace/crates/app/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![],
        );
        member_manifest.package_data[0].package_type = Some(PackageType::Cargo);
        member_manifest.package_data[0].extra_data = Some(HashMap::from([
            ("keywords".to_string(), json!("workspace")),
            ("description".to_string(), json!("workspace")),
        ]));

        let mut files = vec![root, member_manifest];
        let result = assemble(&mut files);

        let member = result
            .packages
            .iter()
            .find(|pkg| pkg.purl.as_deref() == Some("pkg:cargo/app@0.1.0"))
            .expect("member package should be assembled");
        assert_eq!(member.keywords, vec!["cli".to_string(), "tool".to_string()]);
        assert_eq!(member.description.as_deref(), Some("shared description"));
        let leftover: Vec<&String> = member
            .extra_data
            .as_ref()
            .map(|extra| {
                extra
                    .iter()
                    .filter(|(_, v)| v.as_str() == Some("workspace"))
                    .map(|(k, _)| k)
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            leftover.is_empty(),
            "no inheritance marker should leak the literal \"workspace\", found: {leftover:?}"
        );
    }

    #[test]
    fn test_assemble_cargo_workspace_omits_inherited_keywords_when_root_undeclared() {
        let mut root = create_test_file_info(
            "workspace/Cargo.toml",
            DatasourceId::CargoToml,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Cargo);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace".to_string(),
            json!({
                "members": ["crates/app"],
                "package": { "version": "0.1.0" },
            }),
        )]));

        let mut member_manifest = create_test_file_info(
            "workspace/crates/app/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![],
        );
        member_manifest.package_data[0].package_type = Some(PackageType::Cargo);
        member_manifest.package_data[0].extra_data = Some(HashMap::from([
            ("keywords".to_string(), json!("workspace")),
            ("description".to_string(), json!("workspace")),
        ]));

        let mut files = vec![root, member_manifest];
        let result = assemble(&mut files);

        let member = result
            .packages
            .iter()
            .find(|pkg| pkg.purl.as_deref() == Some("pkg:cargo/app@0.1.0"))
            .expect("member package should be assembled");
        assert!(
            member.keywords.is_empty(),
            "unresolvable inherited keywords must be omitted, not emitted as \"workspace\""
        );
        assert_eq!(
            member.description, None,
            "unresolvable inherited description must be omitted"
        );
        assert_eq!(
            member
                .extra_data
                .as_ref()
                .and_then(|extra| extra.get("keywords")),
            None,
            "stale workspace keywords marker must be removed even when unresolvable"
        );
    }

    #[test]
    fn test_assemble_cargo_workspace_keeps_root_package_when_root_is_real_member() {
        let mut root = create_test_file_info(
            "workspace/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/workspace-root@1.0.0"),
            Some("workspace-root"),
            Some("1.0.0"),
            vec![create_test_dependency("pkg:cargo/serde", Some("1.0"), None)],
        );
        root.package_data[0].package_type = Some(PackageType::Cargo);
        root.package_data[0].extra_data = Some(HashMap::from([(
            "workspace".to_string(),
            json!({ "members": ["crates/app"] }),
        )]));

        let mut member_manifest = create_test_file_info(
            "workspace/crates/app/Cargo.toml",
            DatasourceId::CargoToml,
            Some("pkg:cargo/app@0.1.0"),
            Some("app"),
            Some("0.1.0"),
            vec![],
        );
        member_manifest.package_data[0].package_type = Some(PackageType::Cargo);

        let mut files = vec![root, member_manifest];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 2);
        assert!(
            result
                .packages
                .iter()
                .any(|package| package.purl.as_deref() == Some("pkg:cargo/workspace-root@1.0.0"))
        );
        assert!(
            result
                .packages
                .iter()
                .any(|package| package.purl.as_deref() == Some("pkg:cargo/app@0.1.0"))
        );
    }

    #[test]
    fn test_assemble_mix_umbrella_dangling_in_umbrella_dependency_is_dropped() {
        let mut root = create_test_file_info(
            "umbrella/mix.exs",
            DatasourceId::HexMixExs,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Hex);
        root.package_data[0].extra_data =
            Some(HashMap::from([("apps_path".to_string(), json!("apps"))]));

        let mut member = create_test_file_info(
            "umbrella/apps/app_one/mix.exs",
            DatasourceId::HexMixExs,
            Some("pkg:hex/app_one@0.1.0"),
            Some("app_one"),
            Some("0.1.0"),
            vec![create_test_dependency(
                "pkg:hex/missing_sibling",
                None,
                Some(HashMap::from([
                    ("app".to_string(), json!("missing_sibling")),
                    ("in_umbrella".to_string(), json!(true)),
                ])),
            )],
        );
        member.package_data[0].package_type = Some(PackageType::Hex);

        let mut files = vec![root, member];
        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert!(
            result.dependencies.is_empty(),
            "a dangling in_umbrella reference must be dropped rather than fabricated, found: {:?}",
            result
                .dependencies
                .iter()
                .map(|d| d.purl.clone())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_assemble_mix_umbrella_apps_filter_excludes_app_from_lock_attribution() {
        let mut root = create_test_file_info(
            "umbrella/mix.exs",
            DatasourceId::HexMixExs,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Hex);
        root.package_data[0].extra_data = Some(HashMap::from([
            ("apps_path".to_string(), json!("apps")),
            ("apps".to_string(), json!(["app_one"])),
        ]));

        let mut root_lock = create_test_file_info(
            "umbrella/mix.lock",
            DatasourceId::HexMixLock,
            None,
            None,
            None,
            vec![create_test_dependency(
                "pkg:hex/ecto@3.10.0",
                Some("3.10.0"),
                Some(HashMap::from([("app".to_string(), json!("ecto"))])),
            )],
        );
        root_lock.package_data[0].package_type = Some(PackageType::Hex);

        let mut app_one = create_test_file_info(
            "umbrella/apps/app_one/mix.exs",
            DatasourceId::HexMixExs,
            Some("pkg:hex/app_one@0.1.0"),
            Some("app_one"),
            Some("0.1.0"),
            vec![],
        );
        app_one.package_data[0].package_type = Some(PackageType::Hex);

        // Excluded by `apps:` above, so it is not part of the umbrella
        // domain even though it declares the same lock entry.
        let mut app_two = create_test_file_info(
            "umbrella/apps/app_two/mix.exs",
            DatasourceId::HexMixExs,
            Some("pkg:hex/app_two@0.2.0"),
            Some("app_two"),
            Some("0.2.0"),
            vec![create_test_dependency(
                "pkg:hex/ecto",
                Some(">= 3.0.0"),
                Some(HashMap::from([("app".to_string(), json!("ecto"))])),
            )],
        );
        app_two.package_data[0].package_type = Some(PackageType::Hex);

        let mut files = vec![root, root_lock, app_one, app_two];
        let result = assemble(&mut files);

        let app_one_pkg = result
            .packages
            .iter()
            .find(|p| p.purl.as_deref() == Some("pkg:hex/app_one@0.1.0"))
            .expect("app_one should assemble to a package");

        // app_two is excluded from the umbrella domain by `apps:`, so the
        // shared lock's ecto entry is not attributed to it even though its
        // own (non-umbrella) mix.exs declares the same app name.
        assert!(
            !result.dependencies.iter().any(|d| {
                d.purl.as_deref() == Some("pkg:hex/ecto@3.10.0")
                    && d.for_package_uid.as_ref() == Some(&app_one_pkg.package_uid)
            }),
            "excluded app_two must not cause the lock entry to be misattributed to app_one"
        );

        // The excluded app's own mix.exs file must not be swept into
        // app_one's for_packages via the umbrella's root-fallback rule.
        let app_two_file = files
            .iter()
            .find(|f| f.path == "umbrella/apps/app_two/mix.exs")
            .expect("app_two mix.exs should be present");
        assert!(
            !app_two_file.for_packages.contains(&app_one_pkg.package_uid),
            "excluded app_two's manifest must not be attributed to app_one"
        );
    }

    #[test]
    fn test_assemble_mix_umbrella_member_local_lock_is_assembled() {
        let mut root = create_test_file_info(
            "umbrella/mix.exs",
            DatasourceId::HexMixExs,
            None,
            None,
            None,
            vec![],
        );
        root.package_data[0].package_type = Some(PackageType::Hex);
        root.package_data[0].extra_data =
            Some(HashMap::from([("apps_path".to_string(), json!("apps"))]));

        // No root mix.lock in this fixture: only app_one carries its own
        // mix.lock directly inside its member directory.
        let mut app_one = create_test_file_info(
            "umbrella/apps/app_one/mix.exs",
            DatasourceId::HexMixExs,
            Some("pkg:hex/app_one@0.1.0"),
            Some("app_one"),
            Some("0.1.0"),
            vec![create_test_dependency(
                "pkg:hex/jason",
                Some("~> 1.4"),
                Some(HashMap::from([("app".to_string(), json!("jason"))])),
            )],
        );
        app_one.package_data[0].package_type = Some(PackageType::Hex);

        let mut app_one_lock = create_test_file_info(
            "umbrella/apps/app_one/mix.lock",
            DatasourceId::HexMixLock,
            None,
            None,
            None,
            vec![create_test_dependency(
                "pkg:hex/jason@1.4.1",
                Some("1.4.1"),
                Some(HashMap::from([("app".to_string(), json!("jason"))])),
            )],
        );
        app_one_lock.package_data[0].package_type = Some(PackageType::Hex);

        let mut app_two = create_test_file_info(
            "umbrella/apps/app_two/mix.exs",
            DatasourceId::HexMixExs,
            Some("pkg:hex/app_two@0.2.0"),
            Some("app_two"),
            Some("0.2.0"),
            vec![],
        );
        app_two.package_data[0].package_type = Some(PackageType::Hex);

        let mut files = vec![root, app_one, app_one_lock, app_two];
        let result = assemble(&mut files);

        let app_one_pkg = result
            .packages
            .iter()
            .find(|p| p.purl.as_deref() == Some("pkg:hex/app_one@0.1.0"))
            .expect("app_one should assemble to a package");

        assert!(
            result.dependencies.iter().any(|d| {
                d.purl.as_deref() == Some("pkg:hex/jason@1.4.1")
                    && d.datasource_id == DatasourceId::HexMixLock
                    && d.for_package_uid.as_ref() == Some(&app_one_pkg.package_uid)
            }),
            "app_one's own member-local mix.lock must be assembled onto app_one, found: {:?}",
            result
                .dependencies
                .iter()
                .map(|d| (d.purl.clone(), d.for_package_uid.clone()))
                .collect::<Vec<_>>()
        );

        let app_one_lock_file = files
            .iter()
            .find(|f| f.path == "umbrella/apps/app_one/mix.lock")
            .expect("app_one mix.lock should be present");
        assert!(
            app_one_lock_file
                .for_packages
                .contains(&app_one_pkg.package_uid),
            "app_one's own mix.lock must be attributed to app_one"
        );
    }

    #[test]
    fn test_assemble_python_pyproject_with_uv_lock() {
        let mut files = vec![
            create_test_file_info(
                "project/pyproject.toml",
                DatasourceId::PypiPyprojectToml,
                Some("pkg:pypi/uv-demo@0.1.0"),
                Some("uv-demo"),
                Some("0.1.0"),
                vec![],
            ),
            create_test_file_info(
                "project/uv.lock",
                DatasourceId::PypiUvLock,
                Some("pkg:pypi/uv-demo@0.1.0"),
                Some("uv-demo"),
                Some("0.1.0"),
                vec![Dependency {
                    purl: Some("pkg:pypi/requests@2.32.5".to_string()),
                    extracted_requirement: Some(">=2.32.5".to_string()),
                    scope: None,
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        files[0].package_data[0].description = Some("Demo package".to_string());
        files[0].package_data[0].keywords = vec!["workflow".to_string(), "dag".to_string()];
        files[0].package_data[0].homepage_url = Some("https://example.com/home".to_string());
        files[0].package_data[0].bug_tracking_url = Some("https://example.com/issues".to_string());
        files[0].package_data[0].code_view_url = Some("https://example.com/source".to_string());
        files[0].package_data[0].parties = vec![crate::models::Party {
            r#type: None,
            role: Some("author".to_string()),
            name: Some("Example Author".to_string()),
            email: Some("author@example.com".to_string()),
            url: None,
            organization: None,
            organization_url: None,
            timezone: None,
        }];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            1,
            "Expected exactly one merged Python package"
        );
        let package = &result.packages[0];
        assert_eq!(package.name, Some("uv-demo".to_string()));
        assert_eq!(package.description.as_deref(), Some("Demo package"));
        assert_eq!(
            package.keywords,
            vec!["workflow".to_string(), "dag".to_string()]
        );
        assert_eq!(
            package.homepage_url.as_deref(),
            Some("https://example.com/home")
        );
        assert_eq!(
            package.bug_tracking_url.as_deref(),
            Some("https://example.com/issues")
        );
        assert_eq!(
            package.code_view_url.as_deref(),
            Some("https://example.com/source")
        );
        assert_eq!(package.parties.len(), 1);
        assert_eq!(package.parties[0].name.as_deref(), Some("Example Author"));
        assert!(
            package
                .datafile_paths
                .contains(&"project/pyproject.toml".to_string())
        );
        assert!(
            package
                .datafile_paths
                .contains(&"project/uv.lock".to_string())
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::PypiPyprojectToml)
        );
        assert!(package.datasource_ids.contains(&DatasourceId::PypiUvLock));

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].purl.as_deref(),
            Some("pkg:pypi/requests@2.32.5")
        );
        assert_eq!(files[0].for_packages.len(), 1);
        assert_eq!(files[1].for_packages.len(), 1);
    }

    #[test]
    fn test_assemble_poetry_pyproject_with_uv_lock() {
        let mut files = vec![
            create_test_file_info(
                "project/pyproject.toml",
                DatasourceId::PypiPoetryPyprojectToml,
                Some("pkg:pypi/uv-demo@0.1.0"),
                Some("uv-demo"),
                Some("0.1.0"),
                vec![],
            ),
            create_test_file_info(
                "project/uv.lock",
                DatasourceId::PypiUvLock,
                Some("pkg:pypi/uv-demo@0.1.0"),
                Some("uv-demo"),
                Some("0.1.0"),
                vec![Dependency {
                    purl: Some("pkg:pypi/requests@2.32.5".to_string()),
                    extracted_requirement: Some(">=2.32.5".to_string()),
                    scope: None,
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.name.as_deref(), Some("uv-demo"));
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::PypiPoetryPyprojectToml)
        );
        assert!(package.datasource_ids.contains(&DatasourceId::PypiUvLock));
        assert_eq!(result.dependencies.len(), 1);
    }

    #[test]
    fn test_assemble_poetry_pyproject_with_poetry_lock() {
        let mut files = vec![
            create_test_file_info(
                "project/pyproject.toml",
                DatasourceId::PypiPoetryPyprojectToml,
                Some("pkg:pypi/poetry-demo@0.1.0"),
                Some("poetry-demo"),
                Some("0.1.0"),
                vec![],
            ),
            create_test_file_info(
                "project/poetry.lock",
                DatasourceId::PypiPoetryLock,
                Some("pkg:pypi/poetry-demo@0.1.0"),
                Some("poetry-demo"),
                Some("0.1.0"),
                vec![Dependency {
                    purl: Some("pkg:pypi/requests@2.32.5".to_string()),
                    extracted_requirement: Some(">=2.32.5".to_string()),
                    scope: Some("main".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.name.as_deref(), Some("poetry-demo"));
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::PypiPoetryPyprojectToml)
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::PypiPoetryLock)
        );
        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].purl.as_deref(),
            Some("pkg:pypi/requests@2.32.5")
        );
    }

    #[test]
    fn test_assemble_python_pyproject_with_uv_lock_backfills_version_and_refreshes_uids() {
        let mut files = vec![
            create_test_file_info(
                "project/pyproject.toml",
                DatasourceId::PypiPyprojectToml,
                Some("pkg:pypi/uv-demo"),
                Some("uv-demo"),
                None,
                vec![Dependency {
                    purl: Some("pkg:pypi/httpx@0.27.0".to_string()),
                    extracted_requirement: Some(">=0.27.0".to_string()),
                    scope: None,
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(false),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
            create_test_file_info(
                "project/uv.lock",
                DatasourceId::PypiUvLock,
                Some("pkg:pypi/uv-demo@0.1.0"),
                Some("uv-demo"),
                Some("0.1.0"),
                vec![Dependency {
                    purl: Some("pkg:pypi/anyio@4.4.0".to_string()),
                    extracted_requirement: Some("==4.4.0".to_string()),
                    scope: Some("dev".to_string()),
                    is_runtime: Some(false),
                    is_optional: Some(true),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.version.as_deref(), Some("0.1.0"));
        assert_eq!(package.purl.as_deref(), Some("pkg:pypi/uv-demo@0.1.0"));
        assert!(
            package
                .package_uid
                .starts_with("pkg:pypi/uv-demo@0.1.0?uuid=")
        );
        assert_eq!(result.dependencies.len(), 2);
        assert!(
            result.dependencies.iter().all(|dep| {
                dep.for_package_uid.as_deref() == Some(package.package_uid.as_str())
            })
        );
    }

    #[test]
    fn test_assemble_python_pyproject_skips_uv_lock_with_same_name_different_version() {
        let mut files = vec![
            create_test_file_info(
                "project/pyproject.toml",
                DatasourceId::PypiPyprojectToml,
                Some("pkg:pypi/uv-demo@0.1.0"),
                Some("uv-demo"),
                Some("0.1.0"),
                vec![],
            ),
            create_test_file_info(
                "project/uv.lock",
                DatasourceId::PypiUvLock,
                Some("pkg:pypi/uv-demo@0.2.0"),
                Some("uv-demo"),
                Some("0.2.0"),
                vec![Dependency {
                    purl: Some("pkg:pypi/requests@2.32.5".to_string()),
                    extracted_requirement: Some("==2.32.5".to_string()),
                    scope: None,
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].version.as_deref(), Some("0.1.0"));
        assert_eq!(
            result.packages[0].datafile_paths,
            vec!["project/pyproject.toml".to_string()]
        );
        assert!(result.dependencies.is_empty());
        assert!(files[1].for_packages.is_empty());
    }

    #[test]
    fn test_assemble_python_pyproject_skips_uv_lock_with_same_version_different_name() {
        let mut files = vec![
            create_test_file_info(
                "project/pyproject.toml",
                DatasourceId::PypiPyprojectToml,
                Some("pkg:pypi/uv-demo@0.1.0"),
                Some("uv-demo"),
                Some("0.1.0"),
                vec![],
            ),
            create_test_file_info(
                "project/uv.lock",
                DatasourceId::PypiUvLock,
                Some("pkg:pypi/other-demo@0.1.0"),
                Some("other-demo"),
                Some("0.1.0"),
                vec![Dependency {
                    purl: Some("pkg:pypi/requests@2.32.5".to_string()),
                    extracted_requirement: Some("==2.32.5".to_string()),
                    scope: None,
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].name.as_deref(), Some("uv-demo"));
        assert_eq!(
            result.packages[0].datafile_paths,
            vec!["project/pyproject.toml".to_string()]
        );
        assert!(result.dependencies.is_empty());
        assert!(files[1].for_packages.is_empty());
    }

    #[test]
    fn test_assemble_python_pyproject_with_pylock_toml() {
        let mut files = vec![
            create_test_file_info(
                "project/pyproject.toml",
                DatasourceId::PypiPyprojectToml,
                Some("pkg:pypi/pylock-demo@0.1.0"),
                Some("pylock-demo"),
                Some("0.1.0"),
                vec![],
            ),
            create_test_file_info(
                "project/pylock.toml",
                DatasourceId::PypiPylockToml,
                None,
                None,
                None,
                vec![Dependency {
                    purl: Some("pkg:pypi/requests@2.32.3".to_string()),
                    extracted_requirement: None,
                    scope: None,
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            1,
            "Expected exactly one merged Python package"
        );
        let package = &result.packages[0];
        assert_eq!(package.name, Some("pylock-demo".to_string()));
        assert!(
            package
                .datafile_paths
                .contains(&"project/pyproject.toml".to_string())
        );
        assert!(
            package
                .datafile_paths
                .contains(&"project/pylock.toml".to_string())
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::PypiPyprojectToml)
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::PypiPylockToml)
        );

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].purl.as_deref(),
            Some("pkg:pypi/requests@2.32.3")
        );
        assert_eq!(files[0].for_packages.len(), 1);
        assert_eq!(files[1].for_packages.len(), 1);
    }

    #[test]
    fn test_assemble_hackage_multiple_cabal_files_do_not_collapse_into_one_package() {
        let mut files = vec![
            create_test_file_info(
                "project/alpha.cabal",
                DatasourceId::HackageCabal,
                Some("pkg:hackage/alpha@1.0.0"),
                Some("alpha"),
                Some("1.0.0"),
                vec![Dependency {
                    purl: Some("pkg:hackage/base".to_string()),
                    extracted_requirement: Some(">=4.14 && <5".to_string()),
                    scope: Some("build-depends".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(false),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
            create_test_file_info(
                "project/beta.cabal",
                DatasourceId::HackageCabal,
                Some("pkg:hackage/beta@2.0.0"),
                Some("beta"),
                Some("2.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/cabal.project",
                DatasourceId::HackageCabalProject,
                None,
                None,
                None,
                vec![Dependency {
                    purl: Some("pkg:hackage/lens@5.2.1".to_string()),
                    extracted_requirement: Some("5.2.1".to_string()),
                    scope: Some("extra-packages".to_string()),
                    is_runtime: None,
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 2);
        assert!(
            result
                .packages
                .iter()
                .any(|package| package.name.as_deref() == Some("alpha"))
        );
        assert!(
            result
                .packages
                .iter()
                .any(|package| package.name.as_deref() == Some("beta"))
        );
        assert!(
            result
                .packages
                .iter()
                .all(|package| package.datafile_paths.len() == 1)
        );
        assert!(
            result.dependencies.iter().any(|dependency| {
                dependency.purl.as_deref() == Some("pkg:hackage/lens@5.2.1")
                    && dependency.for_package_uid.is_none()
            }),
            "project-level Hackage dependency should stay unowned when multiple sibling manifests exist"
        );
        assert!(files[0].for_packages.len() == 1);
        assert!(files[1].for_packages.len() == 1);
        assert!(files[2].for_packages.is_empty());
    }

    #[test]
    fn test_assemble_bun_workspace_hoists_root_lockfile_dependencies() {
        let mut root_file = create_test_file_info(
            "project/package.json",
            DatasourceId::NpmPackageJson,
            None,
            None,
            None,
            vec![Dependency {
                purl: Some("pkg:npm/typescript".to_string()),
                extracted_requirement: Some("^5.0.0".to_string()),
                scope: Some("devDependencies".to_string()),
                is_runtime: Some(false),
                is_optional: Some(true),
                is_pinned: Some(false),
                is_direct: Some(true),
                resolved_package: None,
                extra_data: None,
            }],
        );
        root_file.package_data[0].extra_data = Some(HashMap::from([(
            "workspaces".to_string(),
            serde_json::json!(["packages/*"]),
        )]));

        let mut files = vec![
            root_file,
            create_test_file_info(
                "project/bun.lock",
                DatasourceId::BunLock,
                None,
                None,
                None,
                vec![Dependency {
                    purl: Some("pkg:npm/typescript@5.8.3".to_string()),
                    extracted_requirement: Some("5.8.3".to_string()),
                    scope: Some("devDependencies".to_string()),
                    is_runtime: Some(false),
                    is_optional: Some(true),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
            create_test_file_info(
                "project/packages/core/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/%40myorg/core@1.0.0"),
                Some("@myorg/core"),
                Some("1.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/packages/utils/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/%40myorg/utils@2.0.0"),
                Some("@myorg/utils"),
                Some("2.0.0"),
                vec![],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 2);
        assert!(
            result
                .packages
                .iter()
                .any(|pkg| pkg.name.as_deref() == Some("@myorg/core"))
        );
        assert!(
            result
                .packages
                .iter()
                .any(|pkg| pkg.name.as_deref() == Some("@myorg/utils"))
        );

        let bun_dep = result
            .dependencies
            .iter()
            .find(|dep| dep.datasource_id == DatasourceId::BunLock)
            .expect("expected bun.lock hoisted dependency");
        assert_eq!(bun_dep.purl.as_deref(), Some("pkg:npm/typescript@5.8.3"));
        assert_eq!(bun_dep.datafile_path, "project/bun.lock");
        assert!(bun_dep.for_package_uid.is_none());

        let bun_file = files
            .iter()
            .find(|file| file.path == "project/bun.lock")
            .expect("expected bun.lock file");
        assert_eq!(bun_file.for_packages.len(), 2);
    }

    #[test]
    fn test_assemble_bun_lockb_workspace_hoists_root_lockfile_dependencies() {
        let mut root_file = create_test_file_info(
            "project/package.json",
            DatasourceId::NpmPackageJson,
            None,
            None,
            None,
            vec![Dependency {
                purl: Some("pkg:npm/typescript".to_string()),
                extracted_requirement: Some("^5.0.0".to_string()),
                scope: Some("devDependencies".to_string()),
                is_runtime: Some(false),
                is_optional: Some(true),
                is_pinned: Some(false),
                is_direct: Some(true),
                resolved_package: None,
                extra_data: None,
            }],
        );
        root_file.package_data[0].extra_data = Some(HashMap::from([(
            "workspaces".to_string(),
            serde_json::json!(["packages/*"]),
        )]));

        let mut files = vec![
            root_file,
            create_test_file_info(
                "project/bun.lockb",
                DatasourceId::BunLockb,
                None,
                None,
                None,
                vec![Dependency {
                    purl: Some("pkg:npm/typescript@5.8.3".to_string()),
                    extracted_requirement: Some("5.8.3".to_string()),
                    scope: Some("devDependencies".to_string()),
                    is_runtime: Some(false),
                    is_optional: Some(true),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
            create_test_file_info(
                "project/packages/core/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/%40myorg/core@1.0.0"),
                Some("@myorg/core"),
                Some("1.0.0"),
                vec![],
            ),
            create_test_file_info(
                "project/packages/utils/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/%40myorg/utils@2.0.0"),
                Some("@myorg/utils"),
                Some("2.0.0"),
                vec![],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 2);
        let bun_dep = result
            .dependencies
            .iter()
            .find(|dep| dep.datasource_id == DatasourceId::BunLockb)
            .expect("expected bun.lockb hoisted dependency");
        assert_eq!(bun_dep.purl.as_deref(), Some("pkg:npm/typescript@5.8.3"));
        assert_eq!(bun_dep.datafile_path, "project/bun.lockb");
        assert!(bun_dep.for_package_uid.is_none());

        let bun_file = files
            .iter()
            .find(|file| file.path == "project/bun.lockb")
            .expect("expected bun.lockb file");
        assert_eq!(bun_file.for_packages.len(), 2);
    }

    #[test]
    fn test_assemble_python_pyproject_with_named_pylock_toml() {
        let mut files = vec![
            create_test_file_info(
                "project/pyproject.toml",
                DatasourceId::PypiPyprojectToml,
                Some("pkg:pypi/pylock-demo@0.1.0"),
                Some("pylock-demo"),
                Some("0.1.0"),
                vec![],
            ),
            create_test_file_info(
                "project/pylock.dev.toml",
                DatasourceId::PypiPylockToml,
                None,
                None,
                None,
                vec![Dependency {
                    purl: Some("pkg:pypi/pytest@8.3.5".to_string()),
                    extracted_requirement: None,
                    scope: Some("dev".to_string()),
                    is_runtime: Some(false),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            1,
            "Expected exactly one merged Python package"
        );
        let package = &result.packages[0];
        assert!(
            package
                .datafile_paths
                .contains(&"project/pylock.dev.toml".to_string())
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::PypiPylockToml)
        );
        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].purl.as_deref(),
            Some("pkg:pypi/pytest@8.3.5")
        );
    }

    #[test]
    fn test_assemble_python_pip_cache_origin_with_wheel_archive() {
        let mut files = vec![
            create_test_file_info(
                ".cache/pip/wheels/eb/60/37/hash/construct-2.10.68-py3-none-any.whl",
                DatasourceId::PypiWheel,
                Some("pkg:pypi/construct@2.10.68?extension=py3-none-any"),
                Some("construct"),
                Some("2.10.68"),
                vec![],
            ),
            create_test_file_info(
                ".cache/pip/wheels/eb/60/37/hash/origin.json",
                DatasourceId::PypiPipOriginJson,
                Some("pkg:pypi/construct@2.10.68?extension=py3-none-any"),
                Some("construct"),
                Some("2.10.68"),
                vec![],
            ),
        ];

        files[1].package_data[0].download_url = Some(
            "https://files.pythonhosted.org/packages/source/c/construct/construct-2.10.68.tar.gz"
                .to_string(),
        );

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1);
        let package = &result.packages[0];
        assert_eq!(package.name.as_deref(), Some("construct"));
        assert_eq!(package.version.as_deref(), Some("2.10.68"));
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:pypi/construct@2.10.68?extension=py3-none-any")
        );
        assert_eq!(
            package.download_url.as_deref(),
            Some(
                "https://files.pythonhosted.org/packages/source/c/construct/construct-2.10.68.tar.gz"
            )
        );
        assert!(package.datasource_ids.contains(&DatasourceId::PypiWheel));
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::PypiPipOriginJson)
        );
        assert_eq!(package.datafile_paths.len(), 2);
        assert!(files.iter().all(|file| file.for_packages.len() == 1));
        assert_eq!(files[0].for_packages[0], package.package_uid);
        assert_eq!(files[1].for_packages[0], package.package_uid);
    }

    #[test]
    fn test_one_per_package_data_hoists_purlless_record_dependencies() {
        // A purl-less manifest under OnePerPackageData cannot become a package,
        // but its dependencies must still be hoisted (unowned) rather than
        // dropped — the visibility they had before the datasource was assembled.
        let mut files = vec![
            create_test_file_info(
                "proj/meson.build",
                DatasourceId::MesonBuild,
                Some("pkg:meson/proj@1.0"),
                Some("proj"),
                Some("1.0"),
                vec![create_test_dependency("pkg:generic/meson/zlib", None, None)],
            ),
            create_test_file_info(
                "proj/sub/meson.build",
                DatasourceId::MesonBuild,
                None,
                None,
                None,
                vec![create_test_dependency(
                    "pkg:generic/meson/openssl",
                    None,
                    None,
                )],
            ),
        ];

        let result = assemble(&mut files);

        let proj = result
            .packages
            .iter()
            .find(|p| p.name.as_deref() == Some("proj"))
            .expect("project()-bearing meson.build should promote to a package");
        assert!(
            result.dependencies.iter().any(|d| {
                d.purl.as_deref() == Some("pkg:generic/meson/zlib")
                    && d.for_package_uid.as_ref() == Some(&proj.package_uid)
            }),
            "the named project must own its dependency"
        );
        let hoisted = result
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:generic/meson/openssl"))
            .expect("a purl-less record's dependency must be hoisted, not dropped");
        assert!(
            hoisted.for_package_uid.is_none(),
            "the hoisted dependency has no owning package"
        );
    }

    #[test]
    fn test_assemble_python_standalone_wheel_creates_top_level_package() {
        let mut files = vec![create_test_file_info(
            "dist/construct-2.10.68-py3-none-any.whl",
            DatasourceId::PypiWheel,
            Some("pkg:pypi/construct@2.10.68?extension=py3-none-any"),
            Some("construct"),
            Some("2.10.68"),
            vec![],
        )];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 1, "packages: {:#?}", result.packages);
        let package = &result.packages[0];
        assert_eq!(package.name.as_deref(), Some("construct"));
        assert_eq!(package.version.as_deref(), Some("2.10.68"));
        assert_eq!(
            package.purl.as_deref(),
            Some("pkg:pypi/construct@2.10.68?extension=py3-none-any")
        );
        assert_eq!(
            package.datafile_paths,
            vec!["dist/construct-2.10.68-py3-none-any.whl"]
        );
        assert_eq!(files[0].for_packages, vec![package.package_uid.clone()]);
    }

    #[test]
    fn test_assemble_python_distinct_wheels_in_one_dir_stay_separate_packages() {
        // A wheelhouse-style directory holding several distinct wheels yields one
        // package per identity. The `construct` wheel and its same-identity
        // `origin.json` merge into one package; the distinct `otherpkg` wheel
        // surfaces as its own package rather than being dropped.
        let mut files = vec![
            create_test_file_info(
                "wheelhouse/construct-2.10.68-py3-none-any.whl",
                DatasourceId::PypiWheel,
                Some("pkg:pypi/construct@2.10.68?extension=py3-none-any"),
                Some("construct"),
                Some("2.10.68"),
                vec![],
            ),
            create_test_file_info(
                "wheelhouse/origin.json",
                DatasourceId::PypiPipOriginJson,
                Some("pkg:pypi/construct@2.10.68?extension=py3-none-any"),
                Some("construct"),
                Some("2.10.68"),
                vec![],
            ),
            create_test_file_info(
                "wheelhouse/otherpkg-9.9.9-py3-none-any.whl",
                DatasourceId::PypiWheel,
                Some("pkg:pypi/otherpkg@9.9.9?extension=py3-none-any"),
                Some("otherpkg"),
                Some("9.9.9"),
                vec![],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(result.packages.len(), 2, "packages: {:#?}", result.packages);
        let construct = result
            .packages
            .iter()
            .find(|p| p.name.as_deref() == Some("construct"))
            .expect("construct package should be present");
        assert_eq!(construct.datafile_paths.len(), 2);
        let otherpkg = result
            .packages
            .iter()
            .find(|p| p.name.as_deref() == Some("otherpkg"))
            .expect("distinct otherpkg wheel should surface as its own package");

        assert!(files[0].for_packages.contains(&construct.package_uid));
        assert!(files[1].for_packages.contains(&construct.package_uid));
        assert!(files[2].for_packages.contains(&otherpkg.package_uid));
    }

    #[test]
    fn test_assemble_deno_json_with_deno_lock() {
        let mut files = vec![
            create_test_file_info(
                "project/deno.json",
                DatasourceId::DenoJson,
                Some("pkg:generic/%40provenant/deno-sample@1.0.0"),
                Some("@provenant/deno-sample"),
                Some("1.0.0"),
                vec![Dependency {
                    purl: Some("pkg:npm/chalk".to_string()),
                    extracted_requirement: Some("npm:chalk@5".to_string()),
                    scope: Some("imports".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(false),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
            create_test_file_info(
                "project/deno.lock",
                DatasourceId::DenoLock,
                None,
                None,
                None,
                vec![Dependency {
                    purl: Some("pkg:npm/chalk@5.6.2".to_string()),
                    extracted_requirement: Some("npm:chalk@5".to_string()),
                    scope: Some("imports".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(true),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            1,
            "Expected exactly one merged Deno package"
        );
        let package = &result.packages[0];
        assert_eq!(package.name, Some("@provenant/deno-sample".to_string()));
        assert!(
            package
                .datafile_paths
                .contains(&"project/deno.json".to_string())
        );
        assert!(
            package
                .datafile_paths
                .contains(&"project/deno.lock".to_string())
        );
        assert!(package.datasource_ids.contains(&DatasourceId::DenoJson));
        assert!(package.datasource_ids.contains(&DatasourceId::DenoLock));

        assert_eq!(result.dependencies.len(), 2);
        assert_eq!(files[0].for_packages.len(), 1);
        assert_eq!(files[1].for_packages.len(), 1);
    }

    #[test]
    fn test_assemble_go_mod_with_go_work() {
        let mut files = vec![
            create_test_file_info(
                "project/go.mod",
                DatasourceId::GoMod,
                Some("pkg:golang/example.com/project"),
                Some("project"),
                None,
                vec![],
            ),
            create_test_file_info(
                "project/go.work",
                DatasourceId::GoWork,
                None,
                None,
                None,
                vec![Dependency {
                    purl: Some("pkg:golang/example.com/mymodule".to_string()),
                    extracted_requirement: Some("./mymodule".to_string()),
                    scope: Some("use".to_string()),
                    is_runtime: Some(true),
                    is_optional: Some(false),
                    is_pinned: Some(false),
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                }],
            ),
        ];

        files[1].package_data[0].extra_data = Some(std::collections::HashMap::from([(
            "use_paths".to_string(),
            serde_json::json!(["./mymodule"]),
        )]));

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            1,
            "Expected exactly one merged Go package"
        );
        let package = &result.packages[0];
        assert_eq!(package.name, Some("project".to_string()));
        assert!(
            package
                .datafile_paths
                .contains(&"project/go.mod".to_string())
        );
        assert!(
            package
                .datafile_paths
                .contains(&"project/go.work".to_string())
        );
        assert!(package.datasource_ids.contains(&DatasourceId::GoMod));
        assert!(package.datasource_ids.contains(&DatasourceId::GoWork));
        let extra_data = package
            .extra_data
            .as_ref()
            .expect("merged extra_data missing");
        assert!(extra_data.contains_key("use_paths"));
        assert_eq!(result.dependencies.len(), 1);
    }

    #[test]
    fn test_assemble_no_matching_datasource() {
        let mut files = vec![create_test_file_info(
            "project/unknown.json",
            DatasourceId::Readme,
            Some("pkg:unknown/pkg@1.0.0"),
            Some("pkg"),
            Some("1.0.0"),
            vec![],
        )];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            0,
            "Expected no packages for unknown datasource"
        );
        assert_eq!(
            result.dependencies.len(),
            0,
            "Expected no dependencies for unknown datasource"
        );
    }

    #[test]
    fn test_assemble_single_file_no_sibling() {
        let dep = Dependency {
            purl: Some("pkg:npm/lodash@4.17.21".to_string()),
            extracted_requirement: Some("^4.17.0".to_string()),
            scope: Some("dependencies".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(false),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        };

        let mut files = vec![create_test_file_info(
            "project/package.json",
            DatasourceId::NpmPackageJson,
            Some("pkg:npm/solo-app@2.0.0"),
            Some("solo-app"),
            Some("2.0.0"),
            vec![dep],
        )];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            1,
            "Expected one package even without lockfile"
        );
        let package = &result.packages[0];
        assert_eq!(package.name, Some("solo-app".to_string()));
        assert_eq!(
            package.datafile_paths.len(),
            1,
            "Expected only one file in datafile_paths"
        );
        assert_eq!(package.datafile_paths[0], "project/package.json");
        assert_eq!(
            package.datasource_ids.len(),
            1,
            "Expected only one datasource ID"
        );

        assert_eq!(result.dependencies.len(), 1, "Expected one dependency");
    }

    #[test]
    fn test_assemble_no_purl_no_package() {
        let mut files = vec![create_test_file_info(
            "project/package.json",
            DatasourceId::NpmPackageJson,
            None,
            Some("no-purl-app"),
            None,
            vec![],
        )];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            0,
            "Expected no packages when PackageData has no purl"
        );
    }

    #[test]
    fn test_assemble_npm_lockfile_does_not_create_package_when_manifest_has_no_purl() {
        let dep = Dependency {
            purl: Some("pkg:npm/express@4.18.0".to_string()),
            extracted_requirement: Some("4.18.0".to_string()),
            scope: Some("dependencies".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(true),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        };

        let mut files = vec![
            create_test_file_info(
                "project/package.json",
                DatasourceId::NpmPackageJson,
                None,
                None,
                None,
                vec![],
            ),
            create_test_file_info(
                "project/package-lock.json",
                DatasourceId::NpmPackageLockJson,
                Some("pkg:npm/lock-only@1.0.0"),
                Some("lock-only"),
                Some("1.0.0"),
                vec![dep],
            ),
        ];

        let result = assemble(&mut files);

        assert!(result.packages.is_empty());
        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(result.dependencies[0].for_package_uid, None);
        assert!(files[0].for_packages.is_empty());
        assert!(files[1].for_packages.is_empty());
    }

    #[test]
    fn test_build_package_uid_format() {
        use crate::models::PackageUid;

        let purl = "pkg:npm/test@1.0.0";
        let uid = PackageUid::new(purl);

        assert!(
            uid.as_str().starts_with("pkg:npm/test@1.0.0?uuid="),
            "Expected UUID to be added as qualifier"
        );
        assert!(uid.as_str().contains("uuid="), "Expected uuid qualifier");

        let purl_with_qualifier = "pkg:npm/test@1.0.0?arch=x64";
        let uid2 = PackageUid::new(purl_with_qualifier);

        assert!(
            uid2.as_str().contains("&uuid="),
            "Expected UUID to be appended with & when qualifiers exist"
        );
        assert!(
            uid2.as_str()
                .starts_with("pkg:npm/test@1.0.0?arch=x64&uuid=")
        );
    }

    #[test]
    fn test_package_update_merges_fields() {
        let initial_pkg_data = PackageData {
            datasource_id: Some(DatasourceId::NpmPackageJson),
            purl: Some("pkg:npm/test@1.0.0".to_string()),
            name: Some("test".to_string()),
            version: Some("1.0.0".to_string()),
            description: Some("Initial description".to_string()),
            ..Default::default()
        };

        let mut package = Package::from_package_data(&initial_pkg_data, "file1.json".to_string());

        let update_pkg_data = PackageData {
            datasource_id: Some(DatasourceId::NpmPackageLockJson),
            purl: Some("pkg:npm/test@1.0.0".to_string()),
            name: Some("test".to_string()),
            version: Some("1.0.0".to_string()),
            homepage_url: Some("https://example.com".to_string()),
            sha256: Some(
                Sha256Digest::from_hex(
                    "abc1230000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
            ),
            ..Default::default()
        };

        package.update(&update_pkg_data, "file2.json".to_string());

        assert_eq!(package.datafile_paths.len(), 2);
        assert_eq!(package.datasource_ids.len(), 2);
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::NpmPackageLockJson)
        );
        assert!(
            package
                .datasource_ids
                .contains(&DatasourceId::NpmPackageJson)
        );
        assert_eq!(
            package.description,
            Some("Initial description".to_string()),
            "Original description should be preserved"
        );
        assert_eq!(
            package.homepage_url,
            Some("https://example.com".to_string()),
            "New homepage should be filled"
        );
        assert_eq!(
            package.sha256,
            Some(
                Sha256Digest::from_hex(
                    "abc1230000000000000000000000000000000000000000000000000000000000"
                )
                .unwrap()
            ),
            "New sha256 should be filled"
        );
        assert_eq!(
            package.homepage_url,
            Some("https://example.com".to_string()),
            "New homepage should be filled"
        );
        assert_eq!(
            package.sha256,
            Some(
                Sha256Digest::from_hex(
                    "abc1230000000000000000000000000000000000000000000000000000000000"
                )
                .unwrap()
            ),
            "New sha256 should be filled"
        );
    }

    #[test]
    fn test_package_update_refreshes_purl_when_version_is_backfilled() {
        let initial_pkg_data = PackageData {
            datasource_id: Some(DatasourceId::PypiPyprojectToml),
            purl: Some("pkg:pypi/test-package".to_string()),
            name: Some("test-package".to_string()),
            version: None,
            ..Default::default()
        };

        let mut package =
            Package::from_package_data(&initial_pkg_data, "pyproject.toml".to_string());
        let original_uid = package.package_uid.clone();

        let update_pkg_data = PackageData {
            datasource_id: Some(DatasourceId::PypiUvLock),
            purl: Some("pkg:pypi/test-package@0.2.0".to_string()),
            name: Some("test-package".to_string()),
            version: Some("0.2.0".to_string()),
            ..Default::default()
        };

        package.update(&update_pkg_data, "uv.lock".to_string());

        assert_eq!(package.purl.as_deref(), Some("pkg:pypi/test-package@0.2.0"));
        assert_ne!(package.package_uid, original_uid);
        assert!(
            package
                .package_uid
                .starts_with("pkg:pypi/test-package@0.2.0?uuid=")
        );
    }

    #[test]
    fn test_matches_pattern_exact() {
        use crate::assembly::sibling_merge::matches_pattern;

        assert!(matches_pattern("package.json", "package.json"));
        assert!(!matches_pattern("package-lock.json", "package.json"));
    }

    #[test]
    fn test_matches_pattern_case_insensitive() {
        use crate::assembly::sibling_merge::matches_pattern;

        assert!(matches_pattern("Cargo.toml", "cargo.toml"));
        assert!(matches_pattern("cargo.toml", "Cargo.toml"));
        assert!(matches_pattern("CARGO.TOML", "cargo.toml"));
    }

    #[test]
    fn test_matches_pattern_glob() {
        use crate::assembly::sibling_merge::matches_pattern;

        assert!(matches_pattern("MyLib.podspec", "*.podspec"));
        assert!(matches_pattern("test.podspec", "*.podspec"));
        assert!(!matches_pattern("podspec", "*.podspec"));
        assert!(!matches_pattern("test.txt", "*.podspec"));

        assert!(matches_pattern("MyLib.podspec.json", "*.podspec.json"));
        assert!(!matches_pattern("MyLib.podspec", "*.podspec.json"));
    }

    #[test]
    fn test_assemble_one_per_package_data_mode() {
        let dep = Dependency {
            purl: Some("pkg:alpine/scanelf".to_string()),
            extracted_requirement: None,
            scope: Some("install".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(false),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        };

        let path = "rootfs/lib/apk/db/installed";
        let file_name = "installed";
        let extension = "";

        let mut files = vec![FileInfo {
            name: file_name.to_string(),
            base_name: file_name.to_string(),
            extension: extension.to_string(),
            path: path.to_string(),
            file_type: FileType::File,
            mime_type: Some("text/plain".to_string()),
            file_type_label: None,
            size: 5000,
            date: None,
            sha1: None,
            md5: None,
            sha256: None,
            programming_language: None,
            package_data: vec![
                PackageData {
                    datasource_id: Some(DatasourceId::AlpineInstalledDb),
                    purl: Some("pkg:alpine/musl@1.2.3-r0".to_string()),
                    name: Some("musl".to_string()),
                    version: Some("1.2.3-r0".to_string()),
                    dependencies: vec![dep],
                    ..Default::default()
                },
                PackageData {
                    datasource_id: Some(DatasourceId::AlpineInstalledDb),
                    purl: Some("pkg:alpine/busybox@1.35.0-r13".to_string()),
                    name: Some("busybox".to_string()),
                    version: Some("1.35.0-r13".to_string()),
                    dependencies: vec![],
                    ..Default::default()
                },
            ],
            detected_license_expression: None,
            detected_license_expression_spdx: None,
            license_detections: vec![],
            license_clues: vec![],
            percentage_of_license_text: None,
            copyrights: vec![],
            holders: vec![],
            authors: vec![],
            emails: vec![],
            urls: vec![],
            for_packages: vec![],
            scan_diagnostics: vec![],
            license_policy: None,
            is_source: None,
            files_count: None,
            dirs_count: None,
            size_count: None,
            source_count: None,
            is_legal: false,
            is_manifest: false,
            is_readme: false,
            is_top_level: false,
            is_key_file: false,
            is_referenced: false,
            is_community: false,
            is_generated: None,
            sha1_git: None,
            is_binary: None,
            is_text: None,
            is_archive: None,
            is_media: None,
            is_script: None,
            facets: vec![],
            tallies: None,
        }];

        let result = assemble(&mut files);

        assert_eq!(
            result.packages.len(),
            2,
            "Expected two independent packages from one database file"
        );

        let musl = result
            .packages
            .iter()
            .find(|p| p.name == Some("musl".to_string()));
        let busybox = result
            .packages
            .iter()
            .find(|p| p.name == Some("busybox".to_string()));

        assert!(musl.is_some(), "Expected musl package");
        assert!(busybox.is_some(), "Expected busybox package");

        let musl = musl.unwrap();
        assert_eq!(musl.version, Some("1.2.3-r0".to_string()));
        assert_eq!(musl.datafile_paths, vec![path.to_string()]);
        assert!(musl.package_uid.contains("uuid="));

        assert_eq!(result.dependencies.len(), 1);
        assert_eq!(
            result.dependencies[0].purl,
            Some("pkg:alpine/scanelf".to_string())
        );

        assert_eq!(
            files[0].for_packages.len(),
            2,
            "Expected database file to reference both packages"
        );
    }

    /// Builds a fingerprint of an assembly run that is independent of the
    /// random UUID suffix baked into every `PackageUid`, but still sensitive to
    /// assembler-order-dependent decisions such as which package claims a file.
    ///
    /// Each `PackageUid` is resolved to its owning package's stable identity
    /// (purl + name + version + datafile_paths + datasource_ids) so that two runs
    /// producing the same logical assignment compare equal, while a run that
    /// assigns a file to a different package (the symptom of nondeterministic
    /// assembler order) compares unequal.
    fn assembly_fingerprint(files: &mut [FileInfo]) -> String {
        use std::collections::HashMap;

        let result = assemble(files);

        let mut uid_to_identity: HashMap<String, String> = HashMap::new();
        for package in &result.packages {
            let identity = format!(
                "purl={:?}|name={:?}|version={:?}|datafiles={:?}|datasources={:?}",
                package.purl,
                package.name,
                package.version,
                package.datafile_paths,
                package
                    .datasource_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            );
            uid_to_identity.insert(package.package_uid.to_string(), identity);
        }

        let resolve = |uid: &PackageUid| -> String {
            uid_to_identity
                .get(uid.as_ref())
                .cloned()
                .unwrap_or_else(|| format!("<unknown:{uid}>"))
        };

        let mut fingerprint = String::new();

        let mut package_lines: Vec<String> = uid_to_identity.values().cloned().collect();
        package_lines.sort();
        fingerprint.push_str("PACKAGES\n");
        for line in package_lines {
            fingerprint.push_str(&line);
            fingerprint.push('\n');
        }

        fingerprint.push_str("DEPENDENCIES\n");
        let mut dep_lines: Vec<String> = result
            .dependencies
            .iter()
            .map(|dep| {
                format!(
                    "purl={:?}|req={:?}|scope={:?}|datafile={}|datasource={}|for={}",
                    dep.purl,
                    dep.extracted_requirement,
                    dep.scope,
                    dep.datafile_path,
                    dep.datasource_id,
                    dep.for_package_uid
                        .as_ref()
                        .map(&resolve)
                        .unwrap_or_else(|| "<none>".to_string()),
                )
            })
            .collect();
        dep_lines.sort();
        for line in dep_lines {
            fingerprint.push_str(&line);
            fingerprint.push('\n');
        }

        fingerprint.push_str("FILE_OWNERSHIP\n");
        let mut file_lines: Vec<String> = files
            .iter()
            .map(|file| {
                let mut owners: Vec<String> = file.for_packages.iter().map(&resolve).collect();
                owners.sort();
                format!("{} -> {:?}", file.path, owners)
            })
            .collect();
        file_lines.sort();
        for line in file_lines {
            fingerprint.push_str(&line);
            fingerprint.push('\n');
        }

        fingerprint
    }

    /// Regression guard for nondeterministic per-directory assembler order
    /// (issue #1026, bug 1). A polyglot directory contains manifests from
    /// multiple ecosystems, so the set of active assembler configs has more than
    /// one member. Iterating that set in `HashSet` order made file ownership
    /// nondeterministic run-to-run; this asserts the resolved assignment is
    /// stable across many repeated runs.
    #[test]
    fn test_assemble_polyglot_directory_is_deterministic() {
        let build_files = || {
            let mut npm = create_test_file_info(
                "repo/package.json",
                DatasourceId::NpmPackageJson,
                Some("pkg:npm/poly@1.0.0"),
                Some("poly"),
                Some("1.0.0"),
                vec![create_test_dependency(
                    "pkg:npm/left-pad",
                    Some("^1.0.0"),
                    None,
                )],
            );
            npm.package_data[0].package_type = Some(PackageType::Npm);

            let mut cargo = create_test_file_info(
                "repo/Cargo.toml",
                DatasourceId::CargoToml,
                Some("pkg:cargo/poly-rs@2.0.0"),
                Some("poly-rs"),
                Some("2.0.0"),
                vec![create_test_dependency("pkg:cargo/serde", Some("1"), None)],
            );
            cargo.package_data[0].package_type = Some(PackageType::Cargo);

            let mut composer = create_test_file_info(
                "repo/composer.json",
                DatasourceId::PhpComposerJson,
                Some("pkg:composer/poly-php@3.0.0"),
                Some("poly-php"),
                Some("3.0.0"),
                vec![],
            );
            composer.package_data[0].package_type = Some(PackageType::Composer);

            vec![npm, cargo, composer]
        };

        // The active config keys must be visited in a stable, sorted order. A
        // `HashSet` here would order keys by hash seeding, breaking this.
        let files = build_files();
        let file_indices: Vec<usize> = (0..files.len()).collect();
        let keys = super::super::active_config_keys(
            &files,
            &file_indices,
            &super::super::ASSEMBLER_LOOKUP,
        );
        let collected: Vec<DatasourceId> = keys.iter().copied().collect();
        let mut sorted = collected.clone();
        sorted.sort();
        assert!(
            collected.len() >= 2,
            "polyglot fixture should activate multiple assemblers, got {collected:?}"
        );
        assert_eq!(
            collected, sorted,
            "active assembler config keys must be visited in deterministic sorted order"
        );

        let mut baseline_files = build_files();
        let baseline = assembly_fingerprint(&mut baseline_files);

        for run in 0..50 {
            let mut files = build_files();
            let fingerprint = assembly_fingerprint(&mut files);
            assert_eq!(
                fingerprint, baseline,
                "assembly output diverged on run {run}; per-directory assembler order is not deterministic"
            );
        }
    }
}
