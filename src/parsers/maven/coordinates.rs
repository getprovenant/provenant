// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

const MAVEN_REPOSITORY_BASE_URL: &str = "https://repo1.maven.org/maven2";

pub(super) struct MavenPathCoordinates {
    pub(super) group_id: String,
    pub(super) artifact_id: String,
}

pub(super) fn infer_meta_inf_maven_coordinates(path: &Path) -> Option<MavenPathCoordinates> {
    let components: Vec<_> = path
        .components()
        .map(|component| component.as_os_str())
        .collect();

    for window in components.windows(4) {
        if window[0] == OsStr::new("META-INF") && window[1] == OsStr::new("maven") {
            let group_id = window[2].to_string_lossy();
            let artifact_id = window[3].to_string_lossy();

            if !group_id.is_empty() && !artifact_id.is_empty() {
                return Some(MavenPathCoordinates {
                    group_id: group_id.into_owned(),
                    artifact_id: artifact_id.into_owned(),
                });
            }
        }
    }

    None
}

fn normalize_maven_packaging(packaging: Option<&str>) -> Option<&str> {
    match packaging.map(str::trim).filter(|value| !value.is_empty()) {
        Some(
            "ejb3" | "ear" | "aar" | "apk" | "gem" | "jar" | "nar" | "pom" | "so" | "swc" | "tar"
            | "tar.gz" | "war" | "xar" | "zip",
        ) => packaging.map(str::trim),
        Some(_) => Some("jar"),
        None => None,
    }
}

pub(super) fn build_maven_qualifiers(
    classifier: Option<&str>,
    packaging: Option<&str>,
) -> Option<HashMap<String, String>> {
    let mut qualifiers = HashMap::new();

    if let Some(classifier) = classifier.filter(|value| !value.trim().is_empty()) {
        qualifiers.insert("classifier".to_string(), classifier.to_string());
    }

    if let Some(packaging) = normalize_maven_packaging(packaging)
        .filter(|value| !value.is_empty() && *value != "jar" && *value != "pom")
    {
        qualifiers.insert("type".to_string(), packaging.to_string());
    }

    (!qualifiers.is_empty()).then_some(qualifiers)
}

pub(super) fn build_maven_purl(
    group_id: &str,
    artifact_id: &str,
    version: Option<&str>,
    classifier: Option<&str>,
    packaging: Option<&str>,
) -> String {
    let mut purl = format!(
        "pkg:maven/{}/{}",
        percent_encode_purl_component(group_id),
        percent_encode_purl_component(artifact_id)
    );

    if let Some(version) = version.filter(|value| !value.trim().is_empty()) {
        purl.push('@');
        purl.push_str(&percent_encode_purl_component(version));
    }

    if let Some(qualifiers) = build_maven_qualifiers(classifier, packaging) {
        let mut query_parts = Vec::new();
        if let Some(classifier) = qualifiers.get("classifier") {
            query_parts.push(format!(
                "classifier={}",
                percent_encode_purl_component(classifier)
            ));
        }
        if let Some(type_) = qualifiers.get("type") {
            query_parts.push(format!("type={}", percent_encode_purl_component(type_)));
        }

        if !query_parts.is_empty() {
            purl.push('?');
            purl.push_str(&query_parts.join("&"));
        }
    }

    purl
}

fn percent_encode_purl_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

pub(super) fn build_maven_repository_url(
    group_id: &str,
    artifact_id: &str,
    version: Option<&str>,
    filename: Option<&str>,
) -> String {
    let group_path = group_id.replace('.', "/");
    let mut url = format!("{MAVEN_REPOSITORY_BASE_URL}/{group_path}/{artifact_id}/");

    if let Some(version) = version.filter(|value| !value.trim().is_empty()) {
        url.push_str(version);
        url.push('/');
    }

    if let Some(filename) = filename.filter(|value| !value.trim().is_empty()) {
        url.push_str(filename);
    }

    url
}

pub(super) fn build_maven_download_url(
    group_id: &str,
    artifact_id: &str,
    version: &str,
    classifier: Option<&str>,
    packaging: Option<&str>,
) -> String {
    let extension = normalize_maven_packaging(packaging)
        .filter(|value| *value != "pom")
        .unwrap_or("jar");
    let classifier_suffix = classifier
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("-{value}"))
        .unwrap_or_default();
    let filename = format!("{artifact_id}-{version}{classifier_suffix}.{extension}");

    build_maven_repository_url(group_id, artifact_id, Some(version), Some(&filename))
}

pub(super) fn build_maven_source_package(namespace: &str, name: &str, version: &str) -> String {
    build_maven_purl(namespace, name, Some(version), Some("sources"), None)
}
