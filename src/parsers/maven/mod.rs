// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Parser for Apache Maven pom.xml files.
//!
//! Extracts package metadata, dependencies, and license information from
//! Maven Project Object Model (POM) files.
//!
//! # Supported Formats
//! - pom.xml (Project Object Model)
//! - pom.properties
//! - MANIFEST.MF (JAR manifest)
//!
//! # Key Features
//! - Property value substitution (`${project.version}`)
//! - `is_pinned` analysis (exact version vs ranges like `[1.0,2.0)`)
//! - Dependency scope handling (compile, test, provided, runtime, system)
//! - Package URL (purl) generation
//! - Multiple license support (combined with " OR ")

mod coordinates;
mod jar;
mod manifest;
mod pom;
mod properties;

pub use self::jar::{JvmArchiveKind, extract_jvm_archive};

#[cfg(test)]
mod jar_scan_test;
#[cfg(test)]
mod jar_test;
#[cfg(test)]
mod manifest_test;
#[cfg(test)]
mod pom_test;
#[cfg(test)]
mod properties_test;
#[cfg(test)]
mod scan_test;

use self::{manifest::parse_manifest_mf, pom::parse_pom_xml, properties::parse_pom_properties};
use super::PackageParser;
use super::metadata::ParserMetadata;
use crate::models::{DatasourceId, PackageData, PackageType};
use std::path::Path;

/// Maven package parser supporting pom.xml, pom.properties, and MANIFEST.MF files.
pub struct MavenParser;

impl PackageParser for MavenParser {
    const PACKAGE_TYPE: PackageType = PackageType::Maven;

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "Apache Maven POM",
            file_patterns: &[
                "**/*.pom",
                "**/pom.xml",
                "**/pom.properties",
                "**/META-INF/MANIFEST.MF",
            ],
            package_type: "maven",
            primary_language: "Java",
            documentation_url: Some("https://maven.apache.org/pom.html"),
        }]
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
            if filename == "pom.properties" {
                return vec![parse_pom_properties(path)];
            }
            if filename == "MANIFEST.MF" {
                return vec![parse_manifest_mf(path)];
            }
        }

        parse_pom_xml(path)
    }

    fn is_match(path: &Path) -> bool {
        if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
            filename == "pom.xml"
                || filename.ends_with(".pom.xml")
                || filename.ends_with("-pom.xml")
                || filename.ends_with("_pom.xml")
                || filename == "pom.properties"
                || filename == "MANIFEST.MF"
                || filename.ends_with(".pom")
        } else {
            false
        }
    }
}

fn default_package_data(datasource_id: DatasourceId) -> PackageData {
    PackageData {
        package_type: Some(PackageType::Maven),
        datasource_id: Some(datasource_id),
        ..Default::default()
    }
}
