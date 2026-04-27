// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::super::default_package_data;
use super::dependencies::{
    MavenDependencyData, build_maven_download_url, build_maven_purl, build_maven_qualifiers,
    build_maven_source_package, build_maven_url, dependency_extra_data,
    dependency_management_entry_to_value, is_maven_version_pinned, maven_dependency_to_dependency,
    parse_maven_bool,
};
use super::licenses::{
    MavenLicenseEntry, build_license_statement, build_maven_declared_license_data,
    is_license_like_comment, resolve_license_entry,
};
use super::properties::{
    MavenBuiltinPropertyInputs, PropertyResolver, build_builtin_properties,
    resolve_dependency_data, resolve_maps, resolve_option, resolve_vec,
};
use crate::models::{DatasourceId, Dependency, PackageData, PackageType, Party};
use crate::parser_warn as warn;
use crate::parsers::utils::truncate_field;
use std::collections::HashMap;
use std::path::Path;

pub(super) struct PomParseState {
    package_data: PackageData,
    current_element: Vec<Vec<u8>>,
    in_dependencies: bool,
    current_dependency: Option<Dependency>,
    dependency_data: Vec<MavenDependencyData>,
    current_dependency_data: Option<MavenDependencyData>,
    licenses: Vec<MavenLicenseEntry>,
    xml_license_comments: Vec<String>,
    current_license: Option<MavenLicenseEntry>,
    inception_year: Option<String>,
    scm_connection: Option<String>,
    scm_developer_connection: Option<String>,
    scm_url: Option<String>,
    scm_tag: Option<String>,
    organization_name: Option<String>,
    organization_url: Option<String>,
    in_developers: bool,
    in_contributors: bool,
    current_party: Option<Party>,
    issue_management_system: Option<String>,
    issue_management_url: Option<String>,
    ci_management_system: Option<String>,
    ci_management_url: Option<String>,
    in_distribution_management: bool,
    in_dist_repository: bool,
    in_dist_snapshot_repository: bool,
    in_dist_site: bool,
    dist_download_url: Option<String>,
    dist_repository_id: Option<String>,
    dist_repository_name: Option<String>,
    dist_repository_url: Option<String>,
    dist_repository_layout: Option<String>,
    dist_snapshot_repository_id: Option<String>,
    dist_snapshot_repository_name: Option<String>,
    dist_snapshot_repository_url: Option<String>,
    dist_snapshot_repository_layout: Option<String>,
    dist_site_id: Option<String>,
    dist_site_name: Option<String>,
    dist_site_url: Option<String>,
    in_repositories: bool,
    in_plugin_repositories: bool,
    in_repository: bool,
    repositories: Vec<serde_json::Map<String, serde_json::Value>>,
    plugin_repositories: Vec<serde_json::Map<String, serde_json::Value>>,
    current_repository_id: Option<String>,
    current_repository_name: Option<String>,
    current_repository_url: Option<String>,
    in_modules: bool,
    modules: Vec<String>,
    in_mailing_lists: bool,
    in_mailing_list: bool,
    mailing_lists: Vec<serde_json::Map<String, serde_json::Value>>,
    current_mailing_list_name: Option<String>,
    current_mailing_list_subscribe: Option<String>,
    current_mailing_list_unsubscribe: Option<String>,
    current_mailing_list_post: Option<String>,
    current_mailing_list_archive: Option<String>,
    in_dependency_management: bool,
    dependency_management_entries: Vec<MavenDependencyData>,
    current_dep_mgmt_dependency: Option<MavenDependencyData>,
    in_dep_mgmt_dependency: bool,
    in_parent: bool,
    parent_group_id: Option<String>,
    parent_artifact_id: Option<String>,
    parent_version: Option<String>,
    parent_relative_path: Option<String>,
    in_properties: bool,
    properties: HashMap<String, String>,
    project_name: Option<String>,
    project_description: Option<String>,
    project_packaging: Option<String>,
    project_classifier: Option<String>,
    in_relocation: bool,
    relocation: MavenDependencyData,
}

impl PomParseState {
    pub(super) fn new() -> Self {
        let mut package_data = default_package_data(DatasourceId::MavenPom);
        package_data.package_type = Some(PackageType::Maven);
        package_data.primary_language = Some("Java".to_string());
        package_data.datasource_id = Some(DatasourceId::MavenPom);

        Self {
            package_data,
            current_element: Vec::new(),
            in_dependencies: false,
            current_dependency: None,
            dependency_data: Vec::new(),
            current_dependency_data: None,
            licenses: Vec::new(),
            xml_license_comments: Vec::new(),
            current_license: None,
            inception_year: None,
            scm_connection: None,
            scm_developer_connection: None,
            scm_url: None,
            scm_tag: None,
            organization_name: None,
            organization_url: None,
            in_developers: false,
            in_contributors: false,
            current_party: None,
            issue_management_system: None,
            issue_management_url: None,
            ci_management_system: None,
            ci_management_url: None,
            in_distribution_management: false,
            in_dist_repository: false,
            in_dist_snapshot_repository: false,
            in_dist_site: false,
            dist_download_url: None,
            dist_repository_id: None,
            dist_repository_name: None,
            dist_repository_url: None,
            dist_repository_layout: None,
            dist_snapshot_repository_id: None,
            dist_snapshot_repository_name: None,
            dist_snapshot_repository_url: None,
            dist_snapshot_repository_layout: None,
            dist_site_id: None,
            dist_site_name: None,
            dist_site_url: None,
            in_repositories: false,
            in_plugin_repositories: false,
            in_repository: false,
            repositories: Vec::new(),
            plugin_repositories: Vec::new(),
            current_repository_id: None,
            current_repository_name: None,
            current_repository_url: None,
            in_modules: false,
            modules: Vec::new(),
            in_mailing_lists: false,
            in_mailing_list: false,
            mailing_lists: Vec::new(),
            current_mailing_list_name: None,
            current_mailing_list_subscribe: None,
            current_mailing_list_unsubscribe: None,
            current_mailing_list_post: None,
            current_mailing_list_archive: None,
            in_dependency_management: false,
            dependency_management_entries: Vec::new(),
            current_dep_mgmt_dependency: None,
            in_dep_mgmt_dependency: false,
            in_parent: false,
            parent_group_id: None,
            parent_artifact_id: None,
            parent_version: None,
            parent_relative_path: None,
            in_properties: false,
            properties: HashMap::new(),
            project_name: None,
            project_description: None,
            project_packaging: None,
            project_classifier: None,
            in_relocation: false,
            relocation: MavenDependencyData::default(),
        }
    }

