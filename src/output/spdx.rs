// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{self, Write};
use std::path::Path;

use sha1::{Digest, Sha1};

use crate::license_detection::spdx_lid::is_spdx_exception;
use crate::output_schema::{
    Output, OutputFileInfo as FileInfo, OutputFileType, OutputMatch as Match, OutputPackage,
};
use crate::utils::time::{convert_header_timestamp_to_iso_utc, fallback_iso_utc_timestamp};

use super::sbom;
use super::shared::{sorted_files, xml_escape};
use super::spdx_plan::{
    SpdxPackagePlan, inventory_spdx_ids, plan_spdx_packages, primary_package_name,
    sanitize_spdx_package_name,
};
use super::{OutputWriteConfig, SPDX_DOCUMENT_NOTICE};

const DEPENDS_ON_RELATIONSHIP: &str = "http://spdx.org/rdf/terms#relationshipType_dependsOn";

const EMPTY_SHA1_HEX: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";

struct ExtractedLicenseInfo {
    license_id: String,
    name: String,
    extracted_text: String,
    comment: String,
}

pub(crate) fn write_spdx_tag_value(
    output: &Output,
    writer: &mut dyn Write,
    config: &OutputWriteConfig,
) -> io::Result<()> {
    let files = spdx_files(output);
    let inventory = sbom::build_inventory(output);
    let fallback_name = primary_package_name(output, config);
    // Promoted dependencies intentionally own no scanned files; do not treat
    // an empty file list as "no results" when the inventory still has packages.
    if files.is_empty() && inventory.is_empty() {
        writeln!(writer, "# No results for package '{}'.", fallback_name)?;
        return Ok(());
    }

    let plans = plan_spdx_packages(output, &files, config, &inventory);
    let entry_ids = inventory_spdx_ids(&inventory);
    let dependency_edges = sbom::dependency_edges(&inventory, &output.dependencies, &entry_ids);
    let document_namespace = document_namespace_for(output, config);
    let extracted_license_infos = spdx_extracted_license_infos(output, &files);
    let created = spdx_created_timestamp(output);
    let creator = spdx_creator();

    // Global file index → SPDXRef so CONTAINS relationships can point at files.
    let mut file_spdx_ids: HashMap<&str, String> = HashMap::new();
    for (file_index, file) in (1usize..).zip(files.iter()) {
        file_spdx_ids.insert(file.path.as_str(), format!("SPDXRef-{file_index}"));
    }

    writeln!(writer, "## Document Information")?;
    writeln!(writer, "SPDXVersion: SPDX-2.2")?;
    writeln!(writer, "DataLicense: CC0-1.0")?;
    writeln!(writer, "SPDXID: SPDXRef-DOCUMENT")?;
    writeln!(writer, "DocumentName: SPDX Document created by Provenant")?;
    writeln!(writer, "DocumentNamespace: {}", document_namespace)?;
    writeln!(
        writer,
        "DocumentComment: <text>{}</text>",
        sanitize_spdx_text_content(SPDX_DOCUMENT_NOTICE)
    )?;
    writeln!(writer, "## Creation Information")?;
    writeln!(writer, "Creator: {}", creator)?;
    writeln!(writer, "Created: {}", created)?;

    // Emit the scanned-subject packages (FilesAnalyzed: true) before the file
    // section. In SPDX tag-value, a file's package association is positional —
    // files bind to the most recently declared package — so promoted-dependency
    // packages (FilesAnalyzed: false, which own no scanned files) MUST come
    // after the file section, or the files would bind to a FilesAnalyzed: false
    // package and fail spec validation.
    for plan in plans.iter().filter(|plan| plan.role.files_analyzed()) {
        write_spdx_tag_value_package(writer, plan)?;
    }

    writeln!(writer, "## File Information")?;
    for (file_index, file) in (1usize..).zip(files.iter()) {
        let sha1 = file.sha1.as_deref().unwrap_or(EMPTY_SHA1_HEX);
        // Exception symbols are not licenses; keep them out of the id list, and
        // strip any floating (non-`WITH`) exception from the concluded expression.
        let file_license_info: Vec<String> = spdx_file_license_info(file)
            .into_iter()
            .filter(|id| !is_spdx_exception(id))
            .collect();
        let file_license_concluded = spdx_file_license_concluded(file)
            .and_then(|expression| strip_unattached_exceptions(&expression));
        writeln!(
            writer,
            "FileName: {}",
            spdx_relative_file_name(&file.path, config.scanned_path.as_deref())
        )?;
        writeln!(writer, "SPDXID: SPDXRef-{}", file_index)?;
        writeln!(writer, "FileChecksum: SHA1: {}", sha1)?;
        writeln!(
            writer,
            "LicenseConcluded: {}",
            spdx_tv_license_value(file_license_concluded.as_deref())
        )?;
        if file_license_info.is_empty() {
            writeln!(writer, "LicenseInfoInFile: NONE")?;
        } else {
            for license_id in file_license_info {
                writeln!(writer, "LicenseInfoInFile: {}", license_id)?;
            }
        }

        if file.copyrights.is_empty() {
            writeln!(writer, "FileCopyrightText: NONE")?;
        } else {
            let text = file
                .copyrights
                .iter()
                .map(|c| c.copyright.clone())
                .collect::<Vec<_>>()
                .join("\\n");
            writeln!(
                writer,
                "FileCopyrightText: {}",
                format_spdx_text_field(&text)
            )?;
        }

        writeln!(writer)?;
    }

    // Promoted-dependency packages (FilesAnalyzed: false) trail the file
    // section so no file positionally binds to them (see the subject-package
    // loop above).
    for plan in plans.iter().filter(|plan| !plan.role.files_analyzed()) {
        write_spdx_tag_value_package(writer, plan)?;
    }

    writeln!(writer, "## Relationships")?;
    for plan in &plans {
        // The document DESCRIBES the scanned subject packages; promoted
        // dependencies are attached to the graph via DEPENDS_ON below instead.
        if plan.role.is_described_by_document() {
            writeln!(
                writer,
                "Relationship: SPDXRef-DOCUMENT DESCRIBES {}",
                plan.spdx_id
            )?;
        }
        for file in &plan.files {
            if let Some(file_id) = file_spdx_ids.get(file.path.as_str()) {
                writeln!(
                    writer,
                    "Relationship: {} CONTAINS {}",
                    plan.spdx_id, file_id
                )?;
            }
        }
    }
    for (owner_id, dependency_ids) in &dependency_edges {
        for dependency_id in dependency_ids {
            writeln!(
                writer,
                "Relationship: {} DEPENDS_ON {}",
                owner_id, dependency_id
            )?;
        }
    }

    // Exception refs are stripped from every expression above, so drop their
    // now-unreferenced ExtractedLicensingInfo declarations too.
    let extracted_license_infos: Vec<ExtractedLicenseInfo> = extracted_license_infos
        .into_iter()
        .filter(|info| !is_spdx_exception(&info.license_id))
        .collect();
    if !extracted_license_infos.is_empty() {
        writeln!(writer, "## License Information")?;
        for info in extracted_license_infos {
            writeln!(writer, "LicenseID: {}", info.license_id)?;
            writeln!(
                writer,
                "ExtractedText: <text>{}",
                sanitize_spdx_text_content(&info.extracted_text)
            )?;
            writeln!(writer, "</text>")?;
            writeln!(writer, "LicenseName: {}", info.name)?;
            writeln!(
                writer,
                "LicenseComment: <text>{}",
                sanitize_spdx_text_content(&info.comment)
            )?;
            writeln!(writer, "</text>")?;
        }
    }

    Ok(())
}

