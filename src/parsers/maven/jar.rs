// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Bounded introspection of JVM archives (`.jar`, `.war`, `.aar`).
//!
//! Reads `META-INF/MANIFEST.MF` and `META-INF/maven/<groupId>/<artifactId>/pom.properties`
//! from the archive using the shared bounded-ZIP reader and reuses the Maven
//! MANIFEST.MF interpreter to recover name/version/vendor. Maven `pom.properties`
//! coordinates take precedence for namespace/version when present, since they are
//! the authoritative build coordinates. The archive's own datasource id and package
//! type are preserved so per-format assembly classification stays intact.
//!
//! Parsing is static and bounded: nothing is extracted to disk and no archive
//! content is executed.

use std::path::Path;

use super::coordinates::build_maven_purl;
use super::manifest::interpret_manifest_mf;
use super::properties::interpret_pom_properties;
use crate::models::{DatasourceId, PackageData, PackageType};
use crate::parser_warn as warn;
use crate::parsers::archive::{
    ValidatedZipEntry, find_entries_by_suffix, find_entry_by_name, open_bounded_zip,
    read_entry_to_string,
};
use crate::parsers::utils::truncate_field;

/// Which JVM archive flavor is being introspected. Each maps to a distinct
/// datasource id and default package type so downstream assembly stays
/// format-aware.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JvmArchiveKind {
    Jar,
    War,
    Aar,
}

impl JvmArchiveKind {
    fn datasource_id(self) -> DatasourceId {
        match self {
            Self::Jar => DatasourceId::JavaJar,
            Self::War => DatasourceId::JavaWarArchive,
            Self::Aar => DatasourceId::AndroidAarLibrary,
        }
    }

    fn package_type(self) -> PackageType {
        match self {
            Self::Jar => PackageType::Jar,
            Self::War => PackageType::War,
            Self::Aar => PackageType::AndroidLib,
        }
    }
}

fn bare_recognizer_row(kind: JvmArchiveKind) -> PackageData {
    PackageData {
        package_type: Some(kind.package_type()),
        datasource_id: Some(kind.datasource_id()),
        ..Default::default()
    }
}

/// Introspect a JVM archive into a single `PackageData`.
///
/// Falls back to the bare recognizer row (package type + datasource id only) when
/// the archive cannot be opened safely or carries no recoverable metadata, so the
/// `datasource_id` is always set.
pub fn extract_jvm_archive(path: &Path, kind: JvmArchiveKind) -> Vec<PackageData> {
    let Some((mut archive, entries)) = open_bounded_zip(path) else {
        return vec![bare_recognizer_row(kind)];
    };

    let manifest = read_named_entry(&mut archive, &entries, "META-INF/MANIFEST.MF", path);
    let pom_properties = read_pom_properties(&mut archive, &entries, path);

    if manifest.is_none() && pom_properties.is_none() {
        return vec![bare_recognizer_row(kind)];
    }

    let mut package_data = match &manifest {
        Some(content) => interpret_manifest_mf(content, path),
        None => bare_recognizer_row(kind),
    };

    apply_pom_properties(&mut package_data, pom_properties.as_deref());

    // Preserve the archive's own format identity for assembly classification,
    // regardless of what the embedded manifest claimed.
    package_data.datasource_id = Some(kind.datasource_id());
    if package_data.primary_language.is_none() {
        package_data.primary_language = Some("Java".to_string());
    }

    // The MANIFEST.MF interpreter sets `package_type = Maven` once it recovers
    // coordinates; honor that for archives carrying real Maven identity, but fall
    // back to the archive-specific type when nothing identified the package.
    let unidentified = package_data.name.is_none() && package_data.version.is_none();
    if unidentified || package_data.package_type.is_none() {
        package_data.package_type = Some(kind.package_type());
    }

    vec![package_data]
}

fn read_named_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    entries: &[ValidatedZipEntry],
    name: &str,
    path: &Path,
) -> Option<String> {
    let entry = find_entry_by_name(entries, name)?;
    match read_entry_to_string(archive, entry, path) {
        Ok(content) => Some(content),
        Err(e) => {
            warn!("Failed to read {} from {:?}: {}", name, path, e);
            None
        }
    }
}

/// Read the most specific `pom.properties` from `META-INF/maven/**`.
///
/// A fat/shaded archive can embed several; the shallowest path is the archive's
/// own coordinates rather than a bundled dependency, so prefer it.
fn read_pom_properties(
    archive: &mut zip::ZipArchive<std::fs::File>,
    entries: &[ValidatedZipEntry],
    path: &Path,
) -> Option<String> {
    let mut candidates: Vec<&ValidatedZipEntry> =
        find_entries_by_suffix(entries, "/pom.properties")
            .into_iter()
            .filter(|entry| entry.name().starts_with("META-INF/maven/"))
            .collect();

    candidates.sort_by(|a, b| {
        let a_depth = a.name().matches('/').count();
        let b_depth = b.name().matches('/').count();
        a_depth.cmp(&b_depth).then_with(|| a.name().cmp(b.name()))
    });

    let entry = candidates.first()?;
    match read_entry_to_string(archive, entry, path) {
        Ok(content) => Some(content),
        Err(e) => {
            warn!("Failed to read {} from {:?}: {}", entry.name(), path, e);
            None
        }
    }
}

fn apply_pom_properties(package_data: &mut PackageData, pom_properties: Option<&str>) {
    let Some(content) = pom_properties else {
        return;
    };

    let coords = interpret_pom_properties(content);
    if let Some(group_id) = coords.group_id.as_ref() {
        package_data.namespace = Some(truncate_field(group_id.clone()));
    }
    if let Some(artifact_id) = coords.artifact_id.as_ref() {
        package_data.name = Some(truncate_field(artifact_id.clone()));
    }
    if let Some(version) = coords.version.as_ref() {
        package_data.version = Some(truncate_field(version.clone()));
    }

    // Build the Maven purl only when pom.properties itself carries all three
    // coordinates, matching the standalone pom.properties parser contract. This
    // avoids folding a MANIFEST.MF version into pom.properties group/artifact (or
    // vice versa) and emitting a purl the standalone parser never would.
    if let (Some(group_id), Some(artifact_id), Some(version)) =
        (coords.group_id, coords.artifact_id, coords.version)
    {
        package_data.package_type = Some(PackageType::Maven);
        package_data.purl = Some(truncate_field(build_maven_purl(
            &group_id,
            &artifact_id,
            Some(&version),
            None,
            None,
        )));
    }
}