    pub(super) fn handle_start(&mut self, element_name: Vec<u8>) {
        self.current_element.push(element_name.clone());

        match element_name.as_slice() {
            b"parent" => self.in_parent = true,
            b"dependencyManagement" => self.in_dependency_management = true,
            b"dependencies" if self.in_dependency_management => {}
            b"dependencies" => self.in_dependencies = true,
            b"dependency" if self.in_dependency_management => {
                self.in_dep_mgmt_dependency = true;
                self.current_dep_mgmt_dependency = Some(MavenDependencyData::default());
            }
            b"dependency" if self.in_dependencies => {
                self.current_dependency = Some(Dependency {
                    purl: None,
                    extracted_requirement: None,
                    scope: None,
                    is_runtime: None,
                    is_optional: Some(false),
                    is_pinned: None,
                    is_direct: Some(true),
                    resolved_package: None,
                    extra_data: None,
                });
                self.current_dependency_data = Some(MavenDependencyData::default());
            }
            b"properties" => self.in_properties = true,
            b"developers" => self.in_developers = true,
            b"developer" if self.in_developers => {
                self.current_party = Some(Party {
                    r#type: Some("person".to_string()),
                    role: Some("developer".to_string()),
                    name: None,
                    email: None,
                    url: None,
                    organization: None,
                    organization_url: None,
                    timezone: None,
                });
            }
            b"contributors" => self.in_contributors = true,
            b"contributor" if self.in_contributors => {
                self.current_party = Some(Party {
                    r#type: Some("person".to_string()),
                    role: Some("contributor".to_string()),
                    name: None,
                    email: None,
                    url: None,
                    organization: None,
                    organization_url: None,
                    timezone: None,
                });
            }
            b"license" => self.current_license = Some(MavenLicenseEntry::default()),
            b"distributionManagement" => self.in_distribution_management = true,
            b"relocation" if self.in_distribution_management => {
                self.in_relocation = true;
                self.relocation = MavenDependencyData::default();
            }
            b"repository" if self.in_distribution_management => self.in_dist_repository = true,
            b"snapshotRepository" if self.in_distribution_management => {
                self.in_dist_snapshot_repository = true;
            }
            b"site" if self.in_distribution_management => self.in_dist_site = true,
            b"repositories" => self.in_repositories = true,
            b"pluginRepositories" => self.in_plugin_repositories = true,
            b"repository" if self.in_repositories && !self.in_distribution_management => {
                self.in_repository = true;
                self.current_repository_id = None;
                self.current_repository_name = None;
                self.current_repository_url = None;
            }
            b"pluginRepository" if self.in_plugin_repositories => {
                self.in_repository = true;
                self.current_repository_id = None;
                self.current_repository_name = None;
                self.current_repository_url = None;
            }
            b"modules" => self.in_modules = true,
            b"mailingLists" => self.in_mailing_lists = true,
            b"mailingList" if self.in_mailing_lists => {
                self.in_mailing_list = true;
                self.current_mailing_list_name = None;
                self.current_mailing_list_subscribe = None;
                self.current_mailing_list_unsubscribe = None;
                self.current_mailing_list_post = None;
                self.current_mailing_list_archive = None;
            }
            _ => {}
        }
    }

