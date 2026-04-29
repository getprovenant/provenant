// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::context::RepositoryCollection;
use super::{insert_extra_data_array, serialize_non_empty_object};
use crate::parsers::maven::pom::properties::{PropertyResolver, resolve_option, resolve_vec};
use crate::parsers::maven::pom::tags::KnownTag;
use derive_builder::Builder;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Builder, Default, Serialize)]
#[builder(default, setter(into, strip_option))]
pub(super) struct RepositoryEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

impl RepositoryEntry {
    fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        resolve_option(resolver, &mut self.id);
        resolve_option(resolver, &mut self.name);
        resolve_option(resolver, &mut self.url);
    }
}

impl RepositoryEntryBuilder {
    pub(super) fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
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

    pub(super) fn finish(self) -> Option<RepositoryEntry> {
        let entry = self.build().ok()?;
        serialize_non_empty_object(&entry)?;
        Some(entry)
    }
}

#[derive(Builder, Default, Serialize)]
#[builder(default, setter(into, strip_option))]
pub(super) struct MailingListEntry {
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

impl MailingListEntry {
    fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        resolve_option(resolver, &mut self.name);
        resolve_option(resolver, &mut self.subscribe);
        resolve_option(resolver, &mut self.unsubscribe);
        resolve_option(resolver, &mut self.post);
        resolve_option(resolver, &mut self.archive);
    }
}

impl MailingListEntryBuilder {
    pub(super) fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
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

    pub(super) fn finish(self) -> Option<MailingListEntry> {
        let entry = self.build().ok()?;
        serialize_non_empty_object(&entry)?;
        Some(entry)
    }
}

#[derive(Default)]
pub(super) struct CollectionData {
    repositories: Vec<RepositoryEntry>,
    plugin_repositories: Vec<RepositoryEntry>,
    modules: Vec<String>,
    mailing_lists: Vec<MailingListEntry>,
}

impl CollectionData {
    pub(super) fn push_repository(
        &mut self,
        collection: RepositoryCollection,
        entry: RepositoryEntry,
    ) {
        match collection {
            RepositoryCollection::Repositories => self.repositories.push(entry),
            RepositoryCollection::PluginRepositories => self.plugin_repositories.push(entry),
        }
    }

    pub(super) fn push_mailing_list(&mut self, entry: MailingListEntry) {
        self.mailing_lists.push(entry);
    }

    pub(super) fn push_module(&mut self, module: String) {
        self.modules.push(module);
    }

    pub(super) fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        for repository in &mut self.repositories {
            repository.resolve_fields(resolver);
        }

        for repository in &mut self.plugin_repositories {
            repository.resolve_fields(resolver);
        }

        resolve_vec(resolver, &mut self.modules);

        for mailing_list in &mut self.mailing_lists {
            mailing_list.resolve_fields(resolver);
        }
    }

    pub(super) fn has_extra_data(&self) -> bool {
        !self.repositories.is_empty()
            || !self.plugin_repositories.is_empty()
            || !self.modules.is_empty()
            || !self.mailing_lists.is_empty()
    }

    pub(super) fn populate_extra_data(
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
}
