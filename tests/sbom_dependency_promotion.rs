// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Renderer-level coverage for ADR 0012: promote resolved dependencies into
//! CycloneDX / SPDX inventory with honest licenses and no dangling edges.

use provenant::models::{
    DatasourceId, DependencyUid, ExtraData, FileInfo, FileType, Header, Output, Package,
    PackageData, PackageType, PackageUid, ResolvedPackage, Sha1Digest, SystemEnvironment,
    TopLevelDependency,
};
use provenant::output_schema::Output as OutputSchemaOutput;
use provenant::{OutputFormat, OutputWriteConfig, OutputWriter, writer_for_format};
use serde_json::Value;

const EMPTY_SHA1: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";

/// A root package with two resolved dependencies: `licensed` declares MIT and
/// is required; `bare` is optional with no resolved metadata.
fn sample_promotion_output(with_manifest_file: bool) -> Output {
    let root_uid = PackageUid::from_raw(
        "pkg:npm/root@1.0.0?uuid=00000000-0000-0000-0000-000000000000".to_string(),
    );
    let root_pkg = {
        let mut pkg = Package::from_package_data(
            &PackageData {
                package_type: Some(PackageType::Npm),
                name: Some("root".to_string()),
                version: Some("1.0.0".to_string()),
                purl: Some("pkg:npm/root@1.0.0".to_string()),
                ..Default::default()
            },
            "scan/package.json".to_string(),
        );
        pkg.package_uid = root_uid.clone();
        pkg
    };

    let files = if with_manifest_file {
        let mut manifest = FileInfo::new(
            "package.json".to_string(),
            "package".to_string(),
            ".json".to_string(),
            "scan/package.json".to_string(),
            FileType::File,
            Some("text/plain".to_string()),
            None,
            20,
            None,
            Some(Sha1Digest::from_hex(EMPTY_SHA1).unwrap()),
            None,
            None,
            None,
            vec![],
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
        );
        manifest.for_packages = vec![root_uid.clone()];
        vec![manifest]
    } else {
        vec![]
    };

    let licensed_resolved = {
        let mut rp = ResolvedPackage::new(
            PackageType::Npm,
            String::new(),
            "licensed".to_string(),
            "1.0.0".to_string(),
        );
        rp.purl = Some("pkg:npm/licensed@1.0.0".to_string());
        rp.declared_license_expression = Some("mit".to_string());
        rp.declared_license_expression_spdx = Some("MIT".to_string());
        rp
    };
    let licensed_dep = TopLevelDependency {
        purl: Some("pkg:npm/licensed@1.0.0".to_string()),
        extracted_requirement: Some("^1.0.0".to_string()),
        scope: Some("dependencies".to_string()),
        is_runtime: Some(true),
        is_optional: Some(false),
        is_pinned: Some(true),
        is_direct: Some(true),
        resolved_package: Some(Box::new(licensed_resolved)),
        extra_data: None,
        dependency_uid: DependencyUid::from_raw(
            "pkg:npm/licensed@1.0.0?uuid=00000000-0000-0000-0000-000000000001".to_string(),
        ),
        for_package_uid: Some(root_uid.clone()),
        datafile_path: "scan/package-lock.json".to_string(),
        datasource_id: DatasourceId::NpmPackageLockJson,
        namespace: None,
    };

    let bare_dep = TopLevelDependency {
        purl: Some("pkg:npm/bare@3.0.0".to_string()),
        extracted_requirement: Some("^3.0.0".to_string()),
        scope: Some("optionalDependencies".to_string()),
        is_runtime: Some(true),
        is_optional: Some(true),
        is_pinned: Some(true),
        is_direct: Some(true),
        resolved_package: None,
        extra_data: None,
        dependency_uid: DependencyUid::from_raw(
            "pkg:npm/bare@3.0.0?uuid=00000000-0000-0000-0000-000000000002".to_string(),
        ),
        for_package_uid: Some(root_uid),
        datafile_path: "scan/package-lock.json".to_string(),
        datasource_id: DatasourceId::NpmPackageLockJson,
        namespace: None,
    };

    Output {
        summary: None,
        tallies: None,
        tallies_of_key_files: None,
        tallies_by_facet: None,
        headers: vec![Header {
            tool_name: "provenant".to_string(),
            tool_version: provenant::version::BUILD_VERSION.to_string(),
            options: serde_json::Map::new(),
            notice: provenant::models::HEADER_NOTICE.to_string(),
            start_timestamp: "2026-01-01T000000.000000".to_string(),
            end_timestamp: "2026-01-01T000001.000000".to_string(),
            output_format_version: "4.1.0".to_string(),
            duration: 1.0,
            errors: vec![],
            warnings: vec![],
            extra_data: ExtraData {
                system_environment: SystemEnvironment {
                    operating_system: "linux".to_string(),
                    cpu_architecture: "64".to_string(),
                    platform: "linux".to_string(),
                    platform_version: "unknown".to_string(),
                    rust_version: "1.93.0".to_string(),
                },
                spdx_license_list_version: "3.27".to_string(),
                files_count: files.len(),
                directories_count: 0,
                excluded_count: 0,
                license_index_provenance: None,
            },
        }],
        packages: vec![root_pkg],
        dependencies: vec![licensed_dep, bare_dep],
        license_detections: vec![],
        files,
        license_references: vec![],
        license_rule_references: vec![],
    }
}

