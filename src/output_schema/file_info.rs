// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize, Serializer};
use serde_json::Map;

use super::author::OutputAuthor;
use super::copyright::OutputCopyright;
use super::email::OutputEmail;
use super::file_type::OutputFileType;
use super::holder::OutputHolder;
use super::license_detection::OutputLicenseDetection;
use super::license_match::OutputMatch;
use super::license_policy_entry::OutputLicensePolicyEntry;
use super::package_data::OutputPackageData;
use super::serde_helpers::insert_json;
use super::tallies::OutputTallies;
use super::url::OutputURL;

#[derive(Debug, Clone, Deserialize)]
pub struct OutputFileInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub base_name: String,
    #[serde(default)]
    pub extension: String,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: OutputFileType,
    pub mime_type: Option<String>,
    #[serde(rename = "file_type")]
    pub file_type_label: Option<String>,
    #[serde(default)]
    pub size: u64,
    pub date: Option<String>,
    pub sha1: Option<String>,
    pub md5: Option<String>,
    pub sha256: Option<String>,
    pub sha1_git: Option<String>,
    pub programming_language: Option<String>,
    #[serde(default)]
    pub package_data: Vec<OutputPackageData>,
    #[serde(rename = "detected_license_expression_spdx")]
    pub license_expression: Option<String>,
    #[serde(default)]
    pub license_detections: Vec<OutputLicenseDetection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub license_clues: Vec<OutputMatch>,
    pub percentage_of_license_text: Option<f64>,
    #[serde(default)]
    pub copyrights: Vec<OutputCopyright>,
    #[serde(default)]
    pub holders: Vec<OutputHolder>,
    #[serde(default)]
    pub authors: Vec<OutputAuthor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<OutputEmail>,
    #[serde(default)]
    pub urls: Vec<OutputURL>,
    #[serde(default)]
    pub for_packages: Vec<String>,
    #[serde(default)]
    pub scan_errors: Vec<String>,
    pub license_policy: Option<Vec<OutputLicensePolicyEntry>>,
    pub is_generated: Option<bool>,
    pub is_binary: Option<bool>,
    pub is_text: Option<bool>,
    pub is_archive: Option<bool>,
    pub is_media: Option<bool>,
    pub is_source: Option<bool>,
    pub is_script: Option<bool>,
    pub files_count: Option<usize>,
    pub dirs_count: Option<usize>,
    pub size_count: Option<u64>,
    pub source_count: Option<usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_legal: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_manifest: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_readme: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_top_level: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_key_file: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_referenced: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_community: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facets: Vec<String>,
    pub tallies: Option<OutputTallies>,
}

impl OutputFileInfo {
    pub(crate) fn should_serialize_info_surface(&self) -> bool {
        self.date.is_some()
            || self.sha1.is_some()
            || self.md5.is_some()
            || self.sha256.is_some()
            || self.sha1_git.is_some()
            || self.mime_type.is_some()
            || self.file_type_label.is_some()
            || self.programming_language.is_some()
            || self.is_binary.is_some()
            || self.is_text.is_some()
            || self.is_archive.is_some()
            || self.is_media.is_some()
            || self.is_source.is_some()
            || self.is_script.is_some()
            || self.files_count.is_some()
            || self.dirs_count.is_some()
            || self.size_count.is_some()
    }

    pub(crate) fn should_serialize_license_surface(&self) -> bool {
        self.license_expression.is_some()
            || !self.license_detections.is_empty()
            || !self.license_clues.is_empty()
            || self.percentage_of_license_text.is_some()
    }

    /// The scancode-key counterpart of [`Self::detected_license_expression_spdx`].
    /// Mirrors the same three-tier fallback (file detections, then package-data
    /// detections, then the carried expression) but on the non-SPDX
    /// `license_expression` and with the non-strict combiner, since scancode keys
    /// such as `proprietary-license` are not valid SPDX tokens.
    pub(crate) fn detected_license_expression(&self) -> Option<String> {
        let combine = |expressions: Vec<String>| -> Option<String> {
            let expressions: Vec<String> = expressions
                .into_iter()
                .filter(|expression| !expression.is_empty())
                .collect();
            if expressions.is_empty() {
                return None;
            }
            crate::utils::spdx::select_primary_license_expression(expressions.clone()).or_else(
                || {
                    crate::utils::spdx::combine_license_expressions_preserving_structure(
                        expressions,
                    )
                },
            )
        };

        combine(
            self.license_detections
                .iter()
                .map(|detection| detection.license_expression.clone())
                .collect(),
        )
        .or_else(|| {
            combine(
                self.package_data
                    .iter()
                    .flat_map(|package_data| package_data.license_detections.iter())
                    .map(|detection| detection.license_expression.clone())
                    .collect(),
            )
        })
        .or_else(|| {
            self.license_expression
                .clone()
                .filter(|expression| !expression.is_empty())
        })
    }

