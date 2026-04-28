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
    resolve_dependency_data, resolve_option, resolve_vec,
};
use super::tags::{KnownTag, Tag};
use crate::models::{DatasourceId, Dependency, PackageData, PackageType, Party};
use crate::parser_warn as warn;
use crate::parsers::utils::truncate_field;
use derive_builder::Builder;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone)]
struct ElementPath {
    current: Tag,
    parent: Option<Tag>,
    depth: usize,
}

impl ElementPath {
    fn from_stack(stack: &[ElementFrame]) -> Option<Self> {
        let current = stack.last()?.tag().clone();
        let parent = stack
            .len()
            .checked_sub(2)
            .map(|index| stack[index].tag().clone());

        Some(Self {
            current,
            parent,
            depth: stack.len(),
        })
    }

    fn parent_is(&self, expected: KnownTag) -> bool {
        self.parent
            .as_ref()
            .is_some_and(|parent| parent.is(expected))
    }

    fn is_project_field(&self) -> bool {
        self.depth == 2
    }

    fn current_known(&self) -> Option<KnownTag> {
        self.current.known()
    }

    fn current_bytes(&self) -> &[u8] {
        self.current.as_bytes()
    }
}

struct ElementFrame {
    tag: Tag,
    context: FrameContext,
}

impl ElementFrame {
    fn new(tag: Tag, context: FrameContext) -> Self {
        Self { tag, context }
    }

