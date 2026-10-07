// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::Path;

use crate::models::{DatasourceId, FileType};

fn test_file(path: &str, package_data: Vec<PackageData>) -> FileInfo {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let base_name = Path::new(&file_name)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let extension = Path::new(&file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_string();

    FileInfo::new(
        file_name,
        base_name,
        extension,
        path.to_string(),
        FileType::File,
        Some("text/plain".to_string()),
        None,
        0,
        None,
        None,
        None,
        None,
        None,
        package_data,
        None,
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
}

#[test]
fn test_has_nested_patterns() {
    let config_nested = AssemblerConfig {
        datasource_ids: &[DatasourceId::MavenPom],
        sibling_file_patterns: &["pom.xml", "**/META-INF/MANIFEST.MF"],
        mode: crate::assembly::AssemblyMode::SiblingMerge,
        directory_merger: None,
    };
    assert!(has_nested_patterns(&config_nested));

    let config_simple = AssemblerConfig {
        datasource_ids: &[DatasourceId::NpmPackageJson],
        sibling_file_patterns: &["package.json", "package-lock.json"],
        mode: crate::assembly::AssemblyMode::SiblingMerge,
        directory_merger: None,
    };
    assert!(!has_nested_patterns(&config_simple));
}

#[test]
fn test_matches_nested_pattern() {
    assert!(matches_nested_pattern(
        "my-lib/META-INF/MANIFEST.MF",
        "**/META-INF/MANIFEST.MF"
    ));
    assert!(matches_nested_pattern(
        "path/to/jar/META-INF/MANIFEST.MF",
        "**/META-INF/MANIFEST.MF"
    ));
    assert!(!matches_nested_pattern(
        "path/to/jar/pom.xml",
        "**/META-INF/MANIFEST.MF"
    ));
}

#[test]
fn test_matches_simple_pattern() {
    assert!(matches_simple_pattern("pom.xml", "pom.xml"));
    assert!(matches_simple_pattern("Cargo.toml", "cargo.toml"));
    assert!(matches_simple_pattern("MyLib.podspec", "*.podspec"));
    assert!(!matches_simple_pattern("package.json", "pom.xml"));
}

#[test]
fn test_find_package_root() {
    use crate::models::FileType;

    let files = vec![
        FileInfo::new(
            "pom.xml".to_string(),
            "pom".to_string(),
            "xml".to_string(),
            "my-lib/pom.xml".to_string(),
            FileType::File,
            Some("application/xml".to_string()),
            None,
            100,
            None,
            None,
            None,
            None,
            None,
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        FileInfo::new(
            "MANIFEST.MF".to_string(),
            "MANIFEST".to_string(),
            "MF".to_string(),
            "my-lib/META-INF/MANIFEST.MF".to_string(),
            FileType::File,
            Some("text/plain".to_string()),
            None,
            50,
            None,
            None,
            None,
            None,
            None,
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        ),
    ];

    let root = find_package_root(&[0, 1], &files);
    assert_eq!(root, Some(PathBuf::from("my-lib")));
}

#[test]
fn test_find_package_root_debian() {
    use crate::models::FileType;

    let files = vec![
        FileInfo::new(
            "control".to_string(),
            "control".to_string(),
            "".to_string(),
            "my-pkg/debian/control".to_string(),
            FileType::File,
            Some("text/plain".to_string()),
            None,
            200,
            None,
            None,
            None,
            None,
            None,
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        FileInfo::new(
            "copyright".to_string(),
            "copyright".to_string(),
            "".to_string(),
            "my-pkg/debian/copyright".to_string(),
            FileType::File,
            Some("text/plain".to_string()),
            None,
            150,
            None,
            None,
            None,
            None,
            None,
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        ),
    ];

    let root = find_package_root(&[0, 1], &files);
    assert_eq!(root, Some(PathBuf::from("my-pkg")));
}

fn uberjar_config() -> AssemblerConfig {
    AssemblerConfig {
        datasource_ids: &[
            DatasourceId::MavenPom,
            DatasourceId::MavenPomProperties,
            DatasourceId::JavaJarManifest,
        ],
        sibling_file_patterns: &["pom.xml", "pom.properties", "**/META-INF/MANIFEST.MF"],
        mode: crate::assembly::AssemblyMode::SiblingMerge,
        directory_merger: None,
    }
}

fn maven_file(
    path: &str,
    datasource_id: DatasourceId,
    namespace: Option<&str>,
    name: &str,
    version: &str,
) -> FileInfo {
    test_file(
        path,
        vec![PackageData {
            datasource_id: Some(datasource_id),
            package_type: Some(crate::models::PackageType::Maven),
            primary_language: Some("Java".to_string()),
            purl: namespace.map(|ns| format!("pkg:maven/{ns}/{name}@{version}")),
            name: Some(name.to_string()),
            namespace: namespace.map(str::to_string),
            version: Some(version.to_string()),
            ..Default::default()
        }],
    )
}

fn uberjar_poms() -> Vec<FileInfo> {
    vec![
        maven_file(
            "uberjar/META-INF/maven/com.example/app-one/pom.xml",
            DatasourceId::MavenPom,
            Some("com.example"),
            "app-one",
            "1.0.0",
        ),
        maven_file(
            "uberjar/META-INF/maven/com.example/app-one/pom.properties",
            DatasourceId::MavenPomProperties,
            Some("com.example"),
            "app-one",
            "1.0.0",
        ),
        maven_file(
            "uberjar/META-INF/maven/org.shaded/app-two/pom.xml",
            DatasourceId::MavenPom,
            Some("org.shaded"),
            "app-two",
            "2.0.0",
        ),
    ]
}

#[test]
fn test_maven_nested_merge_multiple_poms_merges_manifest_into_matching_pom() {
    let mut files = uberjar_poms();
    files.push(maven_file(
        "uberjar/META-INF/MANIFEST.MF",
        DatasourceId::JavaJarManifest,
        Some("com.example"),
        "app-one",
        "1.0.0",
    ));

    let (package, _, mut affected) =
        assemble_nested_patterns(&files, &uberjar_config()).expect("manifest should merge");

    assert_eq!(
        package.purl.as_deref(),
        Some("pkg:maven/com.example/app-one@1.0.0")
    );
    affected.sort_unstable();
    assert_eq!(affected, vec![0, 1, 3]);
}

#[test]
fn test_maven_nested_merge_multiple_poms_matches_namespace_less_manifest() {
    let mut files = uberjar_poms();
    files.push(maven_file(
        "uberjar/META-INF/MANIFEST.MF",
        DatasourceId::JavaJarManifest,
        None,
        "app-two",
        "2.0.0",
    ));

    let (package, _, mut affected) =
        assemble_nested_patterns(&files, &uberjar_config()).expect("manifest should merge");

    assert_eq!(
        package.purl.as_deref(),
        Some("pkg:maven/org.shaded/app-two@2.0.0")
    );
    affected.sort_unstable();
    assert_eq!(affected, vec![2, 3]);
}

#[test]
fn test_maven_nested_merge_multiple_poms_skips_non_matching_manifest() {
    let mut files = uberjar_poms();
    files.push(maven_file(
        "uberjar/META-INF/MANIFEST.MF",
        DatasourceId::JavaJarManifest,
        Some("com.example"),
        "app-one",
        "9.9.9",
    ));

    assert!(assemble_nested_patterns(&files, &uberjar_config()).is_none());
}

#[test]
fn test_maven_nested_merge_multiple_poms_skips_ambiguous_manifest() {
    let mut files = uberjar_poms();
    files.push(maven_file(
        "uberjar/META-INF/maven/org.other/app-one/pom.xml",
        DatasourceId::MavenPom,
        Some("org.other"),
        "app-one",
        "1.0.0",
    ));
    files.push(maven_file(
        "uberjar/META-INF/MANIFEST.MF",
        DatasourceId::JavaJarManifest,
        None,
        "app-one",
        "1.0.0",
    ));

    assert!(assemble_nested_patterns(&files, &uberjar_config()).is_none());
}

#[test]
fn test_maven_nested_merge_multiple_poms_without_manifest_skips() {
    assert!(assemble_nested_patterns(&uberjar_poms(), &uberjar_config()).is_none());
}

#[test]
fn test_maven_nested_merge_skips_source_reactor_poms() {
    // A source multi-module reactor has many sibling `pom.xml` files in nested
    // directories, none under `META-INF/maven/`. The nested merge must not fold
    // these independent module packages into the root reactor POM; the
    // per-directory sibling merge already models each module as its own package.
    let config = AssemblerConfig {
        datasource_ids: &[
            DatasourceId::MavenPom,
            DatasourceId::MavenPomProperties,
            DatasourceId::JavaJarManifest,
        ],
        sibling_file_patterns: &["pom.xml", "pom.properties", "**/META-INF/MANIFEST.MF"],
        mode: crate::assembly::AssemblyMode::SiblingMerge,
        directory_merger: None,
    };

    let module = |path: &str, name: &str, version: &str| {
        test_file(
            path,
            vec![PackageData {
                datasource_id: Some(DatasourceId::MavenPom),
                package_type: Some(crate::models::PackageType::Maven),
                primary_language: Some("Java".to_string()),
                purl: Some(format!("pkg:maven/com.example/{name}@{version}")),
                name: Some(name.to_string()),
                namespace: Some("com.example".to_string()),
                version: Some(version.to_string()),
                ..Default::default()
            }],
        )
    };

    let files = vec![
        module("reactor/pom.xml", "reactor", "1.0.0"),
        module("reactor/module-a/pom.xml", "module-a", "1.0.0"),
        module("reactor/module-b/pom.xml", "module-b", "1.0.0"),
    ];

    let assembled = assemble_nested_patterns(&files, &config);

    assert!(assembled.is_none());
}
