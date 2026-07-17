// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};

use crate::output_schema::{
    Output, OutputFileInfo as FileInfo, OutputFileType, OutputMatch as Match, OutputPackage,
};
use crate::utils::time::{convert_header_timestamp_to_iso_utc, fallback_iso_utc_timestamp};

use super::shared::{sorted_files, xml_escape};
use super::{OutputWriteConfig, SPDX_DOCUMENT_NOTICE};

const EMPTY_SHA1_HEX: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";

struct ExtractedLicenseInfo {
    license_id: String,
    name: String,
    extracted_text: String,
    comment: String,
}

/// One SPDX Package to emit, with the files it owns.
struct SpdxPackagePlan<'a> {
    spdx_id: String,
    name: String,
    files: Vec<&'a FileInfo>,
}

pub(crate) fn write_spdx_tag_value(
    output: &Output,
    writer: &mut dyn Write,
    config: &OutputWriteConfig,
) -> io::Result<()> {
    let files = spdx_files(output);
    let fallback_name = primary_package_name(output, config);
    if files.is_empty() {
        writeln!(writer, "# No results for package '{}'.", fallback_name)?;
        return Ok(());
    }

    let plans = plan_spdx_packages(output, &files, config);
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

    for plan in &plans {
        let package_verification_code = spdx_package_verification_code(&plan.files);
        let package_license_info_from_files = spdx_package_license_info_from_files(&plan.files);
        let package_copyright_text = spdx_package_copyright_text(&plan.files);

        writeln!(writer, "## Package Information")?;
        writeln!(writer, "PackageName: {}", plan.name)?;
        writeln!(writer, "SPDXID: {}", plan.spdx_id)?;
        writeln!(writer, "PackageDownloadLocation: NOASSERTION")?;
        writeln!(writer, "FilesAnalyzed: true")?;
        writeln!(
            writer,
            "PackageVerificationCode: {}",
            package_verification_code
        )?;
        writeln!(writer, "PackageLicenseConcluded: NOASSERTION")?;
        for license_id in &package_license_info_from_files {
            writeln!(writer, "PackageLicenseInfoFromFiles: {}", license_id)?;
        }
        if package_license_info_from_files.is_empty() {
            writeln!(writer, "PackageLicenseInfoFromFiles: NONE")?;
        }
        writeln!(writer, "PackageLicenseDeclared: NOASSERTION")?;
        writeln!(
            writer,
            "PackageCopyrightText: {}",
            format_spdx_text_field(&package_copyright_text)
        )?;
    }

    writeln!(writer, "## File Information")?;
    for (file_index, file) in (1usize..).zip(files.iter()) {
        let sha1 = file.sha1.as_deref().unwrap_or(EMPTY_SHA1_HEX);
        let file_license_info = spdx_file_license_info(file);
        writeln!(
            writer,
            "FileName: {}",
            spdx_relative_file_name(&file.path, config.scanned_path.as_deref())
        )?;
        writeln!(writer, "SPDXID: SPDXRef-{}", file_index)?;
        writeln!(writer, "FileChecksum: SHA1: {}", sha1)?;
        writeln!(writer, "LicenseConcluded: NOASSERTION")?;
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

    writeln!(writer, "## Relationships")?;
    for plan in &plans {
        writeln!(
            writer,
            "Relationship: SPDXRef-DOCUMENT DESCRIBES {}",
            plan.spdx_id
        )?;
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

pub(crate) fn write_spdx_rdf_xml(
    output: &Output,
    writer: &mut dyn Write,
    config: &OutputWriteConfig,
) -> io::Result<()> {
    let fallback_name = primary_package_name(output, config);
    let files = spdx_files(output);
    if files.is_empty() {
        writeln!(
            writer,
            "<!-- No results for package '{}'. -->",
            fallback_name
        )?;
        return Ok(());
    }

    let plans = plan_spdx_packages(output, &files, config);
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
        let package_verification_code = spdx_package_verification_code(&plan.files);
        let package_license_info_from_files = spdx_package_license_info_from_files(&plan.files);
        let package_copyright_text = xml_escape(&spdx_package_copyright_text(&plan.files));
        let package_name = xml_escape(&plan.name);

        xml.push_str("  <spdx:Package rdf:about=\"");
        xml.push_str(&document_namespace_xml);
        xml.push('#');
        xml.push_str(&xml_escape(&plan.spdx_id));
        xml.push_str("\">\n");
        xml.push_str("    <spdx:filesAnalyzed rdf:datatype=\"http://www.w3.org/2001/XMLSchema#boolean\">true</spdx:filesAnalyzed>\n");
        xml.push_str(
            "    <spdx:downloadLocation rdf:resource=\"http://spdx.org/rdf/terms#noassertion\"/>\n",
        );
        xml.push_str(
            "    <spdx:licenseConcluded rdf:resource=\"http://spdx.org/rdf/terms#noassertion\"/>\n",
        );
        xml.push_str(
            "    <spdx:licenseDeclared rdf:resource=\"http://spdx.org/rdf/terms#noassertion\"/>\n",
        );
        if package_license_info_from_files.is_empty() {
            xml.push_str(
                "    <spdx:licenseInfoFromFiles rdf:resource=\"http://spdx.org/rdf/terms#none\"/>\n",
            );
        } else {
            for license_id in &package_license_info_from_files {
                xml.push_str("    <spdx:licenseInfoFromFiles rdf:resource=\"");
                xml.push_str(&xml_escape(&spdx_license_rdf_resource(license_id)));
                xml.push_str("\"/>\n");
            }
        }
        xml.push_str("    <spdx:packageVerificationCode><spdx:PackageVerificationCode><spdx:packageVerificationCodeValue>");
        xml.push_str(&package_verification_code);
        xml.push_str("</spdx:packageVerificationCodeValue></spdx:PackageVerificationCode></spdx:packageVerificationCode>\n");

        for file in &plan.files {
            let Some(file_id) = file_spdx_ids.get(file.path.as_str()) else {
                continue;
            };
            let file_license_info = spdx_file_license_info(file);
            xml.push_str("    <spdx:relationship><spdx:Relationship>");
            xml.push_str("<spdx:relationshipType rdf:resource=\"http://spdx.org/rdf/terms#relationshipType_contains\"/>");
            xml.push_str("<spdx:relatedSpdxElement><spdx:File rdf:about=\"");
            xml.push_str(&document_namespace_xml);
            xml.push('#');
            xml.push_str(&xml_escape(file_id));
            xml.push_str("\">");
            xml.push_str(
                "<spdx:licenseConcluded rdf:resource=\"http://spdx.org/rdf/terms#noassertion\"/>",
            );
            if file_license_info.is_empty() {
                xml.push_str(
                    "<spdx:licenseInfoInFile rdf:resource=\"http://spdx.org/rdf/terms#none\"/>",
                );
            } else {
                for license_id in file_license_info {
                    xml.push_str("<spdx:licenseInfoInFile rdf:resource=\"");
                    xml.push_str(&xml_escape(&spdx_license_rdf_resource(&license_id)));
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
    for info in extracted_license_infos {
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

fn primary_package_name(output: &Output, config: &OutputWriteConfig) -> String {
    if output.packages.len() == 1
        && let Some(name) = output.packages.first().and_then(|p| p.name.clone())
    {
        return sanitize_spdx_package_name(&name);
    }

    if let Some(scanned_path) = &config.scanned_path {
        let path = PathBuf::from(scanned_path);
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && !name.is_empty()
        {
            return sanitize_spdx_package_name(name);
        }
    }

    output
        .packages
        .first()
        .and_then(|p| p.name.clone())
        .map(|name| sanitize_spdx_package_name(&name))
        .unwrap_or_else(|| "provenant-analyzed-package".to_string())
}

fn sanitize_spdx_package_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "provenant-analyzed-package".to_string()
    } else {
        out
    }
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

fn spdx_license_rdf_resource(license_id: &str) -> String {
    format!("http://spdx.org/licenses/{}", license_id)
}

fn spdx_ids_from_expression(expression: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut token = String::new();

    let flush = |token: &mut String, ids: &mut Vec<String>| {
        if token.is_empty() {
            return;
        }
        if !matches!(token.as_str(), "AND" | "OR" | "WITH") {
            ids.push(token.clone());
        }
        token.clear();
    };

    for ch in expression.chars() {
        // Keep underscores so LicenseRef- and SPDX ids that use them stay intact.
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '+' | '_') {
            token.push(ch);
        } else {
            flush(&mut token, &mut ids);
        }
    }
    flush(&mut token, &mut ids);

    ids
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
    let path = Path::new(path.trim_start_matches("./"));
    let relative = match scanned_path {
        Some(root) => match path.strip_prefix(Path::new(root)) {
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

/// Plan SPDX packages: one per assembled package (with owned files), plus a
/// scan-root fallback package for files that no assembled package claims.
/// When there are no assembled packages, keep a single synthetic package
/// (`SPDXRef-001`) owning every file — the historic no-package contract.
fn plan_spdx_packages<'a>(
    output: &'a Output,
    files: &[&'a FileInfo],
    config: &OutputWriteConfig,
) -> Vec<SpdxPackagePlan<'a>> {
    if output.packages.is_empty() {
        return vec![SpdxPackagePlan {
            spdx_id: "SPDXRef-001".to_string(),
            name: primary_package_name(output, config),
            files: files.to_vec(),
        }];
    }

    let mut assigned_paths: HashSet<&str> = HashSet::new();
    let mut plans = Vec::with_capacity(output.packages.len() + 1);

    for (idx, package) in output.packages.iter().enumerate() {
        let owned: Vec<&FileInfo> = files
            .iter()
            .copied()
            .filter(|file| {
                file.for_packages
                    .iter()
                    .any(|uid| uid == &package.package_uid)
            })
            .collect();
        for file in &owned {
            assigned_paths.insert(file.path.as_str());
        }
        plans.push(SpdxPackagePlan {
            spdx_id: format!("SPDXRef-Package-{}", idx + 1),
            name: spdx_assembled_package_name(package, idx),
            files: owned,
        });
    }

    let unassigned: Vec<&FileInfo> = files
        .iter()
        .copied()
        .filter(|file| !assigned_paths.contains(file.path.as_str()))
        .collect();
    if !unassigned.is_empty() {
        plans.push(SpdxPackagePlan {
            spdx_id: "SPDXRef-Package-unassigned".to_string(),
            name: primary_package_name(output, config),
            files: unassigned,
        });
    }

    plans
}

fn spdx_assembled_package_name(package: &OutputPackage, idx: usize) -> String {
    package
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(sanitize_spdx_package_name)
        .unwrap_or_else(|| format!("package-{}", idx + 1))
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
        let plans = plan_spdx_packages(
            &schema,
            &files,
            &OutputWriteConfig {
                format: crate::output::OutputFormat::SpdxTv,
                custom_template: None,
                scanned_path: Some("umbrella".to_string()),
            },
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
}