    fn tag(&self) -> &Tag {
        &self.tag
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
    fn for_start(state: &mut PomParseState, element_name: &Tag) -> Self {
        Self::start_dependency_context(state, element_name)
            .or_else(|| Self::start_party_context(state, element_name))
            .or_else(|| Self::start_distribution_context(state, element_name))
            .or_else(|| Self::start_repository_context(state, element_name))
            .or_else(|| Self::start_section_context(state, element_name))
            .or_else(|| Self::start_item_context(state, element_name))
            .unwrap_or(Self::Plain)
    }

    fn start_dependency_context(state: &PomParseState, element_name: &Tag) -> Option<Self> {
        match element_name {
            Tag::Known(KnownTag::Dependency)
                if state.current_context(FrameContext::dependency_context)
                    == Some(DependencyContext::ManagementEntries) =>
            {
                Some(Self::Dependency(ActiveDependency::Management(
                    MavenDependencyData::default(),
                )))
            }
            Tag::Known(KnownTag::Dependency)
                if state.current_context(FrameContext::dependency_context)
                    == Some(DependencyContext::PackageEntries) =>
            {
                Some(Self::Dependency(ActiveDependency::Package {
                    package: new_package_dependency(),
                    data: MavenDependencyData::default(),
                }))
            }
            Tag::Known(KnownTag::DependencyManagement) => Some(Self::DependencyContext(
                DependencyContext::ManagementContainer,
            )),
            Tag::Known(KnownTag::Dependencies)
                if state.current_context(FrameContext::dependency_context)
                    == Some(DependencyContext::ManagementContainer) =>
            {
                Some(Self::DependencyContext(
                    DependencyContext::ManagementEntries,
                ))
            }
            Tag::Known(KnownTag::Dependencies) => {
                Some(Self::DependencyContext(DependencyContext::PackageEntries))
            }
            _ => None,
        }
    }

    fn start_party_context(state: &PomParseState, element_name: &Tag) -> Option<Self> {
        match element_name {
            Tag::Known(KnownTag::Developers) => Some(Self::PartyList(PartyList::Developers)),
            Tag::Known(KnownTag::Contributors) => Some(Self::PartyList(PartyList::Contributors)),
            Tag::Known(KnownTag::Developer)
                if state.current_context(FrameContext::party_list)
                    == Some(PartyList::Developers) =>
            {
                Some(Self::Party(Party::person("developer", None, None)))
            }
            Tag::Known(KnownTag::Contributor)
                if state.current_context(FrameContext::party_list)
                    == Some(PartyList::Contributors) =>
            {
                Some(Self::Party(Party::person("contributor", None, None)))
            }
            _ => None,
        }
    }

    fn start_distribution_context(state: &PomParseState, element_name: &Tag) -> Option<Self> {
        match element_name {
            Tag::Known(KnownTag::DistributionManagement) => {
                Some(Self::Distribution(DistributionSection::Management))
            }
            Tag::Known(KnownTag::Repository)
                if state
                    .current_context(FrameContext::distribution_section)
                    .is_some() =>
            {
                Some(Self::Distribution(DistributionSection::Repository))
            }
            Tag::Known(KnownTag::SnapshotRepository)
                if state
                    .current_context(FrameContext::distribution_section)
                    .is_some() =>
            {
                Some(Self::Distribution(DistributionSection::SnapshotRepository))
            }
            Tag::Known(KnownTag::Site)
                if state
                    .current_context(FrameContext::distribution_section)
                    .is_some() =>
            {
                Some(Self::Distribution(DistributionSection::Site))
            }
            _ => None,
        }
    }

    fn start_repository_context(state: &PomParseState, element_name: &Tag) -> Option<Self> {
        match element_name {
            Tag::Known(KnownTag::Repositories) => Some(Self::RepositoryCollection(
                RepositoryCollection::Repositories,
            )),
            Tag::Known(KnownTag::PluginRepositories) => Some(Self::RepositoryCollection(
                RepositoryCollection::PluginRepositories,
            )),
            Tag::Known(KnownTag::Repository)
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
            Tag::Known(KnownTag::PluginRepository)
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

    fn start_section_context(state: &mut PomParseState, element_name: &Tag) -> Option<Self> {
        match element_name {
            Tag::Known(KnownTag::Parent) => Some(Self::Section(ActiveSection::Parent)),
            Tag::Known(KnownTag::Properties) => Some(Self::Section(ActiveSection::Properties)),
            Tag::Known(KnownTag::Relocation)
                if state
                    .current_context(FrameContext::distribution_section)
                    .is_some() =>
            {
                state.acc.relocation = MavenDependencyData::default();
                Some(Self::Section(ActiveSection::Relocation))
            }
            Tag::Known(KnownTag::Modules) => Some(Self::Section(ActiveSection::Modules)),
            Tag::Known(KnownTag::MailingLists) => Some(Self::Section(ActiveSection::MailingLists)),
            _ => None,
        }
    }

    fn start_item_context(state: &PomParseState, element_name: &Tag) -> Option<Self> {
        match (element_name, state.current_context(FrameContext::section)) {
            (Tag::Known(KnownTag::License), _) => Some(Self::License(MavenLicenseEntry::default())),
            (Tag::Known(KnownTag::MailingList), Some(ActiveSection::MailingLists)) => {
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
                license.apply_text(path.current_known(), text);
                true
            }
            Self::Party(party) => {
                party.apply_text(path.current_known(), text);
                true
            }
            Self::Repository { builder, .. } => {
                builder.apply_text(path.current_known(), text);
                true
            }
            Self::MailingList(mailing_list) => {
                mailing_list.apply_text(path.current_known(), text);
                true
            }
            Self::Section(section) => section.apply_text(acc, source_path, path, text),
            Self::Distribution(distribution) => {
                distribution.apply_text(acc, path.current_known(), text)
            }
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
    fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Id) => {
                self.id(text.to_string());
            }
            Some(KnownTag::Name) => {
                self.name(text.to_string());
            }
            Some(KnownTag::Url) => {
                self.url(text.to_string());
            }
            _ => {}
        }
    }

    fn finish(self) -> Option<RepositoryEntry> {
        let entry = self.build().ok()?;
        serialize_non_empty_object(&entry)?;
        Some(entry)
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
    fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Name) => {
                self.name(text.to_string());
            }
            Some(KnownTag::Subscribe) => {
                self.subscribe(text.to_string());
            }
            Some(KnownTag::Unsubscribe) => {
                self.unsubscribe(text.to_string());
            }
            Some(KnownTag::Post) => {
                self.post(text.to_string());
            }
            Some(KnownTag::Archive) => {
                self.archive(text.to_string());
            }
            _ => {}
        }
    }

    fn finish(self) -> Option<MailingListEntry> {
        let entry = self.build().ok()?;
        serialize_non_empty_object(&entry)?;
        Some(entry)
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

impl DistributionRepositoryEntry {
    fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Id) => self.id = Some(text.to_string()),
            Some(KnownTag::Name) => self.name = Some(text.to_string()),
            Some(KnownTag::Url) => self.url = Some(text.to_string()),
            Some(KnownTag::Layout) => self.layout = Some(text.to_string()),
            _ => {}
        }
    }

    fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        resolve_option(resolver, &mut self.id);
        resolve_option(resolver, &mut self.name);
        resolve_option(resolver, &mut self.url);
        resolve_option(resolver, &mut self.layout);
    }

    fn has_data(&self) -> bool {
        self.id.is_some() || self.name.is_some() || self.url.is_some() || self.layout.is_some()
    }
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

