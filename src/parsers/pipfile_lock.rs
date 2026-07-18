// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Parser for Pipfile.lock lockfiles.
//!
//! Extracts resolved dependency information from Pipfile.lock files which store
//! locked dependency versions for Python projects using pipenv.
//!
//! # Supported Formats
//! - Pipfile.lock (JSON-based lockfile with per-environment dependency sections)
//!
//! # Key Features
//! - Dependency extraction from default and develop sections
//! - Runtime vs develop scope tracking from lock section membership; Pipfile.lock's
//!   `default`/`develop` sections are the full flattened closure, so `is_direct` and
//!   `is_optional` are left unset for the lock (only the Pipfile manifest proves
//!   `is_direct`)
//! - Exact version resolution with pinned artifact hashes surfaced as dependency `hash_options`
//! - Package URL (purl) generation for PyPI packages
//! - Markers and extras dependency handling
//!
//! # Implementation Notes
//! - Uses JSON parsing via `serde_json` and TOML for secondary parsing
//! - All lockfile versions are pinned (`is_pinned: Some(true)`)
//! - Graceful error handling with `warn!()` logs
//! - Integrates with Python parser utilities for PyPI URL building

use std::collections::HashMap;
use std::path::Path;

use crate::parser_warn as warn;
use packageurl::PackageUrl;
use serde_json::Value as JsonValue;
use toml::Value as TomlValue;
use toml::map::Map as TomlMap;

use crate::models::{DatasourceId, Dependency, PackageData, PackageType, Sha256Digest};
use crate::parsers::python::read_toml_file;
use crate::parsers::utils::{CappedIterExt, read_file_to_string, truncate_field};

use super::PackageParser;
use super::metadata::ParserMetadata;

const FIELD_META: &str = "_meta";
const FIELD_HASH: &str = "hash";
const FIELD_SHA256: &str = "sha256";
const FIELD_DEFAULT: &str = "default";
const FIELD_DEVELOP: &str = "develop";
const FIELD_VERSION: &str = "version";
const FIELD_HASHES: &str = "hashes";

const FIELD_PACKAGES: &str = "packages";
const FIELD_DEV_PACKAGES: &str = "dev-packages";
const FIELD_REQUIRES: &str = "requires";
const FIELD_SOURCE: &str = "source";
const FIELD_PYTHON_VERSION: &str = "python_version";

/// Pipenv lockfile and manifest parser for Pipfile.lock and Pipfile files.
///
/// Extracts Python package dependencies from Pipenv-managed projects, supporting
/// both locked versions (Pipfile.lock) and declared dependencies (Pipfile).
pub struct PipfileLockParser;

impl PackageParser for PipfileLockParser {
    const PACKAGE_TYPE: PackageType = PackageType::Pypi;

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "Pipenv lockfile and manifest",
            file_patterns: &["**/Pipfile.lock", "**/Pipfile"],
            package_type: "pypi",
            primary_language: "Python",
            documentation_url: Some("https://github.com/pypa/pipfile"),
        }]
    }

    fn is_match(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == "Pipfile.lock" || name == "Pipfile")
            .unwrap_or(false)
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        vec![match path.file_name().and_then(|name| name.to_str()) {
            Some("Pipfile.lock") => extract_from_pipfile_lock(path),
            Some("Pipfile") => extract_from_pipfile(path),
            _ => default_package_data(None),
        }]
    }
}

fn extract_from_pipfile_lock(path: &Path) -> PackageData {
    let content = match read_file_to_string(path, None) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to read Pipfile.lock at {:?}: {}", path, e);
            return default_package_data(Some(DatasourceId::PipfileLock));
        }
    };

    let json_content: JsonValue = match serde_json::from_str(&content) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to parse Pipfile.lock at {:?}: {}", path, e);
            return default_package_data(Some(DatasourceId::PipfileLock));
        }
    };

    parse_pipfile_lock(&json_content)
}

fn parse_pipfile_lock(json_content: &JsonValue) -> PackageData {
    let mut package_data = default_package_data(Some(DatasourceId::PipfileLock));
    package_data.sha256 = extract_lockfile_sha256(json_content);

    let meta = json_content
        .get(FIELD_META)
        .and_then(|value| value.as_object());
    let pipfile_spec = meta.and_then(|value| value.get("pipfile-spec"));
    let sources = meta.and_then(|value| value.get("sources"));
    let requires = meta.and_then(|value| value.get("requires"));
    let _ = (pipfile_spec, sources, requires);

    let default_deps = extract_lockfile_dependencies(json_content, FIELD_DEFAULT, "install", true);
    let develop_deps = extract_lockfile_dependencies(json_content, FIELD_DEVELOP, "develop", false);
    package_data.dependencies = [default_deps, develop_deps].concat();

    package_data
}

