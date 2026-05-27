// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::LicenseDetection;
use super::Md5Digest;
use super::Party;
use super::Sha1Digest;
use super::Sha256Digest;
use super::Sha512Digest;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct PackageCore {
    #[serde(default)]
    pub qualifiers: Option<HashMap<String, String>>,
    pub subpath: Option<String>,
    pub primary_language: Option<String>,
    pub description: Option<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub parties: Vec<Party>,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub homepage_url: Option<String>,
    pub download_url: Option<String>,
    pub size: Option<u64>,
    pub sha1: Option<Sha1Digest>,
    pub md5: Option<Md5Digest>,
    pub sha256: Option<Sha256Digest>,
    pub sha512: Option<Sha512Digest>,
    pub bug_tracking_url: Option<String>,
    pub code_view_url: Option<String>,
    pub vcs_url: Option<String>,
    pub copyright: Option<String>,
    pub holder: Option<String>,
    pub declared_license_expression: Option<String>,
    pub declared_license_expression_spdx: Option<String>,
    #[serde(default)]
    pub license_detections: Vec<LicenseDetection>,
    pub other_license_expression: Option<String>,
    pub other_license_expression_spdx: Option<String>,
    #[serde(default)]
    pub other_license_detections: Vec<LicenseDetection>,
    pub extracted_license_statement: Option<String>,
    pub notice_text: Option<String>,
    #[serde(default)]
    pub source_packages: Vec<String>,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub is_virtual: bool,
    #[serde(default)]
    pub extra_data: Option<HashMap<String, serde_json::Value>>,
    pub repository_homepage_url: Option<String>,
    pub repository_download_url: Option<String>,
    pub api_data_url: Option<String>,
    pub purl: Option<String>,
}

impl PackageCore {
    pub fn fill_if_empty_from(&mut self, other: &PackageCore) {
        macro_rules! fill_if_empty {
            ($field:ident) => {
                if self.$field.is_none() {
                    self.$field = other.$field.clone();
                }
            };
        }

        fill_if_empty!(qualifiers);
        fill_if_empty!(subpath);
        fill_if_empty!(primary_language);
        fill_if_empty!(description);
        fill_if_empty!(release_date);
        fill_if_empty!(homepage_url);
        fill_if_empty!(download_url);
        fill_if_empty!(size);
        fill_if_empty!(sha1);
        fill_if_empty!(md5);
        fill_if_empty!(sha256);
        fill_if_empty!(sha512);
        fill_if_empty!(bug_tracking_url);
        fill_if_empty!(code_view_url);
        fill_if_empty!(vcs_url);
        fill_if_empty!(copyright);
        fill_if_empty!(holder);
        fill_if_empty!(declared_license_expression);
        fill_if_empty!(declared_license_expression_spdx);
        fill_if_empty!(other_license_expression);
        fill_if_empty!(other_license_expression_spdx);
        fill_if_empty!(extracted_license_statement);
        fill_if_empty!(notice_text);
        fill_if_empty!(repository_homepage_url);
        fill_if_empty!(repository_download_url);
        fill_if_empty!(api_data_url);
        fill_if_empty!(purl);

        match (&mut self.extra_data, &other.extra_data) {
            (None, Some(extra_data)) => {
                self.extra_data = Some(extra_data.clone());
            }
            (Some(existing), Some(incoming)) => {
                for (key, value) in incoming {
                    existing.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
            _ => {}
        }

        for party in &other.parties {
            if let Some(existing) = self.parties.iter_mut().find(|p| {
                p.role == party.role
                    && ((p.name.is_some() && p.name == party.name)
                        || (p.email.is_some() && p.email == party.email))
            }) {
                if existing.name.is_none() {
                    existing.name = party.name.clone();
                }
                if existing.email.is_none() {
                    existing.email = party.email.clone();
                }
            } else {
                self.parties.push(party.clone());
            }
        }

        for keyword in &other.keywords {
            if !self.keywords.contains(keyword) {
                self.keywords.push(keyword.clone());
            }
        }

        for detection in &other.license_detections {
            self.license_detections.push(detection.clone());
        }

        for detection in &other.other_license_detections {
            self.other_license_detections.push(detection.clone());
        }

        for source_pkg in &other.source_packages {
            if !self.source_packages.contains(source_pkg) {
                self.source_packages.push(source_pkg.clone());
            }
        }
    }
}
