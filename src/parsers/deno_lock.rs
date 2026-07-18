// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::parser_warn as warn;
use packageurl::PackageUrl;
use serde_json::Value;
use url::Url;

use crate::models::{
    DatasourceId, Dependency, PackageData, PackageType, ResolvedPackage, Sha256Digest, Sha512Digest,
};

use super::PackageParser;
use super::metadata::ParserMetadata;
use super::utils::{
    CappedIterExt, capped_iteration_limit, parse_sri, read_file_to_string, truncate_field,
};

const FIELD_VERSION: &str = "version";
const FIELD_SPECIFIERS: &str = "specifiers";
const FIELD_JSR: &str = "jsr";
const FIELD_NPM: &str = "npm";
const FIELD_REMOTE: &str = "remote";
const FIELD_REDIRECTS: &str = "redirects";
const FIELD_WORKSPACE: &str = "workspace";
const FIELD_DEPENDENCIES: &str = "dependencies";

pub struct DenoLockParser;

impl PackageParser for DenoLockParser {
    const PACKAGE_TYPE: PackageType = PackageType::Deno;

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "Deno lockfile",
            file_patterns: &["**/deno.lock"],
            package_type: "deno",
            primary_language: "TypeScript",
            documentation_url: Some("https://docs.deno.com/runtime/fundamentals/modules/"),
        }]
    }

    fn is_match(path: &Path) -> bool {
        path.file_name().and_then(|name| name.to_str()) == Some("deno.lock")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(content) => content,
            Err(e) => {
                warn!("Failed to read deno.lock at {:?}: {}", path, e);
                return vec![default_package_data()];
            }
        };

        let json: Value = match serde_json::from_str(&content) {
            Ok(json) => json,
            Err(e) => {
                warn!("Failed to parse deno.lock at {:?}: {}", path, e);
                return vec![default_package_data()];
            }
        };

        vec![parse_deno_lock(&json)]
    }
}

fn parse_deno_lock(json: &Value) -> PackageData {
    let lock_version = json.get(FIELD_VERSION).and_then(Value::as_str);

    // deno.lock has gone through several incompatible layouts; dispatch on the declared
    // version so older lockfiles still in the wild are not silently dropped:
    //   * v1 (Deno 1.0-1.17): remote ESM URL imports only, no registry dependencies.
    //   * v2/v3 (Deno 1.18-1.40): npm dependencies nested under npm.{specifiers,packages};
    //     no jsr or workspace sections.
    //   * v4 (Deno 1.45+) and v5 (current): flat top-level specifiers/npm/jsr/workspace.
    // Unknown/newer versions fall back to the latest (flat) layout rather than dropping.
    let (mut dependencies, workspace_direct) = match lock_version {
        Some("1") => (Vec::new(), Vec::new()),
        Some("2") | Some("3") => (extract_nested_npm_dependencies(json), Vec::new()),
        Some("4") | Some("5") => extract_flat_dependencies(json),
        other => {
            warn!(
                "Unrecognized deno.lock version {:?}; parsing with the latest known layout",
                other
            );
            extract_flat_dependencies(json)
        }
    };
    dependencies.extend(extract_redirect_dependencies(json));

    let mut extra_data = HashMap::new();
    if let Some(version) = lock_version {
        extra_data.insert(
            FIELD_VERSION.to_string(),
            Value::String(version.to_string()),
        );
    }
    if !workspace_direct.is_empty() {
        extra_data.insert(
            "workspace_dependencies".to_string(),
            Value::Array(workspace_direct.into_iter().map(Value::String).collect()),
        );
    }

    PackageData {
        package_type: Some(DenoLockParser::PACKAGE_TYPE),
        primary_language: Some("TypeScript".to_string()),
        dependencies,
        extra_data: (!extra_data.is_empty()).then_some(extra_data),
        datasource_id: Some(DatasourceId::DenoLock),
        ..Default::default()
    }
}

