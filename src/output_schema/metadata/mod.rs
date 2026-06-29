// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputTypeDoc {
    pub type_name: &'static str,
    pub json_paths: &'static [&'static str],
    pub summary: &'static str,
    pub fields: &'static [OutputFieldDoc],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFieldDoc {
    pub json_name: &'static str,
    pub rust_field: &'static str,
    pub value_shape: &'static str,
    pub presence: &'static str,
    pub meaning: &'static str,
}

mod dependency;
mod document;
mod file_evidence;
mod file_info;
mod license;
mod package;
mod summary;

use dependency::{DEPENDENCY_FIELDS, RESOLVED_PACKAGE_FIELDS, TOP_LEVEL_DEPENDENCY_FIELDS};
use document::{
    EXTRA_DATA_FIELDS, HEADER_FIELDS, LICENSE_INDEX_PROVENANCE_FIELDS, OUTPUT_FIELDS,
    SYSTEM_ENVIRONMENT_FIELDS,
};
use file_evidence::{AUTHOR_FIELDS, COPYRIGHT_FIELDS, EMAIL_FIELDS, HOLDER_FIELDS, URL_FIELDS};
use file_info::FILE_INFO_FIELDS;
use license::{
    LICENSE_DETECTION_FIELDS, LICENSE_POLICY_ENTRY_FIELDS, LICENSE_REFERENCE_FIELDS,
    LICENSE_RULE_REFERENCE_FIELDS, MATCH_FIELDS, TOP_LEVEL_LICENSE_DETECTION_FIELDS,
};
use package::{FILE_REFERENCE_FIELDS, PACKAGE_DATA_FIELDS, PACKAGE_FIELDS, PARTY_FIELDS};
use summary::{
    FACET_TALLIES_FIELDS, LICENSE_CLARITY_SCORE_FIELDS, SUMMARY_FIELDS, TALLIES_FIELDS,
    TALLY_ENTRY_FIELDS,
};

const EMPTY_FIELDS: &[OutputFieldDoc] = &[];