    pub(crate) fn detected_license_expression_spdx(&self) -> Option<String> {
        {
            let expressions: Option<Vec<String>> = self
                .license_detections
                .iter()
                .map(|detection| {
                    (!detection.license_expression_spdx.is_empty())
                        .then(|| detection.license_expression_spdx.clone())
                })
                .collect();
            expressions.and_then(|expressions| {
                crate::utils::spdx::select_primary_license_expression_strict(expressions.clone())
                    .or_else(|| {
                        crate::utils::spdx::combine_license_expressions_preserving_structure_strict(
                            expressions,
                        )
                    })
            })
        }
        .or_else(|| {
            let expressions: Option<Vec<String>> = self
                .package_data
                .iter()
                .flat_map(|package_data| package_data.license_detections.iter())
                .map(|detection| {
                    (!detection.license_expression_spdx.is_empty())
                        .then(|| detection.license_expression_spdx.clone())
                })
                .collect();
            expressions.and_then(|expressions| {
                crate::utils::spdx::select_primary_license_expression_strict(expressions.clone())
                    .or_else(|| {
                        crate::utils::spdx::combine_license_expressions_preserving_structure_strict(
                            expressions,
                        )
                    })
            })
        })
        .or_else(|| {
            self.license_expression
                .clone()
                .filter(|expression| !expression.is_empty())
                .and_then(|expression| {
                    crate::utils::spdx::combine_license_expressions_preserving_structure_strict([
                        expression,
                    ])
                })
        })
    }
}

impl Serialize for OutputFileInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = Map::new();
        insert_json(&mut map, "path", &self.path)?;
        insert_json(&mut map, "type", self.file_type)?;
        insert_json(&mut map, "name", &self.name)?;
        insert_json(&mut map, "base_name", &self.base_name)?;
        insert_json(&mut map, "extension", &self.extension)?;
        insert_json(&mut map, "size", self.size)?;

        if self.should_serialize_info_surface() {
            insert_json(&mut map, "date", &self.date)?;
            insert_json(&mut map, "sha1", self.sha1.as_ref())?;
            insert_json(&mut map, "md5", self.md5.as_ref())?;
            insert_json(&mut map, "sha256", self.sha256.as_ref())?;
            insert_json(&mut map, "sha1_git", self.sha1_git.as_ref())?;
            insert_json(&mut map, "mime_type", &self.mime_type)?;
            insert_json(&mut map, "file_type", &self.file_type_label)?;
            insert_json(&mut map, "programming_language", &self.programming_language)?;
            insert_json(&mut map, "is_binary", self.is_binary)?;
            insert_json(&mut map, "is_text", self.is_text)?;
            insert_json(&mut map, "is_archive", self.is_archive)?;
            insert_json(&mut map, "is_media", self.is_media)?;
            insert_json(&mut map, "is_source", self.is_source)?;
            insert_json(&mut map, "is_script", self.is_script)?;
            insert_json(&mut map, "files_count", self.files_count)?;
            insert_json(&mut map, "dirs_count", self.dirs_count)?;
            insert_json(&mut map, "size_count", self.size_count)?;
        }

        insert_json(&mut map, "package_data", &self.package_data)?;
        insert_json(
            &mut map,
            "detected_license_expression",
            self.detected_license_expression(),
        )?;
        insert_json(
            &mut map,
            "detected_license_expression_spdx",
            self.detected_license_expression_spdx(),
        )?;
        insert_json(&mut map, "license_detections", &self.license_detections)?;
        if self.should_serialize_license_surface() {
            insert_json(&mut map, "license_clues", &self.license_clues)?;
        }
        if self.percentage_of_license_text.is_some() {
            insert_json(
                &mut map,
                "percentage_of_license_text",
                self.percentage_of_license_text,
            )?;
        }
        insert_json(&mut map, "copyrights", &self.copyrights)?;
        insert_json(&mut map, "holders", &self.holders)?;
        insert_json(&mut map, "authors", &self.authors)?;
        if !self.emails.is_empty() {
            insert_json(&mut map, "emails", &self.emails)?;
        }
        insert_json(&mut map, "urls", &self.urls)?;
        insert_json(&mut map, "for_packages", &self.for_packages)?;
        insert_json(&mut map, "scan_errors", &self.scan_errors)?;
        if self.license_policy.is_some() {
            insert_json(&mut map, "license_policy", &self.license_policy)?;
        }
        if self.is_generated.is_some() {
            insert_json(&mut map, "is_generated", self.is_generated)?;
        }
        if self.source_count.is_some() {
            insert_json(&mut map, "source_count", self.source_count)?;
        }
        if self.is_legal {
            insert_json(&mut map, "is_legal", self.is_legal)?;
        }
        if self.is_manifest {
            insert_json(&mut map, "is_manifest", self.is_manifest)?;
        }
        if self.is_readme {
            insert_json(&mut map, "is_readme", self.is_readme)?;
        }
        if self.is_top_level {
            insert_json(&mut map, "is_top_level", self.is_top_level)?;
        }
        if self.is_key_file {
            insert_json(&mut map, "is_key_file", self.is_key_file)?;
        }
        if self.is_referenced {
            insert_json(&mut map, "is_referenced", self.is_referenced)?;
        }
        if self.is_community {
            insert_json(&mut map, "is_community", self.is_community)?;
        }
        if !self.facets.is_empty() {
            insert_json(&mut map, "facets", &self.facets)?;
        }
        if self.tallies.is_some() {
            insert_json(&mut map, "tallies", &self.tallies)?;
        }

        map.serialize(serializer)
    }
}