impl DistributionSiteEntry {
    fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Id) => self.id = Some(text.to_string()),
            Some(KnownTag::Name) => self.name = Some(text.to_string()),
            Some(KnownTag::Url) => self.url = Some(text.to_string()),
            _ => {}
        }
    }

    fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        resolve_option(resolver, &mut self.id);
        resolve_option(resolver, &mut self.name);
        resolve_option(resolver, &mut self.url);
    }

    fn has_data(&self) -> bool {
        self.id.is_some() || self.name.is_some() || self.url.is_some()
    }
}

#[derive(Default)]
struct DistributionData {
    download_url: Option<String>,
    repository: DistributionRepositoryEntry,
    snapshot_repository: DistributionRepositoryEntry,
    site: DistributionSiteEntry,
}

impl DistributionData {
    fn apply_text(
        &mut self,
        section: DistributionSection,
        current: Option<KnownTag>,
        text: &str,
    ) -> bool {
        match section {
            DistributionSection::Repository => {
                self.repository.apply_text(current, text);
                true
            }
            DistributionSection::SnapshotRepository => {
                self.snapshot_repository.apply_text(current, text);
                true
            }
            DistributionSection::Site => {
                self.site.apply_text(current, text);
                true
            }
            DistributionSection::Management => false,
        }
    }

    fn apply_download_url(&mut self, text: &str) {
        self.download_url = Some(text.to_string());
    }

    fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        resolve_option(resolver, &mut self.download_url);
        self.repository.resolve_fields(resolver);
        self.snapshot_repository.resolve_fields(resolver);
        self.site.resolve_fields(resolver);
    }

    fn has_extra_data(&self) -> bool {
        self.download_url.is_some()
            || self.repository.has_data()
            || self.snapshot_repository.has_data()
            || self.site.has_data()
    }

    fn populate_extra_data(&mut self, extra_data: &mut HashMap<String, serde_json::Value>) {
        if let Some(url) = self.download_url.take() {
            extra_data.insert(
                "distribution_download_url".to_string(),
                serde_json::Value::String(url),
            );
        }
        insert_extra_data_object(
            extra_data,
            "distribution_repository",
            std::mem::take(&mut self.repository),
        );
        insert_extra_data_object(
            extra_data,
            "distribution_snapshot_repository",
            std::mem::take(&mut self.snapshot_repository),
        );
        insert_extra_data_object(
            extra_data,
            "distribution_site",
            std::mem::take(&mut self.site),
        );
    }

    fn download_url(&self) -> Option<&str> {
        self.download_url.as_deref()
    }
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