    pub(super) fn handle_text(&mut self, path: &Path, text: String) {
        let current_path = self.current_element.last().map(|v| v.as_slice());
        let current_parent = self
            .current_element
            .len()
            .checked_sub(2)
            .map(|index| self.current_element[index].as_slice());

        if self.in_properties
            && self.current_element.len() >= 2
            && self.current_element[self.current_element.len() - 2] == b"properties"
        {
            if let Some(property_name) = self
                .current_element
                .last()
                .and_then(|name| std::str::from_utf8(name).ok())
            {
                self.properties
                    .insert(property_name.to_string(), truncate_field(text));
            } else {
                warn!("Failed to decode Maven property name in {:?}", path);
            }
        } else if self.in_dep_mgmt_dependency {
            if let Some(dep_mgmt) = self.current_dep_mgmt_dependency.as_mut() {
                match current_path {
                    Some(b"groupId") if current_parent == Some(b"dependency") => {
                        dep_mgmt.group_id = Some(text)
                    }
                    Some(b"artifactId") if current_parent == Some(b"dependency") => {
                        dep_mgmt.artifact_id = Some(text)
                    }
                    Some(b"version") if current_parent == Some(b"dependency") => {
                        dep_mgmt.version = Some(text)
                    }
                    Some(b"scope") if current_parent == Some(b"dependency") => {
                        dep_mgmt.scope = Some(text)
                    }
                    Some(b"type") if current_parent == Some(b"dependency") => {
                        dep_mgmt.type_ = Some(text)
                    }
                    Some(b"classifier") if current_parent == Some(b"dependency") => {
                        dep_mgmt.classifier = Some(text)
                    }
                    Some(b"optional") if current_parent == Some(b"dependency") => {
                        dep_mgmt.optional = Some(text)
                    }
                    _ => {}
                }
            }
        } else if let Some(license) = &mut self.current_license {
            match current_path {
                Some(b"name") => license.name = Some(text),
                Some(b"url") => license.url = Some(text),
                Some(b"comments") => license.comments = Some(text),
                _ => {}
            }
        } else if let Some(party) = &mut self.current_party {
            match current_path {
                Some(b"name") => party.name = Some(text),
                Some(b"email") => party.email = Some(text),
                Some(b"url") => party.url = Some(text),
                Some(b"organization") => party.organization = Some(text),
                Some(b"organizationUrl") => party.organization_url = Some(text),
                Some(b"timezone") => party.timezone = Some(text),
                _ => {}
            }
        } else if let Some(dep) = &mut self.current_dependency {
            match current_path {
                Some(b"groupId") => {
                    if current_parent == Some(b"dependency")
                        && let Some(coords) = self.current_dependency_data.as_mut()
                    {
                        coords.group_id = Some(text);
                    }
                }
                Some(b"artifactId") => {
                    if current_parent == Some(b"dependency")
                        && let Some(coords) = self.current_dependency_data.as_mut()
                    {
                        coords.artifact_id = Some(text);
                    }
                }
                Some(b"version") => {
                    if current_parent == Some(b"dependency")
                        && let Some(coords) = self.current_dependency_data.as_mut()
                    {
                        coords.version = Some(text);
                    }
                }
                Some(b"scope") => {
                    if current_parent == Some(b"dependency") {
                        dep.scope = Some(text.clone());
                        dep.is_optional = Some(text == "test" || text == "provided");
                        dep.is_runtime = Some(text != "test" && text != "provided");
                    }
                    if current_parent == Some(b"dependency")
                        && let Some(coords) = self.current_dependency_data.as_mut()
                    {
                        coords.scope = Some(text);
                    }
                }
                Some(b"optional") => {
                    if current_parent == Some(b"dependency")
                        && let Some(coords) = self.current_dependency_data.as_mut()
                    {
                        coords.optional = Some(text);
                    }
                }
                Some(b"type") => {
                    if current_parent == Some(b"dependency")
                        && let Some(coords) = self.current_dependency_data.as_mut()
                    {
                        coords.type_ = Some(text);
                    }
                }
                Some(b"classifier") => {
                    if current_parent == Some(b"dependency")
                        && let Some(coords) = self.current_dependency_data.as_mut()
                    {
                        coords.classifier = Some(text);
                    }
                }
                Some(b"systemPath") => {
                    if current_parent == Some(b"dependency")
                        && let Some(coords) = self.current_dependency_data.as_mut()
                    {
                        coords.system_path = Some(text);
                    }
                }
                _ => {}
            }
        } else if self.in_relocation {
            match current_path {
                Some(b"groupId") => self.relocation.group_id = Some(text),
                Some(b"artifactId") => self.relocation.artifact_id = Some(text),
                Some(b"version") => self.relocation.version = Some(text),
                Some(b"classifier") => self.relocation.classifier = Some(text),
                Some(b"type") => self.relocation.type_ = Some(text),
                Some(b"message") => self.relocation.message = Some(text),
                _ => {}
            }
        } else if self.in_parent {
            match current_path {
                Some(b"groupId") => self.parent_group_id = Some(text),
                Some(b"artifactId") => self.parent_artifact_id = Some(text),
                Some(b"version") => self.parent_version = Some(text),
                Some(b"relativePath") => self.parent_relative_path = Some(text),
                _ => {}
            }
        } else {
            match current_path {
                Some(b"groupId") if self.current_element.len() == 2 => {
                    self.package_data.namespace = Some(text)
                }
                Some(b"artifactId") if self.current_element.len() == 2 => {
                    self.package_data.name = Some(text)
                }
                Some(b"version") if self.current_element.len() == 2 => {
                    self.package_data.version = Some(text)
                }
                Some(b"name") if self.current_element.len() == 2 => self.project_name = Some(text),
                Some(b"description") if self.current_element.len() == 2 => {
                    self.project_description = Some(text)
                }
                Some(b"packaging") if self.current_element.len() == 2 => {
                    self.project_packaging = Some(text)
                }
                Some(b"classifier") if self.current_element.len() == 2 => {
                    self.project_classifier = Some(text)
                }
                Some(b"url") if self.current_element.len() == 2 => {
                    self.package_data.homepage_url = Some(text)
                }
                Some(b"inceptionYear") if self.current_element.len() == 2 => {
                    self.inception_year = Some(text)
                }
                Some(b"connection")
                    if self.current_element.len() >= 3
                        && self.current_element[self.current_element.len() - 2] == b"scm" =>
                {
                    self.scm_connection = if text.starts_with("scm:git:") {
                        Some(text.replacen("scm:git:", "git+", 1))
                    } else if text.starts_with("scm:") {
                        Some(text.replacen("scm:", "", 1))
                    } else {
                        Some(text)
                    };
                }
                Some(b"developerConnection")
                    if self.current_element.len() >= 3
                        && self.current_element[self.current_element.len() - 2] == b"scm" =>
                {
                    self.scm_developer_connection = if text.starts_with("scm:git:") {
                        Some(text.replacen("scm:git:", "git+", 1))
                    } else if text.starts_with("scm:") {
                        Some(text.replacen("scm:", "", 1))
                    } else {
                        Some(text)
                    };
                }
                Some(b"url")
                    if self.current_element.len() >= 3
                        && self.current_element[self.current_element.len() - 2] == b"scm" =>
                {
                    self.scm_url = Some(text);
                }
                Some(b"tag")
                    if self.current_element.len() >= 3
                        && self.current_element[self.current_element.len() - 2] == b"scm" =>
                {
                    self.scm_tag = Some(text);
                }
                Some(b"name")
                    if self.current_element.len() >= 2
                        && self.current_element[self.current_element.len() - 2]
                            == b"organization" =>
                {
                    self.organization_name = Some(text);
                }
                Some(b"url")
                    if self.current_element.len() >= 2
                        && self.current_element[self.current_element.len() - 2]
                            == b"organization" =>
                {
                    self.organization_url = Some(text);
                }
                Some(b"system")
                    if self.current_element.len() >= 2
                        && self.current_element[self.current_element.len() - 2]
                            == b"issueManagement" =>
                {
                    self.issue_management_system = Some(text);
                }
                Some(b"url")
                    if self.current_element.len() >= 2
                        && self.current_element[self.current_element.len() - 2]
                            == b"issueManagement" =>
                {
                    self.issue_management_url = Some(text);
                }
                Some(b"system")
                    if self.current_element.len() >= 2
                        && self.current_element[self.current_element.len() - 2]
                            == b"ciManagement" =>
                {
                    self.ci_management_system = Some(text);
                }
                Some(b"url")
                    if self.current_element.len() >= 2
                        && self.current_element[self.current_element.len() - 2]
                            == b"ciManagement" =>
                {
                    self.ci_management_url = Some(text);
                }
                Some(b"downloadUrl")
                    if self.current_element.len() >= 2
                        && self.current_element[self.current_element.len() - 2]
                            == b"distributionManagement" =>
                {
                    self.dist_download_url = Some(text);
                }
                Some(b"id") if self.in_dist_repository => self.dist_repository_id = Some(text),
                Some(b"name") if self.in_dist_repository => self.dist_repository_name = Some(text),
                Some(b"url") if self.in_dist_repository => self.dist_repository_url = Some(text),
                Some(b"layout") if self.in_dist_repository => {
                    self.dist_repository_layout = Some(text)
                }
                Some(b"id") if self.in_dist_snapshot_repository => {
                    self.dist_snapshot_repository_id = Some(text)
                }
                Some(b"name") if self.in_dist_snapshot_repository => {
                    self.dist_snapshot_repository_name = Some(text)
                }
                Some(b"url") if self.in_dist_snapshot_repository => {
                    self.dist_snapshot_repository_url = Some(text)
                }
                Some(b"layout") if self.in_dist_snapshot_repository => {
                    self.dist_snapshot_repository_layout = Some(text)
                }
                Some(b"id") if self.in_dist_site => self.dist_site_id = Some(text),
                Some(b"name") if self.in_dist_site => self.dist_site_name = Some(text),
                Some(b"url") if self.in_dist_site => self.dist_site_url = Some(text),
                Some(b"id") if self.in_repository => self.current_repository_id = Some(text),
                Some(b"name") if self.in_repository => self.current_repository_name = Some(text),
                Some(b"url") if self.in_repository => self.current_repository_url = Some(text),
                Some(b"module") if self.in_modules => self.modules.push(text),
                Some(b"name") if self.in_mailing_list => {
                    self.current_mailing_list_name = Some(text)
                }
                Some(b"subscribe") if self.in_mailing_list => {
                    self.current_mailing_list_subscribe = Some(text)
                }
                Some(b"unsubscribe") if self.in_mailing_list => {
                    self.current_mailing_list_unsubscribe = Some(text)
                }
                Some(b"post") if self.in_mailing_list => {
                    self.current_mailing_list_post = Some(text)
                }
                Some(b"archive") if self.in_mailing_list => {
                    self.current_mailing_list_archive = Some(text)
                }
                _ => {}
            }
        }
    }

