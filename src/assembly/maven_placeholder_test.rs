// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn maven_data(
    datasource_id: DatasourceId,
    namespace: &str,
    name: &str,
    version: &str,
    purl: &str,
) -> PackageData {
    PackageData {
        datasource_id: Some(datasource_id),
        package_type: Some(PackageType::Maven),
        namespace: Some(namespace.to_string()),
        name: Some(name.to_string()),
        version: Some(version.to_string()),
        purl: Some(purl.to_string()),
        ..Default::default()
    }
}

fn placeholder_pom() -> PackageData {
    maven_data(
        DatasourceId::MavenPom,
        "com.example",
        "lib",
        "${revision}",
        "pkg:maven/com.example/lib@%24%7Brevision%7D",
    )
}

fn properties(namespace: &str, name: &str, version: &str) -> PackageData {
    maven_data(
        DatasourceId::MavenPomProperties,
        namespace,
        name,
        version,
        &format!("pkg:maven/{namespace}/{name}@{version}"),
    )
}

fn package_from(pkg_data: &PackageData) -> Package {
    Package::from_package_data(pkg_data, "pom.xml".to_string())
}

#[test]
fn adopts_concrete_version_from_pom_properties() {
    let mut package = package_from(&placeholder_pom());

    adopt(&mut package, &properties("com.example", "lib", "1.0.0"));

    assert_eq!(package.version.as_deref(), Some("1.0.0"));
    assert_eq!(
        package.purl.as_deref(),
        Some("pkg:maven/com.example/lib@1.0.0")
    );
    assert!(
        package
            .package_uid
            .starts_with("pkg:maven/com.example/lib@1.0.0?uuid=")
    );
}

#[test]
fn adopts_placeholder_group_and_artifact_and_keeps_qualifiers() {
    let pom = maven_data(
        DatasourceId::MavenPom,
        "${project.groupId}",
        "${artifactId}",
        "2.0",
        "pkg:maven/%24%7Bproject.groupId%7D/%24%7BartifactId%7D@2.0?type=bundle",
    );
    let mut package = package_from(&pom);

    adopt(&mut package, &properties("org.acme", "core", "2.0"));

    assert_eq!(package.namespace.as_deref(), Some("org.acme"));
    assert_eq!(package.name.as_deref(), Some("core"));
    assert_eq!(
        package.purl.as_deref(),
        Some("pkg:maven/org.acme/core@2.0?type=bundle")
    );
}

#[test]
fn keeps_package_when_concrete_coordinates_disagree() {
    let mut package = package_from(&placeholder_pom());
    let before = package.purl.clone();

    adopt(&mut package, &properties("com.example", "other", "1.0.0"));

    assert_eq!(package.version.as_deref(), Some("${revision}"));
    assert_eq!(package.purl, before);
}

#[test]
fn keeps_placeholder_when_no_source_is_concrete() {
    let mut package = package_from(&placeholder_pom());
    let before = package.purl.clone();

    adopt(
        &mut package,
        &properties("com.example", "lib", "${revision}"),
    );

    assert_eq!(package.version.as_deref(), Some("${revision}"));
    assert_eq!(package.purl, before);
}

#[test]
fn ignores_non_pom_properties_sources() {
    let mut package = package_from(&placeholder_pom());
    let mut manifest = properties("com.example", "lib", "1.0.0");
    manifest.datasource_id = Some(DatasourceId::JavaJarManifest);

    adopt(&mut package, &manifest);

    assert_eq!(package.version.as_deref(), Some("${revision}"));
}

fn adopt(package: &mut Package, pkg_data: &PackageData) {
    adopt_resolved_maven_coordinates(package, pkg_data, "pom.properties");
}

#[test]
fn ignores_pom_properties_outside_the_pom_directory() {
    let mut package = package_from(&placeholder_pom());

    adopt_resolved_maven_coordinates(
        &mut package,
        &properties("com.example", "lib", "1.0.0"),
        "META-INF/maven/org.shaded/lib/pom.properties",
    );

    assert_eq!(package.version.as_deref(), Some("${revision}"));
}

#[test]
fn resolving_properties_requires_a_single_agreeing_source() {
    let pom = placeholder_pom();
    let one = properties("com.example", "lib", "1.0.0");
    let two = properties("com.example", "lib", "2.0.0");
    let unrelated = properties("com.example", "other", "3.0.0");

    assert_eq!(
        resolving_properties(&pom, [&pom, &one, &unrelated]).and_then(|p| p.purl.as_deref()),
        Some("pkg:maven/com.example/lib@1.0.0")
    );
    assert!(resolving_properties(&pom, [&one, &two]).is_none());
    assert!(resolving_properties(&pom, [&unrelated]).is_none());
    assert!(resolving_properties(&one, [&one]).is_none());
}

