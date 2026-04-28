// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::context::DistributionSection;
use super::insert_extra_data_object;
use crate::parsers::maven::pom::properties::{PropertyResolver, resolve_option};
use crate::parsers::maven::pom::tags::KnownTag;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Default, Serialize)]
pub(super) struct DistributionRepositoryEntry {
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
pub(super) struct DistributionSiteEntry {
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
pub(super) struct DistributionData {
    download_url: Option<String>,
    repository: DistributionRepositoryEntry,
    snapshot_repository: DistributionRepositoryEntry,
    site: DistributionSiteEntry,
}

impl DistributionData {
    pub(super) fn apply_text(
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

    pub(super) fn apply_download_url(&mut self, text: &str) {
        self.download_url = Some(text.to_string());
    }

    pub(super) fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        resolve_option(resolver, &mut self.download_url);
        self.repository.resolve_fields(resolver);
        self.snapshot_repository.resolve_fields(resolver);
        self.site.resolve_fields(resolver);
    }

    pub(super) fn has_extra_data(&self) -> bool {
        self.download_url.is_some()
            || self.repository.has_data()
            || self.snapshot_repository.has_data()
            || self.site.has_data()
    }

    pub(super) fn populate_extra_data(
        &mut self,
        extra_data: &mut HashMap<String, serde_json::Value>,
    ) {
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

    pub(super) fn download_url(&self) -> Option<&str> {
        self.download_url.as_deref()
    }
}

impl DistributionSection {
    pub(super) fn apply_text(
        self,
        state: &mut super::PomAccumulator,
        current: Option<KnownTag>,
        text: &str,
    ) -> bool {
        state.distribution.apply_text(self, current, text)
    }
}
