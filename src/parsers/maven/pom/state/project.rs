// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use crate::models::{PackageData, Party, PartyType};
use crate::parsers::maven::coordinates::build_maven_qualifiers;
use crate::parsers::maven::pom::properties::{PropertyResolver, resolve_option};
use crate::parsers::maven::pom::tags::KnownTag;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct ProjectMetadata {
    scm_connection: Option<String>,
    scm_developer_connection: Option<String>,
    scm_url: Option<String>,
    scm_tag: Option<String>,
    organization_name: Option<String>,
    organization_url: Option<String>,
    issue_management_system: Option<String>,
    issue_management_url: Option<String>,
    ci_management_system: Option<String>,
    ci_management_url: Option<String>,
}

impl ProjectMetadata {
    fn normalize_scm_connection(text: String) -> String {
        if text.starts_with("scm:git:") {
            text.replacen("scm:git:", "git+", 1)
        } else if text.starts_with("scm:") {
            text.replacen("scm:", "", 1)
        } else {
            text
        }
    }

    pub(super) fn apply_scm_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Connection) => {
                self.scm_connection = Some(Self::normalize_scm_connection(text.to_string()))
            }
            Some(KnownTag::DeveloperConnection) => {
                self.scm_developer_connection =
                    Some(Self::normalize_scm_connection(text.to_string()));
            }
            Some(KnownTag::Url) => self.scm_url = Some(text.to_string()),
            Some(KnownTag::Tag) => self.scm_tag = Some(text.to_string()),
            _ => {}
        }
    }

    pub(super) fn apply_organization_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Name) => self.organization_name = Some(text.to_string()),
            Some(KnownTag::Url) => self.organization_url = Some(text.to_string()),
            _ => {}
        }
    }

    pub(super) fn apply_issue_management_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::System) => self.issue_management_system = Some(text.to_string()),
            Some(KnownTag::Url) => self.issue_management_url = Some(text.to_string()),
            _ => {}
        }
    }

    pub(super) fn apply_ci_management_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::System) => self.ci_management_system = Some(text.to_string()),
            Some(KnownTag::Url) => self.ci_management_url = Some(text.to_string()),
            _ => {}
        }
    }

    pub(super) fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        resolve_option(resolver, &mut self.scm_connection);
        resolve_option(resolver, &mut self.scm_developer_connection);
        resolve_option(resolver, &mut self.scm_url);
        resolve_option(resolver, &mut self.scm_tag);
        resolve_option(resolver, &mut self.organization_name);
        resolve_option(resolver, &mut self.organization_url);
        resolve_option(resolver, &mut self.issue_management_system);
        resolve_option(resolver, &mut self.issue_management_url);
        resolve_option(resolver, &mut self.ci_management_system);
        resolve_option(resolver, &mut self.ci_management_url);
    }

    pub(super) fn has_extra_data(&self) -> bool {
        self.organization_name.is_some()
            || self.organization_url.is_some()
            || self.scm_tag.is_some()
            || self.scm_developer_connection.is_some()
            || self.issue_management_system.is_some()
            || self.ci_management_system.is_some()
            || self.ci_management_url.is_some()
    }

    pub(super) fn populate_extra_data(
        &mut self,
        extra_data: &mut HashMap<String, serde_json::Value>,
    ) {
        if let Some(name) = self.organization_name.take() {
            extra_data.insert(
                "organization_name".to_string(),
                serde_json::Value::String(name),
            );
        }
        if let Some(url) = self.organization_url.take() {
            extra_data.insert(
                "organization_url".to_string(),
                serde_json::Value::String(url),
            );
        }
        if let Some(tag) = self.scm_tag.take() {
            extra_data.insert("scm_tag".to_string(), serde_json::Value::String(tag));
        }
        if let Some(dev_conn) = self.scm_developer_connection.take() {
            extra_data.insert(
                "scm_developer_connection".to_string(),
                serde_json::Value::String(dev_conn),
            );
        }
        if let Some(system) = self.issue_management_system.take() {
            extra_data.insert(
                "issue_tracking_system".to_string(),
                serde_json::Value::String(system),
            );
        }
        if let Some(system) = self.ci_management_system.take() {
            extra_data.insert("ci_system".to_string(), serde_json::Value::String(system));
        }
        if let Some(url) = self.ci_management_url.take() {
            extra_data.insert("ci_url".to_string(), serde_json::Value::String(url));
        }
    }

    pub(super) fn apply_related_urls(&self, package_data: &mut PackageData) {
        package_data.vcs_url = self
            .scm_connection
            .clone()
            .or_else(|| self.scm_developer_connection.clone())
            .or_else(|| self.scm_url.clone());

        if let Some(url) = &self.scm_url {
            package_data.code_view_url = Some(url.clone());
        }
        if let Some(url) = &self.issue_management_url {
            package_data.bug_tracking_url = Some(url.clone());
        }
    }

    pub(super) fn add_owner_party(&self, package_data: &mut PackageData) {
        if self.organization_name.is_some() || self.organization_url.is_some() {
            package_data.parties.push(Party {
                r#type: Some(PartyType::Organization),
                role: Some("owner".to_string()),
                name: self.organization_name.clone(),
                email: None,
                url: self.organization_url.clone(),
                organization: None,
                organization_url: None,
                timezone: None,
            });
        }
    }
}

