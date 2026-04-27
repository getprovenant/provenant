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
use derive_builder::Builder;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Copy)]
struct ElementPath<'a> {
    current: &'a [u8],
    parent: Option<&'a [u8]>,
    depth: usize,
}

impl<'a> ElementPath<'a> {
    fn from_stack(stack: &'a [Vec<u8>]) -> Option<Self> {
        let current = stack.last()?.as_slice();
        let parent = stack
            .len()
            .checked_sub(2)
            .map(|index| stack[index].as_slice());

        Some(Self {
            current,
            parent,
            depth: stack.len(),
        })
    }

    fn parent_is(self, expected: &[u8]) -> bool {
        self.parent == Some(expected)
    }

    fn is_project_field(self) -> bool {
        self.depth == 2
    }
}

#[derive(Builder, Default, Serialize)]
#[builder(default, setter(into, strip_option))]
struct RepositoryEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

impl RepositoryEntryBuilder {
    fn set_field(&mut self, field: RepositoryField, value: String) {
        match field {
            RepositoryField::Id => {
                self.id(value);
            }
            RepositoryField::Name => {
                self.name(value);
            }
            RepositoryField::Url => {
                self.url(value);
            }
        }
    }

    fn finish(self) -> Option<serde_json::Map<String, serde_json::Value>> {
        serialize_non_empty_object(self.build().ok()?)
    }
}

#[derive(Builder, Default, Serialize)]
#[builder(default, setter(into, strip_option))]
struct MailingListEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subscribe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unsubscribe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    post: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archive: Option<String>,
}

impl MailingListEntryBuilder {
    fn set_field(&mut self, field: MailingListField, value: String) {
        match field {
            MailingListField::Name => {
                self.name(value);
            }
            MailingListField::Subscribe => {
                self.subscribe(value);
            }
            MailingListField::Unsubscribe => {
                self.unsubscribe(value);
            }
            MailingListField::Post => {
                self.post(value);
            }
            MailingListField::Archive => {
                self.archive(value);
            }
        }
    }

    fn finish(self) -> Option<serde_json::Map<String, serde_json::Value>> {
        serialize_non_empty_object(self.build().ok()?)
    }
}

