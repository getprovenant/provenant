// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

mod collections;
mod context;
mod dependencies;
mod distribution;
mod licenses;
mod parent;
mod parties;
mod project;

use self::collections::{CollectionData, MailingListEntryBuilder, RepositoryEntryBuilder};
use self::context::{
    ActiveSection, DependencyContext, DistributionSection, PartyList, RepositoryCollection,
};
use self::dependencies::{ActiveDependency, DependencyScratchData};
use self::distribution::DistributionData;
use self::licenses::LicenseData;
use self::parent::ParentEntry;
use self::project::{ProjectDetails, ProjectMetadata};
use super::super::coordinates::{
    build_maven_download_url, build_maven_purl, build_maven_repository_url,
    build_maven_source_package, infer_meta_inf_maven_coordinates,
};
use super::super::default_package_data;
use super::licenses::{MavenLicenseEntry, is_license_like_comment};
use super::properties::{
    MavenBuiltinPropertyInputs, PropertyResolver, build_builtin_properties, resolve_option,
};
use super::tags::{KnownTag, Tag};
use crate::models::{DatasourceId, PackageData, PackageType, Party};
use crate::parser_warn as warn;
use crate::parsers::utils::truncate_field;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

struct ContextFrame {
    tag: Tag,
    state: ContextFrameState,
}

impl ContextFrame {
    fn new(tag: Tag, state: ContextFrameState) -> Self {
        Self { tag, state }
    }

    fn tag(&self) -> &Tag {
        &self.tag
    }
}

#[derive(Default)]
struct ContextStack {
    frames: Vec<ContextFrame>,
}

impl ContextStack {
    fn text_position(&self) -> Option<(Tag, Option<Tag>, usize)> {
        let current = self.frames.last()?.tag().clone();
        let parent = self
            .frames
            .len()
            .checked_sub(2)
            .map(|index| self.frames[index].tag().clone());

        Some((current, parent, self.frames.len()))
    }

    fn current_section(&self) -> Option<ActiveSection> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| match frame.state {
                ContextFrameState::Section(section) => Some(section),
                _ => None,
            })
    }

    fn current_party_list(&self) -> Option<PartyList> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| match frame.state {
                ContextFrameState::PartyList(party_list) => Some(party_list),
                _ => None,
            })
    }

    fn current_repository_collection(&self) -> Option<RepositoryCollection> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| match frame.state {
                ContextFrameState::RepositoryCollection(collection) => Some(collection),
                _ => None,
            })
    }

    fn current_distribution_section(&self) -> Option<DistributionSection> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| match frame.state {
                ContextFrameState::Distribution(section) => Some(section),
                _ => None,
            })
    }

    fn current_dependency_context(&self) -> Option<DependencyContext> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| match frame.state {
                ContextFrameState::DependencyContext(context) => Some(context),
                _ => None,
            })
    }

    fn apply_text(
        &mut self,
        acc: &mut PomAccumulator,
        source_path: &Path,
        current_tag: &Tag,
        parent_tag: Option<&Tag>,
        depth: usize,
        text: &str,
    ) -> bool {
        for frame in self.frames.iter_mut().rev() {
            if frame
                .state
                .apply_text(acc, source_path, current_tag, parent_tag, depth, text)
            {
                return true;
            }
        }

        false
    }

    fn push(&mut self, tag: Tag, state: ContextFrameState) {
        self.frames.push(ContextFrame::new(tag, state));
    }

    fn finish_current_frame(&mut self, acc: &mut PomAccumulator) {
        if let Some(frame) = self.frames.last_mut() {
            let state = std::mem::replace(&mut frame.state, ContextFrameState::Plain);
            state.finish(acc);
        }
    }

    fn pop(&mut self) {
        self.frames.pop();
    }

    fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

