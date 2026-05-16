// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Parser for CocoaPods Podfile manifest files.
//!
//! Extracts dependency declarations from Podfile using regex-based Ruby Domain-Specific
//! Language (DSL) parsing without full Ruby AST parsing.
//!
//! # Supported Formats
//! - Podfile (CocoaPods manifest with Ruby DSL syntax)
//!
//! # Key Features
//! - Regex-based Ruby DSL parsing for dependency declarations
//! - Support for git, path, and source dependencies
//! - Pod groups and target-specific dependencies
//! - Version constraint parsing (exact, ranges, pessimistic)
//! - Source URL extraction for custom pod repositories
//!
//! # Implementation Notes
//! - Uses regex for pattern matching (not full Ruby parser)
//! - Supports syntax: `pod 'Name', 'version'`, `pod 'Name', :git => 'url'`
//! - Local path dependencies (`:path =>`) are tracked as dependencies
//! - Graceful error handling with `warn!()` logs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::parser_warn as warn;
use packageurl::PackageUrl;
use regex::Regex;

use super::metadata::ParserMetadata;
use crate::models::{DatasourceId, Dependency, PackageData, PackageType};
use crate::parsers::PackageParser;
use crate::parsers::utils::{MAX_ITERATION_COUNT, read_file_to_string, truncate_field};

/// Parses CocoaPods Podfile dependency files.
///
/// Extracts dependency declarations from Podfile using regex-based Ruby DSL parsing.
///
/// # Supported Syntax
/// - `pod 'Name', 'version'` - Standard pod with version
/// - `pod 'Name'` - Pod without version constraint
/// - `pod 'Name', :git => 'url'` - Git dependency
/// - `pod 'Name', :path => '../LocalPod'` - Local path dependency
/// - `pod 'Firebase/Analytics'` - Subspecs
/// - Version operators: `~>`, `>=`, `<=`, etc.
pub struct PodfileParser;

impl PackageParser for PodfileParser {
    const PACKAGE_TYPE: PackageType = PackageType::Cocoapods;

    fn metadata() -> Vec<ParserMetadata> {
        vec![ParserMetadata {
            description: "CocoaPods Podfile",
            file_patterns: &["**/Podfile"],
            package_type: "cocoapods",
            primary_language: "Objective-C",
            documentation_url: Some("https://guides.cocoapods.org/using/the-podfile.html"),
        }]
    }

    fn is_match(path: &Path) -> bool {
        path.file_name().is_some_and(|name| name == "Podfile")
    }

    fn extract_packages(path: &Path) -> Vec<PackageData> {
        let content = match read_file_to_string(path, None) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read {:?}: {}", path, e);
                return vec![default_package_data()];
            }
        };

        let dependencies = extract_dependencies_with_context(&content, path.parent());

        vec![PackageData {
            package_type: Some(Self::PACKAGE_TYPE),
            namespace: None,
            name: None,
            version: None,
            qualifiers: None,
            subpath: None,
            primary_language: Some("Objective-C".to_string()),
            description: None,
            release_date: None,
            parties: Vec::new(),
            keywords: Vec::new(),
            homepage_url: None,
            download_url: None,
            size: None,
            sha1: None,
            md5: None,
            sha256: None,
            sha512: None,
            bug_tracking_url: None,
            code_view_url: None,
            vcs_url: None,
            copyright: None,
            holder: None,
            declared_license_expression: None,
            declared_license_expression_spdx: None,
            license_detections: Vec::new(),
            other_license_expression: None,
            other_license_expression_spdx: None,
            other_license_detections: Vec::new(),
            extracted_license_statement: None,
            notice_text: None,
            source_packages: Vec::new(),
            file_references: Vec::new(),
            extra_data: None,
            dependencies,
            repository_homepage_url: None,
            repository_download_url: None,
            api_data_url: None,
            datasource_id: Some(DatasourceId::CocoapodsPodfile),
            purl: None,
            is_private: false,
            is_virtual: false,
        }]
    }
}

fn default_package_data() -> PackageData {
    PackageData {
        package_type: Some(PodfileParser::PACKAGE_TYPE),
        primary_language: Some("Objective-C".to_string()),
        datasource_id: Some(DatasourceId::CocoapodsPodfile),
        ..Default::default()
    }
}

static POD_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*pod\s+['"]([^'"]+)['"](?:\s*,\s*(.+))?$"#).expect("valid regex")
});

static POD_HASH_LOOKUP_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*\[\s*['"]([^'"]+)['"]\s*\]"#).expect("valid regex")
});

static POD_QUOTED_VALUE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*['"]([^'"]+)['"]"#).expect("valid regex"));

static POD_OPTION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:^|,\s*):([A-Za-z_][A-Za-z0-9_]*)\s*=>\s*['"]([^'"]+)['"]"#)
        .expect("valid regex")
});