#[test]
fn resolved_identity_purl_keeps_pom_qualifiers() {
    let one = properties("com.example", "lib", "1.0.0");
    let mut war_pom = placeholder_pom();
    war_pom.purl = Some("pkg:maven/com.example/lib@%24%7Brevision%7D?type=war".to_string());

    assert_eq!(
        resolved_identity_purl(&placeholder_pom(), &one).as_deref(),
        Some("pkg:maven/com.example/lib@1.0.0")
    );
    assert_eq!(
        resolved_identity_purl(&war_pom, &one).as_deref(),
        Some("pkg:maven/com.example/lib@1.0.0?type=war")
    );
}

#[test]
fn rebuilds_repository_links_and_sources_purl() {
    let mut pom = placeholder_pom();
    pom.repository_homepage_url =
        Some("https://repo1.maven.org/maven2/com/example/lib/${revision}/".to_string());
    pom.repository_download_url = Some(
        "https://repo1.maven.org/maven2/com/example/lib/${revision}/lib-${revision}.jar"
            .to_string(),
    );
    pom.api_data_url = Some(
        "https://repo1.maven.org/maven2/com/example/lib/${revision}/lib-${revision}.pom"
            .to_string(),
    );
    pom.source_packages = vec![
        "pkg:maven/com.example/lib@%24%7Brevision%7D?classifier=sources".to_string(),
        "pkg:maven/org.unrelated/other@1.0?classifier=sources".to_string(),
    ];
    let mut package = package_from(&pom);

    adopt(&mut package, &properties("com.example", "lib", "1.0.0"));

    assert_eq!(
        package.repository_homepage_url.as_deref(),
        Some("https://repo1.maven.org/maven2/com/example/lib/1.0.0/")
    );
    assert_eq!(
        package.repository_download_url.as_deref(),
        Some("https://repo1.maven.org/maven2/com/example/lib/1.0.0/lib-1.0.0.jar")
    );
    assert_eq!(
        package.api_data_url.as_deref(),
        Some("https://repo1.maven.org/maven2/com/example/lib/1.0.0/lib-1.0.0.pom")
    );
    assert_eq!(
        package.source_packages,
        vec![
            "pkg:maven/com.example/lib@1.0.0?classifier=sources".to_string(),
            "pkg:maven/org.unrelated/other@1.0?classifier=sources".to_string(),
        ]
    );
}

fn file(path: &str, package_data: PackageData) -> crate::models::FileInfo {
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    crate::models::FileInfo::new(
        file_name.clone(),
        file_name,
        String::new(),
        path.to_string(),
        crate::models::FileType::File,
        Some("text/plain".to_string()),
        None,
        0,
        None,
        None,
        None,
        None,
        None,
        vec![package_data],
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
fn per_identity_merge_keeps_artifacts_with_distinct_qualifiers_separate() {
    let config = crate::assembly::AssemblerConfig {
        datasource_ids: &[DatasourceId::MavenPom, DatasourceId::MavenPomProperties],
        sibling_file_patterns: &["pom.xml", "*.pom", "pom.properties"],
        mode: crate::assembly::AssemblyMode::SiblingMergePerIdentity,
        directory_merger: None,
    };
    let mut war_pom = placeholder_pom();
    war_pom.purl = Some("pkg:maven/com.example/lib@%24%7Brevision%7D?type=war".to_string());
    let files = vec![
        file(
            "dir/pom.properties",
            properties("com.example", "lib", "1.0.0"),
        ),
        file("dir/pom.xml", war_pom),
        file("dir/lib.pom", placeholder_pom()),
        file(
            "dir/other-2.0.pom",
            maven_data(
                DatasourceId::MavenPom,
                "org.other",
                "other",
                "2.0",
                "pkg:maven/org.other/other@2.0",
            ),
        ),
    ];

    let results = crate::assembly::sibling_merge::assemble_siblings_per_identity(
        &config,
        &files,
        &[0, 1, 2, 3],
    );

    let mut packages: Vec<(String, usize)> = results
        .iter()
        .filter_map(|(package, _, affected)| {
            Some((package.as_ref()?.purl.clone()?, affected.len()))
        })
        .collect();
    packages.sort();
    assert_eq!(
        packages,
        vec![
            ("pkg:maven/com.example/lib@1.0.0".to_string(), 2),
            ("pkg:maven/com.example/lib@1.0.0?type=war".to_string(), 1),
            ("pkg:maven/org.other/other@2.0".to_string(), 1),
        ]
    );
}
