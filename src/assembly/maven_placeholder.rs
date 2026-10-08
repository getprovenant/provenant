// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Resolves unresolved `${...}` Maven POM coordinates from a sibling
//! `pom.properties`, which Maven writes at build time with the concrete group,
//! artifact, and version of the packaged artifact.

use std::path::Path;
use std::str::FromStr;

use packageurl::PackageUrl;

use crate::models::{DatasourceId, Package, PackageData, PackageType, PackageUid, normalize_purl};
use crate::parsers::build_maven_repository_links;
use crate::parsers::utils::namespaced_purl;

/// `[namespace, name, version]`.
type Coordinates<'a> = [Option<&'a str>; 3];

fn is_unresolved(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.contains("${"))
}

fn is_concrete(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty() && !value.contains("${"))
}

fn package_data_coordinates(pkg_data: &PackageData) -> Coordinates<'_> {
    [
        pkg_data.namespace.as_deref(),
        pkg_data.name.as_deref(),
        pkg_data.version.as_deref(),
    ]
}

/// Replaces each unresolved coordinate in `current` with the concrete value from
/// `properties`. Returns `None` when `properties` is not a `pom.properties`,
/// nothing gets resolved, or a coordinate concrete on both sides disagrees.
fn resolve_against<'a>(
    current: Coordinates<'a>,
    properties: &'a PackageData,
) -> Option<Coordinates<'a>> {
    if properties.datasource_id != Some(DatasourceId::MavenPomProperties) {
        return None;
    }

    let mut resolved = current;
    for (slot, candidate) in resolved
        .iter_mut()
        .zip(package_data_coordinates(properties))
    {
        if is_unresolved(*slot) {
            if is_concrete(candidate) {
                *slot = candidate;
            }
        } else if is_concrete(*slot) && is_concrete(candidate) && *slot != candidate {
            return None;
        }
    }

    (resolved != current).then_some(resolved)
}

/// For a POM whose coordinates contain `${...}` placeholders, the single sibling
/// `pom.properties` that resolves them (all agreeing candidates must share one
/// identity).
pub(super) fn resolving_properties<'a>(
    pom: &PackageData,
    siblings: impl IntoIterator<Item = &'a PackageData>,
) -> Option<&'a PackageData> {
    if pom.datasource_id != Some(DatasourceId::MavenPom) {
        return None;
    }
    let current = package_data_coordinates(pom);
    let mut candidates = siblings
        .into_iter()
        .filter(|properties| resolve_against(current, properties).is_some());
    let first = candidates.next()?;
    let identity = package_data_coordinates(first);
    candidates
        .all(|other| package_data_coordinates(other) == identity)
        .then_some(first)
}

/// Grouping purl for a placeholder POM: its resolved coordinates plus its own
/// qualifiers, so distinct `type`/`classifier` artifacts stay separate.
pub(super) fn resolved_identity_purl(
    pom: &PackageData,
    properties: &PackageData,
) -> Option<String> {
    let [Some(namespace), Some(name), version] =
        resolve_against(package_data_coordinates(pom), properties)?
    else {
        return None;
    };
    let qualifiers = purl_qualifiers(pom.purl.as_deref());
    if qualifiers.is_empty() {
        return properties.purl.clone();
    }
    build_purl(namespace, name, version, &qualifiers)
}

fn purl_qualifiers(purl: Option<&str>) -> Vec<(String, String)> {
    purl.and_then(|purl| PackageUrl::from_str(purl).ok())
        .map(|purl| {
            purl.qualifiers()
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Adopts the concrete coordinates `pom.properties` declares for any `${...}`
/// placeholder in a Maven package, rebuilding its purl, uid, repository links,
/// and sources purl to match. Only a `pom.properties` beside the POM that seeded
/// the package is trusted.
pub(super) fn adopt_resolved_maven_coordinates(
    package: &mut Package,
    pkg_data: &PackageData,
    datafile_path: &str,
) {
    if package.package_type != Some(PackageType::Maven) {
        return;
    }
    let seed_dir = package
        .datafile_paths
        .first()
        .and_then(|seed| Path::new(seed).parent());
    if seed_dir.is_none() || seed_dir != Path::new(datafile_path).parent() {
        return;
    }
    let current = [
        package.namespace.as_deref(),
        package.name.as_deref(),
        package.version.as_deref(),
    ];
    let Some(resolved @ [Some(namespace), Some(name), version]) =
        resolve_against(current, pkg_data)
    else {
        return;
    };
    let qualifiers = purl_qualifiers(package.purl.as_deref());
    let Some(purl) = build_purl(namespace, name, version, &qualifiers) else {
        return;
    };
    let qualifier = |wanted: &str| {
        qualifiers
            .iter()
            .find(|(key, _)| key == wanted)
            .map(|(_, value)| value.as_str())
    };
    let links = build_maven_repository_links(
        namespace,
        name,
        version,
        qualifier("classifier"),
        qualifier("type"),
    );
    let sources = build_purl(namespace, name, version, &sources_qualifier());

    let source_packages = package
        .source_packages
        .iter()
        .filter_map(|source| {
            if is_sources_purl_of(source, current) {
                sources.clone()
            } else {
                Some(source.clone())
            }
        })
        .collect();
    let [namespace, name, version] = resolved.map(|value| value.map(str::to_string));
    package.namespace = namespace;
    package.name = name;
    package.version = version;
    package.package_uid = PackageUid::new(&purl);
    package.purl = Some(purl);
    package.repository_homepage_url = Some(links.homepage_url);
    package.repository_download_url = links.download_url;
    package.api_data_url = links.api_data_url;
    package.source_packages = source_packages;
}

fn sources_qualifier() -> [(String, String); 1] {
    [("classifier".to_string(), "sources".to_string())]
}

fn build_purl(
    namespace: &str,
    name: &str,
    version: Option<&str>,
    qualifiers: &[(String, String)],
) -> Option<String> {
    let mut purl =
        PackageUrl::from_str(&namespaced_purl("maven", namespace, name, version)?).ok()?;
    for (key, value) in qualifiers {
        purl.add_qualifier(key.clone(), value.clone()).ok()?;
    }
    Some(normalize_purl(&purl.to_string()))
}

fn is_sources_purl_of(source: &str, [namespace, name, version]: Coordinates<'_>) -> bool {
    PackageUrl::from_str(source).is_ok_and(|purl| {
        purl.ty() == "maven"
            && purl.namespace() == namespace
            && Some(purl.name()) == name
            && purl.version() == version
            && purl
                .qualifiers()
                .get("classifier")
                .is_some_and(|value| value == "sources")
    })
}

#[cfg(test)]
#[path = "maven_placeholder_test.rs"]
mod tests;