fn serialize_non_empty_object<T: Serialize>(
    value: T,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    match serde_json::to_value(value).ok()? {
        serde_json::Value::Object(map) if !map.is_empty() => Some(map),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum DependencyField {
    GroupId,
    ArtifactId,
    Version,
    Scope,
    Optional,
    Type,
    Classifier,
    SystemPath,
}

#[derive(Clone, Copy)]
enum LicenseField {
    Name,
    Url,
    Comments,
}

#[derive(Clone, Copy)]
enum PartyField {
    Name,
    Email,
    Url,
    Organization,
    OrganizationUrl,
    Timezone,
}

#[derive(Clone, Copy)]
enum RelocationField {
    GroupId,
    ArtifactId,
    Version,
    Classifier,
    Type,
    Message,
}

#[derive(Clone, Copy)]
enum ParentField {
    GroupId,
    ArtifactId,
    Version,
    RelativePath,
}

#[derive(Clone, Copy)]
enum ProjectField {
    GroupId,
    ArtifactId,
    Version,
    Name,
    Description,
    Packaging,
    Classifier,
    Url,
    InceptionYear,
}

#[derive(Clone, Copy)]
enum ScmField {
    Connection,
    DeveloperConnection,
    Url,
    Tag,
}

#[derive(Clone, Copy)]
enum OrganizationField {
    Name,
    Url,
}

#[derive(Clone, Copy)]
enum ManagementField {
    System,
    Url,
}

#[derive(Clone, Copy)]
enum DistributionField {
    DownloadUrl,
}

#[derive(Clone, Copy)]
enum DistRepositoryField {
    Id,
    Name,
    Url,
    Layout,
}

#[derive(Clone, Copy)]
enum DistSiteField {
    Id,
    Name,
    Url,
}

#[derive(Clone, Copy)]
enum RepositoryField {
    Id,
    Name,
    Url,
}

#[derive(Clone, Copy)]
enum MailingListField {
    Name,
    Subscribe,
    Unsubscribe,
    Post,
    Archive,
}

#[derive(Clone, Copy)]
enum TextTarget {
    Ignore,
    DependencyManagement(DependencyField),
    License(LicenseField),
    Party(PartyField),
    Dependency(DependencyField),
    Relocation(RelocationField),
    Parent(ParentField),
    Project(ProjectField),
    Scm(ScmField),
    Organization(OrganizationField),
    IssueManagement(ManagementField),
    CiManagement(ManagementField),
    DistributionManagement(DistributionField),
    DistRepository(DistRepositoryField),
    DistSnapshotRepository(DistRepositoryField),
    DistSite(DistSiteField),
    Repository(RepositoryField),
    Module,
    MailingList(MailingListField),
}

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
    current_repository: Option<RepositoryEntryBuilder>,
    in_modules: bool,
    modules: Vec<String>,
    in_mailing_lists: bool,
    in_mailing_list: bool,
    mailing_lists: Vec<serde_json::Map<String, serde_json::Value>>,
    current_mailing_list: Option<MailingListEntryBuilder>,
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
            current_repository: None,
            in_modules: false,
            modules: Vec::new(),
            in_mailing_lists: false,
            in_mailing_list: false,
            mailing_lists: Vec::new(),
            current_mailing_list: None,
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
                self.current_repository = Some(RepositoryEntryBuilder::default());
            }
            b"pluginRepository" if self.in_plugin_repositories => {
                self.in_repository = true;
                self.current_repository = Some(RepositoryEntryBuilder::default());
            }
            b"modules" => self.in_modules = true,
            b"mailingLists" => self.in_mailing_lists = true,
            b"mailingList" if self.in_mailing_lists => {
                self.in_mailing_list = true;
                self.current_mailing_list = Some(MailingListEntryBuilder::default());
            }
            _ => {}
        }
    }

    pub(super) fn handle_text(&mut self, path: &Path, text: String) {
        let Some(element_path) = ElementPath::from_stack(&self.current_element) else {
            return;
        };

        if self.in_properties && element_path.parent_is(b"properties") {
            if let Ok(property_name) = std::str::from_utf8(element_path.current) {
                self.properties
                    .insert(property_name.to_string(), truncate_field(text));
            } else {
                warn!("Failed to decode Maven property name in {:?}", path);
            }
            return;
        }

        match self.text_target(element_path) {
            TextTarget::Ignore => {}
            TextTarget::DependencyManagement(field) => {
                self.apply_dependency_management_text(field, text)
            }
            TextTarget::License(field) => self.apply_license_text(field, text),
            TextTarget::Party(field) => self.apply_party_text(field, text),
            TextTarget::Dependency(field) => self.apply_dependency_text(field, text),
            TextTarget::Relocation(field) => self.apply_relocation_text(field, text),
            TextTarget::Parent(field) => self.apply_parent_text(field, text),
            TextTarget::Project(field) => self.apply_project_text(field, text),
            TextTarget::Scm(field) => self.apply_scm_text(field, text),
            TextTarget::Organization(field) => self.apply_organization_text(field, text),
            TextTarget::IssueManagement(field) => self.apply_issue_management_text(field, text),
            TextTarget::CiManagement(field) => self.apply_ci_management_text(field, text),
            TextTarget::DistributionManagement(field) => {
                self.apply_distribution_management_text(field, text)
            }
            TextTarget::DistRepository(field) => self.apply_dist_repository_text(field, text),
            TextTarget::DistSnapshotRepository(field) => {
                self.apply_dist_snapshot_repository_text(field, text)
            }
            TextTarget::DistSite(field) => self.apply_dist_site_text(field, text),
            TextTarget::Repository(field) => self.apply_repository_text(field, text),
            TextTarget::Module => self.modules.push(text),
            TextTarget::MailingList(field) => self.apply_mailing_list_text(field, text),
        }
    }

    fn text_target(&self, path: ElementPath<'_>) -> TextTarget {
        if self.in_dep_mgmt_dependency && path.parent_is(b"dependency") {
            return match path.current {
                b"groupId" => TextTarget::DependencyManagement(DependencyField::GroupId),
                b"artifactId" => TextTarget::DependencyManagement(DependencyField::ArtifactId),
                b"version" => TextTarget::DependencyManagement(DependencyField::Version),
                b"scope" => TextTarget::DependencyManagement(DependencyField::Scope),
                b"type" => TextTarget::DependencyManagement(DependencyField::Type),
                b"classifier" => TextTarget::DependencyManagement(DependencyField::Classifier),
                b"optional" => TextTarget::DependencyManagement(DependencyField::Optional),
                _ => TextTarget::Ignore,
            };
        }

        if self.current_license.is_some() {
            return match path.current {
                b"name" => TextTarget::License(LicenseField::Name),
                b"url" => TextTarget::License(LicenseField::Url),
                b"comments" => TextTarget::License(LicenseField::Comments),
                _ => TextTarget::Ignore,
            };
        }

        if self.current_party.is_some() {
            return match path.current {
                b"name" => TextTarget::Party(PartyField::Name),
                b"email" => TextTarget::Party(PartyField::Email),
                b"url" => TextTarget::Party(PartyField::Url),
                b"organization" => TextTarget::Party(PartyField::Organization),
                b"organizationUrl" => TextTarget::Party(PartyField::OrganizationUrl),
                b"timezone" => TextTarget::Party(PartyField::Timezone),
                _ => TextTarget::Ignore,
            };
        }

        if self.current_dependency.is_some() && path.parent_is(b"dependency") {
            return match path.current {
                b"groupId" => TextTarget::Dependency(DependencyField::GroupId),
                b"artifactId" => TextTarget::Dependency(DependencyField::ArtifactId),
                b"version" => TextTarget::Dependency(DependencyField::Version),
                b"scope" => TextTarget::Dependency(DependencyField::Scope),
                b"optional" => TextTarget::Dependency(DependencyField::Optional),
                b"type" => TextTarget::Dependency(DependencyField::Type),
                b"classifier" => TextTarget::Dependency(DependencyField::Classifier),
                b"systemPath" => TextTarget::Dependency(DependencyField::SystemPath),
                _ => TextTarget::Ignore,
            };
        }

        if self.in_relocation {
            return match path.current {
                b"groupId" => TextTarget::Relocation(RelocationField::GroupId),
                b"artifactId" => TextTarget::Relocation(RelocationField::ArtifactId),
                b"version" => TextTarget::Relocation(RelocationField::Version),
                b"classifier" => TextTarget::Relocation(RelocationField::Classifier),
                b"type" => TextTarget::Relocation(RelocationField::Type),
                b"message" => TextTarget::Relocation(RelocationField::Message),
                _ => TextTarget::Ignore,
            };
        }

        if self.in_parent {
            return match path.current {
                b"groupId" => TextTarget::Parent(ParentField::GroupId),
                b"artifactId" => TextTarget::Parent(ParentField::ArtifactId),
                b"version" => TextTarget::Parent(ParentField::Version),
                b"relativePath" => TextTarget::Parent(ParentField::RelativePath),
                _ => TextTarget::Ignore,
            };
        }

        if path.is_project_field() {
            return match path.current {
                b"groupId" => TextTarget::Project(ProjectField::GroupId),
                b"artifactId" => TextTarget::Project(ProjectField::ArtifactId),
                b"version" => TextTarget::Project(ProjectField::Version),
                b"name" => TextTarget::Project(ProjectField::Name),
                b"description" => TextTarget::Project(ProjectField::Description),
                b"packaging" => TextTarget::Project(ProjectField::Packaging),
                b"classifier" => TextTarget::Project(ProjectField::Classifier),
                b"url" => TextTarget::Project(ProjectField::Url),
                b"inceptionYear" => TextTarget::Project(ProjectField::InceptionYear),
                _ => TextTarget::Ignore,
            };
        }

        if path.parent_is(b"scm") {
            return match path.current {
                b"connection" => TextTarget::Scm(ScmField::Connection),
                b"developerConnection" => TextTarget::Scm(ScmField::DeveloperConnection),
                b"url" => TextTarget::Scm(ScmField::Url),
                b"tag" => TextTarget::Scm(ScmField::Tag),
                _ => TextTarget::Ignore,
            };
        }

        if path.parent_is(b"organization") {
            return match path.current {
                b"name" => TextTarget::Organization(OrganizationField::Name),
                b"url" => TextTarget::Organization(OrganizationField::Url),
                _ => TextTarget::Ignore,
            };
        }

        if path.parent_is(b"issueManagement") {
            return match path.current {
                b"system" => TextTarget::IssueManagement(ManagementField::System),
                b"url" => TextTarget::IssueManagement(ManagementField::Url),
                _ => TextTarget::Ignore,
            };
        }

        if path.parent_is(b"ciManagement") {
            return match path.current {
                b"system" => TextTarget::CiManagement(ManagementField::System),
                b"url" => TextTarget::CiManagement(ManagementField::Url),
                _ => TextTarget::Ignore,
            };
        }

        if path.parent_is(b"distributionManagement") && path.current == b"downloadUrl" {
            return TextTarget::DistributionManagement(DistributionField::DownloadUrl);
        }

        if self.in_dist_repository {
            return match path.current {
                b"id" => TextTarget::DistRepository(DistRepositoryField::Id),
                b"name" => TextTarget::DistRepository(DistRepositoryField::Name),
                b"url" => TextTarget::DistRepository(DistRepositoryField::Url),
                b"layout" => TextTarget::DistRepository(DistRepositoryField::Layout),
                _ => TextTarget::Ignore,
            };
        }

        if self.in_dist_snapshot_repository {
            return match path.current {
                b"id" => TextTarget::DistSnapshotRepository(DistRepositoryField::Id),
                b"name" => TextTarget::DistSnapshotRepository(DistRepositoryField::Name),
                b"url" => TextTarget::DistSnapshotRepository(DistRepositoryField::Url),
                b"layout" => TextTarget::DistSnapshotRepository(DistRepositoryField::Layout),
                _ => TextTarget::Ignore,
            };
        }

        if self.in_dist_site {
            return match path.current {
                b"id" => TextTarget::DistSite(DistSiteField::Id),
                b"name" => TextTarget::DistSite(DistSiteField::Name),
                b"url" => TextTarget::DistSite(DistSiteField::Url),
                _ => TextTarget::Ignore,
            };
        }

        if self.in_repository {
            return match path.current {
                b"id" => TextTarget::Repository(RepositoryField::Id),
                b"name" => TextTarget::Repository(RepositoryField::Name),
                b"url" => TextTarget::Repository(RepositoryField::Url),
                _ => TextTarget::Ignore,
            };
        }

        if self.in_modules && path.current == b"module" {
            return TextTarget::Module;
        }

        if self.in_mailing_list {
            return match path.current {
                b"name" => TextTarget::MailingList(MailingListField::Name),
                b"subscribe" => TextTarget::MailingList(MailingListField::Subscribe),
                b"unsubscribe" => TextTarget::MailingList(MailingListField::Unsubscribe),
                b"post" => TextTarget::MailingList(MailingListField::Post),
                b"archive" => TextTarget::MailingList(MailingListField::Archive),
                _ => TextTarget::Ignore,
            };
        }

        TextTarget::Ignore
    }

    fn apply_dependency_management_text(&mut self, field: DependencyField, text: String) {
        if let Some(dep_mgmt) = self.current_dep_mgmt_dependency.as_mut() {
            Self::set_dependency_field(dep_mgmt, field, text);
        }
    }

    fn apply_license_text(&mut self, field: LicenseField, text: String) {
        if let Some(license) = self.current_license.as_mut() {
            match field {
                LicenseField::Name => license.name = Some(text),
                LicenseField::Url => license.url = Some(text),
                LicenseField::Comments => license.comments = Some(text),
            }
        }
    }

    fn apply_party_text(&mut self, field: PartyField, text: String) {
        if let Some(party) = self.current_party.as_mut() {
            match field {
                PartyField::Name => party.name = Some(text),
                PartyField::Email => party.email = Some(text),
                PartyField::Url => party.url = Some(text),
                PartyField::Organization => party.organization = Some(text),
                PartyField::OrganizationUrl => party.organization_url = Some(text),
                PartyField::Timezone => party.timezone = Some(text),
            }
        }
    }

    fn apply_dependency_text(&mut self, field: DependencyField, text: String) {
        if matches!(field, DependencyField::Scope)
            && let Some(dep) = self.current_dependency.as_mut()
        {
            dep.scope = Some(text.clone());
            dep.is_optional = Some(text == "test" || text == "provided");
            dep.is_runtime = Some(text != "test" && text != "provided");
        }

        if let Some(coords) = self.current_dependency_data.as_mut() {
            Self::set_dependency_field(coords, field, text);
        }
    }

    fn apply_relocation_text(&mut self, field: RelocationField, text: String) {
        match field {
            RelocationField::GroupId => self.relocation.group_id = Some(text),
            RelocationField::ArtifactId => self.relocation.artifact_id = Some(text),
            RelocationField::Version => self.relocation.version = Some(text),
            RelocationField::Classifier => self.relocation.classifier = Some(text),
            RelocationField::Type => self.relocation.type_ = Some(text),
            RelocationField::Message => self.relocation.message = Some(text),
        }
    }

    fn apply_parent_text(&mut self, field: ParentField, text: String) {
        match field {
            ParentField::GroupId => self.parent_group_id = Some(text),
            ParentField::ArtifactId => self.parent_artifact_id = Some(text),
            ParentField::Version => self.parent_version = Some(text),
            ParentField::RelativePath => self.parent_relative_path = Some(text),
        }
    }

    fn apply_project_text(&mut self, field: ProjectField, text: String) {
        match field {
            ProjectField::GroupId => self.package_data.namespace = Some(text),
            ProjectField::ArtifactId => self.package_data.name = Some(text),
            ProjectField::Version => self.package_data.version = Some(text),
            ProjectField::Name => self.project_name = Some(text),
            ProjectField::Description => self.project_description = Some(text),
            ProjectField::Packaging => self.project_packaging = Some(text),
            ProjectField::Classifier => self.project_classifier = Some(text),
            ProjectField::Url => self.package_data.homepage_url = Some(text),
            ProjectField::InceptionYear => self.inception_year = Some(text),
        }
    }

    fn apply_scm_text(&mut self, field: ScmField, text: String) {
        match field {
            ScmField::Connection => {
                self.scm_connection = Some(Self::normalize_scm_connection(text))
            }
            ScmField::DeveloperConnection => {
                self.scm_developer_connection = Some(Self::normalize_scm_connection(text));
            }
            ScmField::Url => self.scm_url = Some(text),
            ScmField::Tag => self.scm_tag = Some(text),
        }
    }

    fn apply_organization_text(&mut self, field: OrganizationField, text: String) {
        match field {
            OrganizationField::Name => self.organization_name = Some(text),
            OrganizationField::Url => self.organization_url = Some(text),
        }
    }

    fn apply_issue_management_text(&mut self, field: ManagementField, text: String) {
        match field {
            ManagementField::System => self.issue_management_system = Some(text),
            ManagementField::Url => self.issue_management_url = Some(text),
        }
    }

    fn apply_ci_management_text(&mut self, field: ManagementField, text: String) {
        match field {
            ManagementField::System => self.ci_management_system = Some(text),
            ManagementField::Url => self.ci_management_url = Some(text),
        }
    }

    fn apply_distribution_management_text(&mut self, field: DistributionField, text: String) {
        match field {
            DistributionField::DownloadUrl => self.dist_download_url = Some(text),
        }
    }

    fn apply_dist_repository_text(&mut self, field: DistRepositoryField, text: String) {
        match field {
            DistRepositoryField::Id => self.dist_repository_id = Some(text),
            DistRepositoryField::Name => self.dist_repository_name = Some(text),
            DistRepositoryField::Url => self.dist_repository_url = Some(text),
            DistRepositoryField::Layout => self.dist_repository_layout = Some(text),
        }
    }

    fn apply_dist_snapshot_repository_text(&mut self, field: DistRepositoryField, text: String) {
        match field {
            DistRepositoryField::Id => self.dist_snapshot_repository_id = Some(text),
            DistRepositoryField::Name => self.dist_snapshot_repository_name = Some(text),
            DistRepositoryField::Url => self.dist_snapshot_repository_url = Some(text),
            DistRepositoryField::Layout => self.dist_snapshot_repository_layout = Some(text),
        }
    }

    fn apply_dist_site_text(&mut self, field: DistSiteField, text: String) {
        match field {
            DistSiteField::Id => self.dist_site_id = Some(text),
            DistSiteField::Name => self.dist_site_name = Some(text),
            DistSiteField::Url => self.dist_site_url = Some(text),
        }
    }

    fn apply_repository_text(&mut self, field: RepositoryField, text: String) {
        if let Some(repository) = self.current_repository.as_mut() {
            repository.set_field(field, text);
        }
    }

    fn apply_mailing_list_text(&mut self, field: MailingListField, text: String) {
        if let Some(mailing_list) = self.current_mailing_list.as_mut() {
            mailing_list.set_field(field, text);
        }
    }

    fn set_dependency_field(
        dependency: &mut MavenDependencyData,
        field: DependencyField,
        text: String,
    ) {
        match field {
            DependencyField::GroupId => dependency.group_id = Some(text),
            DependencyField::ArtifactId => dependency.artifact_id = Some(text),
            DependencyField::Version => dependency.version = Some(text),
            DependencyField::Scope => dependency.scope = Some(text),
            DependencyField::Optional => dependency.optional = Some(text),
            DependencyField::Type => dependency.type_ = Some(text),
            DependencyField::Classifier => dependency.classifier = Some(text),
            DependencyField::SystemPath => dependency.system_path = Some(text),
        }
    }

    fn normalize_scm_connection(text: String) -> String {
        if text.starts_with("scm:git:") {
            text.replacen("scm:git:", "git+", 1)
        } else if text.starts_with("scm:") {
            text.replacen("scm:", "", 1)
        } else {
            text
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
                if let Some(repo) = self
                    .current_repository
                    .take()
                    .and_then(RepositoryEntryBuilder::finish)
                {
                    self.repositories.push(repo);
                }
            }
            b"pluginRepository" if self.in_plugin_repositories => {
                self.in_repository = false;
                if let Some(repo) = self
                    .current_repository
                    .take()
                    .and_then(RepositoryEntryBuilder::finish)
                {
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
                if let Some(ml) = self
                    .current_mailing_list
                    .take()
                    .and_then(MailingListEntryBuilder::finish)
                {
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