fn render(format: OutputFormat, output: &Output) -> Vec<u8> {
    let mut bytes = Vec::new();
    let schema_output = OutputSchemaOutput::from(output);
    writer_for_format(format)
        .write(
            &schema_output,
            &mut bytes,
            &OutputWriteConfig {
                format,
                custom_template: None,
                scanned_path: Some("scan".to_string()),
            },
        )
        .expect("SBOM output should be generated");
    bytes
}

#[test]
fn cyclonedx_promotes_resolved_dependencies_with_honest_licenses_and_scope() {
    let bom: Value = serde_json::from_slice(&render(
        OutputFormat::CycloneDxJson,
        &sample_promotion_output(true),
    ))
    .expect("cyclonedx json should parse");

    let components = bom["components"].as_array().expect("components array");
    let by_ref = |bom_ref: &str| {
        components
            .iter()
            .find(|c| c["bom-ref"] == bom_ref)
            .unwrap_or_else(|| panic!("component {bom_ref} should be present"))
    };

    let licensed = by_ref("pkg:npm/licensed@1.0.0");
    assert_eq!(licensed["name"], "licensed");
    assert_eq!(licensed["version"], "1.0.0");
    assert_eq!(licensed["licenses"][0]["expression"], "MIT");
    assert_eq!(licensed["scope"], "required");

    let bare = by_ref("pkg:npm/bare@3.0.0");
    assert_eq!(bare["name"], "bare");
    assert_eq!(bare["version"], "3.0.0");
    assert!(
        bare.get("licenses").is_none(),
        "a dependency with no known license must not carry one"
    );
    assert_eq!(bare["scope"], "optional");

    let component_refs: std::collections::BTreeSet<&str> = components
        .iter()
        .map(|c| c["bom-ref"].as_str().expect("bom-ref"))
        .collect();
    for dependency in bom["dependencies"].as_array().expect("dependencies array") {
        for target in dependency["dependsOn"].as_array().expect("dependsOn array") {
            let target = target.as_str().expect("dependsOn ref");
            assert!(
                component_refs.contains(target),
                "dangling dependsOn ref: {target}"
            );
        }
    }
}

#[test]
fn spdx_tv_promotes_resolved_dependencies_as_packages_with_depends_on() {
    let rendered = String::from_utf8(render(OutputFormat::SpdxTv, &sample_promotion_output(true)))
        .expect("spdx tv should be utf-8");

    assert!(rendered.contains("PackageName: licensed"));
    assert!(rendered.contains("PackageName: bare"));
    assert!(rendered.contains("SPDXID: SPDXRef-Package-Dependency-1"));
    assert!(rendered.contains("SPDXID: SPDXRef-Package-Dependency-2"));

    // Assert FilesAnalyzed: false on the dependency packages themselves, not
    // merely somewhere in the document.
    for package_name in ["licensed", "bare"] {
        let marker = format!("PackageName: {package_name}");
        let start = rendered
            .find(&marker)
            .unwrap_or_else(|| panic!("missing package {package_name}"));
        let block = &rendered[start..];
        let end = block.find("\n## ").unwrap_or(block.len());
        let block = &block[..end];
        assert!(
            block.contains("FilesAnalyzed: false"),
            "{package_name} must report FilesAnalyzed: false"
        );
    }

    assert!(rendered.contains("PackageLicenseDeclared: MIT"));
    assert!(
        rendered
            .contains("Relationship: SPDXRef-Package-1 DEPENDS_ON SPDXRef-Package-Dependency-1")
    );
    assert!(
        rendered
            .contains("Relationship: SPDXRef-Package-1 DEPENDS_ON SPDXRef-Package-Dependency-2")
    );
    assert!(!rendered.contains("DESCRIBES SPDXRef-Package-Dependency-1"));
}

#[test]
fn spdx_tv_emits_promoted_dependencies_when_output_has_no_file_records() {
    // Greptile P1 / ADR 0012: promoted packages use FilesAnalyzed: false, so an
    // empty file list must not early-return before building the inventory.
    let rendered = String::from_utf8(render(
        OutputFormat::SpdxTv,
        &sample_promotion_output(false),
    ))
    .expect("spdx tv should be utf-8");

    assert!(
        !rendered.contains("# No results for package"),
        "dependency-bearing output must not use the empty-scan sentinel"
    );
    assert!(rendered.contains("PackageName: licensed"));
    assert!(rendered.contains("PackageName: bare"));
    assert!(
        rendered
            .contains("Relationship: SPDXRef-Package-1 DEPENDS_ON SPDXRef-Package-Dependency-1")
    );
}