/// Write one SPDX tag-value `## Package Information` block. Called for the
/// scanned-subject packages before the file section and the promoted-dependency
/// packages after it (see `write_spdx_tag_value`).
fn write_spdx_tag_value_package(
    writer: &mut dyn Write,
    plan: &SpdxPackagePlan<'_>,
) -> io::Result<()> {
    let package_copyright_text = spdx_package_copyright_text(&plan.files);
    let package_license_concluded = spdx_package_license_concluded(plan.package)
        .and_then(|expression| strip_unattached_exceptions(&expression));
    let package_license_declared = spdx_package_license_declared(plan.package)
        .and_then(|expression| strip_unattached_exceptions(&expression));

    writeln!(writer, "## Package Information")?;
    writeln!(writer, "PackageName: {}", plan.name)?;
    writeln!(writer, "SPDXID: {}", plan.spdx_id)?;
    writeln!(writer, "PackageDownloadLocation: NOASSERTION")?;
    writeln!(writer, "FilesAnalyzed: {}", plan.role.files_analyzed())?;
    // SPDX 2.2: a package verification code and per-file license info are
    // only meaningful (and only valid) when files were analyzed. A promoted
    // dependency owns no scanned files, so both are omitted for it.
    if plan.role.files_analyzed() {
        writeln!(
            writer,
            "PackageVerificationCode: {}",
            spdx_package_verification_code(&plan.files)
        )?;
    }
    writeln!(
        writer,
        "PackageLicenseConcluded: {}",
        spdx_tv_license_value(package_license_concluded.as_deref())
    )?;
    if plan.role.files_analyzed() {
        let package_license_info_from_files: Vec<String> =
            spdx_package_license_info_from_files(&plan.files)
                .into_iter()
                .filter(|id| !is_spdx_exception(id))
                .collect();
        for license_id in &package_license_info_from_files {
            writeln!(writer, "PackageLicenseInfoFromFiles: {}", license_id)?;
        }
        if package_license_info_from_files.is_empty() {
            writeln!(writer, "PackageLicenseInfoFromFiles: NONE")?;
        }
    }
    writeln!(
        writer,
        "PackageLicenseDeclared: {}",
        spdx_tv_license_value(package_license_declared.as_deref())
    )?;
    writeln!(
        writer,
        "PackageCopyrightText: {}",
        format_spdx_text_field(&package_copyright_text)
    )?;
    Ok(())
}

