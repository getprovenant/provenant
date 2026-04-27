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

#[derive(Clone)]
struct ElementPath {
    current: Vec<u8>,
    parent: Option<Vec<u8>>,
    depth: usize,
}

impl ElementPath {
    fn from_stack(stack: &[ElementFrame]) -> Option<Self> {
        let current = stack.last()?.name().to_vec();
        let parent = stack
            .len()
            .checked_sub(2)
            .map(|index| stack[index].name().to_vec());

        Some(Self {
            current,
            parent,
            depth: stack.len(),
        })
    }

    fn parent_is(&self, expected: &[u8]) -> bool {
        self.parent.as_deref() == Some(expected)
    }

    fn is_project_field(&self) -> bool {
        self.depth == 2
    }

    fn current(&self) -> &[u8] {
        self.current.as_slice()
    }
}

struct ElementFrame {
    name: Vec<u8>,
    context: FrameContext,
}

impl ElementFrame {
    fn new(name: Vec<u8>, context: FrameContext) -> Self {
        Self { name, context }
    }

    fn name(&self) -> &[u8] {
        self.name.as_slice()
    }
}

enum FrameContext {
    Plain,
    Section(ActiveSection),
    PartyList(PartyList),
    RepositoryCollection(RepositoryCollection),
    Distribution(DistributionSection),
    DependencyContext(DependencyContext),
    License(MavenLicenseEntry),
    Party(Party),
    Repository {
        collection: RepositoryCollection,
        builder: RepositoryEntryBuilder,
    },
    MailingList(MailingListEntryBuilder),
    Dependency(ActiveDependency),
}

impl FrameContext {
    fn for_start(state: &mut PomParseState, element_name: &[u8]) -> Self {
        Self::start_dependency_context(state, element_name)
            .or_else(|| Self::start_party_context(state, element_name))
            .or_else(|| Self::start_distribution_context(state, element_name))
            .or_else(|| Self::start_repository_context(state, element_name))
            .or_else(|| Self::start_section_context(state, element_name))
            .or_else(|| Self::start_item_context(state, element_name))
            .unwrap_or(Self::Plain)
    }

    fn start_dependency_context(state: &PomParseState, element_name: &[u8]) -> Option<Self> {
        match element_name {
            b"dependency"
                if state.current_context(FrameContext::dependency_context)
                    == Some(DependencyContext::ManagementEntries) =>
            {
                Some(Self::Dependency(ActiveDependency::Management(
                    MavenDependencyData::default(),
                )))
            }
            b"dependency"
                if state.current_context(FrameContext::dependency_context)
                    == Some(DependencyContext::PackageEntries) =>
            {
                Some(Self::Dependency(ActiveDependency::Package {
                    package: PomAccumulator::new_package_dependency(),
                    data: MavenDependencyData::default(),
                }))
            }
            b"dependencyManagement" => Some(Self::DependencyContext(
                DependencyContext::ManagementContainer,
            )),
            b"dependencies"
                if state.current_context(FrameContext::dependency_context)
                    == Some(DependencyContext::ManagementContainer) =>
            {
                Some(Self::DependencyContext(
                    DependencyContext::ManagementEntries,
                ))
            }
            b"dependencies" => Some(Self::DependencyContext(DependencyContext::PackageEntries)),
            _ => None,
        }
    }

    fn start_party_context(state: &PomParseState, element_name: &[u8]) -> Option<Self> {
        match element_name {
            b"developers" => Some(Self::PartyList(PartyList::Developers)),
            b"contributors" => Some(Self::PartyList(PartyList::Contributors)),
            b"developer"
                if state.current_context(FrameContext::party_list)
                    == Some(PartyList::Developers) =>
            {
                Some(Self::Party(PomAccumulator::new_party("developer")))
            }
            b"contributor"
                if state.current_context(FrameContext::party_list)
                    == Some(PartyList::Contributors) =>
            {
                Some(Self::Party(PomAccumulator::new_party("contributor")))
            }
            _ => None,
        }
    }

    fn start_distribution_context(state: &PomParseState, element_name: &[u8]) -> Option<Self> {
        match element_name {
            b"distributionManagement" => Some(Self::Distribution(DistributionSection::Management)),
            b"repository"
                if state
                    .current_context(FrameContext::distribution_section)
                    .is_some() =>
            {
                Some(Self::Distribution(DistributionSection::Repository))
            }
            b"snapshotRepository"
                if state
                    .current_context(FrameContext::distribution_section)
                    .is_some() =>
            {
                Some(Self::Distribution(DistributionSection::SnapshotRepository))
            }
            b"site"
                if state
                    .current_context(FrameContext::distribution_section)
                    .is_some() =>
            {
                Some(Self::Distribution(DistributionSection::Site))
            }
            _ => None,
        }
    }