fn extract_lockfile_sha256(json_content: &JsonValue) -> Option<Sha256Digest> {
    json_content
        .get(FIELD_META)
        .and_then(|meta| meta.get(FIELD_HASH))
        .and_then(|hash| hash.get(FIELD_SHA256))
        .and_then(|value| value.as_str())
        .and_then(|s| Sha256Digest::from_hex(s).ok())
}

fn extract_lockfile_dependencies(
    json_content: &JsonValue,
    section: &str,
    scope: &str,
    is_runtime: bool,
) -> Vec<Dependency> {
    let mut dependencies = Vec::new();

    if let Some(section_map) = json_content
        .get(section)
        .and_then(|value| value.as_object())
    {
        for (name, value) in section_map.iter().capped("Pipfile.lock section packages") {
            if let Some(dependency) = build_lockfile_dependency(name, value, scope, is_runtime) {
                dependencies.push(dependency);
            }
        }
    }

    dependencies
}

fn build_lockfile_dependency(
    name: &str,
    value: &JsonValue,
    scope: &str,
    is_runtime: bool,
) -> Option<Dependency> {
    let normalized_name = normalize_pypi_name(name);
    let requirement = extract_lockfile_requirement(value)?;
    let version = strip_pipfile_lock_version(&requirement);
    let purl = create_pypi_purl(&normalized_name, version.as_deref());

    Some(Dependency {
        purl,
        extracted_requirement: Some(truncate_field(requirement)),
        scope: Some(scope.to_string()),
        is_runtime: Some(is_runtime),
        is_optional: None,
        is_pinned: Some(true),
        is_direct: None,
        resolved_package: None,
        extra_data: build_lockfile_dependency_extra_data(value),
    })
}

/// Surface the per-package pinned artifact hashes (the `hashes` array in each
/// Pipfile.lock entry) as `hash_options`, mirroring the requirements.txt parser.
///
/// These are the expected hashes of the upstream wheels/sdists this lock pins,
/// which is distinct from the scanned lockfile's own content hash. Returns
/// `None` when no hashes are present so dependencies without pins stay clean.
fn build_lockfile_dependency_extra_data(value: &JsonValue) -> Option<HashMap<String, JsonValue>> {
    let hashes = extract_lockfile_hashes(value);
    if hashes.is_empty() {
        return None;
    }
    let mut extra_data = HashMap::new();
    extra_data.insert(
        "hash_options".to_string(),
        JsonValue::Array(hashes.into_iter().map(JsonValue::String).collect()),
    );
    Some(extra_data)
}

