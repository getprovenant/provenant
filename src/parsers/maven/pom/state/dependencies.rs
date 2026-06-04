// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use super::context::DependencyContext;
use crate::models::Dependency;
use crate::parsers::maven::coordinates::build_maven_purl;
use crate::parsers::maven::pom::dependencies::{
    MavenDependencyData, dependency_extra_data, dependency_management_entry_to_value,
    is_maven_version_pinned, maven_dependency_to_dependency, parse_maven_bool,
};
use crate::parsers::maven::pom::properties::{PropertyResolver, resolve_dependency_data};
use crate::parsers::maven::pom::tags::{KnownTag, Tag};
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn new_package_dependency() -> Dependency {
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

pub(super) enum ActiveDependency {
    Package {
        package: Dependency,
        data: MavenDependencyData,
    },
    Management(MavenDependencyData),
}

impl ActiveDependency {
    pub(super) fn for_start(context: DependencyContext, tag: &Tag) -> Option<Self> {
        match (context, tag) {
            (DependencyContext::ManagementEntries, Tag::Known(KnownTag::Dependency)) => {
                Some(Self::Management(MavenDependencyData::default()))
            }
            (DependencyContext::PackageEntries, Tag::Known(KnownTag::Dependency)) => {
                Some(Self::Package {
                    package: new_package_dependency(),
                    data: MavenDependencyData::default(),
                })
            }
            _ => None,
        }
    }

    pub(super) fn apply_text(
        &mut self,
        current: Option<KnownTag>,
        parent: Option<&Tag>,
        text: &str,
    ) -> bool {
        if !parent.is_some_and(|tag| tag.is(KnownTag::Dependency)) {
            return false;
        }

        match self {
            Self::Management(dependency) => {
                match current {
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
                match current {
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

    pub(super) fn finish_into(
        self,
        package_dependencies: &mut Vec<Dependency>,
        scratch: &mut DependencyScratchData,
    ) {
        match self {
            Self::Management(dependency) if dependency.has_management_coordinates() => {
                scratch.push_management_entry(dependency);
            }
            Self::Management(_) => {}
            Self::Package { package, data } => {
                package_dependencies.push(package);
                scratch.push_package_entry(data);
            }
        }
    }
}

#[derive(Default)]
pub(super) struct DependencyScratchData {
    management_entries: Vec<MavenDependencyData>,
    package_entries: Vec<MavenDependencyData>,
    relocation: MavenDependencyData,
}

impl DependencyScratchData {
    pub(super) fn push_management_entry(&mut self, entry: MavenDependencyData) {
        self.management_entries.push(entry);
    }

    pub(super) fn push_package_entry(&mut self, entry: MavenDependencyData) {
        self.package_entries.push(entry);
    }

    pub(super) fn reset_relocation(&mut self) {
        self.relocation = MavenDependencyData::default();
    }

    pub(super) fn apply_relocation_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::GroupId) => self.relocation.group_id = Some(text.to_string()),
            Some(KnownTag::ArtifactId) => self.relocation.artifact_id = Some(text.to_string()),
            Some(KnownTag::Version) => self.relocation.version = Some(text.to_string()),
            Some(KnownTag::Classifier) => self.relocation.classifier = Some(text.to_string()),
            Some(KnownTag::Type) => self.relocation.type_ = Some(text.to_string()),
            Some(KnownTag::Message) => self.relocation.message = Some(text.to_string()),
            _ => {}
        }
    }

    fn has_relocation_data(&self) -> bool {
        self.relocation.group_id.is_some()
            || self.relocation.artifact_id.is_some()
            || self.relocation.version.is_some()
            || self.relocation.message.is_some()
    }

    pub(super) fn has_extra_data(&self) -> bool {
        !self.management_entries.is_empty() || self.has_relocation_data()
    }

    pub(super) fn populate_extra_data(&mut self, extra_data: &mut HashMap<String, Value>) {
        if !self.management_entries.is_empty() {
            extra_data.insert(
                "dependency_management".to_string(),
                Value::Array(
                    self.management_entries
                        .iter()
                        .map(|dependency| {
                            Value::Object(dependency_management_entry_to_value(dependency))
                        })
                        .collect(),
                ),
            );
        }

        if self.has_relocation_data() {
            extra_data.insert(
                "relocation".to_string(),
                Value::Object(dependency_management_entry_to_value(&self.relocation)),
            );
        }
    }

    pub(super) fn resolve_fields(
        &mut self,
        resolver: &mut PropertyResolver,
        package_dependencies: &mut [Dependency],
    ) {
        for dependency in &mut self.management_entries {
            resolve_dependency_data(resolver, dependency);
        }
        resolve_dependency_data(resolver, &mut self.relocation);

        for (dependency, coords) in package_dependencies
            .iter_mut()
            .zip(self.package_entries.iter_mut())
        {
            resolve_dependency_data(resolver, coords);
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

    pub(super) fn expand_entries(&self, package_dependencies: &mut Vec<Dependency>) {
        for dependency in &self.management_entries {
            if dependency.scope.as_deref() == Some("import")
                && let Some(import_dependency) =
                    maven_dependency_to_dependency(dependency, Some("import"), true)
            {
                package_dependencies.push(import_dependency);
            }

            let mut dependency_management_copy = dependency.clone();
            dependency_management_copy.scope = Some("dependencymanagement".to_string());

            if let Some(converted) = maven_dependency_to_dependency(
                &dependency_management_copy,
                Some("dependencymanagement"),
                true,
            ) {
                package_dependencies.push(converted);
            }
        }

        if (self.relocation.group_id.is_some()
            || self.relocation.artifact_id.is_some()
            || self.relocation.version.is_some())
            && let Some(converted) =
                maven_dependency_to_dependency(&self.relocation, Some("relocation"), true)
        {
            package_dependencies.push(converted);
        }
    }
}