static REQUIRE_RELATIVE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*require_relative\s+['"]([^'"]+)['"]"#).expect("valid regex")
});

static HASH_ASSIGNMENT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?ms)^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\{(.*?)\}"#).expect("valid regex")
});

static HASH_ENTRY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"['"]([^'"]+)['"]\s*=>\s*['"]([^'"]+)['"]"#).expect("valid regex")
});

/// Extract dependencies from Podfile
#[cfg(test)]
fn extract_dependencies(content: &str) -> Vec<Dependency> {
    extract_dependencies_with_context(content, None)
}

fn extract_dependencies_with_context(content: &str, base_dir: Option<&Path>) -> Vec<Dependency> {
    let mut dependencies = Vec::new();
    let contexts = load_podfile_contexts(content, base_dir);
    let version_hashes = extract_podfile_hash_assignments(&contexts);

    for line in content.lines().take(MAX_ITERATION_COUNT) {
        let cleaned_line = pre_process(line);
        if let Some(caps) = POD_PATTERN.captures(&cleaned_line) {
            let name = truncate_field(caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string());
            let args = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let version_req = extract_pod_version_requirement(args, &version_hashes);
            let git_url = extract_pod_option(args, "git");
            let local_path = extract_pod_option(args, "path");

            if let Some(dep) = create_dependency(name, version_req, git_url, local_path) {
                dependencies.push(dep);
            }
        }
    }

    dependencies
}

/// Create a Dependency from parsed components
fn create_dependency(
    name: String,
    version_req: Option<String>,
    _git_url: Option<String>,
    _local_path: Option<String>,
) -> Option<Dependency> {
    if name.is_empty() {
        return None;
    }

    let purl = PackageUrl::new("cocoapods", &name).ok()?;

    let is_pinned = version_req
        .as_ref()
        .map(|v| !v.contains(&['~', '>', '<', '='][..]))
        .unwrap_or(false);

    Some(Dependency {
        purl: Some(truncate_field(purl.to_string())),
        extracted_requirement: version_req.map(truncate_field),
        scope: Some("dependencies".to_string()),
        is_runtime: None,
        is_optional: None,
        is_pinned: Some(is_pinned),
        is_direct: Some(true),
        resolved_package: None,
        extra_data: None,
    })
}

/// Pre-process a line by removing comments and trimming
fn pre_process(line: &str) -> String {
    let line = if let Some(comment_pos) = line.find('#') {
        &line[..comment_pos]
    } else {
        line
    };
    line.trim().to_string()
}

fn extract_pod_version_requirement(
    args: &str,
    version_hashes: &HashMap<String, HashMap<String, String>>,
) -> Option<String> {
    if args.is_empty() {
        return None;
    }

    if let Some(captures) = POD_QUOTED_VALUE_PATTERN.captures(args) {
        return captures
            .get(1)
            .map(|value| truncate_field(value.as_str().to_string()));
    }

    let captures = POD_HASH_LOOKUP_PATTERN.captures(args)?;
    let hash_name = captures.get(1)?.as_str();
    let key = captures.get(2)?.as_str();
    version_hashes
        .get(hash_name)
        .and_then(|entries| entries.get(key))
        .cloned()
        .map(truncate_field)
}

fn extract_pod_option(args: &str, key: &str) -> Option<String> {
    POD_OPTION_PATTERN.captures_iter(args).find_map(|captures| {
        (captures.get(1)?.as_str() == key)
            .then(|| {
                captures
                    .get(2)
                    .map(|value| truncate_field(value.as_str().to_string()))
            })
            .flatten()
    })
}

fn load_podfile_contexts(content: &str, base_dir: Option<&Path>) -> Vec<String> {
    let mut contexts = vec![content.to_string()];
    let Some(base_dir) = base_dir else {
        return contexts;
    };
    let Ok(allowed_root) = base_dir.canonicalize() else {
        return contexts;
    };

    for captures in REQUIRE_RELATIVE_PATTERN
        .captures_iter(content)
        .take(MAX_ITERATION_COUNT)
    {
        let Some(required) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        for candidate in candidate_require_relative_paths(base_dir, required) {
            let Ok(canonical_candidate) = candidate.canonicalize() else {
                continue;
            };
            if !canonical_candidate.starts_with(&allowed_root) {
                continue;
            }
            if let Ok(required_content) = read_file_to_string(&canonical_candidate, None) {
                contexts.push(required_content);
                break;
            }
        }
    }

    contexts
}

fn candidate_require_relative_paths(base_dir: &Path, required: &str) -> Vec<PathBuf> {
    let required = if required.ends_with(".rb") {
        required.to_string()
    } else {
        format!("{required}.rb")
    };
    vec![base_dir.join(required)]
}

fn extract_podfile_hash_assignments(
    contexts: &[String],
) -> HashMap<String, HashMap<String, String>> {
    let mut hashes = HashMap::new();

    for context in contexts.iter().take(MAX_ITERATION_COUNT) {
        for captures in HASH_ASSIGNMENT_PATTERN
            .captures_iter(context)
            .take(MAX_ITERATION_COUNT)
        {
            let Some(hash_name) = captures.get(1).map(|value| value.as_str().to_string()) else {
                continue;
            };
            let Some(body) = captures.get(2).map(|value| value.as_str()) else {
                continue;
            };

            let mut entries = HashMap::new();
            for entry in HASH_ENTRY_PATTERN
                .captures_iter(body)
                .take(MAX_ITERATION_COUNT)
            {
                let Some(key) = entry.get(1).map(|value| value.as_str().to_string()) else {
                    continue;
                };
                let Some(value) = entry.get(2).map(|value| value.as_str().to_string()) else {
                    continue;
                };
                entries.insert(key, value);
            }

            if !entries.is_empty() {
                hashes.insert(hash_name, entries);
            }
        }
    }

    hashes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_match() {
        assert!(PodfileParser::is_match(Path::new("Podfile")));
        assert!(PodfileParser::is_match(Path::new("project/Podfile")));
        assert!(!PodfileParser::is_match(Path::new("Podfile.lock")));
        assert!(!PodfileParser::is_match(Path::new("FooPodfile")));
        assert!(!PodfileParser::is_match(Path::new("config.podfile")));
        assert!(!PodfileParser::is_match(Path::new("MyLib.podspec")));
        assert!(!PodfileParser::is_match(Path::new("MyLib.podspec.json")));
    }

    #[test]
    fn test_extract_simple_pod() {
        let content = r#"
platform :ios, '9.0'

target 'MyApp' do
  pod 'AFNetworking', '~> 4.0'
  pod 'Alamofire'
end
"#;
        let deps = extract_dependencies(content);
        assert_eq!(deps.len(), 2);

        assert_eq!(deps[0].purl, Some("pkg:cocoapods/AFNetworking".to_string()));
        assert_eq!(deps[0].extracted_requirement, Some("~> 4.0".to_string()));
        assert_eq!(deps[0].is_pinned, Some(false));
        assert_eq!(deps[0].scope, Some("dependencies".to_string()));
        assert_eq!(deps[0].is_runtime, None);
        assert_eq!(deps[0].is_optional, None);

        assert_eq!(deps[1].purl, Some("pkg:cocoapods/Alamofire".to_string()));
        assert_eq!(deps[1].extracted_requirement, None);
    }

    #[test]
    fn test_extract_pod_with_git() {
        let content = r#"
pod 'AFNetworking', :git => 'https://github.com/AFNetworking/AFNetworking.git'
"#;
        let deps = extract_dependencies(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].purl, Some("pkg:cocoapods/AFNetworking".to_string()));
    }

    #[test]
    fn test_extract_pod_with_path() {
        let content = r#"
pod 'MyLocalPod', :path => '../MyLocalPod'
"#;
        let deps = extract_dependencies(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].purl, Some("pkg:cocoapods/MyLocalPod".to_string()));
    }

    #[test]
    fn test_extract_pod_with_version_and_git() {
        let content = r#"
pod 'RestKit', '~> 0.20', :git => 'https://github.com/RestKit/RestKit.git'
"#;
        let deps = extract_dependencies(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].purl, Some("pkg:cocoapods/RestKit".to_string()));
        assert_eq!(deps[0].extracted_requirement, Some("~> 0.20".to_string()));
    }

    #[test]
    fn test_ignores_comments() {
        let content = r#"
# pod 'Commented', '1.0'
pod 'Active', '2.0'  # inline comment
"#;
        let deps = extract_dependencies(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].purl, Some("pkg:cocoapods/Active".to_string()));
    }

    #[test]
    fn test_extract_pod_version_from_required_hash() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let version_file = temp_dir.path().join("PodVersions.rb");
        std::fs::write(
            &version_file,
            r#"
versions = {
  'Flipper' => '0.125.0',
}
"#,
        )
        .expect("write version helper");
        let podfile_path = temp_dir.path().join("Podfile");
        std::fs::write(
            &podfile_path,
            r#"
require_relative 'PodVersions'

target 'Example' do
  pod 'FlipperKit', versions['Flipper']
end
"#,
        )
        .expect("write podfile");

        let package_data = PodfileParser::extract_first_package(&podfile_path);
        assert_eq!(package_data.dependencies.len(), 1);
        assert_eq!(
            package_data.dependencies[0].purl.as_deref(),
            Some("pkg:cocoapods/FlipperKit")
        );
        assert_eq!(
            package_data.dependencies[0]
                .extracted_requirement
                .as_deref(),
            Some("0.125.0")
        );
        assert_eq!(package_data.dependencies[0].is_pinned, Some(true));
    }
}