impl From<&crate::models::FileInfo> for OutputFileInfo {
    fn from(value: &crate::models::FileInfo) -> Self {
        Self::from_with_compat_mode(value, crate::cli::CompatibilityMode::Native)
    }
}

impl OutputFileInfo {
    pub fn from_with_compat_mode(
        value: &crate::models::FileInfo,
        mode: crate::cli::CompatibilityMode,
    ) -> Self {
        Self {
            name: value.name.clone(),
            base_name: value.base_name.clone(),
            extension: value.extension.clone(),
            path: value.path.clone(),
            file_type: OutputFileType::from(&value.file_type),
            mime_type: value.mime_type.clone(),
            file_type_label: value.file_type_label.clone(),
            size: value.size,
            date: value.date.clone(),
            sha1: value.sha1.as_ref().map(|d| d.as_hex()),
            md5: value.md5.as_ref().map(|d| d.as_hex()),
            sha256: value.sha256.as_ref().map(|d| d.as_hex()),
            sha1_git: value.sha1_git.as_ref().map(|d| d.as_hex()),
            programming_language: value.programming_language.clone(),
            package_data: value
                .package_data
                .iter()
                .map(OutputPackageData::from)
                .collect(),
            license_expression: value.detected_license_expression.clone(),
            license_detections: value
                .license_detections
                .iter()
                .map(OutputLicenseDetection::from)
                .collect(),
            license_clues: value.license_clues.iter().map(OutputMatch::from).collect(),
            percentage_of_license_text: value.percentage_of_license_text,
            copyrights: value
                .copyrights
                .iter()
                .map(|copyright| OutputCopyright::from_with_compat_mode(copyright, mode))
                .collect(),
            holders: value.holders.iter().map(OutputHolder::from).collect(),
            authors: value.authors.iter().map(OutputAuthor::from).collect(),
            emails: value.emails.iter().map(OutputEmail::from).collect(),
            urls: value.urls.iter().map(OutputURL::from).collect(),
            for_packages: value
                .for_packages
                .iter()
                .map(|uid| uid.to_string())
                .collect(),
            scan_errors: value
                .scan_diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect(),
            license_policy: value
                .license_policy
                .as_ref()
                .map(|v| v.iter().map(OutputLicensePolicyEntry::from).collect()),
            is_generated: value.is_generated,
            is_binary: value.is_binary,
            is_text: value.is_text,
            is_archive: value.is_archive,
            is_media: value.is_media,
            is_source: value.is_source,
            is_script: value.is_script,
            files_count: value.files_count,
            dirs_count: value.dirs_count,
            size_count: value.size_count,
            source_count: value.source_count,
            is_legal: value.is_legal,
            is_manifest: value.is_manifest,
            is_readme: value.is_readme,
            is_top_level: value.is_top_level,
            is_key_file: value.is_key_file,
            is_referenced: value.is_referenced,
            is_community: value.is_community,
            facets: value.facets.clone(),
            tallies: value.tallies.as_ref().map(OutputTallies::from),
        }
    }
}

