// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::parsers::maven::pom::properties::{PropertyResolver, resolve_option};
use crate::parsers::maven::pom::tags::KnownTag;
use serde::Serialize;

#[derive(Default, Serialize)]
pub(super) struct ParentEntry {
    #[serde(rename = "groupId", skip_serializing_if = "Option::is_none")]
    group_id: Option<String>,
    #[serde(rename = "artifactId", skip_serializing_if = "Option::is_none")]
    artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(rename = "relativePath", skip_serializing_if = "Option::is_none")]
    relative_path: Option<String>,
}

impl ParentEntry {
    pub(super) fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::GroupId) => self.group_id = Some(text.to_string()),
            Some(KnownTag::ArtifactId) => self.artifact_id = Some(text.to_string()),
            Some(KnownTag::Version) => self.version = Some(text.to_string()),
            Some(KnownTag::RelativePath) => self.relative_path = Some(text.to_string()),
            _ => {}
        }
    }

    pub(super) fn resolve_fields(&mut self, resolver: &mut PropertyResolver) {
        resolve_option(resolver, &mut self.group_id);
        resolve_option(resolver, &mut self.artifact_id);
        resolve_option(resolver, &mut self.version);
        resolve_option(resolver, &mut self.relative_path);
    }

    pub(super) fn has_data(&self) -> bool {
        self.group_id.is_some()
            || self.artifact_id.is_some()
            || self.version.is_some()
            || self.relative_path.is_some()
    }

    pub(super) fn group_id(&self) -> &Option<String> {
        &self.group_id
    }

    pub(super) fn artifact_id(&self) -> &Option<String> {
        &self.artifact_id
    }

    pub(super) fn version(&self) -> &Option<String> {
        &self.version
    }

    pub(super) fn apply_fallbacks(
        &self,
        namespace: &mut Option<String>,
        version: &mut Option<String>,
    ) {
        if namespace.is_none() {
            *namespace = self.group_id.clone();
        }
        if version.is_none() {
            *version = self.version.clone();
        }
    }
}