    pub(super) fn handle_comment(&mut self, comment: String) {
        if self.current_element.is_empty()
            && !comment.is_empty()
            && is_license_like_comment(&comment)
        {
            self.xml_license_comments.push(comment);
        }
    }

    pub(super) fn handle_end(&mut self, element_name: &[u8]) {
        if !self.current_element.is_empty() {
            self.current_element.pop();
        }

        match element_name {
            b"parent" => self.in_parent = false,
            b"dependencyManagement" => self.in_dependency_management = false,
            b"dependencies" => self.in_dependencies = false,
            b"dependency" if self.in_dep_mgmt_dependency => {
                self.in_dep_mgmt_dependency = false;
                if let Some(dep_mgmt) = self.current_dep_mgmt_dependency.take()
                    && (dep_mgmt.group_id.is_some()
                        || dep_mgmt.artifact_id.is_some()
                        || dep_mgmt.version.is_some())
                {
                    self.dependency_management_entries.push(dep_mgmt);
                }
            }
            b"dependency" => {
                if let (Some(dep), Some(coords)) = (
                    self.current_dependency.take(),
                    self.current_dependency_data.take(),
                ) {
                    self.package_data.dependencies.push(dep);
                    self.dependency_data.push(coords);
                } else if let Some(dep) = self.current_dependency.take() {
                    self.package_data.dependencies.push(dep);
                }
            }
            b"license" => {
                if let Some(license) = self.current_license.take()
                    && (license.name.is_some()
                        || license.url.is_some()
                        || license.comments.is_some())
                {
                    self.licenses.push(license);
                }
            }
            b"developers" => self.in_developers = false,
            b"developer" => {
                if let Some(party) = self.current_party.take() {
                    self.package_data.parties.push(party);
                }
            }
            b"contributors" => self.in_contributors = false,
            b"contributor" => {
                if let Some(party) = self.current_party.take() {
                    self.package_data.parties.push(party);
                }
            }
            b"distributionManagement" => self.in_distribution_management = false,
            b"relocation" => self.in_relocation = false,
            b"repository" if !self.in_dependencies && self.in_distribution_management => {
                self.in_dist_repository = false
            }
            b"repository" if !self.in_dependencies && self.in_repositories => {
                self.in_repository = false;
                if self.current_repository_id.is_some()
                    || self.current_repository_name.is_some()
                    || self.current_repository_url.is_some()
                {
                    let mut repo = serde_json::Map::new();
                    if let Some(id) = self.current_repository_id.take() {
                        repo.insert("id".to_string(), serde_json::Value::String(id));
                    }
                    if let Some(name) = self.current_repository_name.take() {
                        repo.insert("name".to_string(), serde_json::Value::String(name));
                    }
                    if let Some(url) = self.current_repository_url.take() {
                        repo.insert("url".to_string(), serde_json::Value::String(url));
                    }
                    self.repositories.push(repo);
                }
            }
            b"pluginRepository" if self.in_plugin_repositories => {
                self.in_repository = false;
                if self.current_repository_id.is_some()
                    || self.current_repository_name.is_some()
                    || self.current_repository_url.is_some()
                {
                    let mut repo = serde_json::Map::new();
                    if let Some(id) = self.current_repository_id.take() {
                        repo.insert("id".to_string(), serde_json::Value::String(id));
                    }
                    if let Some(name) = self.current_repository_name.take() {
                        repo.insert("name".to_string(), serde_json::Value::String(name));
                    }
                    if let Some(url) = self.current_repository_url.take() {
                        repo.insert("url".to_string(), serde_json::Value::String(url));
                    }
                    self.plugin_repositories.push(repo);
                }
            }
            b"repositories" => self.in_repositories = false,
            b"properties" => self.in_properties = false,
            b"pluginRepositories" => self.in_plugin_repositories = false,
            b"modules" => self.in_modules = false,
            b"mailingLists" => self.in_mailing_lists = false,
            b"mailingList" => {
                self.in_mailing_list = false;
                if self.current_mailing_list_name.is_some()
                    || self.current_mailing_list_subscribe.is_some()
                    || self.current_mailing_list_unsubscribe.is_some()
                    || self.current_mailing_list_post.is_some()
                    || self.current_mailing_list_archive.is_some()
                {
                    let mut ml = serde_json::Map::new();
                    if let Some(name) = self.current_mailing_list_name.take() {
                        ml.insert("name".to_string(), serde_json::Value::String(name));
                    }
                    if let Some(subscribe) = self.current_mailing_list_subscribe.take() {
                        ml.insert(
                            "subscribe".to_string(),
                            serde_json::Value::String(subscribe),
                        );
                    }
                    if let Some(unsubscribe) = self.current_mailing_list_unsubscribe.take() {
                        ml.insert(
                            "unsubscribe".to_string(),
                            serde_json::Value::String(unsubscribe),
                        );
                    }
                    if let Some(post) = self.current_mailing_list_post.take() {
                        ml.insert("post".to_string(), serde_json::Value::String(post));
                    }
                    if let Some(archive) = self.current_mailing_list_archive.take() {
                        ml.insert("archive".to_string(), serde_json::Value::String(archive));
                    }
                    self.mailing_lists.push(ml);
                }
            }
            b"snapshotRepository" => self.in_dist_snapshot_repository = false,
            b"site" => self.in_dist_site = false,
            _ => {}
        }
    }

