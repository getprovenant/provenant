// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use super::jar::{JvmArchiveKind, extract_jvm_archive};
use crate::models::{DatasourceId, PackageType};

#[test]
fn test_jar_recovers_manifest_and_pom_properties() {
    let path = PathBuf::from("testdata/jvm-archive-golden/demo-lib-1.2.3.jar");
    let packages = extract_jvm_archive(&path, JvmArchiveKind::Jar);

    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.datasource_id, Some(DatasourceId::JavaJar));
    // pom.properties coordinates are authoritative.
    assert_eq!(pkg.namespace.as_deref(), Some("org.example"));
    assert_eq!(pkg.name.as_deref(), Some("demo-lib"));
    assert_eq!(pkg.version.as_deref(), Some("1.2.3"));
    assert_eq!(pkg.package_type, Some(PackageType::Maven));
    assert_eq!(
        pkg.purl.as_deref(),
        Some("pkg:maven/org.example/demo-lib@1.2.3")
    );
    // Vendor recovered from the MANIFEST.MF interpreter.
    assert!(
        pkg.parties
            .iter()
            .any(|party| party.name.as_deref() == Some("Example Org"))
    );
}

#[test]
fn test_osgi_jar_without_pom_properties_uses_manifest() {
    let path = PathBuf::from("testdata/jvm-archive-golden/example-bundle.jar");
    let packages = extract_jvm_archive(&path, JvmArchiveKind::Jar);

    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    // The archive datasource id is preserved even for OSGi bundles.
    assert_eq!(pkg.datasource_id, Some(DatasourceId::JavaJar));
    assert_eq!(pkg.name.as_deref(), Some("com.example.bundle"));
    assert_eq!(pkg.version.as_deref(), Some("2.0.0"));
    assert_eq!(
        pkg.purl.as_deref(),
        Some("pkg:osgi/com.example.bundle@2.0.0")
    );
}

#[test]
fn test_war_recovers_metadata() {
    let path = PathBuf::from("testdata/jvm-archive-golden/web-app-3.4.5.war");
    let packages = extract_jvm_archive(&path, JvmArchiveKind::War);

    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.datasource_id, Some(DatasourceId::JavaWarArchive));
    assert_eq!(pkg.namespace.as_deref(), Some("com.example.web"));
    assert_eq!(pkg.name.as_deref(), Some("web-app"));
    assert_eq!(pkg.version.as_deref(), Some("3.4.5"));
    assert_eq!(
        pkg.purl.as_deref(),
        Some("pkg:maven/com.example.web/web-app@3.4.5")
    );
}

#[test]
fn test_aar_recovers_pom_properties_only() {
    let path = PathBuf::from("testdata/jvm-archive-golden/ui-lib-0.9.0.aar");
    let packages = extract_jvm_archive(&path, JvmArchiveKind::Aar);

    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.datasource_id, Some(DatasourceId::AndroidAarLibrary));
    assert_eq!(pkg.namespace.as_deref(), Some("com.example.android"));
    assert_eq!(pkg.name.as_deref(), Some("ui-lib"));
    assert_eq!(pkg.version.as_deref(), Some("0.9.0"));
}

#[test]
fn test_pom_properties_without_version_does_not_emit_purl() {
    // pom.properties supplies group/artifact but no version; the MANIFEST.MF has a
    // version. We must not fold the manifest version into a pom-coordinate purl, to
    // stay consistent with the standalone pom.properties parser contract.
    let path = PathBuf::from("testdata/jvm-archive-golden/partial-no-version.jar");
    let packages = extract_jvm_archive(&path, JvmArchiveKind::Jar);

    let pkg = &packages[0];
    assert_eq!(pkg.datasource_id, Some(DatasourceId::JavaJar));
    assert_eq!(pkg.namespace.as_deref(), Some("org.partial"));
    assert_eq!(pkg.name.as_deref(), Some("partial-lib"));
    // Manifest version is retained on the package, but no purl is synthesized.
    assert_eq!(pkg.version.as_deref(), Some("9.9.9"));
    assert!(pkg.purl.is_none());
}

#[test]
fn test_missing_archive_falls_back_to_bare_row() {
    let path = PathBuf::from("testdata/jvm-archive-golden/does-not-exist.jar");
    let packages = extract_jvm_archive(&path, JvmArchiveKind::Jar);

    assert_eq!(packages.len(), 1);
    let pkg = &packages[0];
    assert_eq!(pkg.datasource_id, Some(DatasourceId::JavaJar));
    assert_eq!(pkg.package_type, Some(PackageType::Jar));
    assert!(pkg.name.is_none());
    assert!(pkg.version.is_none());
    assert!(pkg.purl.is_none());
}