enum ContextFrameState {
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

impl ContextFrameState {
    fn for_start(stack: &ContextStack, acc: &mut PomAccumulator, element_name: &Tag) -> Self {
        let dependency_context = stack.current_dependency_context();
        let party_list = stack.current_party_list();
        let distribution_section = stack.current_distribution_section();
        let repository_collection = stack.current_repository_collection();
        let section = stack.current_section();

        if let Some(dependency) =
            dependency_context.and_then(|context| context.start_dependency(element_name))
        {
            return Self::Dependency(dependency);
        }

        if let Some(context) = DependencyContext::for_start(dependency_context, element_name) {
            return Self::DependencyContext(context);
        }

        if let Some(party) = party_list.and_then(|list| list.start_party(element_name)) {
            return Self::Party(party);
        }

        if let Some(list) = PartyList::for_start(element_name) {
            return Self::PartyList(list);
        }

        if let Some(distribution) =
            DistributionSection::for_start(distribution_section, element_name)
        {
            return Self::Distribution(distribution);
        }

        if let Some((collection, builder)) = repository_collection
            .and_then(|collection| collection.start_repository(dependency_context, element_name))
        {
            return Self::Repository {
                collection,
                builder,
            };
        }

        if let Some(collection) = RepositoryCollection::for_start(element_name) {
            return Self::RepositoryCollection(collection);
        }

        if let Some(active_section) = ActiveSection::for_start(distribution_section, element_name) {
            if active_section == ActiveSection::Relocation {
                acc.dependency_scratch.reset_relocation();
            }
            return Self::Section(active_section);
        }

        if let Some(license) = MavenLicenseEntry::for_start(element_name) {
            return Self::License(license);
        }

        if let Some(mailing_list) =
            section.and_then(|active| active.start_mailing_list(element_name))
        {
            return Self::MailingList(mailing_list);
        }

        Self::Plain
    }

    fn apply_text(
        &mut self,
        acc: &mut PomAccumulator,
        source_path: &Path,
        current_tag: &Tag,
        parent_tag: Option<&Tag>,
        depth: usize,
        text: &str,
    ) -> bool {
        let current_known = current_tag.known();

        match self {
            Self::Dependency(dependency) => dependency.apply_text(current_known, parent_tag, text),
            Self::License(license) => {
                license.apply_text(current_known, text);
                true
            }
            Self::Party(party) => {
                party.apply_text(current_known, text);
                true
            }
            Self::Repository { builder, .. } => {
                builder.apply_text(current_known, text);
                true
            }
            Self::MailingList(mailing_list) => {
                mailing_list.apply_text(current_known, text);
                true
            }
            Self::Section(section) => Self::apply_section_text(
                *section,
                acc,
                source_path,
                current_tag,
                parent_tag,
                depth,
                text,
            ),
            Self::Distribution(distribution) => distribution.apply_text(acc, current_known, text),
            _ => false,
        }
    }

    fn apply_section_text(
        section: ActiveSection,
        acc: &mut PomAccumulator,
        source_path: &Path,
        current_tag: &Tag,
        parent_tag: Option<&Tag>,
        depth: usize,
        text: &str,
    ) -> bool {
        let current_known = current_tag.known();

        match section {
            ActiveSection::Relocation => {
                acc.dependency_scratch
                    .apply_relocation_text(current_known, text);
                true
            }
            ActiveSection::Parent => {
                acc.parent.apply_text(current_known, text);
                true
            }
            ActiveSection::Modules => {
                if current_known == Some(KnownTag::Module) {
                    acc.collections.push_module(text.to_string());
                }
                true
            }
            ActiveSection::Properties => {
                if depth >= 2 && parent_tag.is_some_and(|tag| tag.is(KnownTag::Properties)) {
                    if let Ok(property_name) = std::str::from_utf8(current_tag.as_bytes()) {
                        acc.properties
                            .insert(property_name.to_string(), truncate_field(text.to_string()));
                    } else {
                        warn!("Failed to decode Maven property name in {:?}", source_path);
                    }
                    true
                } else {
                    false
                }
            }
            ActiveSection::MailingLists => false,
        }
    }