fn insert_extra_data_array<T: Serialize>(
    extra_data: &mut HashMap<String, serde_json::Value>,
    key: &str,
    values: Vec<T>,
) {
    if values.is_empty() {
        return;
    }

    if let Ok(value) = serde_json::to_value(values) {
        extra_data.insert(key.to_string(), value);
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
        if !path.parent_is(KnownTag::Dependency) {
            return false;
        }

        match self {
            Self::Management(dependency) => {
                match path.current_known() {
                    Some(KnownTag::GroupId) => dependency.group_id = Some(text.to_string()),
                    Some(KnownTag::ArtifactId) => dependency.artifact_id = Some(text.to_string()),
                    Some(KnownTag::Version) => dependency.version = Some(text.to_string()),
                    Some(KnownTag::Scope) => dependency.scope = Some(text.to_string()),
                    Some(KnownTag::Type) => dependency.type_ = Some(text.to_string()),
                    Some(KnownTag::Classifier) => dependency.classifier = Some(text.to_string()),
                    Some(KnownTag::Optional) => dependency.optional = Some(text.to_string()),
                    _ => {}
                }
                true
            }
            Self::Package { package, data } => {
                match path.current_known() {
                    Some(KnownTag::GroupId) => data.group_id = Some(text.to_string()),
                    Some(KnownTag::ArtifactId) => data.artifact_id = Some(text.to_string()),
                    Some(KnownTag::Version) => data.version = Some(text.to_string()),
                    Some(KnownTag::Scope) => {
                        let scope = text.to_string();
                        package.scope = Some(scope.clone());
                        package.is_optional = Some(scope == "test" || scope == "provided");
                        package.is_runtime = Some(scope != "test" && scope != "provided");
                        data.scope = Some(scope);
                    }
                    Some(KnownTag::Optional) => data.optional = Some(text.to_string()),
                    Some(KnownTag::Type) => data.type_ = Some(text.to_string()),
                    Some(KnownTag::Classifier) => data.classifier = Some(text.to_string()),
                    Some(KnownTag::SystemPath) => data.system_path = Some(text.to_string()),
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
                match path.current_known() {
                    Some(KnownTag::GroupId) => state.relocation.group_id = Some(text.to_string()),
                    Some(KnownTag::ArtifactId) => {
                        state.relocation.artifact_id = Some(text.to_string())
                    }
                    Some(KnownTag::Version) => state.relocation.version = Some(text.to_string()),
                    Some(KnownTag::Classifier) => {
                        state.relocation.classifier = Some(text.to_string())
                    }
                    Some(KnownTag::Type) => state.relocation.type_ = Some(text.to_string()),
                    Some(KnownTag::Message) => state.relocation.message = Some(text.to_string()),
                    _ => {}
                }
                true
            }
            Self::Parent => {
                match path.current_known() {
                    Some(KnownTag::GroupId) => state.parent_group_id = Some(text.to_string()),
                    Some(KnownTag::ArtifactId) => state.parent_artifact_id = Some(text.to_string()),
                    Some(KnownTag::Version) => state.parent_version = Some(text.to_string()),
                    Some(KnownTag::RelativePath) => {
                        state.parent_relative_path = Some(text.to_string())
                    }
                    _ => {}
                }
                true
            }
            Self::Modules => {
                if path.current_known() == Some(KnownTag::Module) {
                    state.modules.push(text.to_string());
                }
                true
            }
            Self::Properties => {
                if path.parent_is(KnownTag::Properties) {
                    if let Ok(property_name) = std::str::from_utf8(path.current_bytes()) {
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
    fn apply_text(self, state: &mut PomAccumulator, current: Option<KnownTag>, text: &str) -> bool {
        state.distribution.apply_text(self, current, text)
    }
}

impl MavenLicenseEntry {
    fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Name) => self.name = Some(text.to_string()),
            Some(KnownTag::Url) => self.url = Some(text.to_string()),
            Some(KnownTag::Comments) => self.comments = Some(text.to_string()),
            _ => {}
        }
    }
}

impl Party {
    fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Name) => self.name = Some(text.to_string()),
            Some(KnownTag::Email) => self.email = Some(text.to_string()),
            Some(KnownTag::Url) => self.url = Some(text.to_string()),
            Some(KnownTag::Organization) => self.organization = Some(text.to_string()),
            Some(KnownTag::OrganizationUrl) => self.organization_url = Some(text.to_string()),
            Some(KnownTag::Timezone) => self.timezone = Some(text.to_string()),
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
    distribution: DistributionData,
    repositories: Vec<RepositoryEntry>,
    plugin_repositories: Vec<RepositoryEntry>,
    modules: Vec<String>,
    mailing_lists: Vec<MailingListEntry>,
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
            distribution: DistributionData::default(),
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

    fn apply_structural_text(&mut self, path: ElementPath, text: &str) {
        if path.is_project_field() {
            match path.current_known() {
                Some(KnownTag::GroupId) => self.package_data.namespace = Some(text.to_string()),
                Some(KnownTag::ArtifactId) => self.package_data.name = Some(text.to_string()),
                Some(KnownTag::Version) => self.package_data.version = Some(text.to_string()),
                Some(KnownTag::Name) => self.project_name = Some(text.to_string()),
                Some(KnownTag::Description) => self.project_description = Some(text.to_string()),
                Some(KnownTag::Packaging) => self.project_packaging = Some(text.to_string()),
                Some(KnownTag::Classifier) => self.project_classifier = Some(text.to_string()),
                Some(KnownTag::Url) => self.package_data.homepage_url = Some(text.to_string()),
                Some(KnownTag::InceptionYear) => self.inception_year = Some(text.to_string()),
                _ => {}
            }
            return;
        }

        if path.parent_is(KnownTag::Scm) {
            match path.current_known() {
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
            return;
        }

        if path.parent_is(KnownTag::Organization) {
            match path.current_known() {
                Some(KnownTag::Name) => self.organization_name = Some(text.to_string()),
                Some(KnownTag::Url) => self.organization_url = Some(text.to_string()),
                _ => {}
            }
            return;
        }

        if path.parent_is(KnownTag::IssueManagement) {
            match path.current_known() {
                Some(KnownTag::System) => self.issue_management_system = Some(text.to_string()),
                Some(KnownTag::Url) => self.issue_management_url = Some(text.to_string()),
                _ => {}
            }
            return;
        }

        if path.parent_is(KnownTag::CiManagement) {
            match path.current_known() {
                Some(KnownTag::System) => self.ci_management_system = Some(text.to_string()),
                Some(KnownTag::Url) => self.ci_management_url = Some(text.to_string()),
                _ => {}
            }
            return;
        }

        if path.parent_is(KnownTag::DistributionManagement)
            && path.current_known() == Some(KnownTag::DownloadUrl)
        {
            self.distribution.apply_download_url(text);
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
            || self.distribution.has_extra_data()
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

    fn populate_scalar_extra_data(&mut self, extra_data: &mut HashMap<String, serde_json::Value>) {
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
    }

    fn populate_distribution_extra_data(
        &mut self,
        extra_data: &mut HashMap<String, serde_json::Value>,
    ) {
        self.distribution.populate_extra_data(extra_data);
    }

    fn populate_collection_extra_data(
        &mut self,
        extra_data: &mut HashMap<String, serde_json::Value>,
    ) {
        insert_extra_data_array(
            extra_data,
            "repositories",
            std::mem::take(&mut self.repositories),
        );
        insert_extra_data_array(
            extra_data,
            "plugin_repositories",
            std::mem::take(&mut self.plugin_repositories),
        );
        insert_extra_data_array(extra_data, "modules", std::mem::take(&mut self.modules));
        insert_extra_data_array(
            extra_data,
            "mailing_lists",
            std::mem::take(&mut self.mailing_lists),
        );
    }

    fn populate_dependency_extra_data(
        &mut self,
        extra_data: &mut HashMap<String, serde_json::Value>,
    ) {
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
    }

    fn populate_parent_extra_data(&mut self, extra_data: &mut HashMap<String, serde_json::Value>) {
        insert_extra_data_object(
            extra_data,
            "parent",
            ParentEntry {
                group_id: self.parent_group_id.take(),
                artifact_id: self.parent_artifact_id.take(),
                version: self.parent_version.take(),
                relative_path: self.parent_relative_path.take(),
            },
        );
    }

    fn populate_extra_data(&mut self) {
        let mut extra_data = self.package_data.extra_data.take().unwrap_or_default();

        self.populate_scalar_extra_data(&mut extra_data);
        self.populate_distribution_extra_data(&mut extra_data);
        self.populate_collection_extra_data(&mut extra_data);
        self.populate_dependency_extra_data(&mut extra_data);
        self.populate_parent_extra_data(&mut extra_data);

        self.package_data.extra_data = Some(extra_data);
    }

    fn into_package_data(self) -> PackageData {
        self.package_data
    }

    fn resolve_accumulated_fields(&mut self) {
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
        self.distribution.resolve_fields(&mut resolver);
        resolve_option(&mut resolver, &mut self.parent_group_id);
        resolve_option(&mut resolver, &mut self.parent_artifact_id);
        resolve_option(&mut resolver, &mut self.parent_version);
        resolve_option(&mut resolver, &mut self.parent_relative_path);
        resolve_option(&mut resolver, &mut self.project_name);
        resolve_option(&mut resolver, &mut self.project_description);
        resolve_option(&mut resolver, &mut self.project_packaging);
        resolve_option(&mut resolver, &mut self.project_classifier);
        resolve_vec(&mut resolver, &mut self.modules);
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
    }

    fn apply_parent_fallbacks(&mut self) {
        if self.package_data.namespace.is_none() {
            self.package_data.namespace = self.parent_group_id.clone();
        }
        if self.package_data.version.is_none() {
            self.package_data.version = self.parent_version.clone();
        }
    }

    fn finalize_package_metadata(&mut self) {
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
    }

    fn infer_meta_inf_coordinates(&mut self, path: &Path) {
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
    }

    fn finalize_package_urls(&mut self) {
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
    }

    fn finalize_related_urls(&mut self) {
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
        if let Some(url) = self.distribution.download_url() {
            self.package_data.download_url = Some(url.to_string());
        }
    }

    fn add_organization_owner_party(&mut self) {
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
    }

    fn expand_dependency_entries(&mut self) {
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
    }

    fn finalize_license_data(&mut self) {
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
    }

    fn truncate_package_fields(&mut self) {
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
    }

    fn finalize(mut self, path: &Path) -> PackageData {
        let has_extra_data = self.has_extra_data();

        self.resolve_accumulated_fields();
        self.apply_parent_fallbacks();
        self.finalize_package_metadata();
        self.infer_meta_inf_coordinates(path);
        self.finalize_package_urls();
        self.finalize_related_urls();
        self.add_organization_owner_party();
        self.expand_dependency_entries();

        if has_extra_data {
            self.populate_extra_data();
        }

        self.finalize_license_data();
        self.truncate_package_fields();

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

    pub(super) fn handle_start(&mut self, element_name: Tag) {
        let context = FrameContext::for_start(self, &element_name);
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

    pub(super) fn handle_end(&mut self, element_name: Tag) {
        match element_name {
            Tag::Known(KnownTag::Repository)
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
