// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::models::PackageData;
use crate::parsers::maven::pom::licenses::{
    MavenLicenseEntry, build_license_statement, build_maven_declared_license_data,
    resolve_license_entry,
};
use crate::parsers::maven::pom::properties::PropertyResolver;
use crate::parsers::maven::pom::tags::{KnownTag, Tag};
use crate::parsers::utils::truncate_field;

#[derive(Default)]
pub(super) struct LicenseData {
    entries: Vec<MavenLicenseEntry>,
    xml_comments: Vec<String>,
}

impl LicenseData {
    pub(super) fn push_entry(&mut self, entry: MavenLicenseEntry) {
        self.entries.push(entry);
    }

    pub(super) fn push_xml_comment(&mut self, comment: String) {
        self.xml_comments.push(comment);
    }

    pub(super) fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        for comment in &mut self.xml_comments {
            *comment = resolver.resolve_text(comment, 0);
        }
        for license in &mut self.entries {
            resolve_license_entry(resolver, license);
        }

        let license_comments = std::mem::take(&mut self.xml_comments);
        for comment in license_comments {
            if !comment.trim().is_empty() {
                self.entries.push(MavenLicenseEntry {
                    comments: Some(comment),
                    ..Default::default()
                });
            }
        }
    }

    pub(super) fn finalize(self, package_data: &mut PackageData) {
        package_data.extracted_license_statement =
            build_license_statement(&self.entries).map(truncate_field);
        let (declared_license_expression, declared_license_expression_spdx, license_detections) =
            build_maven_declared_license_data(
                &self.entries,
                package_data.extracted_license_statement.as_deref(),
            );
        package_data.declared_license_expression = declared_license_expression;
        package_data.declared_license_expression_spdx = declared_license_expression_spdx;
        package_data.license_detections = license_detections;
    }
}

impl MavenLicenseEntry {
    pub(super) fn for_start(tag: &Tag) -> Option<Self> {
        match tag {
            Tag::Known(KnownTag::License) => Some(Self::default()),
            _ => None,
        }
    }

    pub(super) fn has_data(&self) -> bool {
        self.name.is_some() || self.url.is_some() || self.comments.is_some()
    }

    pub(super) fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Name) => self.name = Some(text.to_string()),
            Some(KnownTag::Url) => self.url = Some(text.to_string()),
            Some(KnownTag::Comments) => self.comments = Some(text.to_string()),
            _ => {}
        }
    }
}