    fn start_repository_context(state: &PomParseState, element_name: &[u8]) -> Option<Self> {
        match element_name {
            b"repositories" => Some(Self::RepositoryCollection(
                RepositoryCollection::Repositories,
            )),
            b"pluginRepositories" => Some(Self::RepositoryCollection(
                RepositoryCollection::PluginRepositories,
            )),
            b"repository"
                if state.current_context(FrameContext::repository_collection)
                    == Some(RepositoryCollection::Repositories)
                    && state.current_context(FrameContext::dependency_context)
                        != Some(DependencyContext::PackageEntries) =>
            {
                Some(Self::Repository {
                    collection: RepositoryCollection::Repositories,
                    builder: RepositoryEntryBuilder::default(),
                })
            }
            b"pluginRepository"
                if state.current_context(FrameContext::repository_collection)
                    == Some(RepositoryCollection::PluginRepositories) =>
            {
                Some(Self::Repository {
                    collection: RepositoryCollection::PluginRepositories,
                    builder: RepositoryEntryBuilder::default(),
                })
            }
            _ => None,
        }
    }

    fn start_section_context(state: &mut PomParseState, element_name: &[u8]) -> Option<Self> {
        match element_name {
            b"parent" => Some(Self::Section(ActiveSection::Parent)),
            b"properties" => Some(Self::Section(ActiveSection::Properties)),
            b"relocation"
                if state
                    .current_context(FrameContext::distribution_section)
                    .is_some() =>
            {
                state.acc.relocation = MavenDependencyData::default();
                Some(Self::Section(ActiveSection::Relocation))
            }
            b"modules" => Some(Self::Section(ActiveSection::Modules)),
            b"mailingLists" => Some(Self::Section(ActiveSection::MailingLists)),
            _ => None,
        }
    }

    fn start_item_context(state: &PomParseState, element_name: &[u8]) -> Option<Self> {
        match (element_name, state.current_context(FrameContext::section)) {
            (b"license", _) => Some(Self::License(MavenLicenseEntry::default())),
            (b"mailingList", Some(ActiveSection::MailingLists)) => {
                Some(Self::MailingList(MailingListEntryBuilder::default()))
            }
            _ => None,
        }
    }

    fn section(&self) -> Option<ActiveSection> {
        match self {
            Self::Section(section) => Some(*section),
            _ => None,
        }
    }

    fn party_list(&self) -> Option<PartyList> {
        match self {
            Self::PartyList(party_list) => Some(*party_list),
            _ => None,
        }
    }

    fn repository_collection(&self) -> Option<RepositoryCollection> {
        match self {
            Self::RepositoryCollection(collection) => Some(*collection),
            _ => None,
        }
    }

    fn distribution_section(&self) -> Option<DistributionSection> {
        match self {
            Self::Distribution(section) => Some(*section),
            _ => None,
        }
    }

    fn dependency_context(&self) -> Option<DependencyContext> {
        match self {
            Self::DependencyContext(context) => Some(*context),
            _ => None,
        }
    }

    fn apply_text(
        &mut self,
        acc: &mut PomAccumulator,
        source_path: &Path,
        path: &ElementPath,
        text: &str,
    ) -> bool {
        match self {
            Self::Dependency(dependency) => dependency.apply_text(path, text),
            Self::License(license) => {
                license.apply_text(path.current(), text);
                true
            }
            Self::Party(party) => {
                party.apply_text(path.current(), text);
                true
            }
            Self::Repository { builder, .. } => {
                builder.apply_text(path.current(), text);
                true
            }
            Self::MailingList(mailing_list) => {
                mailing_list.apply_text(path.current(), text);
                true
            }
            Self::Section(section) => section.apply_text(acc, source_path, path, text),
            Self::Distribution(distribution) => distribution.apply_text(acc, path.current(), text),
            _ => false,
        }
    }