/// Flat top-level layout used by deno.lock v4 and v5: `specifiers`, `npm`, `jsr`, and
/// `workspace` sections. Returns the resolved dependencies plus the workspace's direct
/// specifier list (recorded in extra_data by the caller).
fn extract_flat_dependencies(json: &Value) -> (Vec<Dependency>, Vec<String>) {
    let specifiers = json
        .get(FIELD_SPECIFIERS)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let workspace_direct = extract_workspace_dependencies(json);

    let mut dependencies = Vec::new();
    let mut direct_jsr_keys = HashSet::new();
    let mut direct_npm_keys = HashSet::new();

    let workspace_limit =
        capped_iteration_limit(workspace_direct.len(), "deno.lock workspace specifiers");
    for specifier in workspace_direct.iter().take(workspace_limit) {
        if let Some(resolved_key) = specifiers.get(specifier).and_then(Value::as_str) {
            if specifier.starts_with("jsr:") {
                if let Some(full_key) = resolve_jsr_full_key(specifier, resolved_key)
                    && let Some(dep) =
                        build_jsr_dependency(&full_key, true, &json[FIELD_JSR], Some(specifier))
                {
                    direct_jsr_keys.insert(full_key);
                    dependencies.push(dep);
                }
            } else if specifier.starts_with("npm:")
                && let Some(full_key) = resolve_npm_full_key(specifier, resolved_key)
                && let Some(dep) =
                    build_npm_dependency(&full_key, true, &json[FIELD_NPM], Some(specifier))
            {
                direct_npm_keys.insert(full_key);
                dependencies.push(dep);
            }
        }
    }

    if let Some(jsr_map) = json.get(FIELD_JSR).and_then(Value::as_object) {
        let jsr_limit = capped_iteration_limit(jsr_map.len(), "deno.lock jsr packages");
        for key in jsr_map.keys().take(jsr_limit) {
            if direct_jsr_keys.contains(key) {
                continue;
            }
            if let Some(dep) = build_jsr_dependency(key, false, &json[FIELD_JSR], None) {
                dependencies.push(dep);
            }
        }
    }

    if let Some(npm_map) = json.get(FIELD_NPM).and_then(Value::as_object) {
        let npm_limit = capped_iteration_limit(npm_map.len(), "deno.lock npm packages");
        for key in npm_map.keys().take(npm_limit) {
            if direct_npm_keys.contains(key) {
                continue;
            }
            if let Some(dep) = build_npm_dependency(key, false, &json[FIELD_NPM], None) {
                dependencies.push(dep);
            }
        }
    }

    (dependencies, workspace_direct)
}