const DOCUMENTED_TYPES: &[OutputTypeDoc] = &[
    OutputTypeDoc {
        type_name: "Output",
        json_paths: &["$"],
        summary: "Top-level ScanCode-compatible output object.",
        fields: OUTPUT_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputHeader",
        json_paths: &["$.headers[]"],
        summary: "Per-run metadata block for one scan invocation.",
        fields: HEADER_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputExtraData",
        json_paths: &["$.headers[].extra_data"],
        summary: "Scanner-owned counts and provenance metadata nested under a header block.",
        fields: EXTRA_DATA_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputSummary",
        json_paths: &["$.summary"],
        summary: "Optional codebase-level rollup emitted by summary/classification workflows.",
        fields: SUMMARY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputFileInfo",
        json_paths: &["$.files[]"],
        summary: "File or directory record on the main per-resource output surface.",
        fields: FILE_INFO_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputPackage",
        json_paths: &["$.packages[]"],
        summary: "Assembled top-level package record on the public output contract.",
        fields: PACKAGE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputPackageData",
        json_paths: &["$.files[].package_data[]"],
        summary: "Raw parser-emitted package record attached to a specific file.",
        fields: PACKAGE_DATA_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputDependency",
        json_paths: &["$.files[].package_data[].dependencies[]"],
        summary: "Raw dependency row preserved on parser-emitted package data.",
        fields: DEPENDENCY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputTopLevelDependency",
        json_paths: &["$.dependencies[]"],
        summary: "Hoisted top-level dependency record emitted after assembly.",
        fields: TOP_LEVEL_DEPENDENCY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputTopLevelLicenseDetection",
        json_paths: &["$.license_detections[]"],
        summary: "Grouped top-level license detection block across the scanned codebase.",
        fields: TOP_LEVEL_LICENSE_DETECTION_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputTallies",
        json_paths: &[
            "$.tallies",
            "$.tallies_of_key_files",
            "$.files[].tallies",
            "$.tallies_by_facet[].tallies",
        ],
        summary: "Tally block used on top-level, key-file, facet, and file-level tally surfaces.",
        fields: TALLIES_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputFacetTallies",
        json_paths: &["$.tallies_by_facet[]"],
        summary: "Facet-specific tally wrapper for one user-defined facet label.",
        fields: FACET_TALLIES_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputTallyEntry",
        json_paths: &[
            "$.summary.other_license_expressions[]",
            "$.summary.other_holders[]",
            "$.summary.other_languages[]",
            "$.tallies.*[]",
        ],
        summary: "Single tally bucket entry used throughout summary and tally outputs.",
        fields: TALLY_ENTRY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseClarityScore",
        json_paths: &["$.summary.license_clarity_score"],
        summary: "Structured license-clarity scoring payload on the summary surface.",
        fields: LICENSE_CLARITY_SCORE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputSystemEnvironment",
        json_paths: &["$.headers[].extra_data.system_environment"],
        summary: "Recorded environment metadata for the scan runtime.",
        fields: SYSTEM_ENVIRONMENT_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseIndexProvenance",
        json_paths: &["$.headers[].extra_data.license_index_provenance"],
        summary: "Provenance block for the effective license index used by the scan.",
        fields: LICENSE_INDEX_PROVENANCE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputParty",
        json_paths: &[
            "$.packages[].parties[]",
            "$.files[].package_data[].parties[]",
            "$.dependencies[].resolved_package.parties[]",
        ],
        summary: "Party record used on package and resolved-package surfaces.",
        fields: PARTY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputFileReference",
        json_paths: &[
            "$.files[].package_data[].file_references[]",
            "$.dependencies[].resolved_package.file_references[]",
        ],
        summary: "Referenced-file record used on package-related surfaces.",
        fields: FILE_REFERENCE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicensePolicyEntry",
        json_paths: &["$.files[].license_policy[]"],
        summary: "Policy decoration entry attached to file-level license-policy output.",
        fields: LICENSE_POLICY_ENTRY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputAuthor",
        json_paths: &["$.files[].authors[]"],
        summary: "File-level author evidence record.",
        fields: AUTHOR_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputCopyright",
        json_paths: &["$.files[].copyrights[]"],
        summary: "File-level copyright evidence record.",
        fields: COPYRIGHT_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputEmail",
        json_paths: &["$.files[].emails[]"],
        summary: "File-level email evidence record.",
        fields: EMAIL_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputHolder",
        json_paths: &["$.files[].holders[]"],
        summary: "File-level holder evidence record.",
        fields: HOLDER_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputURL",
        json_paths: &["$.files[].urls[]"],
        summary: "File-level URL evidence record.",
        fields: URL_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseDetection",
        json_paths: &[
            "$.files[].license_detections[]",
            "$.files[].package_data[].license_detections[]",
            "$.packages[].license_detections[]",
            "$.dependencies[].resolved_package.license_detections[]",
        ],
        summary: "Grouped license detection record used on file, package_data, package, and resolved-package surfaces.",
        fields: LICENSE_DETECTION_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputMatch",
        json_paths: &[
            "$.files[].license_clues[]",
            "$.files[].license_detections[].matches[]",
            "$.license_detections[].reference_matches[]",
        ],
        summary: "Match record used for clue output, grouped detections, and top-level representative references.",
        fields: MATCH_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseReference",
        json_paths: &["$.license_references[]"],
        summary: "Top-level license reference record describing one emitted license key and its reference metadata.",
        fields: LICENSE_REFERENCE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputLicenseRuleReference",
        json_paths: &["$.license_rule_references[]"],
        summary: "Top-level license-rule reference record describing one emitted rule and its reference metadata.",
        fields: LICENSE_RULE_REFERENCE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputResolvedPackage",
        json_paths: &[
            "$.dependencies[].resolved_package",
            "$.files[].package_data[].dependencies[].resolved_package",
        ],
        summary: "Resolved package payload nested under dependency rows.",
        fields: RESOLVED_PACKAGE_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputFileType",
        json_paths: &["$.files[].type"],
        summary: "Serialized file-type enum used by file records.",
        fields: EMPTY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputPackageType",
        json_paths: &["$.packages[].type", "$.files[].package_data[].type"],
        summary: "Serialized package-type string newtype used by package-related records.",
        fields: EMPTY_FIELDS,
    },
    OutputTypeDoc {
        type_name: "OutputDatasourceId",
        json_paths: &[
            "$.packages[].datasource_ids[]",
            "$.dependencies[].datasource_id",
            "$.files[].package_data[].datasource_id",
        ],
        summary: "Serialized datasource-id string newtype used by package and dependency records.",
        fields: EMPTY_FIELDS,
    },
];