    fn finish(self, acc: &mut PomAccumulator) {
        match self {
            Self::Dependency(ActiveDependency::Management(dep_mgmt))
                if dep_mgmt.group_id.is_some()
                    || dep_mgmt.artifact_id.is_some()
                    || dep_mgmt.version.is_some() =>
            {
                acc.dependency_management_entries.push(dep_mgmt);
            }
            Self::Dependency(ActiveDependency::Management(_)) => {}
            Self::Dependency(ActiveDependency::Package { package, data }) => {
                acc.package_data.dependencies.push(package);
                acc.dependency_data.push(data);
            }
            Self::License(license)
                if license.name.is_some()
                    || license.url.is_some()
                    || license.comments.is_some() =>
            {
                acc.licenses.push(license);
            }
            Self::Party(party) => {
                acc.package_data.parties.push(party);
            }
            Self::Repository {
                collection,
                builder,
            } => {
                if let Some(repo) = RepositoryEntryBuilder::finish(builder) {
                    match collection {
                        RepositoryCollection::Repositories => acc.repositories.push(repo),
                        RepositoryCollection::PluginRepositories => {
                            acc.plugin_repositories.push(repo)
                        }
                    }
                }
            }
            Self::MailingList(mailing_list) => {
                if let Some(ml) = MailingListEntryBuilder::finish(mailing_list) {
                    acc.mailing_lists.push(ml);
                }
            }
            _ => {}
        }
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
    fn apply_text(&mut self, current: &[u8], text: &str) {
        match current {
            b"id" => {
                self.id(text.to_string());
            }
            b"name" => {
                self.name(text.to_string());
            }
            b"url" => {
                self.url(text.to_string());
            }
            _ => {}
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
    fn apply_text(&mut self, current: &[u8], text: &str) {
        match current {
            b"name" => {
                self.name(text.to_string());
            }
            b"subscribe" => {
                self.subscribe(text.to_string());
            }
            b"unsubscribe" => {
                self.unsubscribe(text.to_string());
            }
            b"post" => {
                self.post(text.to_string());
            }
            b"archive" => {
                self.archive(text.to_string());
            }
            _ => {}
        }
    }

    fn finish(self) -> Option<serde_json::Map<String, serde_json::Value>> {
        serialize_non_empty_object(self.build().ok()?)
    }
}

#[derive(Default, Serialize)]
struct DistributionRepositoryEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout: Option<String>,
}

#[derive(Default, Serialize)]
struct DistributionSiteEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Default, Serialize)]
struct ParentEntry {
    #[serde(rename = "groupId", skip_serializing_if = "Option::is_none")]
    group_id: Option<String>,
    #[serde(rename = "artifactId", skip_serializing_if = "Option::is_none")]
    artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(rename = "relativePath", skip_serializing_if = "Option::is_none")]
    relative_path: Option<String>,
}

fn serialize_non_empty_object<T: Serialize>(
    value: T,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    match serde_json::to_value(value).ok()? {
        serde_json::Value::Object(map) if !map.is_empty() => Some(map),
        _ => None,
    }
}