/// Nested layout used by deno.lock v2 and v3: npm dependencies live under `npm.packages`
/// (the resolved graph), with `npm.specifiers` mapping requested ranges to resolved
/// `name@version` keys. There are no jsr or workspace sections in these versions.
fn extract_nested_npm_dependencies(json: &Value) -> Vec<Dependency> {
    let Some(npm) = json.get(FIELD_NPM) else {
        return Vec::new();
    };
    let Some(packages) = npm.get("packages") else {
        return Vec::new();
    };
    let Some(packages_map) = packages.as_object() else {
        return Vec::new();
    };

    // `npm.specifiers` maps each requested specifier (e.g. "chalk@5") to its resolved
    // `name@version` key (e.g. "chalk@5.3.0"). Invert it so resolved packages referenced
    // by a top-level specifier can be marked direct and keep their requested range; the
    // remaining npm.packages entries are transitive.
    let requested_by_resolved: HashMap<String, String> = npm
        .get("specifiers")
        .and_then(Value::as_object)
        .map(|specifiers| {
            specifiers
                .iter()
                .filter_map(|(specifier, resolved)| {
                    resolved.as_str().map(|resolved| {
                        (
                            resolved.trim_start_matches("npm:").to_string(),
                            specifier.clone(),
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let mut dependencies = Vec::new();
    let packages_limit =
        capped_iteration_limit(packages_map.len(), "deno.lock nested npm packages");
    for key in packages_map.keys().take(packages_limit) {
        let requested = requested_by_resolved.get(key).map(String::as_str);
        if let Some(dep) = build_npm_dependency(key, requested.is_some(), packages, requested) {
            dependencies.push(dep);
        }
    }
    dependencies
}

/// Module redirects, shared across all lockfile versions: each redirect target is a
/// remote module whose integrity hash lives in the `remote` section.
fn extract_redirect_dependencies(json: &Value) -> Vec<Dependency> {
    let mut dependencies = Vec::new();
    if let Some(redirects) = json.get(FIELD_REDIRECTS).and_then(Value::as_object) {
        let redirects_limit = capped_iteration_limit(redirects.len(), "deno.lock redirects");
        for (source, target) in redirects.iter().take(redirects_limit) {
            let Some(target_url) = target.as_str() else {
                continue;
            };
            let hash = json
                .get(FIELD_REMOTE)
                .and_then(Value::as_object)
                .and_then(|remote| remote.get(target_url))
                .and_then(Value::as_str)
                .and_then(|value| Sha256Digest::from_hex(value).ok());

            let name =
                truncate_field(remote_name(target_url).unwrap_or_else(|| source.to_string()));
            let purl = create_remote_purl(target_url).map(truncate_field);
            let resolved_package = ResolvedPackage {
                primary_language: Some("TypeScript".to_string()),
                download_url: Some(truncate_field(target_url.to_string())),
                sha1: None,
                sha256: hash,
                sha512: None,
                md5: None,
                is_virtual: true,
                extra_data: Some(HashMap::from([(
                    "redirect_source".to_string(),
                    Value::String(truncate_field(source.to_string())),
                )])),
                dependencies: Vec::new(),
                repository_homepage_url: None,
                repository_download_url: None,
                api_data_url: None,
                datasource_id: Some(DatasourceId::DenoLock),
                purl: purl.clone(),
                ..ResolvedPackage::new(
                    DenoLockParser::PACKAGE_TYPE,
                    String::new(),
                    name.clone(),
                    String::new(),
                )
            };

            // `redirects` maps any raw URL import to the target it actually resolved to;
            // it does not distinguish a specifier written directly in the workspace's own
            // source from one reached transitively through another remote module's own
            // imports, so `is_runtime`/`is_optional`/`is_direct` are not provable here.
            dependencies.push(Dependency {
                purl,
                extracted_requirement: Some(truncate_field(source.to_string())),
                scope: Some("imports".to_string()),
                is_runtime: None,
                is_optional: None,
                is_pinned: Some(true),
                is_direct: None,
                resolved_package: Some(Box::new(resolved_package)),
                extra_data: None,
            });
        }
    }
    dependencies
}

fn extract_workspace_dependencies(json: &Value) -> Vec<String> {
    json.get(FIELD_WORKSPACE)
        .and_then(Value::as_object)
        .and_then(|workspace| workspace.get(FIELD_DEPENDENCIES))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|value| truncate_field(value.to_string()))
        .collect()
}

fn build_jsr_dependency(
    resolved_key: &str,
    is_direct: bool,
    jsr_section: &Value,
    extracted_requirement: Option<&str>,
) -> Option<Dependency> {
    let jsr_entry = jsr_section.get(resolved_key)?;
    let jsr_object = jsr_entry.as_object()?;
    let (namespace, name, version) = parse_jsr_key(resolved_key)?;
    let namespace = truncate_field(namespace);
    let name = truncate_field(name);
    let version_str = truncate_field(version.to_string());
    let purl = create_generic_purl(
        Some(&format!("jsr.io/{}", namespace)),
        &name,
        Some(&version_str),
    )
    .map(truncate_field);

    // `is_direct` is provable from `workspace.dependencies` (the caller passes whether
    // this key's specifier is in that list). deno.lock has no runtime-vs-dev or
    // required-vs-optional distinction once a package.json dependency has been
    // flattened into `specifiers`, so `is_runtime`/`is_optional` stay unset rather
    // than guessed.
    Some(Dependency {
        purl: purl.clone(),
        extracted_requirement: extracted_requirement.map(|value| truncate_field(value.to_string())),
        scope: Some("imports".to_string()),
        is_runtime: None,
        is_optional: None,
        is_pinned: Some(true),
        is_direct: Some(is_direct),
        resolved_package: Some(Box::new(ResolvedPackage {
            primary_language: Some("TypeScript".to_string()),
            download_url: None,
            sha1: None,
            sha256: jsr_object
                .get("integrity")
                .and_then(Value::as_str)
                .and_then(|value| {
                    parse_sri(value)
                        .and_then(|(algo, hex)| {
                            (algo == "sha256").then(|| Sha256Digest::from_hex(&hex).ok())
                        })
                        .flatten()
                        .or_else(|| Sha256Digest::from_hex(value).ok())
                }),
            sha512: None,
            md5: None,
            is_virtual: true,
            extra_data: None,
            dependencies: extract_jsr_resolved_dependencies(jsr_object),
            repository_homepage_url: None,
            repository_download_url: None,
            api_data_url: None,
            datasource_id: Some(DatasourceId::DenoLock),
            purl,
            ..ResolvedPackage::new(DenoLockParser::PACKAGE_TYPE, namespace, name, version_str)
        })),
        extra_data: None,
    })
}

/// A package's nested `dependencies` field is shaped differently across lockfile
/// versions: v4/v5 `npm[key].dependencies` is an array of resolved `name@version`
/// keys (`["ansi-styles@6.2.1"]`), while v2/v3 `npm.packages[key].dependencies` is an
/// object mapping bare names to those resolved keys (`{"ansi-styles":"ansi-styles@6.2.1"}`).
/// Both encode the same resolved keys, so collect them uniformly from either shape.
fn npm_dependency_keys(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(Value::Object(map)) => map
            .values()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn build_npm_dependency(
    resolved_key: &str,
    is_direct: bool,
    npm_section: &Value,
    extracted_requirement: Option<&str>,
) -> Option<Dependency> {
    let npm_entry = npm_section.get(resolved_key)?;
    let npm_object = npm_entry.as_object()?;
    let (namespace, name, version) = parse_npm_key(resolved_key)?;
    let namespace = namespace.map(truncate_field);
    let name = truncate_field(name);
    let version_str = truncate_field(version.to_string());
    let purl = create_npm_purl(namespace.as_deref(), &name, Some(&version_str)).map(truncate_field);

    let resolved_dependency_keys = npm_dependency_keys(npm_object.get(FIELD_DEPENDENCIES));
    let resolved_dependency_limit = capped_iteration_limit(
        resolved_dependency_keys.len(),
        "deno.lock npm resolved dependencies",
    );

    // Same reasoning as `build_jsr_dependency`: `is_direct` comes from the caller's
    // `workspace.dependencies` membership check, but deno.lock never records a
    // runtime-vs-dev or required-vs-optional split, so `is_runtime`/`is_optional`
    // stay unset.
    Some(Dependency {
        purl: purl.clone(),
        extracted_requirement: extracted_requirement.map(|value| truncate_field(value.to_string())),
        scope: Some("imports".to_string()),
        is_runtime: None,
        is_optional: None,
        is_pinned: Some(true),
        is_direct: Some(is_direct),
        resolved_package: Some(Box::new(ResolvedPackage {
            primary_language: Some("JavaScript".to_string()),
            download_url: npm_object
                .get("tarball")
                .and_then(Value::as_str)
                .map(|value| truncate_field(value.to_string())),
            sha1: None,
            sha256: None,
            sha512: npm_object
                .get("integrity")
                .and_then(Value::as_str)
                .and_then(|value| {
                    parse_sri(value)
                        .and_then(|(algo, hex)| {
                            (algo == "sha512").then(|| Sha512Digest::from_hex(&hex).ok())
                        })
                        .flatten()
                }),
            md5: None,
            is_virtual: true,
            extra_data: None,
            dependencies: resolved_dependency_keys
                .into_iter()
                .take(resolved_dependency_limit)
                .filter_map(|value| {
                    let (namespace, name, version) = parse_npm_key(&value)?;
                    // This is a graph edge from an already-resolved npm package's own
                    // `dependencies`, not the workspace's own specifier list, so none of
                    // `is_runtime`/`is_optional`/`is_direct` is provable here.
                    Some(Dependency {
                        purl: create_npm_purl(namespace.as_deref(), &name, Some(version))
                            .map(truncate_field),
                        extracted_requirement: Some(truncate_field(value.clone())),
                        scope: Some("dependencies".to_string()),
                        is_runtime: None,
                        is_optional: None,
                        is_pinned: Some(true),
                        is_direct: None,
                        resolved_package: None,
                        extra_data: None,
                    })
                })
                .collect(),
            repository_homepage_url: None,
            repository_download_url: None,
            api_data_url: None,
            datasource_id: Some(DatasourceId::DenoLock),
            purl,
            ..ResolvedPackage::new(
                PackageType::Npm,
                namespace.unwrap_or_default(),
                name,
                version_str,
            )
        })),
        extra_data: None,
    })
}

fn extract_jsr_resolved_dependencies(
    jsr_object: &serde_json::Map<String, Value>,
) -> Vec<Dependency> {
    jsr_object
        .get(FIELD_DEPENDENCIES)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .capped("deno.lock jsr resolved dependencies")
        .filter_map(|value| {
            let (namespace, name, version) = parse_jsr_dependency_reference(value)?;
            // A graph edge from an already-resolved jsr package's own `dependencies`
            // array; not the workspace's own specifier list, so `is_runtime`,
            // `is_optional`, and `is_direct` are not provable.
            Some(Dependency {
                purl: create_generic_purl(Some(&format!("jsr.io/{}", namespace)), &name, version)
                    .map(truncate_field),
                extracted_requirement: Some(truncate_field(value.to_string())),
                scope: Some("dependencies".to_string()),
                is_runtime: None,
                is_optional: None,
                is_pinned: Some(version.is_some_and(is_exact_version)),
                is_direct: None,
                resolved_package: None,
                extra_data: None,
            })
        })
        .collect()
}

fn parse_jsr_key(key: &str) -> Option<(String, String, &str)> {
    let scoped = key.strip_prefix('@')?;
    let slash_index = scoped.find('/')?;
    let namespace = format!("@{}", &scoped[..slash_index]);
    let name_and_version = &scoped[slash_index + 1..];
    let at_index = name_and_version.rfind('@')?;
    let name = name_and_version[..at_index].to_string();
    let version = &name_and_version[at_index + 1..];
    Some((namespace, name, version))
}

fn parse_jsr_dependency_reference(value: &str) -> Option<(String, String, Option<&str>)> {
    let rest = value.strip_prefix("jsr:")?;
    let slash_index = rest.find('/')?;
    let namespace = format!("@{}", &rest[1..slash_index]);
    let name_and_version = &rest[slash_index + 1..];
    let (name, version) = split_name_and_version(name_and_version);
    Some((namespace, name.to_string(), version))
}

fn resolve_jsr_full_key(specifier: &str, resolved_version: &str) -> Option<String> {
    let (namespace, name, _) = parse_jsr_dependency_reference(specifier)?;
    Some(format!("{}/{}@{}", namespace, name, resolved_version))
}

fn parse_npm_key(key: &str) -> Option<(Option<String>, String, &str)> {
    if let Some(scoped) = key.strip_prefix('@') {
        let slash_index = scoped.find('/')?;
        let namespace = format!("@{}", &scoped[..slash_index]);
        let name_and_version = &scoped[slash_index + 1..];
        let at_index = name_and_version.rfind('@')?;
        let name = name_and_version[..at_index].to_string();
        let version = &name_and_version[at_index + 1..];
        Some((Some(namespace), name, version))
    } else {
        let at_index = key.rfind('@')?;
        let name = key[..at_index].to_string();
        let version = &key[at_index + 1..];
        Some((None, name, version))
    }
}

fn resolve_npm_full_key(specifier: &str, resolved_version: &str) -> Option<String> {
    let (namespace, name, _) = parse_npm_specifier(specifier)?;
    Some(match namespace {
        Some(namespace) => format!("{}/{}@{}", namespace, name, resolved_version),
        None => format!("{}@{}", name, resolved_version),
    })
}

fn parse_npm_specifier(specifier: &str) -> Option<(Option<String>, String, Option<&str>)> {
    let rest = specifier.strip_prefix("npm:")?;
    let (name_part, version) = split_name_and_version(rest);
    if let Some(scoped) = name_part.strip_prefix('@') {
        let slash_index = scoped.find('/')?;
        let namespace = format!("@{}", &scoped[..slash_index]);
        let name = scoped[slash_index + 1..].to_string();
        Some((Some(namespace), name, version))
    } else {
        Some((None, name_part.to_string(), version))
    }
}

fn split_name_and_version(input: &str) -> (&str, Option<&str>) {
    if let Some(index) = input.rfind('@') {
        let (name, version) = input.split_at(index);
        if !name.is_empty() {
            return (name, Some(&version[1..]));
        }
    }
    (input, None)
}

fn create_npm_purl(namespace: Option<&str>, name: &str, version: Option<&str>) -> Option<String> {
    let mut purl = PackageUrl::new("npm", name).ok()?;
    if let Some(namespace) = namespace {
        purl.with_namespace(namespace).ok()?;
    }
    if let Some(version) = version {
        purl.with_version(version).ok()?;
    }
    Some(purl.to_string())
}

fn create_generic_purl(
    namespace: Option<&str>,
    name: &str,
    version: Option<&str>,
) -> Option<String> {
    let mut purl = PackageUrl::new("generic", name).ok()?;
    if let Some(namespace) = namespace {
        purl.with_namespace(namespace).ok()?;
    }
    if let Some(version) = version {
        purl.with_version(version).ok()?;
    }
    Some(purl.to_string())
}

fn create_remote_purl(specifier: &str) -> Option<String> {
    let url = Url::parse(specifier).ok()?;
    let segments: Vec<&str> = url.path_segments()?.collect();
    let name = segments.last()?.to_string();
    let namespace = if segments.len() > 1 {
        Some(format!(
            "{}/{}",
            url.host_str()?,
            segments[..segments.len() - 1].join("/")
        ))
    } else {
        url.host_str().map(|host| host.to_string())
    };
    create_generic_purl(namespace.as_deref(), &name, None)
}

fn remote_name(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    url.path_segments()?
        .next_back()
        .map(|value| value.to_string())
}

fn is_exact_version(version: &str) -> bool {
    !version.contains('^')
        && !version.contains('~')
        && !version.contains('*')
        && !version.contains('>')
        && !version.contains('<')
        && !version.contains('|')
        && !version.contains(' ')
}

fn default_package_data() -> PackageData {
    PackageData {
        package_type: Some(DenoLockParser::PACKAGE_TYPE),
        primary_language: Some("TypeScript".to_string()),
        datasource_id: Some(DatasourceId::DenoLock),
        ..Default::default()
    }
}