pub fn documented_output_types() -> &'static [OutputTypeDoc] {
    DOCUMENTED_TYPES
}

#[cfg(test)]
mod tests {
    use super::documented_output_types;
    use std::collections::BTreeSet;

    use crate::output_schema::{
        OutputAuthor, OutputCopyright, OutputDatasourceId, OutputDependency, OutputEmail,
        OutputExtraData, OutputFileInfo, OutputFileReference, OutputFileType, OutputHeader,
        OutputHolder, OutputLicenseDetection, OutputLicenseIndexProvenance,
        OutputLicensePolicyEntry, OutputMatch, OutputPackage, OutputPackageData, OutputPackageType,
        OutputParty, OutputSystemEnvironment, OutputTallies, OutputTallyEntry,
        OutputTopLevelDependency, OutputTopLevelLicenseDetection, OutputURL,
    };
    use serde::Serialize;
    use serde_json::{Map, Value};

    fn metadata_field_names(type_name: &str) -> BTreeSet<&'static str> {
        documented_output_types()
            .iter()
            .find(|ty| ty.type_name == type_name)
            .unwrap_or_else(|| panic!("{} should be documented", type_name))
            .fields
            .iter()
            .map(|field| field.json_name)
            .collect()
    }

    fn serialized_object_keys<T: Serialize>(value: &T) -> BTreeSet<String> {
        serde_json::to_value(value)
            .expect("serialize")
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect()
    }

    fn sample_match() -> OutputMatch {
        OutputMatch {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            from_file: Some("src/lib.rs".to_string()),
            start_line: 1,
            end_line: 2,
            matcher: Some("1-hash".to_string()),
            score: 100.0,
            matched_length: Some(42),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: Some("mit_1.RULE".to_string()),
            rule_url: Some("https://example.invalid/rule".to_string()),
            matched_text: Some("Permission is hereby granted".to_string()),
            matched_text_diagnostics: Some("diagnostics".to_string()),
            referenced_filenames: Some(vec!["LICENSE".to_string()]),
        }
    }

    fn sample_license_detection() -> OutputLicenseDetection {
        OutputLicenseDetection {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            matches: vec![sample_match()],
            detection_log: vec!["normalized".to_string()],
            identifier: Some("det-1".to_string()),
        }
    }

    fn sample_tally_entry() -> OutputTallyEntry {
        OutputTallyEntry {
            value: Some("mit".to_string()),
            count: 1,
        }
    }

    fn sample_tallies() -> OutputTallies {
        OutputTallies {
            detected_license_expression: vec![sample_tally_entry()],
            copyrights: vec![sample_tally_entry()],
            holders: vec![sample_tally_entry()],
            authors: vec![sample_tally_entry()],
            programming_language: vec![sample_tally_entry()],
        }
    }

    fn sample_party() -> OutputParty {
        OutputParty {
            r#type: Some("person".to_string()),
            role: Some("author".to_string()),
            name: Some("Example Person".to_string()),
            email: Some("person@example.invalid".to_string()),
            url: Some("https://example.invalid/person".to_string()),
            organization: Some("Example Org".to_string()),
            organization_url: Some("https://example.invalid".to_string()),
            timezone: Some("UTC".to_string()),
        }
    }

    fn sample_file_reference() -> OutputFileReference {
        OutputFileReference {
            path: "LICENSE".to_string(),
            size: Some(123),
            sha1: Some("a".repeat(40)),
            md5: Some("b".repeat(32)),
            sha256: Some("c".repeat(64)),
            sha512: Some("d".repeat(128)),
            extra_data: Some(std::collections::HashMap::from_iter([(
                "hint".to_string(),
                Value::String("local".to_string()),
            )])),
        }
    }

    fn sample_license_policy_entry() -> OutputLicensePolicyEntry {
        OutputLicensePolicyEntry {
            license_key: "mit".to_string(),
            label: "Allowed".to_string(),
            color_code: "#00ff00".to_string(),
            icon: "check".to_string(),
        }
    }

    fn sample_author() -> OutputAuthor {
        OutputAuthor {
            author: "Example Author".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_copyright() -> OutputCopyright {
        OutputCopyright {
            copyright: "Copyright 2026 Example".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_email() -> OutputEmail {
        OutputEmail {
            email: "example@example.invalid".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_holder() -> OutputHolder {
        OutputHolder {
            holder: "Example Holder".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_url() -> OutputURL {
        OutputURL {
            url: "https://example.invalid".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    fn sample_package_data() -> OutputPackageData {
        OutputPackageData {
            package_type: Some(OutputPackageType::from(crate::models::PackageType::Cargo)),
            namespace: Some("example".to_string()),
            name: Some("crate-name".to_string()),
            version: Some("1.2.3".to_string()),
            qualifiers: Some(std::collections::HashMap::from_iter([(
                "arch".to_string(),
                "x86_64".to_string(),
            )])),
            subpath: Some("sub".to_string()),
            primary_language: Some("Rust".to_string()),
            description: Some("Example package data".to_string()),
            release_date: Some("2026-05-31".to_string()),
            parties: vec![sample_party()],
            keywords: vec!["example".to_string()],
            homepage_url: Some("https://example.invalid/home".to_string()),
            download_url: Some("https://example.invalid/download".to_string()),
            size: Some(42),
            sha1: Some("a".repeat(40)),
            md5: Some("b".repeat(32)),
            sha256: Some("c".repeat(64)),
            sha512: Some("d".repeat(128)),
            bug_tracking_url: Some("https://example.invalid/issues".to_string()),
            code_view_url: Some("https://example.invalid/code".to_string()),
            vcs_url: Some("git+https://example.invalid/repo.git".to_string()),
            copyright: Some("Copyright 2026 Example".to_string()),
            holder: Some("Example Holder".to_string()),
            declared_license_expression: Some("mit".to_string()),
            declared_license_expression_spdx: Some("MIT".to_string()),
            license_detections: vec![sample_license_detection()],
            other_license_expression: Some("apache-2.0".to_string()),
            other_license_expression_spdx: Some("Apache-2.0".to_string()),
            other_license_detections: vec![sample_license_detection()],
            extracted_license_statement: Some("MIT".to_string()),
            notice_text: Some("notice".to_string()),
            source_packages: vec!["pkg:cargo/source@1.0.0".to_string()],
            file_references: vec![sample_file_reference()],
            is_private: true,
            is_virtual: true,
            extra_data: Some(std::collections::HashMap::from_iter([(
                "custom".to_string(),
                Value::String("value".to_string()),
            )])),
            dependencies: vec![sample_dependency()],
            repository_homepage_url: Some("https://example.invalid/repo-home".to_string()),
            repository_download_url: Some("https://example.invalid/repo-download".to_string()),
            api_data_url: Some("https://example.invalid/api".to_string()),
            datasource_id: Some(OutputDatasourceId::from(
                crate::models::DatasourceId::CargoToml,
            )),
            purl: Some("pkg:cargo/example/crate-name@1.2.3".to_string()),
        }
    }

    fn sample_dependency() -> OutputDependency {
        OutputDependency {
            purl: Some("pkg:cargo/example/dep@1.0.0".to_string()),
            extracted_requirement: Some("^1.0".to_string()),
            scope: Some("runtime".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(true),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: Some(std::collections::HashMap::from_iter([(
                "kind".to_string(),
                Value::String("normal".to_string()),
            )])),
        }
    }

    fn sample_top_level_dependency() -> OutputTopLevelDependency {
        OutputTopLevelDependency {
            purl: Some("pkg:cargo/example/dep@1.0.0".to_string()),
            extracted_requirement: Some("^1.0".to_string()),
            scope: Some("runtime".to_string()),
            is_runtime: Some(true),
            is_optional: Some(false),
            is_pinned: Some(true),
            is_direct: Some(true),
            resolved_package: None,
            extra_data: Some(std::collections::HashMap::from_iter([(
                "kind".to_string(),
                Value::String("normal".to_string()),
            )])),
            dependency_uid: "dep-uid".to_string(),
            for_package_uid: Some("pkg-uid".to_string()),
            datafile_path: "Cargo.toml".to_string(),
            datasource_id: OutputDatasourceId::from(crate::models::DatasourceId::CargoToml),
            namespace: Some("example".to_string()),
        }
    }

    fn sample_top_level_license_detection() -> OutputTopLevelLicenseDetection {
        OutputTopLevelLicenseDetection {
            identifier: "top-1".to_string(),
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            detection_count: 1,
            detection_log: vec!["grouped".to_string()],
            reference_matches: vec![sample_match()],
        }
    }

    fn sample_header() -> OutputHeader {
        OutputHeader {
            tool_name: "provenant".to_string(),
            tool_version: "0.1.7".to_string(),
            options: Map::from_iter([("--license".to_string(), Value::Bool(true))]),
            notice: "Generated with Provenant".to_string(),
            start_timestamp: "2026-05-31T00:00:00Z".to_string(),
            end_timestamp: "2026-05-31T00:00:10Z".to_string(),
            output_format_version: "3.0.0".to_string(),
            duration: 10.0,
            errors: vec!["none".to_string()],
            warnings: vec!["warning".to_string()],
            extra_data: OutputExtraData {
                system_environment: OutputSystemEnvironment {
                    operating_system: "Linux".to_string(),
                    cpu_architecture: "x86_64".to_string(),
                    platform: "linux".to_string(),
                    platform_version: "6.0".to_string(),
                    rust_version: "1.88.0".to_string(),
                },
                spdx_license_list_version: "3.26".to_string(),
                files_count: 1,
                directories_count: 1,
                excluded_count: 0,
                license_index_provenance: Some(OutputLicenseIndexProvenance {
                    source: "embedded".to_string(),
                    dataset_fingerprint: "fingerprint".to_string(),
                    ignored_rules: vec!["rule-a".to_string()],
                    ignored_licenses: vec!["lic-a".to_string()],
                    ignored_rules_due_to_licenses: vec!["rule-b".to_string()],
                    added_rules: vec!["rule-c".to_string()],
                    replaced_rules: vec!["rule-d".to_string()],
                    added_licenses: vec!["lic-b".to_string()],
                    replaced_licenses: vec!["lic-c".to_string()],
                }),
            },
        }
    }

    fn sample_file_info() -> OutputFileInfo {
        OutputFileInfo {
            name: "mod.rs".to_string(),
            base_name: "mod".to_string(),
            extension: ".rs".to_string(),
            path: "src/mod.rs".to_string(),
            file_type: OutputFileType::File,
            mime_type: Some("text/rust".to_string()),
            file_type_label: Some("source".to_string()),
            size: 123,
            date: Some("2026-05-31".to_string()),
            sha1: Some("a".repeat(40)),
            md5: Some("b".repeat(32)),
            sha256: Some("c".repeat(64)),
            sha1_git: Some("d".repeat(40)),
            programming_language: Some("Rust".to_string()),
            package_data: vec![sample_package_data()],
            license_expression: Some("MIT".to_string()),
            license_expression_spdx: Some("MIT".to_string()),
            license_detections: vec![sample_license_detection()],
            license_clues: vec![sample_match()],
            percentage_of_license_text: Some(50.0),
            copyrights: vec![sample_copyright()],
            holders: vec![sample_holder()],
            authors: vec![sample_author()],
            emails: vec![sample_email()],
            urls: vec![sample_url()],
            for_packages: vec!["pkg-uid".to_string()],
            scan_errors: vec!["parse warning".to_string()],
            license_policy: Some(vec![sample_license_policy_entry()]),
            is_generated: Some(true),
            is_binary: Some(false),
            is_text: Some(true),
            is_archive: Some(false),
            is_media: Some(false),
            is_source: Some(true),
            is_script: Some(false),
            files_count: Some(1),
            dirs_count: Some(0),
            size_count: Some(123),
            source_count: Some(1),
            is_legal: true,
            is_manifest: true,
            is_readme: true,
            is_top_level: true,
            is_key_file: true,
            is_referenced: true,
            is_community: true,
            facets: vec!["core".to_string()],
            tallies: Some(sample_tallies()),
        }
    }

    fn sample_package() -> OutputPackage {
        OutputPackage {
            package_type: Some(OutputPackageType::from(crate::models::PackageType::Cargo)),
            namespace: Some("example".to_string()),
            name: Some("crate-name".to_string()),
            version: Some("1.2.3".to_string()),
            qualifiers: Some(std::collections::HashMap::from_iter([(
                "arch".to_string(),
                "x86_64".to_string(),
            )])),
            subpath: Some("sub".to_string()),
            primary_language: Some("Rust".to_string()),
            description: Some("Example package".to_string()),
            release_date: Some("2026-05-31".to_string()),
            parties: vec![sample_party()],
            keywords: vec!["example".to_string()],
            homepage_url: Some("https://example.invalid/home".to_string()),
            download_url: Some("https://example.invalid/download".to_string()),
            size: Some(42),
            sha1: Some("a".repeat(40)),
            md5: Some("b".repeat(32)),
            sha256: Some("c".repeat(64)),
            sha512: Some("d".repeat(128)),
            bug_tracking_url: Some("https://example.invalid/issues".to_string()),
            code_view_url: Some("https://example.invalid/code".to_string()),
            vcs_url: Some("git+https://example.invalid/repo.git".to_string()),
            copyright: Some("Copyright 2026 Example".to_string()),
            holder: Some("Example Holder".to_string()),
            declared_license_expression: Some("mit".to_string()),
            declared_license_expression_spdx: Some("MIT".to_string()),
            license_detections: vec![sample_license_detection()],
            other_license_expression: Some("apache-2.0".to_string()),
            other_license_expression_spdx: Some("Apache-2.0".to_string()),
            other_license_detections: vec![sample_license_detection()],
            extracted_license_statement: Some("MIT".to_string()),
            notice_text: Some("notice".to_string()),
            source_packages: vec!["pkg:cargo/source@1.0.0".to_string()],
            is_private: true,
            is_virtual: true,
            extra_data: Some(std::collections::HashMap::from_iter([(
                "custom".to_string(),
                Value::String("value".to_string()),
            )])),
            repository_homepage_url: Some("https://example.invalid/repo-home".to_string()),
            repository_download_url: Some("https://example.invalid/repo-download".to_string()),
            api_data_url: Some("https://example.invalid/api".to_string()),
            purl: Some("pkg:cargo/example/crate-name@1.2.3".to_string()),
            package_uid: "pkg-uid".to_string(),
            datafile_paths: vec!["Cargo.toml".to_string()],
            datasource_ids: vec![OutputDatasourceId::from(
                crate::models::DatasourceId::CargoToml,
            )],
        }
    }

    fn assert_metadata_matches_serialized_keys<T: Serialize>(type_name: &str, value: &T) {
        let documented = metadata_field_names(type_name);
        let serialized = serialized_object_keys(value)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let documented_owned = documented
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            documented_owned, serialized,
            "metadata mismatch for {}",
            type_name
        );
    }

    #[test]
    fn documented_type_names_are_unique() {
        let mut seen = BTreeSet::new();
        for ty in documented_output_types() {
            assert!(
                seen.insert(ty.type_name),
                "duplicate type doc: {}",
                ty.type_name
            );
        }
    }

    #[test]
    fn documented_json_paths_are_unique() {
        let mut seen = BTreeSet::new();
        for ty in documented_output_types() {
            for path in ty.json_paths {
                assert!(seen.insert(*path), "duplicate json path doc: {}", path);
            }
        }
    }

    #[test]
    fn documented_field_names_are_unique_per_type() {
        for ty in documented_output_types() {
            let mut seen = BTreeSet::new();
            for field in ty.fields {
                assert!(
                    seen.insert(field.json_name),
                    "duplicate field doc in {}: {}",
                    ty.type_name,
                    field.json_name
                );
            }
        }
    }

    #[test]
    fn output_file_info_doc_starts_with_public_serialization_order() {
        let file_info = documented_output_types()
            .iter()
            .find(|ty| ty.type_name == "OutputFileInfo")
            .expect("OutputFileInfo should be documented");
        let fields = file_info
            .fields
            .iter()
            .map(|field| field.json_name)
            .collect::<Vec<_>>();

        assert_eq!(
            &fields[..5],
            &["path", "type", "name", "base_name", "extension",]
        );
    }

    #[test]
    fn metadata_matches_serialized_keys_for_core_documented_types() {
        assert_metadata_matches_serialized_keys("OutputHeader", &sample_header());
        assert_metadata_matches_serialized_keys("OutputFileInfo", &sample_file_info());
        assert_metadata_matches_serialized_keys("OutputPackage", &sample_package());
        assert_metadata_matches_serialized_keys("OutputPackageData", &sample_package_data());
        assert_metadata_matches_serialized_keys("OutputDependency", &sample_dependency());
        assert_metadata_matches_serialized_keys(
            "OutputTopLevelDependency",
            &sample_top_level_dependency(),
        );
        assert_metadata_matches_serialized_keys(
            "OutputTopLevelLicenseDetection",
            &sample_top_level_license_detection(),
        );
    }
}