#[derive(Default)]
pub(super) struct ProjectDetails {
    inception_year: Option<String>,
    name: Option<String>,
    description: Option<String>,
    packaging: Option<String>,
    classifier: Option<String>,
}

impl ProjectDetails {
    pub(super) fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Name) => self.name = Some(text.to_string()),
            Some(KnownTag::Description) => self.description = Some(text.to_string()),
            Some(KnownTag::Packaging) => self.packaging = Some(text.to_string()),
            Some(KnownTag::Classifier) => self.classifier = Some(text.to_string()),
            Some(KnownTag::InceptionYear) => self.inception_year = Some(text.to_string()),
            _ => {}
        }
    }

    pub(super) fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        resolve_option(resolver, &mut self.inception_year);
        resolve_option(resolver, &mut self.name);
        resolve_option(resolver, &mut self.description);
        resolve_option(resolver, &mut self.packaging);
        resolve_option(resolver, &mut self.classifier);
    }

    pub(super) fn has_extra_data(&self) -> bool {
        self.inception_year.is_some()
    }

    pub(super) fn populate_extra_data(
        &mut self,
        extra_data: &mut HashMap<String, serde_json::Value>,
    ) {
        if let Some(year) = self.inception_year.take() {
            extra_data.insert(
                "inception_year".to_string(),
                serde_json::Value::String(year),
            );
        }
    }

    pub(super) fn apply_package_metadata(&self, package_data: &mut PackageData) {
        package_data.qualifiers =
            build_maven_qualifiers(self.classifier.as_deref(), self.packaging.as_deref());

        package_data.description = match (
            self.name.as_deref().filter(|value| !value.is_empty()),
            self.description
                .as_deref()
                .filter(|value| !value.is_empty()),
        ) {
            (Some(name), Some(description)) if name == description => Some(name.to_string()),
            (Some(name), Some(description)) => Some(format!("{name}\n{description}")),
            (Some(name), None) => Some(name.to_string()),
            (None, Some(description)) => Some(description.to_string()),
            (None, None) => None,
        };
    }

    pub(super) fn name(&self) -> &Option<String> {
        &self.name
    }

    pub(super) fn packaging(&self) -> &Option<String> {
        &self.packaging
    }

    pub(super) fn classifier(&self) -> Option<&str> {
        self.classifier.as_deref()
    }

    pub(super) fn packaging_str(&self) -> Option<&str> {
        self.packaging.as_deref()
    }

    pub(super) fn has_classifier(&self) -> bool {
        self.classifier.is_some()
    }
}