/// Collect the raw pinned hash strings (e.g. `"sha256:..."`) from a Pipfile.lock
/// entry's `hashes` array, preserving the algorithm prefix as requirements.txt does.
fn extract_lockfile_hashes(value: &JsonValue) -> Vec<String> {
    value
        .get(FIELD_HASHES)
        .and_then(|hashes_value| hashes_value.as_array())
        .map(|hash_values| {
            hash_values
                .iter()
                .filter_map(|hash_value| hash_value.as_str())
                .map(|hash| truncate_field(hash.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_lockfile_requirement(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::String(spec) => Some(truncate_field(spec.to_string())),
        JsonValue::Object(map) => map
            .get(FIELD_VERSION)
            .and_then(|version| version.as_str())
            .map(|version| truncate_field(version.to_string())),
        _ => None,
    }
}

fn strip_pipfile_lock_version(requirement: &str) -> Option<String> {
    let trimmed = requirement.trim();
    if let Some(stripped) = trimmed.strip_prefix("==") {
        let version = stripped.trim();
        if version.is_empty() {
            None
        } else {
            Some(truncate_field(version.to_string()))
        }
    } else {
        None
    }
}

fn extract_from_pipfile(path: &Path) -> PackageData {
    let toml_content = match read_toml_file(path) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to read Pipfile at {:?}: {}", path, e);
            return default_package_data(Some(DatasourceId::Pipfile));
        }
    };

    parse_pipfile(&toml_content)
}

fn parse_pipfile(toml_content: &TomlValue) -> PackageData {
    let mut package_data = default_package_data(Some(DatasourceId::Pipfile));

    let packages = toml_content
        .get(FIELD_PACKAGES)
        .and_then(|value| value.as_table());
    let dev_packages = toml_content
        .get(FIELD_DEV_PACKAGES)
        .and_then(|value| value.as_table());

    let mut dependencies = Vec::new();
    if let Some(packages) = packages {
        dependencies.extend(extract_pipfile_dependencies(packages, "install", true));
    }
    if let Some(dev_packages) = dev_packages {
        dependencies.extend(extract_pipfile_dependencies(dev_packages, "develop", false));
    }

    package_data.dependencies = dependencies;
    package_data.extra_data = build_pipfile_extra_data(toml_content);

    package_data
}

fn extract_pipfile_dependencies(
    packages: &TomlMap<String, TomlValue>,
    scope: &str,
    is_runtime: bool,
) -> Vec<Dependency> {
    let mut dependencies = Vec::new();

    for (name, value) in packages.iter().capped("Pipfile packages") {
        if let Some(dependency) = build_pipfile_dependency(name, value, scope, is_runtime) {
            dependencies.push(dependency);
        }
    }

    dependencies
}

fn build_pipfile_dependency(
    name: &str,
    value: &TomlValue,
    scope: &str,
    is_runtime: bool,
) -> Option<Dependency> {
    let normalized_name = normalize_pypi_name(name);
    let requirement = extract_pipfile_requirement(value);
    if requirement.is_none() && is_non_registry_dependency(value) {
        return None;
    }
    let requirement = requirement?;
    let purl = create_pypi_purl(&normalized_name, None);

    Some(Dependency {
        purl,
        extracted_requirement: Some(truncate_field(requirement)),
        scope: Some(scope.to_string()),
        is_runtime: Some(is_runtime),
        is_optional: None,
        is_pinned: Some(false),
        is_direct: Some(true),
        resolved_package: None,
        extra_data: None,
    })
}

fn extract_pipfile_requirement(value: &TomlValue) -> Option<String> {
    match value {
        TomlValue::String(spec) => Some(truncate_field(spec.to_string())),
        TomlValue::Boolean(true) => Some("*".to_string()),
        TomlValue::Table(table) => table
            .get(FIELD_VERSION)
            .and_then(|version| version.as_str())
            .map(|version| truncate_field(version.to_string())),
        _ => None,
    }
}

fn is_non_registry_dependency(value: &TomlValue) -> bool {
    let table = match value {
        TomlValue::Table(table) => table,
        _ => return false,
    };

    ["git", "path", "file", "url", "hg", "svn"]
        .iter()
        .any(|key| table.contains_key(*key))
}

fn build_pipfile_extra_data(
    toml_content: &TomlValue,
) -> Option<HashMap<String, serde_json::Value>> {
    let mut extra_data = HashMap::new();

    if let Some(requires_table) = toml_content
        .get(FIELD_REQUIRES)
        .and_then(|value| value.as_table())
        && let Some(python_version) = requires_table
            .get(FIELD_PYTHON_VERSION)
            .and_then(|value| value.as_str())
    {
        extra_data.insert(
            FIELD_PYTHON_VERSION.to_string(),
            serde_json::Value::String(truncate_field(python_version.to_string())),
        );
    }

    if let Some(source_value) = toml_content.get(FIELD_SOURCE)
        && let Some(sources) = parse_pipfile_sources(source_value)
    {
        extra_data.insert("sources".to_string(), sources);
    }

    if extra_data.is_empty() {
        None
    } else {
        Some(extra_data)
    }
}

fn parse_pipfile_sources(source_value: &TomlValue) -> Option<serde_json::Value> {
    match source_value {
        TomlValue::Array(sources) => {
            let mut json_sources = Vec::new();
            for source in sources {
                if let Some(table) = source.as_table() {
                    let mut json_map = serde_json::Map::new();
                    if let Some(name) = table.get("name").and_then(|value| value.as_str()) {
                        json_map.insert(
                            "name".to_string(),
                            serde_json::Value::String(truncate_field(name.to_string())),
                        );
                    }
                    if let Some(url) = table.get("url").and_then(|value| value.as_str()) {
                        json_map.insert(
                            "url".to_string(),
                            serde_json::Value::String(truncate_field(url.to_string())),
                        );
                    }
                    if let Some(verify_ssl) =
                        table.get("verify_ssl").and_then(|value| value.as_bool())
                    {
                        json_map.insert(
                            "verify_ssl".to_string(),
                            serde_json::Value::Bool(verify_ssl),
                        );
                    }
                    json_sources.push(serde_json::Value::Object(json_map));
                }
            }

            Some(serde_json::Value::Array(json_sources))
        }
        TomlValue::Table(table) => {
            let mut json_map = serde_json::Map::new();
            for (key, value) in table {
                match value {
                    TomlValue::String(value) => {
                        json_map.insert(
                            key.to_string(),
                            serde_json::Value::String(truncate_field(value.to_string())),
                        );
                    }
                    TomlValue::Boolean(value) => {
                        json_map.insert(key.to_string(), serde_json::Value::Bool(*value));
                    }
                    _ => {}
                }
            }
            Some(serde_json::Value::Object(json_map))
        }
        _ => None,
    }
}

fn normalize_pypi_name(name: &str) -> String {
    truncate_field(name.trim().to_ascii_lowercase())
}

fn create_pypi_purl(name: &str, version: Option<&str>) -> Option<String> {
    let mut purl = PackageUrl::new(PipfileLockParser::PACKAGE_TYPE.as_str(), name).ok()?;
    if let Some(version) = version
        && purl.with_version(version).is_err()
    {
        return None;
    }

    Some(purl.to_string())
}

fn default_package_data(datasource_id: Option<DatasourceId>) -> PackageData {
    PackageData {
        package_type: Some(PipfileLockParser::PACKAGE_TYPE),
        primary_language: Some("Python".to_string()),
        datasource_id,
        ..Default::default()
    }
}
