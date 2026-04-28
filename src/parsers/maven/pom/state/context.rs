// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::PomAccumulator;
use super::collections::{MailingListEntryBuilder, RepositoryEntryBuilder};
use super::dependencies::{ActiveDependency, DependencyScratchData};
use crate::models::Party;
use crate::parser_warn as warn;
use crate::parsers::maven::pom::tags::{KnownTag, Tag};
use crate::parsers::utils::truncate_field;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PartyList {
    Developers,
    Contributors,
}

impl PartyList {
    pub(super) fn for_start(tag: &Tag) -> Option<Self> {
        match tag {
            Tag::Known(KnownTag::Developers) => Some(Self::Developers),
            Tag::Known(KnownTag::Contributors) => Some(Self::Contributors),
            _ => None,
        }
    }

    pub(super) fn start_party(self, tag: &Tag) -> Option<Party> {
        match (self, tag) {
            (Self::Developers, Tag::Known(KnownTag::Developer)) => {
                Some(Party::person("developer", None, None))
            }
            (Self::Contributors, Tag::Known(KnownTag::Contributor)) => {
                Some(Party::person("contributor", None, None))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RepositoryCollection {
    Repositories,
    PluginRepositories,
}

impl RepositoryCollection {
    pub(super) fn for_start(tag: &Tag) -> Option<Self> {
        match tag {
            Tag::Known(KnownTag::Repositories) => Some(Self::Repositories),
            Tag::Known(KnownTag::PluginRepositories) => Some(Self::PluginRepositories),
            _ => None,
        }
    }

    pub(super) fn start_repository(
        self,
        dependency_context: Option<DependencyContext>,
        tag: &Tag,
    ) -> Option<(RepositoryCollection, RepositoryEntryBuilder)> {
        match (self, tag) {
            (Self::Repositories, Tag::Known(KnownTag::Repository))
                if dependency_context != Some(DependencyContext::PackageEntries) =>
            {
                Some((Self::Repositories, RepositoryEntryBuilder::default()))
            }
            (Self::PluginRepositories, Tag::Known(KnownTag::PluginRepository)) => {
                Some((Self::PluginRepositories, RepositoryEntryBuilder::default()))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DistributionSection {
    Management,
    Repository,
    SnapshotRepository,
    Site,
}

impl DistributionSection {
    pub(super) fn for_start(current: Option<Self>, tag: &Tag) -> Option<Self> {
        match tag {
            Tag::Known(KnownTag::DistributionManagement) => Some(Self::Management),
            Tag::Known(KnownTag::Repository) if current.is_some() => Some(Self::Repository),
            Tag::Known(KnownTag::SnapshotRepository) if current.is_some() => {
                Some(Self::SnapshotRepository)
            }
            Tag::Known(KnownTag::Site) if current.is_some() => Some(Self::Site),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DependencyContext {
    ManagementContainer,
    ManagementEntries,
    PackageEntries,
}

impl DependencyContext {
    pub(super) fn for_start(current: Option<Self>, tag: &Tag) -> Option<Self> {
        match tag {
            Tag::Known(KnownTag::DependencyManagement) => Some(Self::ManagementContainer),
            Tag::Known(KnownTag::Dependencies) if current == Some(Self::ManagementContainer) => {
                Some(Self::ManagementEntries)
            }
            Tag::Known(KnownTag::Dependencies) => Some(Self::PackageEntries),
            _ => None,
        }
    }

    pub(super) fn start_dependency(self, tag: &Tag) -> Option<ActiveDependency> {
        ActiveDependency::for_start(self, tag)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ActiveSection {
    Parent,
    Properties,
    Relocation,
    Modules,
    MailingLists,
}

impl ActiveSection {
    pub(super) fn for_start(
        distribution: Option<DistributionSection>,
        dependency_scratch: &mut DependencyScratchData,
        tag: &Tag,
    ) -> Option<Self> {
        match tag {
            Tag::Known(KnownTag::Parent) => Some(Self::Parent),
            Tag::Known(KnownTag::Properties) => Some(Self::Properties),
            Tag::Known(KnownTag::Relocation) if distribution.is_some() => {
                dependency_scratch.reset_relocation();
                Some(Self::Relocation)
            }
            Tag::Known(KnownTag::Modules) => Some(Self::Modules),
            Tag::Known(KnownTag::MailingLists) => Some(Self::MailingLists),
            _ => None,
        }
    }

    pub(super) fn start_mailing_list(self, tag: &Tag) -> Option<MailingListEntryBuilder> {
        match (self, tag) {
            (Self::MailingLists, Tag::Known(KnownTag::MailingList)) => {
                Some(MailingListEntryBuilder::default())
            }
            _ => None,
        }
    }

    pub(super) fn apply_text(
        self,
        state: &mut PomAccumulator,
        source_path: &Path,
        current_tag: &Tag,
        parent_tag: Option<&Tag>,
        depth: usize,
        text: &str,
    ) -> bool {
        let current_known = current_tag.known();

        match self {
            Self::Relocation => {
                state
                    .dependency_scratch
                    .apply_relocation_text(current_known, text);
                true
            }
            Self::Parent => {
                state.parent.apply_text(current_known, text);
                true
            }
            Self::Modules => {
                if current_known == Some(KnownTag::Module) {
                    state.collections.push_module(text.to_string());
                }
                true
            }
            Self::Properties => {
                if depth >= 2 && parent_tag.is_some_and(|tag| tag.is(KnownTag::Properties)) {
                    if let Ok(property_name) = std::str::from_utf8(current_tag.as_bytes()) {
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