    pub(super) fn into_package_data(self) -> PackageData {
        self.package_data
    }

    pub(super) fn finalize(mut self, path: &Path) -> PackageData {
        let builtins = build_builtin_properties(MavenBuiltinPropertyInputs {
            namespace: &self.package_data.namespace,
            name: &self.package_data.name,
            version: &self.package_data.version,
            parent_group_id: &self.parent_group_id,
            parent_artifact_id: &self.parent_artifact_id,
            parent_version: &self.parent_version,
            project_name: &self.project_name,
            project_packaging: &self.project_packaging,
        });
        let mut resolver = PropertyResolver::new(self.properties, builtins);

        resolve_option(&mut resolver, &mut self.package_data.namespace);
        resolve_option(&mut resolver, &mut self.package_data.name);
        resolve_option(&mut resolver, &mut self.package_data.version);
        resolve_option(&mut resolver, &mut self.package_data.homepage_url);
        resolve_option(&mut resolver, &mut self.inception_year);
        resolve_option(&mut resolver, &mut self.scm_connection);
        resolve_option(&mut resolver, &mut self.scm_developer_connection);
        resolve_option(&mut resolver, &mut self.scm_url);
        resolve_option(&mut resolver, &mut self.scm_tag);
        resolve_option(&mut resolver, &mut self.organization_name);
        resolve_option(&mut resolver, &mut self.organization_url);
        resolve_option(&mut resolver, &mut self.issue_management_system);
        resolve_option(&mut resolver, &mut self.issue_management_url);
        resolve_option(&mut resolver, &mut self.ci_management_system);
        resolve_option(&mut resolver, &mut self.ci_management_url);
        resolve_option(&mut resolver, &mut self.dist_download_url);
        resolve_option(&mut resolver, &mut self.dist_repository_id);
        resolve_option(&mut resolver, &mut self.dist_repository_name);
        resolve_option(&mut resolver, &mut self.dist_repository_url);
        resolve_option(&mut resolver, &mut self.dist_repository_layout);
        resolve_option(&mut resolver, &mut self.dist_snapshot_repository_id);
        resolve_option(&mut resolver, &mut self.dist_snapshot_repository_name);
        resolve_option(&mut resolver, &mut self.dist_snapshot_repository_url);
        resolve_option(&mut resolver, &mut self.dist_snapshot_repository_layout);
        resolve_option(&mut resolver, &mut self.dist_site_id);
        resolve_option(&mut resolver, &mut self.dist_site_name);
        resolve_option(&mut resolver, &mut self.dist_site_url);
        resolve_option(&mut resolver, &mut self.parent_group_id);
        resolve_option(&mut resolver, &mut self.parent_artifact_id);
        resolve_option(&mut resolver, &mut self.parent_version);
        resolve_option(&mut resolver, &mut self.parent_relative_path);
        resolve_option(&mut resolver, &mut self.project_name);
        resolve_option(&mut resolver, &mut self.project_description);
        resolve_option(&mut resolver, &mut self.project_packaging);
        resolve_option(&mut resolver, &mut self.project_classifier);
        resolve_vec(&mut resolver, &mut self.modules);
        resolve_maps(&mut resolver, &mut self.repositories);
        resolve_maps(&mut resolver, &mut self.plugin_repositories);
        resolve_maps(&mut resolver, &mut self.mailing_lists);
        for comment in &mut self.xml_license_comments {
            *comment = resolver.resolve_text(comment, 0);
        }
        for dependency in &mut self.dependency_management_entries {
            resolve_dependency_data(&mut resolver, dependency);
        }
        resolve_dependency_data(&mut resolver, &mut self.relocation);
        for license in &mut self.licenses {
            resolve_license_entry(&mut resolver, license);
        }
        for comment in self.xml_license_comments {
            if !comment.trim().is_empty() {
                self.licenses.push(MavenLicenseEntry {
                    comments: Some(comment),
                    ..Default::default()
                });
            }
        }

        for (dependency, coords) in self
            .package_data
            .dependencies
            .iter_mut()
            .zip(self.dependency_data.iter_mut())
        {
            resolve_dependency_data(&mut resolver, coords);
            dependency.scope = coords.scope.clone();
            dependency.extracted_requirement = coords.version.clone();
            dependency.extra_data = dependency_extra_data(coords);
            dependency.is_optional = Some(parse_maven_bool(coords.optional.as_deref()));

            match dependency.scope.as_deref() {
                Some("test") | Some("provided") => {
                    dependency.is_runtime = Some(false);
                    dependency.is_optional = Some(true);
                }
                Some(_) => dependency.is_runtime = Some(true),
                None => dependency.is_runtime = None,
            }

            if let Some(version) = &coords.version {
                dependency.is_pinned = Some(is_maven_version_pinned(version));
            }

            if let (Some(group_id), Some(artifact_id)) = (&coords.group_id, &coords.artifact_id) {
                dependency.purl = Some(build_maven_purl(
                    group_id,
                    artifact_id,
                    coords.version.as_deref(),
                    coords.classifier.as_deref(),
                    coords.type_.as_deref(),
                ));
            }
        }

        if self.package_data.namespace.is_none() {
            self.package_data.namespace = self.parent_group_id.clone();
        }
        if self.package_data.version.is_none() {
            self.package_data.version = self.parent_version.clone();
        }

        self.package_data.qualifiers = build_maven_qualifiers(
            self.project_classifier.as_deref(),
            self.project_packaging.as_deref(),
        );

        self.package_data.description = match (
            self.project_name
                .as_deref()
                .filter(|value| !value.is_empty()),
            self.project_description
                .as_deref()
                .filter(|value| !value.is_empty()),
        ) {
            (Some(name), Some(description)) if name == description => Some(name.to_string()),
            (Some(name), Some(description)) => Some(format!("{name}\n{description}")),
            (Some(name), None) => Some(name.to_string()),
            (None, Some(description)) => Some(description.to_string()),
            (None, None) => None,
        };

        if path.to_string_lossy().contains("META-INF/maven/") {
            let path_str = path.to_string_lossy();
            if let Some(meta_inf_pos) = path_str.find("META-INF/maven/") {
                let after_maven = &path_str[meta_inf_pos + "META-INF/maven/".len()..];
                let parts: Vec<&str> = after_maven.split('/').collect();
                if parts.len() >= 2 {
                    if self.package_data.namespace.is_none() {
                        self.package_data.namespace = Some(parts[0].to_string());
                    }
                    if self.package_data.name.is_none() {
                        self.package_data.name = Some(parts[1].to_string());
                    }
                }
            }
        }

        if let (Some(group_id), Some(artifact_id), Some(version)) = (
            &self.package_data.namespace,
            &self.package_data.name,
            &self.package_data.version,
        ) {
            self.package_data.purl = Some(build_maven_purl(
                group_id,
                artifact_id,
                Some(version),
                self.project_classifier.as_deref(),
                self.project_packaging.as_deref(),
            ));
            if self.project_classifier.is_none() {
                self.package_data
                    .source_packages
                    .push(build_maven_source_package(group_id, artifact_id, version));
            }
        }

        if let (Some(_group_id), Some(_artifact_id)) =
            (&self.package_data.namespace, &self.package_data.name)
        {
            self.package_data.repository_homepage_url = build_maven_url(
                &self.package_data.namespace,
                &self.package_data.name,
                &self.package_data.version,
                None,
            );

            if let (Some(group_id), Some(artifact_id), Some(ver)) = (
                self.package_data.namespace.as_deref(),
                self.package_data.name.as_deref(),
                self.package_data.version.as_deref(),
            ) {
                self.package_data.repository_download_url = Some(build_maven_download_url(
                    group_id,
                    artifact_id,
                    ver,
                    self.project_classifier.as_deref(),
                    self.project_packaging.as_deref(),
                ));
            } else {
                self.package_data.repository_download_url = None;
            }

            if let (Some(name), Some(ver)) = (
                self.package_data.name.as_deref(),
                self.package_data.version.as_deref(),
            ) {
                let pom_filename = format!("{}-{}.pom", name, ver);
                self.package_data.api_data_url = build_maven_url(
                    &self.package_data.namespace,
                    &self.package_data.name,
                    &self.package_data.version,
                    Some(&pom_filename),
                );
            }
        }

        self.package_data.vcs_url = self
            .scm_connection
            .or_else(|| self.scm_developer_connection.clone())
            .or_else(|| self.scm_url.clone());

        if let Some(url) = &self.scm_url {
            self.package_data.code_view_url = Some(url.clone());
        }
        if let Some(url) = &self.issue_management_url {
            self.package_data.bug_tracking_url = Some(url.clone());
        }
        if let Some(url) = &self.dist_download_url {
            self.package_data.download_url = Some(url.clone());
        }

        if self.organization_name.is_some() || self.organization_url.is_some() {
            self.package_data.parties.push(Party {
                r#type: Some("organization".to_string()),
                role: Some("owner".to_string()),
                name: self.organization_name.clone(),
                email: None,
                url: self.organization_url.clone(),
                organization: None,
                organization_url: None,
                timezone: None,
            });
        }

        for dependency in &self.dependency_management_entries {
            if dependency.scope.as_deref() == Some("import")
                && let Some(import_dependency) =
                    maven_dependency_to_dependency(dependency, Some("import"), true)
            {
                self.package_data.dependencies.push(import_dependency);
            }

            let mut dependency_management_copy = dependency.clone();
            dependency_management_copy.scope = Some("dependencymanagement".to_string());

            if let Some(converted) = maven_dependency_to_dependency(
                &dependency_management_copy,
                Some("dependencymanagement"),
                true,
            ) {
                self.package_data.dependencies.push(converted);
            }
        }

        if (self.relocation.group_id.is_some()
            || self.relocation.artifact_id.is_some()
            || self.relocation.version.is_some())
            && let Some(converted) =
                maven_dependency_to_dependency(&self.relocation, Some("relocation"), true)
        {
            self.package_data.dependencies.push(converted);
        }

        if self.inception_year.is_some()
            || self.organization_name.is_some()
            || self.organization_url.is_some()
            || self.scm_tag.is_some()
            || self.scm_developer_connection.is_some()
            || self.issue_management_system.is_some()
            || self.ci_management_system.is_some()
            || self.ci_management_url.is_some()
            || self.dist_download_url.is_some()
            || self.dist_repository_id.is_some()
            || self.dist_snapshot_repository_id.is_some()
            || self.dist_site_id.is_some()
            || !self.repositories.is_empty()
            || !self.plugin_repositories.is_empty()
            || !self.modules.is_empty()
            || !self.mailing_lists.is_empty()
            || !self.dependency_management_entries.is_empty()
            || self.parent_group_id.is_some()
            || self.relocation.group_id.is_some()
            || self.relocation.artifact_id.is_some()
            || self.relocation.version.is_some()
            || self.relocation.message.is_some()
        {
            let mut extra_data = self.package_data.extra_data.take().unwrap_or_default();
            if let Some(year) = self.inception_year {
                extra_data.insert(
                    "inception_year".to_string(),
                    serde_json::Value::String(year),
                );
            }
            if let Some(name) = self.organization_name {
                extra_data.insert(
                    "organization_name".to_string(),
                    serde_json::Value::String(name),
                );
            }
            if let Some(url) = self.organization_url {
                extra_data.insert(
                    "organization_url".to_string(),
                    serde_json::Value::String(url),
                );
            }
            if let Some(tag) = self.scm_tag {
                extra_data.insert("scm_tag".to_string(), serde_json::Value::String(tag));
            }
            if let Some(dev_conn) = self.scm_developer_connection {
                extra_data.insert(
                    "scm_developer_connection".to_string(),
                    serde_json::Value::String(dev_conn),
                );
            }
            if let Some(system) = self.issue_management_system {
                extra_data.insert(
                    "issue_tracking_system".to_string(),
                    serde_json::Value::String(system),
                );
            }
            if let Some(system) = self.ci_management_system {
                extra_data.insert("ci_system".to_string(), serde_json::Value::String(system));
            }
            if let Some(url) = self.ci_management_url {
                extra_data.insert("ci_url".to_string(), serde_json::Value::String(url));
            }
            if let Some(url) = self.dist_download_url {
                extra_data.insert(
                    "distribution_download_url".to_string(),
                    serde_json::Value::String(url),
                );
            }

            if self.dist_repository_id.is_some()
                || self.dist_repository_name.is_some()
                || self.dist_repository_url.is_some()
                || self.dist_repository_layout.is_some()
            {
                let mut repo = serde_json::Map::new();
                if let Some(id) = self.dist_repository_id {
                    repo.insert("id".to_string(), serde_json::Value::String(id));
                }
                if let Some(name) = self.dist_repository_name {
                    repo.insert("name".to_string(), serde_json::Value::String(name));
                }
                if let Some(url) = self.dist_repository_url {
                    repo.insert("url".to_string(), serde_json::Value::String(url));
                }
                if let Some(layout) = self.dist_repository_layout {
                    repo.insert("layout".to_string(), serde_json::Value::String(layout));
                }
                extra_data.insert(
                    "distribution_repository".to_string(),
                    serde_json::Value::Object(repo),
                );
            }

            if self.dist_snapshot_repository_id.is_some()
                || self.dist_snapshot_repository_name.is_some()
                || self.dist_snapshot_repository_url.is_some()
                || self.dist_snapshot_repository_layout.is_some()
            {
                let mut repo = serde_json::Map::new();
                if let Some(id) = self.dist_snapshot_repository_id {
                    repo.insert("id".to_string(), serde_json::Value::String(id));
                }
                if let Some(name) = self.dist_snapshot_repository_name {
                    repo.insert("name".to_string(), serde_json::Value::String(name));
                }
                if let Some(url) = self.dist_snapshot_repository_url {
                    repo.insert("url".to_string(), serde_json::Value::String(url));
                }
                if let Some(layout) = self.dist_snapshot_repository_layout {
                    repo.insert("layout".to_string(), serde_json::Value::String(layout));
                }
                extra_data.insert(
                    "distribution_snapshot_repository".to_string(),
                    serde_json::Value::Object(repo),
                );
            }

            if self.dist_site_id.is_some()
                || self.dist_site_name.is_some()
                || self.dist_site_url.is_some()
            {
                let mut site = serde_json::Map::new();
                if let Some(id) = self.dist_site_id {
                    site.insert("id".to_string(), serde_json::Value::String(id));
                }
                if let Some(name) = self.dist_site_name {
                    site.insert("name".to_string(), serde_json::Value::String(name));
                }
                if let Some(url) = self.dist_site_url {
                    site.insert("url".to_string(), serde_json::Value::String(url));
                }
                extra_data.insert(
                    "distribution_site".to_string(),
                    serde_json::Value::Object(site),
                );
            }

            if !self.repositories.is_empty() {
                extra_data.insert(
                    "repositories".to_string(),
                    serde_json::Value::Array(
                        self.repositories
                            .into_iter()
                            .map(serde_json::Value::Object)
                            .collect(),
                    ),
                );
            }

            if !self.plugin_repositories.is_empty() {
                extra_data.insert(
                    "plugin_repositories".to_string(),
                    serde_json::Value::Array(
                        self.plugin_repositories
                            .into_iter()
                            .map(serde_json::Value::Object)
                            .collect(),
                    ),
                );
            }

            if !self.modules.is_empty() {
                extra_data.insert(
                    "modules".to_string(),
                    serde_json::Value::Array(
                        self.modules
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }

            if !self.mailing_lists.is_empty() {
                extra_data.insert(
                    "mailing_lists".to_string(),
                    serde_json::Value::Array(
                        self.mailing_lists
                            .into_iter()
                            .map(serde_json::Value::Object)
                            .collect(),
                    ),
                );
            }

            if !self.dependency_management_entries.is_empty() {
                extra_data.insert(
                    "dependency_management".to_string(),
                    serde_json::Value::Array(
                        self.dependency_management_entries
                            .iter()
                            .map(|dependency| {
                                serde_json::Value::Object(dependency_management_entry_to_value(
                                    dependency,
                                ))
                            })
                            .collect(),
                    ),
                );
            }

            if self.relocation.group_id.is_some()
                || self.relocation.artifact_id.is_some()
                || self.relocation.version.is_some()
                || self.relocation.message.is_some()
            {
                extra_data.insert(
                    "relocation".to_string(),
                    serde_json::Value::Object(dependency_management_entry_to_value(
                        &self.relocation,
                    )),
                );
            }

            if self.parent_group_id.is_some()
                || self.parent_artifact_id.is_some()
                || self.parent_version.is_some()
                || self.parent_relative_path.is_some()
            {
                let mut parent_obj = serde_json::Map::new();
                if let Some(group_id) = self.parent_group_id {
                    parent_obj.insert("groupId".to_string(), serde_json::Value::String(group_id));
                }
                if let Some(artifact_id) = self.parent_artifact_id {
                    parent_obj.insert(
                        "artifactId".to_string(),
                        serde_json::Value::String(artifact_id),
                    );
                }
                if let Some(version) = self.parent_version {
                    parent_obj.insert("version".to_string(), serde_json::Value::String(version));
                }
                if let Some(relative_path) = self.parent_relative_path {
                    parent_obj.insert(
                        "relativePath".to_string(),
                        serde_json::Value::String(relative_path),
                    );
                }
                extra_data.insert("parent".to_string(), serde_json::Value::Object(parent_obj));
            }

            self.package_data.extra_data = Some(extra_data);
        }

        self.package_data.extracted_license_statement =
            build_license_statement(&self.licenses).map(truncate_field);
        let (declared_license_expression, declared_license_expression_spdx, license_detections) =
            build_maven_declared_license_data(
                &self.licenses,
                self.package_data.extracted_license_statement.as_deref(),
            );
        self.package_data.declared_license_expression = declared_license_expression;
        self.package_data.declared_license_expression_spdx = declared_license_expression_spdx;
        self.package_data.license_detections = license_detections;

        self.package_data.namespace = self.package_data.namespace.map(truncate_field);
        self.package_data.name = self.package_data.name.map(truncate_field);
        self.package_data.version = self.package_data.version.map(truncate_field);
        self.package_data.description = self.package_data.description.map(truncate_field);
        self.package_data.homepage_url = self.package_data.homepage_url.map(truncate_field);
        self.package_data.vcs_url = self.package_data.vcs_url.map(truncate_field);
        self.package_data.purl = self.package_data.purl.map(truncate_field);
        self.package_data.code_view_url = self.package_data.code_view_url.map(truncate_field);
        self.package_data.bug_tracking_url = self.package_data.bug_tracking_url.map(truncate_field);
        self.package_data.download_url = self.package_data.download_url.map(truncate_field);
        self.package_data.repository_homepage_url = self
            .package_data
            .repository_homepage_url
            .map(truncate_field);
        self.package_data.repository_download_url = self
            .package_data
            .repository_download_url
            .map(truncate_field);
        self.package_data.api_data_url = self.package_data.api_data_url.map(truncate_field);
        for dep in &mut self.package_data.dependencies {
            dep.purl = dep.purl.take().map(truncate_field);
            dep.extracted_requirement = dep.extracted_requirement.take().map(truncate_field);
        }

        self.package_data
    }
}