    fn finish(self, acc: &mut PomAccumulator) {
        match self {
            Self::Dependency(dependency) => {
                dependency.finish_into(
                    &mut acc.package_data.dependencies,
                    &mut acc.dependency_scratch,
                );
            }
            Self::License(license) if license.has_data() => {
                acc.license_data.push_entry(license);
            }
            Self::License(_) => {}
            Self::Party(party) => {
                acc.package_data.parties.push(party);
            }
            Self::Repository {
                collection,
                builder,
            } => {
                if let Some(repo) = RepositoryEntryBuilder::finish(builder) {
                    acc.collections.push_repository(collection, repo)
                }
            }
            Self::MailingList(mailing_list) => {
                if let Some(entry) = MailingListEntryBuilder::finish(mailing_list) {
                    acc.collections.push_mailing_list(entry);
                }
            }
            _ => {}
        }
    }
}

struct PomAccumulator {
    package_data: PackageData,
    dependency_scratch: DependencyScratchData,
    license_data: LicenseData,
    project_details: ProjectDetails,
    project_metadata: ProjectMetadata,
    distribution: DistributionData,
    collections: CollectionData,
    parent: ParentEntry,
    properties: HashMap<String, String>,
}

pub(super) struct PomParseState {
    context_stack: ContextStack,
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
            dependency_scratch: DependencyScratchData::default(),
            license_data: LicenseData::default(),
            project_details: ProjectDetails::default(),
            project_metadata: ProjectMetadata::default(),
            distribution: DistributionData::default(),
            collections: CollectionData::default(),
            parent: ParentEntry::default(),
            properties: HashMap::new(),
        }
    }

    fn apply_structural_text(
        &mut self,
        current_tag: &Tag,
        parent_tag: Option<&Tag>,
        depth: usize,
        text: &str,
    ) {
        let current_known = current_tag.known();

        if depth == 2 {
            match current_known {
                Some(KnownTag::GroupId) => self.package_data.namespace = Some(text.to_string()),
                Some(KnownTag::ArtifactId) => self.package_data.name = Some(text.to_string()),
                Some(KnownTag::Version) => self.package_data.version = Some(text.to_string()),
                Some(KnownTag::Url) => self.package_data.homepage_url = Some(text.to_string()),
                _ => self.project_details.apply_text(current_known, text),
            }
            return;
        }

        if parent_tag.is_some_and(|tag| tag.is(KnownTag::Scm)) {
            self.project_metadata.apply_scm_text(current_known, text);
            return;
        }

        if parent_tag.is_some_and(|tag| tag.is(KnownTag::Organization)) {
            self.project_metadata
                .apply_organization_text(current_known, text);
            return;
        }

        if parent_tag.is_some_and(|tag| tag.is(KnownTag::IssueManagement)) {
            self.project_metadata
                .apply_issue_management_text(current_known, text);
            return;
        }

        if parent_tag.is_some_and(|tag| tag.is(KnownTag::CiManagement)) {
            self.project_metadata
                .apply_ci_management_text(current_known, text);
            return;
        }

        if parent_tag.is_some_and(|tag| tag.is(KnownTag::DistributionManagement))
            && current_known == Some(KnownTag::DownloadUrl)
        {
            self.distribution.apply_download_url(text);
        }
    }

    fn has_extra_data(&self) -> bool {
        self.project_details.has_extra_data()
            || self.project_metadata.has_extra_data()
            || self.distribution.has_extra_data()
            || self.collections.has_extra_data()
            || self.parent.has_data()
            || self.dependency_scratch.has_extra_data()
    }

    fn populate_scalar_extra_data(&mut self, extra_data: &mut HashMap<String, serde_json::Value>) {
        self.project_details.populate_extra_data(extra_data);
        self.project_metadata.populate_extra_data(extra_data);
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
        self.collections.populate_extra_data(extra_data);
    }

    fn populate_dependency_extra_data(
        &mut self,
        extra_data: &mut HashMap<String, serde_json::Value>,
    ) {
        self.dependency_scratch.populate_extra_data(extra_data);
    }

    fn populate_parent_extra_data(&mut self, extra_data: &mut HashMap<String, serde_json::Value>) {
        insert_extra_data_object(extra_data, "parent", std::mem::take(&mut self.parent));
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
            parent_group_id: self.parent.group_id(),
            parent_artifact_id: self.parent.artifact_id(),
            parent_version: self.parent.version(),
            project_name: self.project_details.name(),
            project_packaging: self.project_details.packaging(),
        });
        let mut resolver = PropertyResolver::new(std::mem::take(&mut self.properties), builtins);

        resolve_option(&mut resolver, &mut self.package_data.namespace);
        resolve_option(&mut resolver, &mut self.package_data.name);
        resolve_option(&mut resolver, &mut self.package_data.version);
        resolve_option(&mut resolver, &mut self.package_data.homepage_url);
        self.project_details.resolve_fields(&mut resolver);
        self.project_metadata.resolve_fields(&mut resolver);
        self.distribution.resolve_fields(&mut resolver);
        self.collections.resolve_fields(&mut resolver);
        self.parent.resolve_fields(&mut resolver);
        self.license_data.resolve_fields(&mut resolver);
        self.dependency_scratch
            .resolve_fields(&mut resolver, &mut self.package_data.dependencies);
    }

    fn apply_parent_fallbacks(&mut self) {
        self.parent.apply_fallbacks(
            &mut self.package_data.namespace,
            &mut self.package_data.version,
        );
    }

    fn finalize_package_metadata(&mut self) {
        self.project_details
            .apply_package_metadata(&mut self.package_data);
    }

    fn infer_meta_inf_coordinates(&mut self, path: &Path) {
        if let Some(coords) = infer_meta_inf_maven_coordinates(path) {
            if self.package_data.namespace.is_none() {
                self.package_data.namespace = Some(coords.group_id);
            }
            if self.package_data.name.is_none() {
                self.package_data.name = Some(coords.artifact_id);
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
                self.project_details.classifier(),
                self.project_details.packaging_str(),
            ));
            if !self.project_details.has_classifier() {
                self.package_data
                    .source_packages
                    .push(build_maven_source_package(
                        &group_id,
                        &artifact_id,
                        &version,
                    ));
            }
        }

        if let (Some(group_id), Some(artifact_id)) = (
            self.package_data.namespace.as_deref(),
            self.package_data.name.as_deref(),
        ) {
            self.package_data.core.repository_homepage_url = Some(build_maven_repository_url(
                group_id,
                artifact_id,
                self.package_data.version.as_deref(),
                None,
            ));

            if let Some(ver) = self.package_data.version.as_deref() {
                self.package_data.core.repository_download_url = Some(build_maven_download_url(
                    group_id,
                    artifact_id,
                    ver,
                    self.project_details.classifier(),
                    self.project_details.packaging_str(),
                ));
            } else {
                self.package_data.core.repository_download_url = None;
            }

            if let Some(ver) = self.package_data.version.as_deref() {
                let pom_filename = format!("{artifact_id}-{ver}.pom");
                self.package_data.core.api_data_url = Some(build_maven_repository_url(
                    group_id,
                    artifact_id,
                    Some(ver),
                    Some(&pom_filename),
                ));
            }
        }
    }

    fn finalize_related_urls(&mut self) {
        self.project_metadata
            .apply_related_urls(&mut self.package_data);
        if let Some(url) = self.distribution.download_url() {
            self.package_data.download_url = Some(url.to_string());
        }
    }

    fn add_organization_owner_party(&mut self) {
        self.project_metadata
            .add_owner_party(&mut self.package_data);
    }

    fn expand_dependency_entries(&mut self) {
        self.dependency_scratch
            .expand_entries(&mut self.package_data.dependencies);
    }

    fn finalize_license_data(&mut self) {
        std::mem::take(&mut self.license_data).finalize(&mut self.package_data);
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
            context_stack: ContextStack::default(),
            acc: PomAccumulator::new(),
        }
    }

    pub(super) fn handle_start(&mut self, element_name: Tag) {
        let state = ContextFrameState::for_start(&self.context_stack, &mut self.acc, &element_name);
        self.context_stack.push(element_name, state);
    }

    pub(super) fn handle_text(&mut self, path: &Path, text: String) {
        let Some((current_tag, parent_tag, depth)) = self.context_stack.text_position() else {
            return;
        };

        if self.context_stack.apply_text(
            &mut self.acc,
            path,
            &current_tag,
            parent_tag.as_ref(),
            depth,
            &text,
        ) {
            return;
        }

        self.acc
            .apply_structural_text(&current_tag, parent_tag.as_ref(), depth, &text);
    }

    pub(super) fn handle_comment(&mut self, comment: String) {
        if self.context_stack.is_empty() && !comment.is_empty() && is_license_like_comment(&comment)
        {
            self.acc.license_data.push_xml_comment(comment);
        }
    }

    pub(super) fn handle_end(&mut self, element_name: Tag) {
        match element_name {
            Tag::Known(KnownTag::Repository)
                if self.context_stack.current_dependency_context()
                    == Some(DependencyContext::PackageEntries) => {}
            _ => self.context_stack.finish_current_frame(&mut self.acc),
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