fn insert_extra_data_object<T: Serialize>(
    extra_data: &mut HashMap<String, serde_json::Value>,
    key: &str,
    value: T,
) {
    if let Some(object) = serialize_non_empty_object(value) {
        extra_data.insert(key.to_string(), serde_json::Value::Object(object));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PartyList {
    Developers,
    Contributors,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RepositoryCollection {
    Repositories,
    PluginRepositories,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DistributionSection {
    Management,
    Repository,
    SnapshotRepository,
    Site,
}

enum ActiveDependency {
    Package {
        package: Dependency,
        data: MavenDependencyData,
    },
    Management(MavenDependencyData),
}

impl ActiveDependency {
    fn apply_text(&mut self, path: &ElementPath, text: &str) -> bool {
        if !path.parent_is(b"dependency") {
            return false;
        }

        match self {
            Self::Management(dependency) => {
                match path.current() {
                    b"groupId" => dependency.group_id = Some(text.to_string()),
                    b"artifactId" => dependency.artifact_id = Some(text.to_string()),
                    b"version" => dependency.version = Some(text.to_string()),
                    b"scope" => dependency.scope = Some(text.to_string()),
                    b"type" => dependency.type_ = Some(text.to_string()),
                    b"classifier" => dependency.classifier = Some(text.to_string()),
                    b"optional" => dependency.optional = Some(text.to_string()),
                    _ => {}
                }
                true
            }
            Self::Package { package, data } => {
                match path.current() {
                    b"groupId" => data.group_id = Some(text.to_string()),
                    b"artifactId" => data.artifact_id = Some(text.to_string()),
                    b"version" => data.version = Some(text.to_string()),
                    b"scope" => {
                        let scope = text.to_string();
                        package.scope = Some(scope.clone());
                        package.is_optional = Some(scope == "test" || scope == "provided");
                        package.is_runtime = Some(scope != "test" && scope != "provided");
                        data.scope = Some(scope);
                    }
                    b"optional" => data.optional = Some(text.to_string()),
                    b"type" => data.type_ = Some(text.to_string()),
                    b"classifier" => data.classifier = Some(text.to_string()),
                    b"systemPath" => data.system_path = Some(text.to_string()),
                    _ => {}
                }
                true
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DependencyContext {
    ManagementContainer,
    ManagementEntries,
    PackageEntries,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveSection {
    Parent,
    Properties,
    Relocation,
    Modules,
    MailingLists,
}

impl ActiveSection {
    fn apply_text(
        self,
        state: &mut PomAccumulator,
        source_path: &Path,
        path: &ElementPath,
        text: &str,
    ) -> bool {
        match self {
            Self::Relocation => {
                match path.current() {
                    b"groupId" => state.relocation.group_id = Some(text.to_string()),
                    b"artifactId" => state.relocation.artifact_id = Some(text.to_string()),
                    b"version" => state.relocation.version = Some(text.to_string()),
                    b"classifier" => state.relocation.classifier = Some(text.to_string()),
                    b"type" => state.relocation.type_ = Some(text.to_string()),
                    b"message" => state.relocation.message = Some(text.to_string()),
                    _ => {}
                }
                true
            }
            Self::Parent => {
                match path.current() {
                    b"groupId" => state.parent_group_id = Some(text.to_string()),
                    b"artifactId" => state.parent_artifact_id = Some(text.to_string()),
                    b"version" => state.parent_version = Some(text.to_string()),
                    b"relativePath" => state.parent_relative_path = Some(text.to_string()),
                    _ => {}
                }
                true
            }
            Self::Modules => {
                if path.current() == b"module" {
                    state.modules.push(text.to_string());
                }
                true
            }
            Self::Properties => {
                if path.parent_is(b"properties") {
                    if let Ok(property_name) = std::str::from_utf8(path.current()) {
                        state
                            .properties
                            .insert(property_name.to_string(), truncate_field(text.to_string()));
                    } else {
                        warn!("Failed to decode Maven property name in {:?}", source_path);
                    }
                    true
                } else {
                    false
                }
            }
            Self::MailingLists => false,
        }
    }
}

impl DistributionSection {
    fn apply_text(self, state: &mut PomAccumulator, current: &[u8], text: &str) -> bool {
        match self {
            Self::Repository => {
                match current {
                    b"id" => state.dist_repository_id = Some(text.to_string()),
                    b"name" => state.dist_repository_name = Some(text.to_string()),
                    b"url" => state.dist_repository_url = Some(text.to_string()),
                    b"layout" => state.dist_repository_layout = Some(text.to_string()),
                    _ => {}
                }
                true
            }
            Self::SnapshotRepository => {
                match current {
                    b"id" => state.dist_snapshot_repository_id = Some(text.to_string()),
                    b"name" => state.dist_snapshot_repository_name = Some(text.to_string()),
                    b"url" => state.dist_snapshot_repository_url = Some(text.to_string()),
                    b"layout" => state.dist_snapshot_repository_layout = Some(text.to_string()),
                    _ => {}
                }
                true
            }
            Self::Site => {
                match current {
                    b"id" => state.dist_site_id = Some(text.to_string()),
                    b"name" => state.dist_site_name = Some(text.to_string()),
                    b"url" => state.dist_site_url = Some(text.to_string()),
                    _ => {}
                }
                true
            }
            Self::Management => false,
        }
    }
}

impl MavenLicenseEntry {
    fn apply_text(&mut self, current: &[u8], text: &str) {
        match current {
            b"name" => self.name = Some(text.to_string()),
            b"url" => self.url = Some(text.to_string()),
            b"comments" => self.comments = Some(text.to_string()),
            _ => {}
        }
    }
}

impl Party {
    fn apply_text(&mut self, current: &[u8], text: &str) {
        match current {
            b"name" => self.name = Some(text.to_string()),
            b"email" => self.email = Some(text.to_string()),
            b"url" => self.url = Some(text.to_string()),
            b"organization" => self.organization = Some(text.to_string()),
            b"organizationUrl" => self.organization_url = Some(text.to_string()),
            b"timezone" => self.timezone = Some(text.to_string()),
            _ => {}
        }
    }
}

struct PomAccumulator {
    package_data: PackageData,
    dependency_data: Vec<MavenDependencyData>,
    licenses: Vec<MavenLicenseEntry>,
    xml_license_comments: Vec<String>,
    inception_year: Option<String>,
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
    repositories: Vec<serde_json::Map<String, serde_json::Value>>,
    plugin_repositories: Vec<serde_json::Map<String, serde_json::Value>>,
    modules: Vec<String>,
    mailing_lists: Vec<serde_json::Map<String, serde_json::Value>>,
    dependency_management_entries: Vec<MavenDependencyData>,
    parent_group_id: Option<String>,
    parent_artifact_id: Option<String>,
    parent_version: Option<String>,
    parent_relative_path: Option<String>,
    properties: HashMap<String, String>,
    project_name: Option<String>,
    project_description: Option<String>,
    project_packaging: Option<String>,
    project_classifier: Option<String>,
    relocation: MavenDependencyData,
}

pub(super) struct PomParseState {
    context_stack: Vec<ElementFrame>,
    acc: PomAccumulator,
}

impl PomAccumulator {
    fn new() -> Self {
        let mut package_data = default_package_data(DatasourceId::MavenPom);
        package_data.package_type = Some(PackageType::Maven);
        package_data.primary_language = Some("Java".to_string());
        package_data.datasource_id = Some(DatasourceId::MavenPom);

        Self {
            package_data,
            dependency_data: Vec::new(),
            licenses: Vec::new(),
            xml_license_comments: Vec::new(),
            inception_year: None,
            scm_connection: None,
            scm_developer_connection: None,
            scm_url: None,
            scm_tag: None,
            organization_name: None,
            organization_url: None,
            issue_management_system: None,
            issue_management_url: None,
            ci_management_system: None,
            ci_management_url: None,
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
            repositories: Vec::new(),
            plugin_repositories: Vec::new(),
            modules: Vec::new(),
            mailing_lists: Vec::new(),
            dependency_management_entries: Vec::new(),
            parent_group_id: None,
            parent_artifact_id: None,
            parent_version: None,
            parent_relative_path: None,
            properties: HashMap::new(),
            project_name: None,
            project_description: None,
            project_packaging: None,
            project_classifier: None,
            relocation: MavenDependencyData::default(),
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

    fn new_party(role: &str) -> Party {
        Party {
            r#type: Some("person".to_string()),
            role: Some(role.to_string()),
            name: None,
            email: None,
            url: None,
            organization: None,
            organization_url: None,
            timezone: None,
        }
    }

    fn new_package_dependency() -> Dependency {
        Dependency {
            purl: None,
            extracted_requirement: None,
            scope: None,
            is_runtime: None,
            is_optional: Some(false),
            is_pinned: None,
            is_direct: Some(true),
            resolved_package: None,
            extra_data: None,
        }
    }

    fn apply_structural_text(&mut self, path: ElementPath, text: &str) {
        if path.is_project_field() {
            match path.current() {
                b"groupId" => self.package_data.namespace = Some(text.to_string()),
                b"artifactId" => self.package_data.name = Some(text.to_string()),
                b"version" => self.package_data.version = Some(text.to_string()),
                b"name" => self.project_name = Some(text.to_string()),
                b"description" => self.project_description = Some(text.to_string()),
                b"packaging" => self.project_packaging = Some(text.to_string()),
                b"classifier" => self.project_classifier = Some(text.to_string()),
                b"url" => self.package_data.homepage_url = Some(text.to_string()),
                b"inceptionYear" => self.inception_year = Some(text.to_string()),
                _ => {}
            }
            return;
        }

        if path.parent_is(b"scm") {
            match path.current() {
                b"connection" => {
                    self.scm_connection = Some(Self::normalize_scm_connection(text.to_string()))
                }
                b"developerConnection" => {
                    self.scm_developer_connection =
                        Some(Self::normalize_scm_connection(text.to_string()));
                }
                b"url" => self.scm_url = Some(text.to_string()),
                b"tag" => self.scm_tag = Some(text.to_string()),
                _ => {}
            }
            return;
        }

        if path.parent_is(b"organization") {
            match path.current() {
                b"name" => self.organization_name = Some(text.to_string()),
                b"url" => self.organization_url = Some(text.to_string()),
                _ => {}
            }
            return;
        }

        if path.parent_is(b"issueManagement") {
            match path.current() {
                b"system" => self.issue_management_system = Some(text.to_string()),
                b"url" => self.issue_management_url = Some(text.to_string()),
                _ => {}
            }
            return;
        }

        if path.parent_is(b"ciManagement") {
            match path.current() {
                b"system" => self.ci_management_system = Some(text.to_string()),
                b"url" => self.ci_management_url = Some(text.to_string()),
                _ => {}
            }
            return;
        }

        if path.parent_is(b"distributionManagement") && path.current() == b"downloadUrl" {
            self.dist_download_url = Some(text.to_string());
        }
    }

    fn has_extra_data(&self) -> bool {
        self.inception_year.is_some()
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
    }

    fn populate_extra_data(&mut self) {
        let mut extra_data = self.package_data.extra_data.take().unwrap_or_default();

        if let Some(year) = self.inception_year.take() {
            extra_data.insert(
                "inception_year".to_string(),
                serde_json::Value::String(year),
            );
        }
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
        if let Some(url) = self.dist_download_url.take() {
            extra_data.insert(
                "distribution_download_url".to_string(),
                serde_json::Value::String(url),
            );
        }

        insert_extra_data_object(
            &mut extra_data,
            "distribution_repository",
            DistributionRepositoryEntry {
                id: self.dist_repository_id.take(),
                name: self.dist_repository_name.take(),
                url: self.dist_repository_url.take(),
                layout: self.dist_repository_layout.take(),
            },
        );
        insert_extra_data_object(
            &mut extra_data,
            "distribution_snapshot_repository",
            DistributionRepositoryEntry {
                id: self.dist_snapshot_repository_id.take(),
                name: self.dist_snapshot_repository_name.take(),
                url: self.dist_snapshot_repository_url.take(),
                layout: self.dist_snapshot_repository_layout.take(),
            },
        );
        insert_extra_data_object(
            &mut extra_data,
            "distribution_site",
            DistributionSiteEntry {
                id: self.dist_site_id.take(),
                name: self.dist_site_name.take(),
                url: self.dist_site_url.take(),
            },
        );

        if !self.repositories.is_empty() {
            extra_data.insert(
                "repositories".to_string(),
                serde_json::Value::Array(
                    self.repositories
                        .drain(..)
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
                        .drain(..)
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
                        .drain(..)
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
                        .drain(..)
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
                serde_json::Value::Object(dependency_management_entry_to_value(&self.relocation)),
            );
        }

        insert_extra_data_object(
            &mut extra_data,
            "parent",
            ParentEntry {
                group_id: self.parent_group_id.take(),
                artifact_id: self.parent_artifact_id.take(),
                version: self.parent_version.take(),
                relative_path: self.parent_relative_path.take(),
            },
        );

        self.package_data.extra_data = Some(extra_data);
    }

    fn into_package_data(self) -> PackageData {
        self.package_data
    }

    fn finalize(mut self, path: &Path) -> PackageData {
        let has_extra_data = self.has_extra_data();
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
        let mut resolver = PropertyResolver::new(std::mem::take(&mut self.properties), builtins);

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
        let license_comments = std::mem::take(&mut self.xml_license_comments);
        for comment in license_comments {
            if !comment.trim().is_empty() {
                self.licenses.push(MavenLicenseEntry {
                    comments: Some(comment),
                    ..Default::default()
                });
            }
        }

        for index in 0..self.package_data.dependencies.len() {
            let dependency = &mut self.package_data.dependencies[index];
            let coords = &mut self.dependency_data[index];

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

        let package_coords = self
            .package_data
            .namespace
            .clone()
            .zip(self.package_data.name.clone())
            .zip(self.package_data.version.clone())
            .map(|((group_id, artifact_id), version)| (group_id, artifact_id, version));

        if let Some((group_id, artifact_id, version)) = package_coords {
            self.package_data.purl = Some(build_maven_purl(
                &group_id,
                &artifact_id,
                Some(&version),
                self.project_classifier.as_deref(),
                self.project_packaging.as_deref(),
            ));
            if self.project_classifier.is_none() {
                self.package_data
                    .source_packages
                    .push(build_maven_source_package(
                        &group_id,
                        &artifact_id,
                        &version,
                    ));
            }
        }

        if self.package_data.namespace.is_some() && self.package_data.name.is_some() {
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
            .clone()
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
            let org_name = self.organization_name.clone();
            let org_url = self.organization_url.clone();
            self.package_data.parties.push(Party {
                r#type: Some("organization".to_string()),
                role: Some("owner".to_string()),
                name: org_name,
                email: None,
                url: org_url,
                organization: None,
                organization_url: None,
                timezone: None,
            });
        }

        let dependency_management_entries = self.dependency_management_entries.clone();
        for dependency in &dependency_management_entries {
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

        if has_extra_data {
            self.populate_extra_data();
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

        self.package_data.namespace = self.package_data.namespace.take().map(truncate_field);
        self.package_data.name = self.package_data.name.take().map(truncate_field);
        self.package_data.version = self.package_data.version.take().map(truncate_field);
        self.package_data.description = self.package_data.description.take().map(truncate_field);
        self.package_data.homepage_url = self.package_data.homepage_url.take().map(truncate_field);
        self.package_data.vcs_url = self.package_data.vcs_url.take().map(truncate_field);
        self.package_data.purl = self.package_data.purl.take().map(truncate_field);
        self.package_data.code_view_url =
            self.package_data.code_view_url.take().map(truncate_field);
        self.package_data.bug_tracking_url = self
            .package_data
            .bug_tracking_url
            .take()
            .map(truncate_field);
        self.package_data.download_url = self.package_data.download_url.take().map(truncate_field);
        self.package_data.repository_homepage_url = self
            .package_data
            .repository_homepage_url
            .take()
            .map(truncate_field);
        self.package_data.repository_download_url = self
            .package_data
            .repository_download_url
            .take()
            .map(truncate_field);
        self.package_data.api_data_url = self.package_data.api_data_url.take().map(truncate_field);
        for dep in &mut self.package_data.dependencies {
            dep.purl = dep.purl.take().map(truncate_field);
            dep.extracted_requirement = dep.extracted_requirement.take().map(truncate_field);
        }

        self.package_data
    }
}

impl PomParseState {
    pub(super) fn new() -> Self {
        Self {
            context_stack: Vec::new(),
            acc: PomAccumulator::new(),
        }
    }

    pub(super) fn handle_start(&mut self, element_name: Vec<u8>) {
        let context = FrameContext::for_start(self, element_name.as_slice());
        self.context_stack
            .push(ElementFrame::new(element_name, context));
    }

    pub(super) fn handle_text(&mut self, path: &Path, text: String) {
        let Some(element_path) = ElementPath::from_stack(&self.context_stack) else {
            return;
        };

        if self.apply_context_text(path, &element_path, &text) {
            return;
        }

        self.acc.apply_structural_text(element_path, &text);
    }

    fn current_context<T>(&self, selector: fn(&FrameContext) -> Option<T>) -> Option<T> {
        self.context_stack
            .iter()
            .rev()
            .find_map(|frame| selector(&frame.context))
    }

    fn apply_context_text(&mut self, source_path: &Path, path: &ElementPath, text: &str) -> bool {
        let (context_stack, acc) = (&mut self.context_stack, &mut self.acc);
        for frame in context_stack.iter_mut().rev() {
            if frame.context.apply_text(acc, source_path, path, text) {
                return true;
            }
        }

        false
    }

    fn take_current_context(&mut self) -> Option<FrameContext> {
        self.context_stack
            .last_mut()
            .map(|frame| std::mem::replace(&mut frame.context, FrameContext::Plain))
    }

    fn finish_current_frame(&mut self) {
        if let Some(context) = self.take_current_context() {
            context.finish(&mut self.acc);
        }
    }

    pub(super) fn handle_comment(&mut self, comment: String) {
        if self.context_stack.is_empty() && !comment.is_empty() && is_license_like_comment(&comment)
        {
            self.acc.xml_license_comments.push(comment);
        }
    }

    pub(super) fn handle_end(&mut self, element_name: &[u8]) {
        match element_name {
            b"repository"
                if self.current_context(FrameContext::dependency_context)
                    == Some(DependencyContext::PackageEntries) => {}
            _ => self.finish_current_frame(),
        }

        if !self.context_stack.is_empty() {
            self.context_stack.pop();
        }
    }

    pub(super) fn into_package_data(self) -> PackageData {
        self.acc.into_package_data()
    }

    pub(super) fn finalize(self, path: &Path) -> PackageData {
        self.acc.finalize(path)
    }
}
