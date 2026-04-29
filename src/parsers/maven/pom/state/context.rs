// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::collections::{MailingListEntryBuilder, RepositoryEntryBuilder};
use super::dependencies::ActiveDependency;
use crate::models::Party;
use crate::parsers::maven::pom::tags::{KnownTag, Tag};

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
    pub(super) fn for_start(distribution: Option<DistributionSection>, tag: &Tag) -> Option<Self> {
        match tag {
            Tag::Known(KnownTag::Parent) => Some(Self::Parent),
            Tag::Known(KnownTag::Properties) => Some(Self::Properties),
            Tag::Known(KnownTag::Relocation) if distribution.is_some() => Some(Self::Relocation),
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
}