pub(crate) fn write_spdx_rdf_xml(
    output: &Output,
    writer: &mut dyn Write,
    config: &OutputWriteConfig,
) -> io::Result<()> {
    let fallback_name = primary_package_name(output, config);
    let files = spdx_files(output);
    let inventory = sbom::build_inventory(output);
    if files.is_empty() && inventory.is_empty() {
        writeln!(
            writer,
            "<!-- No results for package '{}'. -->",
            fallback_name
        )?;
        return Ok(());
    }

    let plans = plan_spdx_packages(output, &files, config, &inventory);
    let entry_ids = inventory_spdx_ids(&inventory);
    let dependency_edges = sbom::dependency_edges(&inventory, &output.dependencies, &entry_ids);
    let document_namespace = document_namespace_for(output, config);
    let document_namespace_xml = xml_escape(&document_namespace);
    let extracted_license_infos = spdx_extracted_license_infos(output, &files);
    let created = xml_escape(&spdx_created_timestamp(output));
    let creator = xml_escape(&spdx_creator());

    let mut file_spdx_ids: HashMap<&str, String> = HashMap::new();
    for (file_index, file) in (1usize..).zip(files.iter()) {
        file_spdx_ids.insert(file.path.as_str(), format!("SPDXRef-{file_index}"));
    }

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" xmlns:rdfs=\"http://www.w3.org/2000/01/rdf-schema#\" xmlns:spdx=\"http://spdx.org/rdf/terms#\">\n");

    for plan in &plans {
        let package_copyright_text = xml_escape(&spdx_package_copyright_text(&plan.files));
        let package_name = xml_escape(&plan.name);
        let package_license_concluded = spdx_package_license_concluded(plan.package)
            .and_then(|expression| strip_unattached_exceptions(&expression));
        let package_license_declared = spdx_package_license_declared(plan.package)
            .and_then(|expression| strip_unattached_exceptions(&expression));
        let files_analyzed = plan.role.files_analyzed();

        xml.push_str("  <spdx:Package rdf:about=\"");
        xml.push_str(&document_namespace_xml);
        xml.push('#');
        xml.push_str(&xml_escape(&plan.spdx_id));
        xml.push_str("\">\n");
        xml.push_str(
            "    <spdx:filesAnalyzed rdf:datatype=\"http://www.w3.org/2001/XMLSchema#boolean\">",
        );
        xml.push_str(if files_analyzed { "true" } else { "false" });
        xml.push_str("</spdx:filesAnalyzed>\n");
        xml.push_str(
            "    <spdx:downloadLocation rdf:resource=\"http://spdx.org/rdf/terms#noassertion\"/>\n",
        );
        xml.push_str("    <spdx:licenseConcluded rdf:resource=\"");
        xml.push_str(&xml_escape(&spdx_rdf_license_resource(
            package_license_concluded.as_deref(),
            &document_namespace,
        )));
        xml.push_str("\"/>\n");
        xml.push_str("    <spdx:licenseDeclared rdf:resource=\"");
        xml.push_str(&xml_escape(&spdx_rdf_license_resource(
            package_license_declared.as_deref(),
            &document_namespace,
        )));
        xml.push_str("\"/>\n");
        // Per-file license info and a verification code are only valid when
        // files were analyzed; a promoted dependency owns no files (ADR 0012).
        if files_analyzed {
            let package_license_info_from_files: Vec<String> =
                spdx_package_license_info_from_files(&plan.files)
                    .into_iter()
                    .filter(|id| !is_spdx_exception(id))
                    .collect();
            if package_license_info_from_files.is_empty() {
                xml.push_str(
                    "    <spdx:licenseInfoFromFiles rdf:resource=\"http://spdx.org/rdf/terms#none\"/>\n",
                );
            } else {
                for license_id in &package_license_info_from_files {
                    xml.push_str("    <spdx:licenseInfoFromFiles rdf:resource=\"");
                    xml.push_str(&xml_escape(&spdx_license_rdf_resource(
                        license_id,
                        &document_namespace,
                    )));
                    xml.push_str("\"/>\n");
                }
            }
            xml.push_str("    <spdx:packageVerificationCode><spdx:PackageVerificationCode><spdx:packageVerificationCodeValue>");
            xml.push_str(&spdx_package_verification_code(&plan.files));
            xml.push_str("</spdx:packageVerificationCodeValue></spdx:PackageVerificationCode></spdx:packageVerificationCode>\n");
        }

        // DEPENDS_ON relationships to the packages this one resolves to.
        if let Some(dependency_ids) = dependency_edges.get(&plan.spdx_id) {
            for dependency_id in dependency_ids {
                xml.push_str("    <spdx:relationship><spdx:Relationship>");
                xml.push_str(&format!(
                    "<spdx:relationshipType rdf:resource=\"{}\"/>",
                    DEPENDS_ON_RELATIONSHIP
                ));
                xml.push_str("<spdx:relatedSpdxElement rdf:resource=\"");
                xml.push_str(&document_namespace_xml);
                xml.push('#');
                xml.push_str(&xml_escape(dependency_id));
                xml.push_str("\"/>");
                xml.push_str("</spdx:Relationship></spdx:relationship>\n");
            }
        }

        for file in &plan.files {
            let Some(file_id) = file_spdx_ids.get(file.path.as_str()) else {
                continue;
            };
            let file_license_info: Vec<String> = spdx_file_license_info(file)
                .into_iter()
                .filter(|id| !is_spdx_exception(id))
                .collect();
            let file_license_concluded = spdx_file_license_concluded(file)
                .and_then(|expression| strip_unattached_exceptions(&expression));
            xml.push_str("    <spdx:relationship><spdx:Relationship>");
            xml.push_str("<spdx:relationshipType rdf:resource=\"http://spdx.org/rdf/terms#relationshipType_contains\"/>");
            xml.push_str("<spdx:relatedSpdxElement><spdx:File rdf:about=\"");
            xml.push_str(&document_namespace_xml);
            xml.push('#');
            xml.push_str(&xml_escape(file_id));
            xml.push_str("\">");
            xml.push_str("<spdx:licenseConcluded rdf:resource=\"");
            xml.push_str(&xml_escape(&spdx_rdf_license_resource(
                file_license_concluded.as_deref(),
                &document_namespace,
            )));
            xml.push_str("\"/>");
            if file_license_info.is_empty() {
                xml.push_str(
                    "<spdx:licenseInfoInFile rdf:resource=\"http://spdx.org/rdf/terms#none\"/>",
                );
            } else {
                for license_id in file_license_info {
                    xml.push_str("<spdx:licenseInfoInFile rdf:resource=\"");
                    xml.push_str(&xml_escape(&spdx_license_rdf_resource(
                        &license_id,
                        &document_namespace,
                    )));
                    xml.push_str("\"/>");
                }
            }
            xml.push_str("<spdx:checksum><spdx:Checksum><spdx:algorithm rdf:resource=\"http://spdx.org/rdf/terms#checksumAlgorithm_sha1\"/>");
            xml.push_str("<spdx:checksumValue>");
            xml.push_str(&xml_escape(file.sha1.as_deref().unwrap_or(EMPTY_SHA1_HEX)));
            xml.push_str("</spdx:checksumValue></spdx:Checksum></spdx:checksum>");
            xml.push_str("<spdx:fileName>");
            xml.push_str(&xml_escape(&spdx_relative_file_name(
                &file.path,
                config.scanned_path.as_deref(),
            )));
            xml.push_str("</spdx:fileName>");
            xml.push_str("<spdx:copyrightText>");
            if file.copyrights.is_empty() {
                xml.push_str("NONE");
            } else {
                xml.push_str(&xml_escape(
                    &file
                        .copyrights
                        .iter()
                        .map(|c| c.copyright.clone())
                        .collect::<Vec<_>>()
                        .join("\\n"),
                ));
            }
            xml.push_str("</spdx:copyrightText>");
            xml.push_str(
                "</spdx:File></spdx:relatedSpdxElement></spdx:Relationship></spdx:relationship>\n",
            );
        }

        xml.push_str("    <spdx:copyrightText>");
        xml.push_str(&package_copyright_text);
        xml.push_str("</spdx:copyrightText>\n");
        xml.push_str("    <spdx:name>");
        xml.push_str(&package_name);
        xml.push_str("</spdx:name>\n");
        xml.push_str("  </spdx:Package>\n");
    }

    // spdx-tools expects SpdxDocument rdf:about = documentNamespace + "#SPDXRef-DOCUMENT".
    xml.push_str("  <spdx:SpdxDocument rdf:about=\"");
    xml.push_str(&document_namespace_xml);
    xml.push_str("#SPDXRef-DOCUMENT\">\n");
    xml.push_str("    <spdx:dataLicense rdf:resource=\"http://spdx.org/licenses/CC0-1.0\"/>\n");
    xml.push_str("    <rdfs:comment>");
    xml.push_str(&xml_escape(SPDX_DOCUMENT_NOTICE));
    xml.push_str("</rdfs:comment>\n");
    for info in extracted_license_infos
        .into_iter()
        .filter(|info| !is_spdx_exception(&info.license_id))
    {
        xml.push_str(
            "    <spdx:hasExtractedLicensingInfo><spdx:ExtractedLicensingInfo rdf:about=\"",
        );
        xml.push_str(&document_namespace_xml);
        xml.push('#');
        xml.push_str(&xml_escape(&info.license_id));
        xml.push_str("\">");
        xml.push_str("<spdx:licenseId>");
        xml.push_str(&xml_escape(&info.license_id));
        xml.push_str("</spdx:licenseId>");
        xml.push_str("<spdx:name>");
        xml.push_str(&xml_escape(&info.name));
        xml.push_str("</spdx:name>");
        xml.push_str("<rdfs:comment>");
        xml.push_str(&xml_escape(&info.comment));
        xml.push_str("</rdfs:comment>");
        xml.push_str("<spdx:extractedText>");
        xml.push_str(&xml_escape(&info.extracted_text));
        xml.push_str("</spdx:extractedText>");
        xml.push_str("</spdx:ExtractedLicensingInfo></spdx:hasExtractedLicensingInfo>\n");
    }
    xml.push_str("    <spdx:name>SPDX Document created by Provenant</spdx:name>\n");
    xml.push_str("    <spdx:specVersion>SPDX-2.2</spdx:specVersion>\n");
    xml.push_str("    <spdx:creationInfo><spdx:CreationInfo>");
    xml.push_str("<spdx:creator>");
    xml.push_str(&creator);
    xml.push_str("</spdx:creator>");
    xml.push_str("<spdx:created>");
    xml.push_str(&created);
    xml.push_str("</spdx:created>");
    xml.push_str("</spdx:CreationInfo></spdx:creationInfo>\n");
    for plan in &plans {
        // The document DESCRIBES the scanned subject packages; promoted
        // dependencies are reached via DEPENDS_ON on their owning package.
        if !plan.role.is_described_by_document() {
            continue;
        }
        xml.push_str("    <spdx:relationship><spdx:Relationship>");
        xml.push_str(
            "<spdx:relationshipType rdf:resource=\"http://spdx.org/rdf/terms#relationshipType_describes\"/>",
        );
        xml.push_str("<spdx:relatedSpdxElement rdf:resource=\"");
        xml.push_str(&document_namespace_xml);
        xml.push('#');
        xml.push_str(&xml_escape(&plan.spdx_id));
        xml.push_str("\"/>");
        xml.push_str("</spdx:Relationship></spdx:relationship>\n");
    }
    xml.push_str("  </spdx:SpdxDocument>\n");

    xml.push_str("</rdf:RDF>\n");
    writer.write_all(xml.as_bytes())
}