impl TryFrom<&OutputFileInfo> for crate::models::FileInfo {
    type Error = String;
    fn try_from(value: &OutputFileInfo) -> Result<Self, Self::Error> {
        let mut package_data = Vec::with_capacity(value.package_data.len());
        for p in &value.package_data {
            package_data.push(crate::models::PackageData::try_from(p)?);
        }
        let mut license_detections = Vec::with_capacity(value.license_detections.len());
        for d in &value.license_detections {
            license_detections.push(crate::models::LicenseDetection::try_from(d)?);
        }
        let mut license_clues = Vec::with_capacity(value.license_clues.len());
        for m in &value.license_clues {
            license_clues.push(crate::models::Match::try_from(m)?);
        }
        let mut copyrights = Vec::with_capacity(value.copyrights.len());
        for c in &value.copyrights {
            copyrights.push(crate::models::Copyright::try_from(c)?);
        }
        let mut holders = Vec::with_capacity(value.holders.len());
        for h in &value.holders {
            holders.push(crate::models::Holder::try_from(h)?);
        }
        let mut authors = Vec::with_capacity(value.authors.len());
        for a in &value.authors {
            authors.push(crate::models::Author::try_from(a)?);
        }
        let mut emails = Vec::with_capacity(value.emails.len());
        for e in &value.emails {
            emails.push(crate::models::OutputEmail::try_from(e)?);
        }
        let mut urls = Vec::with_capacity(value.urls.len());
        for u in &value.urls {
            urls.push(crate::models::OutputURL::try_from(u)?);
        }
        let license_policy = value
            .license_policy
            .as_ref()
            .map(|v| {
                v.iter()
                    .map(crate::models::LicensePolicyEntry::try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        Ok(Self {
            name: value.name.clone(),
            base_name: value.base_name.clone(),
            extension: value.extension.clone(),
            path: value.path.clone(),
            file_type: crate::models::FileType::try_from(value.file_type)?,
            mime_type: value.mime_type.clone(),
            file_type_label: value.file_type_label.clone(),
            size: value.size,
            date: value.date.clone(),
            sha1: value
                .sha1
                .as_ref()
                .map(|s| crate::models::Sha1Digest::from_hex(s))
                .transpose()
                .map_err(|e| format!("invalid sha1: {}", e))?,
            md5: value
                .md5
                .as_ref()
                .map(|s| crate::models::Md5Digest::from_hex(s))
                .transpose()
                .map_err(|e| format!("invalid md5: {}", e))?,
            sha256: value
                .sha256
                .as_ref()
                .map(|s| crate::models::Sha256Digest::from_hex(s))
                .transpose()
                .map_err(|e| format!("invalid sha256: {}", e))?,
            sha1_git: value
                .sha1_git
                .as_ref()
                .map(|s| crate::models::GitSha1::from_hex(s))
                .transpose()
                .map_err(|e| format!("invalid sha1_git: {}", e))?,
            programming_language: value.programming_language.clone(),
            package_data,
            detected_license_expression: value.license_expression.clone(),
            license_detections,
            license_clues,
            percentage_of_license_text: value.percentage_of_license_text,
            copyrights,
            holders,
            authors,
            emails,
            urls,
            for_packages: value
                .for_packages
                .iter()
                .map(|s| crate::models::PackageUid::from_raw(s.clone()))
                .collect(),
            scan_diagnostics: crate::models::diagnostics_from_legacy_scan_errors(
                &value.scan_errors,
            ),
            license_policy,
            is_generated: value.is_generated,
            is_binary: value.is_binary,
            is_text: value.is_text,
            is_archive: value.is_archive,
            is_media: value.is_media,
            is_source: value.is_source,
            is_script: value.is_script,
            files_count: value.files_count,
            dirs_count: value.dirs_count,
            size_count: value.size_count,
            source_count: value.source_count,
            is_legal: value.is_legal,
            is_manifest: value.is_manifest,
            is_readme: value.is_readme,
            is_top_level: value.is_top_level,
            is_key_file: value.is_key_file,
            is_referenced: value.is_referenced,
            is_community: value.is_community,
            facets: value.facets.clone(),
            tallies: value
                .tallies
                .as_ref()
                .map(crate::models::Tallies::try_from)
                .transpose()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::OutputFileInfo;
    use crate::output_schema::OutputFileType;
    use crate::output_schema::license_detection::OutputLicenseDetection;
    use serde_json::json;

    fn base_output_file_info() -> OutputFileInfo {
        OutputFileInfo {
            name: "mod.rs".to_string(),
            base_name: "mod".to_string(),
            extension: ".rs".to_string(),
            path: "mod.rs".to_string(),
            file_type: OutputFileType::File,
            mime_type: None,
            file_type_label: None,
            size: 0,
            date: None,
            sha1: None,
            md5: None,
            sha256: None,
            sha1_git: None,
            programming_language: None,
            package_data: Vec::new(),
            license_expression: None,
            license_detections: Vec::new(),
            license_clues: Vec::new(),
            percentage_of_license_text: None,
            copyrights: Vec::new(),
            holders: Vec::new(),
            authors: Vec::new(),
            emails: Vec::new(),
            urls: Vec::new(),
            for_packages: Vec::new(),
            scan_errors: Vec::new(),
            license_policy: None,
            is_generated: None,
            is_binary: None,
            is_text: None,
            is_archive: None,
            is_media: None,
            is_source: None,
            is_script: None,
            files_count: None,
            dirs_count: None,
            size_count: None,
            source_count: None,
            is_legal: false,
            is_manifest: false,
            is_readme: false,
            is_top_level: false,
            is_key_file: false,
            is_referenced: false,
            is_community: false,
            facets: Vec::new(),
            tallies: None,
        }
    }

    #[test]
    fn detected_license_expression_spdx_does_not_recombine_partial_detection_spdx() {
        let mut file_info = base_output_file_info();
        file_info.license_expression = Some("Apache-2.0 AND MIT".to_string());
        file_info.license_detections = vec![
            OutputLicenseDetection {
                license_expression: "apache-2.0".to_string(),
                license_expression_spdx: "Apache-2.0".to_string(),
                matches: Vec::new(),
                detection_log: Vec::new(),
                identifier: None,
            },
            OutputLicenseDetection {
                license_expression: "mit".to_string(),
                license_expression_spdx: String::new(),
                matches: Vec::new(),
                detection_log: Vec::new(),
                identifier: None,
            },
        ];

        assert_eq!(
            file_info.detected_license_expression_spdx().as_deref(),
            Some("Apache-2.0 AND MIT")
        );
    }

    #[test]
    fn detected_license_expression_spdx_rejects_invalid_fallback_expression() {
        let mut file_info = base_output_file_info();
        file_info.license_expression = Some("MIT\" or malformed".to_string());

        assert_eq!(file_info.detected_license_expression_spdx(), None);
    }

    #[test]
    fn serialize_includes_is_referenced_only_when_true() {
        let mut file_info = base_output_file_info();
        let without_flag = serde_json::to_value(&file_info).expect("file should serialize");
        assert_eq!(
            without_flag,
            json!({
                "path": "mod.rs",
                "type": "file",
                "name": "mod.rs",
                "base_name": "mod",
                "extension": ".rs",
                "size": 0,
                "package_data": [],
                "detected_license_expression": null,
                "detected_license_expression_spdx": null,
                "license_detections": [],
                "copyrights": [],
                "holders": [],
                "authors": [],
                "urls": [],
                "for_packages": [],
                "scan_errors": []
            })
        );

        file_info.is_referenced = true;
        let with_flag = serde_json::to_value(&file_info).expect("file should serialize");
        assert_eq!(with_flag["is_referenced"], json!(true));
    }
}