fn spdx_files(output: &Output) -> Vec<&FileInfo> {
    sorted_files(&output.files)
        .into_iter()
        .filter(|f| f.file_type == OutputFileType::File)
        .collect()
}

fn spdx_package_verification_code(files: &[&FileInfo]) -> String {
    let mut file_sha1s = files
        .iter()
        .map(|f| f.sha1.as_deref().unwrap_or(EMPTY_SHA1_HEX).to_string())
        .collect::<Vec<_>>();
    file_sha1s.sort_unstable();

    let mut hasher = Sha1::new();
    for sha1_hex in file_sha1s {
        hasher.update(sha1_hex.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn spdx_file_license_info(file: &FileInfo) -> Vec<String> {
    let mut license_ids = BTreeSet::new();

    for detection in file.license_detections.iter().chain(
        file.package_data
            .iter()
            .flat_map(|package_data| package_data.license_detections.iter())
            .chain(
                file.package_data
                    .iter()
                    .flat_map(|package_data| package_data.other_license_detections.iter()),
            ),
    ) {
        if detection.matches.is_empty() {
            license_ids.extend(spdx_ids_from_expression(&detection.license_expression_spdx));
            continue;
        }

        for detection_match in &detection.matches {
            let expression = if detection_match.license_expression_spdx.is_empty() {
                &detection.license_expression_spdx
            } else {
                &detection_match.license_expression_spdx
            };
            license_ids.extend(spdx_ids_from_expression(expression));
        }
    }

    license_ids.into_iter().collect()
}

fn spdx_package_license_info_from_files(files: &[&FileInfo]) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for file in files {
        for license_id in spdx_file_license_info(file) {
            unique.insert(license_id);
        }
    }
    unique.into_iter().collect()
}

/// The package's own declared SPDX expression, straight from parser/assembly
/// -normalized manifest data. `None` (rendered as NOASSERTION) when the
/// package declares nothing we could resolve to a valid SPDX expression; this
/// never falls back to detected evidence, since "declared" specifically means
/// what the package's producer stated.
fn spdx_package_license_declared(package: Option<&OutputPackage>) -> Option<String> {
    package
        .and_then(|pkg| pkg.declared_license_expression_spdx.as_deref())
        .and_then(spdx_validated_expression)
}

/// The best honest SPDX conclusion for the package: the declared expression
/// when present (the single most authoritative source we have), otherwise
/// `other_license_expression_spdx` — license text assembly detected in the
/// package's own files that was not folded into the declared expression (see
/// `reference_following.rs`). Falls back to `None` (NOASSERTION) rather than
/// inventing a conclusion from evidence the package itself doesn't carry.
fn spdx_package_license_concluded(package: Option<&OutputPackage>) -> Option<String> {
    package.and_then(|pkg| {
        pkg.declared_license_expression_spdx
            .as_deref()
            .and_then(spdx_validated_expression)
            .or_else(|| {
                pkg.other_license_expression_spdx
                    .as_deref()
                    .and_then(spdx_validated_expression)
            })
    })
}

/// Re-parse an expression through the same strict SPDX combiner the file
/// conclusion path uses, so malformed package fields (newlines, bad tokens)
/// become `None` / NOASSERTION instead of broken tag-value output.
fn spdx_validated_expression(expression: &str) -> Option<String> {
    if expression.is_empty() {
        return None;
    }
    crate::utils::spdx::combine_license_expressions_preserving_structure_strict([
        expression.to_string()
    ])
}

/// The best honest SPDX conclusion for a file: reuses the same three-tier
/// fallback the JSON `detected_license_expression_spdx` field already applies
/// (own detections, then owning package-data detections, then the carried
/// expression). `None` (rendered as NOASSERTION) when nothing was detected.
fn spdx_file_license_concluded(file: &FileInfo) -> Option<String> {
    file.detected_license_expression_spdx()
}

/// Renders an already-resolved SPDX expression for a tag-value field, or the
/// NOASSERTION placeholder when nothing is known.
fn spdx_tv_license_value(expression: Option<&str>) -> &str {
    expression.unwrap_or("NOASSERTION")
}

/// Renders an already-resolved SPDX expression as an RDF resource. Only a
/// single license id (no `AND`/`OR`/`WITH` operators) can be expressed as the
/// simple `rdf:resource` link this writer uses elsewhere (see
/// `spdx_license_rdf_resource`); a compound expression would need a nested
/// `ConjunctiveLicenseSet`/`DisjunctiveLicenseSet` structure this writer does
/// not build, so it honestly falls back to NOASSERTION in RDF only (the
/// tag-value form above keeps the full expression, since tag-value license
/// fields accept SPDX expression syntax directly).
fn spdx_rdf_license_resource(expression: Option<&str>, document_namespace: &str) -> String {
    match expression.map(spdx_ids_from_expression) {
        Some(ids) if ids.len() == 1 => spdx_license_rdf_resource(&ids[0], document_namespace),
        _ => "http://spdx.org/rdf/terms#noassertion".to_string(),
    }
}

fn spdx_package_copyright_text(files: &[&FileInfo]) -> String {
    let copyrights: BTreeSet<String> = files
        .iter()
        .flat_map(|file| file.copyrights.iter())
        .map(|copyright| copyright.copyright.clone())
        .collect();

    if copyrights.is_empty() {
        "NONE".to_string()
    } else {
        copyrights.into_iter().collect::<Vec<_>>().join("\n")
    }
}

fn spdx_extracted_license_infos(output: &Output, files: &[&FileInfo]) -> Vec<ExtractedLicenseInfo> {
    let license_reference_names: HashMap<&str, &str> = output
        .license_references
        .iter()
        .map(|reference| (reference.spdx_license_key.as_str(), reference.name.as_str()))
        .collect();
    let mut seen = HashSet::new();
    let mut infos = Vec::new();

    for file in files {
        for detection in file.license_detections.iter().chain(
            file.package_data
                .iter()
                .flat_map(|package_data| package_data.license_detections.iter())
                .chain(
                    file.package_data
                        .iter()
                        .flat_map(|package_data| package_data.other_license_detections.iter()),
                ),
        ) {
            for detection_match in &detection.matches {
                let expression = if detection_match.license_expression_spdx.is_empty() {
                    &detection.license_expression_spdx
                } else {
                    &detection_match.license_expression_spdx
                };

                for license_id in spdx_ids_from_expression(expression) {
                    if !license_id.starts_with("LicenseRef-") || !seen.insert(license_id.clone()) {
                        continue;
                    }

                    let comment = spdx_license_comment(detection_match);
                    let extracted_text = detection_match
                        .matched_text
                        .clone()
                        .filter(|text| !text.is_empty())
                        .unwrap_or_else(|| comment.clone());
                    let name = license_reference_names
                        .get(license_id.as_str())
                        .copied()
                        .unwrap_or(license_id.as_str())
                        .to_string();

                    infos.push(ExtractedLicenseInfo {
                        license_id,
                        name,
                        extracted_text,
                        comment,
                    });
                }
            }
        }
    }

    infos
}

fn spdx_license_comment(detection_match: &Match) -> String {
    if let Some(rule_url) = detection_match.rule_url.as_deref()
        && !rule_url.is_empty()
    {
        format!("See details at {}", rule_url)
    } else {
        detection_match
            .matched_text
            .clone()
            .unwrap_or_else(|| "NOASSERTION".to_string())
    }
}

/// Listed SPDX licenses use the canonical `spdx.org/licenses/` URI; document-
/// scoped `LicenseRef-*` ids point at the matching `ExtractedLicensingInfo`
/// node (`{documentNamespace}#{LicenseRef-…}`) instead of a non-existent
/// listed-license URL.
fn spdx_license_rdf_resource(license_id: &str, document_namespace: &str) -> String {
    if license_id.starts_with("LicenseRef-") {
        format!("{document_namespace}#{license_id}")
    } else {
        format!("http://spdx.org/licenses/{license_id}")
    }
}

/// Split an SPDX expression into the individual *license* identifiers it names,
/// for the id-list fields (`LicenseInfoInFile`, `PackageLicenseInfoFromFiles`)
/// and single-id RDF resolution. The token immediately after `WITH` is a
/// license *exception*, not a license id; it is only valid inside a `WITH`
/// statement, so it is dropped here (the full `… WITH …` expression is still
/// rendered verbatim in the `LicenseConcluded`/`LicenseDeclared` fields).
fn spdx_ids_from_expression(expression: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut token = String::new();
    let mut expect_exception = false;

    let flush = |token: &mut String, ids: &mut Vec<String>, expect_exception: &mut bool| {
        if token.is_empty() {
            return;
        }
        match token.as_str() {
            "AND" | "OR" => {}
            "WITH" => *expect_exception = true,
            // The symbol following `WITH` is an exception; consume it without
            // emitting it as a standalone license id.
            _ if *expect_exception => *expect_exception = false,
            _ => ids.push(token.clone()),
        }
        token.clear();
    };

    for ch in expression.chars() {
        // Keep underscores so LicenseRef- and SPDX ids that use them stay intact.
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '+' | '_') {
            token.push(ch);
        } else {
            flush(&mut token, &mut ids, &mut expect_exception);
        }
    }
    flush(&mut token, &mut ids, &mut expect_exception);

    ids
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Split an SPDX expression into its top-level operands and the `AND`/`OR`
/// operators between them, respecting parenthesis depth (a `WITH` stays inside
/// its operand).
fn split_top_level_operands(expression: &str) -> (Vec<String>, Vec<String>) {
    let mut operands = Vec::new();
    let mut operators = Vec::new();
    let mut depth: i32 = 0;
    let mut current = String::new();
    for token in expression.split_whitespace() {
        if depth == 0 && (token == "AND" || token == "OR") {
            operands.push(current.trim().to_string());
            operators.push(token.to_string());
            current.clear();
            continue;
        }
        depth += token.matches('(').count() as i32 - token.matches(')').count() as i32;
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(token);
    }
    operands.push(current.trim().to_string());
    (operands, operators)
}

/// Remove exception operands that are not attached to a license via `WITH`.
/// SPDX cannot represent a standalone or `AND`/`OR`-joined exception, so a
/// detected floating exception (e.g. ScanCode's
/// `LicenseRef-scancode-generic-exception`) is dropped from the expression,
/// while a valid `license WITH exception` operand is kept intact. Returns
/// `None` when nothing valid remains.
fn strip_unattached_exceptions(expression: &str) -> Option<String> {
    let (operands, operators) = split_top_level_operands(expression);
    let cleaned: Vec<Option<String>> = operands
        .iter()
        .map(|operand| clean_spdx_operand(operand))
        .collect();

    let mut result = String::new();
    for (index, operand) in cleaned.iter().enumerate() {
        let Some(text) = operand else { continue };
        if !result.is_empty() {
            let operator = operators
                .get(index.wrapping_sub(1))
                .map(String::as_str)
                .unwrap_or("AND");
            result.push_str(&format!(" {operator} "));
        }
        result.push_str(text);
    }
    non_empty(&result)
}

/// Clean a single top-level operand: recurse into a parenthesized group, drop a
/// bare exception symbol, or keep the operand (including a `license WITH
/// exception`) unchanged.
fn clean_spdx_operand(operand: &str) -> Option<String> {
    let operand = operand.trim();
    if let Some(inner) = operand.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        return strip_unattached_exceptions(inner.trim()).map(|cleaned| format!("({cleaned})"));
    }
    // A bare, unattached exception (single token, no `WITH`) cannot stand in an
    // SPDX expression, so drop it. `license WITH exception` contains a space and
    // is kept.
    if !operand.contains(' ') && is_spdx_exception(operand) {
        return None;
    }
    Some(operand.to_string())
}

fn spdx_creator() -> String {
    format!("Tool: Provenant-{}", crate::version::BUILD_VERSION)
}

fn spdx_created_timestamp(output: &Output) -> String {
    output
        .headers
        .first()
        .and_then(|h| convert_header_timestamp_to_iso_utc(&h.start_timestamp))
        .unwrap_or_else(|| fallback_iso_utc_timestamp().to_string())
}

fn sanitize_spdx_text_content(value: &str) -> String {
    // SPDX tag-value `<text>` blocks end at the first `</text>`; neutralize
    // embedded closers so source-derived copyright/license text cannot break
    // the document. Fullwidth brackets keep the content readable.
    value
        .replace("</text>", "＜/text＞")
        .replace("</TEXT>", "＜/TEXT＞")
}

fn format_spdx_text_field(value: &str) -> String {
    let sanitized = sanitize_spdx_text_content(value);
    if sanitized.contains('\n') {
        format!("<text>{sanitized}</text>")
    } else {
        sanitized
    }
}

fn spdx_relative_file_name(path: &str, scanned_path: Option<&str>) -> String {
    // Normalize both sides the same way so a CLI root like `./proj` still
    // strips against collected paths that lose the leading `./`.
    let path = Path::new(trim_spdx_dot_slash(path));
    let relative = match scanned_path {
        Some(root) => match path.strip_prefix(Path::new(trim_spdx_dot_slash(root))) {
            Ok(stripped) => stripped.to_string_lossy().into_owned(),
            Err(_) => path.to_string_lossy().into_owned(),
        },
        None => path.to_string_lossy().into_owned(),
    };

    if relative.is_empty() {
        "./.".to_string()
    } else if let Some(stripped) = relative.strip_prefix('/') {
        // Absolute path that did not strip against scanned_path: keep one slash.
        format!("./{stripped}")
    } else {
        format!("./{relative}")
    }
}

fn trim_spdx_dot_slash(path: &str) -> &str {
    path.trim_start_matches("./")
}

fn document_namespace_for(output: &Output, config: &OutputWriteConfig) -> String {
    let base = if output.packages.len() > 1 {
        config
            .scanned_path
            .as_deref()
            .and_then(|p| Path::new(p).file_name().and_then(|n| n.to_str()))
            .map(sanitize_spdx_package_name)
            .unwrap_or_else(|| "provenant-scan".to_string())
    } else {
        primary_package_name(output, config)
    };
    format!("http://spdx.org/spdxdocs/{base}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_detection::MatcherKind;
    use crate::models::{
        FileType, LicenseDetection, LineNumber, MatchScore, PackageData, PackageType,
    };

    #[test]
    fn spdx_relative_file_name_uses_path_prefix_not_string_prefix() {
        // Path::strip_prefix fails for /tmp/proj vs /tmp/project/..., so the
        // original path is preserved (with ./ prefix), not truncated to ect/...
        assert_eq!(
            spdx_relative_file_name("/tmp/project/src.rs", Some("/tmp/proj")),
            "./tmp/project/src.rs"
        );
        assert_eq!(
            spdx_relative_file_name("/tmp/proj/src.rs", Some("/tmp/proj")),
            "./src.rs"
        );
    }

    #[test]
    fn spdx_relative_file_name_strips_matching_dot_slash_roots() {
        assert_eq!(
            spdx_relative_file_name("./proj/src.rs", Some("./proj")),
            "./src.rs"
        );
        assert_eq!(
            spdx_relative_file_name("proj/src.rs", Some("./proj")),
            "./src.rs"
        );
    }

    #[test]
    fn format_spdx_text_field_neutralizes_embedded_text_closers() {
        let rendered = format_spdx_text_field("before</text>after\nline2");
        assert!(rendered.starts_with("<text>"));
        assert!(rendered.ends_with("</text>"));
        assert!(!rendered.contains("before</text>after"));
        assert!(rendered.contains("before＜/text＞after"));
    }

    #[test]
    fn plan_spdx_packages_emits_one_package_per_assembled_package() {
        let mut file_a = crate::models::FileInfo::new(
            "a/mix.exs".to_string(),
            "mix.exs".to_string(),
            String::new(),
            "a/mix.exs".to_string(),
            FileType::File,
            None,
            None,
            1,
            None,
            None,
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
        file_a.for_packages = vec![crate::models::PackageUid::from_raw("uid-a".to_string())];
        let mut file_b = crate::models::FileInfo::new(
            "b/mix.exs".to_string(),
            "mix.exs".to_string(),
            String::new(),
            "b/mix.exs".to_string(),
            FileType::File,
            None,
            None,
            1,
            None,
            None,
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
        file_b.for_packages = vec![crate::models::PackageUid::from_raw("uid-b".to_string())];
        let file_orphan = crate::models::FileInfo::new(
            "README".to_string(),
            "README".to_string(),
            String::new(),
            "README".to_string(),
            FileType::File,
            None,
            None,
            1,
            None,
            None,
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

        let pkg_a = {
            let mut pkg = crate::models::Package::from_package_data(
                &PackageData {
                    package_type: Some(PackageType::Hex),
                    name: Some("a".to_string()),
                    version: Some("0.1.0".to_string()),
                    purl: Some("pkg:hex/a@0.1.0".to_string()),
                    ..Default::default()
                },
                "a/mix.exs".to_string(),
            );
            pkg.package_uid = crate::models::PackageUid::from_raw("uid-a".to_string());
            pkg
        };
        let pkg_b = {
            let mut pkg = crate::models::Package::from_package_data(
                &PackageData {
                    package_type: Some(PackageType::Hex),
                    name: Some("b".to_string()),
                    version: Some("0.1.0".to_string()),
                    purl: Some("pkg:hex/b@0.1.0".to_string()),
                    ..Default::default()
                },
                "b/mix.exs".to_string(),
            );
            pkg.package_uid = crate::models::PackageUid::from_raw("uid-b".to_string());
            pkg
        };

        let output = crate::models::Output {
            summary: None,
            tallies: None,
            tallies_of_key_files: None,
            tallies_by_facet: None,
            headers: vec![],
            packages: vec![pkg_a, pkg_b],
            dependencies: vec![],
            license_detections: vec![],
            files: vec![file_a, file_b, file_orphan],
            license_references: vec![],
            license_rule_references: vec![],
        };
        let schema = Output::from(&output);
        let files = spdx_files(&schema);
        let inventory = sbom::build_inventory(&schema);
        let plans = plan_spdx_packages(
            &schema,
            &files,
            &OutputWriteConfig {
                format: crate::output::OutputFormat::SpdxTv,
                custom_template: None,
                scanned_path: Some("umbrella".to_string()),
            },
            &inventory,
        );
        assert_eq!(plans.len(), 3);
        assert_eq!(plans[0].spdx_id, "SPDXRef-Package-1");
        assert_eq!(plans[0].name, "a");
        assert_eq!(plans[0].files.len(), 1);
        assert_eq!(plans[1].spdx_id, "SPDXRef-Package-2");
        assert_eq!(plans[2].spdx_id, "SPDXRef-Package-unassigned");
        assert_eq!(plans[2].files.len(), 1);
    }

    #[test]
    fn spdx_file_license_info_includes_manifest_package_data_detections() {
        let mut file = crate::models::FileInfo::new(
            "Cargo.toml".to_string(),
            "Cargo".to_string(),
            ".toml".to_string(),
            "project/Cargo.toml".to_string(),
            FileType::File,
            None,
            None,
            1,
            None,
            None,
            None,
            None,
            None,
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        file.package_data = vec![PackageData {
            package_type: Some(PackageType::Cargo),
            license_detections: vec![LicenseDetection {
                license_expression: "mit".to_string(),
                license_expression_spdx: "MIT".to_string(),
                matches: vec![crate::models::Match {
                    license_expression: "mit".to_string(),
                    license_expression_spdx: "MIT".to_string(),
                    from_file: Some("project/Cargo.toml".to_string()),
                    start_line: LineNumber::ONE,
                    end_line: LineNumber::ONE,
                    matcher: MatcherKind::Declared,
                    score: MatchScore::MAX,
                    matched_length: Some(1),
                    match_coverage: Some(100.0),
                    rule_relevance: Some(100),
                    rule_identifier: String::new(),
                    rule_url: None,
                    matched_text: Some("MIT".to_string()),
                    referenced_filenames: Some(vec!["LICENSE".to_string()]),
                    matched_text_diagnostics: None,
                }],
                detection_log: vec!["unknown-reference-to-local-file".to_string()],
                identifier: String::new(),
            }],
            ..Default::default()
        }];

        let schema_file = crate::output_schema::OutputFileInfo::from(&file);
        assert_eq!(
            spdx_file_license_info(&schema_file),
            vec!["MIT".to_string()]
        );
    }

    fn output_package_with(package_data: PackageData) -> crate::output_schema::OutputPackage {
        let package =
            crate::models::Package::from_package_data(&package_data, "package.json".to_string());
        crate::output_schema::OutputPackage::from(&package)
    }

    #[test]
    fn spdx_package_license_declared_uses_only_the_declared_spdx_expression() {
        let declared = output_package_with(PackageData {
            package_type: Some(PackageType::Npm),
            declared_license_expression_spdx: Some("MIT".to_string()),
            other_license_expression_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        });
        assert_eq!(
            spdx_package_license_declared(Some(&declared)),
            Some("MIT".to_string())
        );

        // Never falls back to detected-but-undeclared evidence, and no package
        // (the synthetic/unassigned SPDX plans) means nothing was declared.
        let undeclared = output_package_with(PackageData {
            package_type: Some(PackageType::Npm),
            other_license_expression_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        });
        assert_eq!(spdx_package_license_declared(Some(&undeclared)), None);
        assert_eq!(spdx_package_license_declared(None), None);
    }

    #[test]
    fn spdx_package_license_helpers_reject_invalid_expressions() {
        let malformed = output_package_with(PackageData {
            package_type: Some(PackageType::Npm),
            declared_license_expression_spdx: Some("MIT\" or malformed".to_string()),
            other_license_expression_spdx: Some("not a@@license".to_string()),
            ..Default::default()
        });
        assert_eq!(spdx_package_license_declared(Some(&malformed)), None);
        assert_eq!(spdx_package_license_concluded(Some(&malformed)), None);

        // Invalid declared must not poison a valid other-license fallback.
        let declared_bad_other_ok = output_package_with(PackageData {
            package_type: Some(PackageType::Npm),
            declared_license_expression_spdx: Some("MIT\" or malformed".to_string()),
            other_license_expression_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        });
        assert_eq!(
            spdx_package_license_concluded(Some(&declared_bad_other_ok)),
            Some("Apache-2.0".to_string())
        );
    }

    #[test]
    fn spdx_license_rdf_resource_uses_document_namespace_for_license_refs() {
        let ns = "http://spdx.org/spdxdocs/demo";
        assert_eq!(
            spdx_license_rdf_resource("MIT", ns),
            "http://spdx.org/licenses/MIT"
        );
        assert_eq!(
            spdx_license_rdf_resource("LicenseRef-Custom", ns),
            "http://spdx.org/spdxdocs/demo#LicenseRef-Custom"
        );
        assert_eq!(
            spdx_rdf_license_resource(Some("LicenseRef-Custom"), ns),
            "http://spdx.org/spdxdocs/demo#LicenseRef-Custom"
        );
    }

    #[test]
    fn spdx_package_license_concluded_prefers_declared_then_falls_back_to_other() {
        let declared = output_package_with(PackageData {
            package_type: Some(PackageType::Npm),
            declared_license_expression_spdx: Some("MIT".to_string()),
            other_license_expression_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        });
        assert_eq!(
            spdx_package_license_concluded(Some(&declared)),
            Some("MIT".to_string())
        );

        let other_only = output_package_with(PackageData {
            package_type: Some(PackageType::Npm),
            other_license_expression_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        });
        assert_eq!(
            spdx_package_license_concluded(Some(&other_only)),
            Some("Apache-2.0".to_string())
        );

        let unknown = output_package_with(PackageData {
            package_type: Some(PackageType::Npm),
            ..Default::default()
        });
        assert_eq!(spdx_package_license_concluded(Some(&unknown)), None);
        assert_eq!(spdx_package_license_concluded(None), None);
    }

    #[test]
    fn spdx_file_license_concluded_reuses_the_detected_spdx_expression() {
        let mut file = crate::models::FileInfo::new(
            "notice.c".to_string(),
            "notice".to_string(),
            ".c".to_string(),
            "project/notice.c".to_string(),
            FileType::File,
            None,
            None,
            1,
            None,
            None,
            None,
            None,
            None,
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        file.license_detections = vec![LicenseDetection {
            license_expression: "mit".to_string(),
            license_expression_spdx: "MIT".to_string(),
            matches: vec![],
            detection_log: vec![],
            identifier: String::new(),
        }];

        let schema_file = crate::output_schema::OutputFileInfo::from(&file);
        assert_eq!(
            spdx_file_license_concluded(&schema_file),
            Some("MIT".to_string())
        );

        let mut undetected = file;
        undetected.license_detections = vec![];
        let schema_undetected = crate::output_schema::OutputFileInfo::from(&undetected);
        assert_eq!(spdx_file_license_concluded(&schema_undetected), None);
    }

    #[test]
    fn spdx_ids_from_expression_drops_the_with_exception_symbol() {
        // The exception after WITH is not a standalone license id (SPDX rejects
        // it in LicenseInfoInFile), but the licenses around it are kept.
        assert_eq!(
            spdx_ids_from_expression("Apache-2.0 WITH LLVM-exception"),
            vec!["Apache-2.0".to_string()]
        );
        assert_eq!(
            spdx_ids_from_expression("(GPL-2.0-only WITH Classpath-exception-2.0) OR MIT"),
            vec!["GPL-2.0-only".to_string(), "MIT".to_string()]
        );
        // Plain expressions are unaffected.
        assert_eq!(
            spdx_ids_from_expression("MIT AND Apache-2.0"),
            vec!["MIT".to_string(), "Apache-2.0".to_string()]
        );
    }

    #[test]
    fn strip_unattached_exceptions_drops_floating_exceptions_but_keeps_with() {
        // Floating exception joined with AND is dropped; the licenses stay.
        assert_eq!(
            strip_unattached_exceptions(
                "MIT AND Unlicense AND LicenseRef-scancode-generic-exception"
            ),
            Some("MIT AND Unlicense".to_string())
        );
        // A standalone exception leaves nothing valid.
        assert_eq!(
            strip_unattached_exceptions("LicenseRef-scancode-generic-exception"),
            None
        );
        // A validly-attached `WITH` exception is preserved, at top level and in
        // a parenthesized group.
        assert_eq!(
            strip_unattached_exceptions("Apache-2.0 WITH LLVM-exception"),
            Some("Apache-2.0 WITH LLVM-exception".to_string())
        );
        assert_eq!(
            strip_unattached_exceptions("(GPL-2.0-only WITH Classpath-exception-2.0) OR MIT"),
            Some("(GPL-2.0-only WITH Classpath-exception-2.0) OR MIT".to_string())
        );
        // Plain expressions are untouched.
        assert_eq!(
            strip_unattached_exceptions("MIT OR Apache-2.0"),
            Some("MIT OR Apache-2.0".to_string())
        );
    }

    #[test]
    fn spdx_rdf_license_resource_only_resolves_a_single_license_id() {
        let ns = "http://spdx.org/spdxdocs/demo";
        assert_eq!(
            spdx_rdf_license_resource(Some("MIT"), ns),
            "http://spdx.org/licenses/MIT"
        );
        // A compound expression would need a nested RDF license-set structure
        // this writer doesn't build, so it honestly reports NOASSERTION.
        assert_eq!(
            spdx_rdf_license_resource(Some("MIT AND Apache-2.0"), ns),
            "http://spdx.org/rdf/terms#noassertion"
        );
        assert_eq!(
            spdx_rdf_license_resource(None, ns),
            "http://spdx.org/rdf/terms#noassertion"
        );
    }
}
