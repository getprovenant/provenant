// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::dependencies::MavenDependencyData;
use crate::parser_warn as warn;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

pub(super) struct PropertyResolver {
    raw: HashMap<String, String>,
    builtins: HashMap<String, String>,
    cache: HashMap<String, String>,
    resolving_set: HashSet<String>,
    resolving_stack: Vec<String>,
    max_depth: usize,
    max_output_len: usize,
    max_substitutions: usize,
    warned_keys: HashSet<String>,
}

impl PropertyResolver {
    pub(super) fn new(raw: HashMap<String, String>, builtins: HashMap<String, String>) -> Self {
        Self {
            raw,
            builtins,
            cache: HashMap::new(),
            resolving_set: HashSet::new(),
            resolving_stack: Vec::new(),
            max_depth: 10,
            max_output_len: 100_000,
            max_substitutions: 1000,
            warned_keys: HashSet::new(),
        }
    }

    fn resolve_key(&mut self, key: &str, depth: usize) -> Option<String> {
        if let Some(value) = self.cache.get(key) {
            return Some(value.clone());
        }

        if depth >= self.max_depth {
            self.warn_once(
                "depth",
                key,
                format!("Maven property depth limit hit resolving {key}"),
            );
            return None;
        }

        if self.resolving_set.contains(key) {
            if self
                .resolving_stack
                .last()
                .is_some_and(|current| current == key)
            {
                return None;
            }

            self.warn_once(
                "cycle",
                key,
                format!(
                    "Maven property cycle detected at {key}: {:?}",
                    self.resolving_stack
                ),
            );
            return None;
        }

        let raw_val = if let Some(value) = self.raw.get(key).or_else(|| self.builtins.get(key)) {
            value.clone()
        } else {
            return None;
        };

        self.resolving_set.insert(key.to_string());
        self.resolving_stack.push(key.to_string());

        let resolved = self.resolve_text(&raw_val, depth + 1);

        self.resolving_stack.pop();
        self.resolving_set.remove(key);

        self.cache.insert(key.to_string(), resolved.clone());
        Some(resolved)
    }

    pub(super) fn resolve_text(&mut self, text: &str, depth: usize) -> String {
        if !text.contains("${") {
            return text.to_string();
        }

        if depth >= self.max_depth {
            warn!("Maven property depth limit hit resolving text");
            return text.to_string();
        }

        let bytes = text.as_bytes();
        let mut output: Vec<u8> = Vec::with_capacity(bytes.len());
        let mut index = 0;
        let mut substitutions = 0;

        while index < bytes.len() {
            if bytes[index] == b'$' && index + 1 < bytes.len() && bytes[index + 1] == b'{' {
                if substitutions >= self.max_substitutions {
                    warn!("Maven property substitution limit hit resolving {text}");
                    return text.to_string();
                }

                let placeholder_start = index;
                let Some((content, closing_index)) =
                    self.parse_placeholder_content(text, index + 2)
                else {
                    warn!("Maven property malformed placeholder in {text}");
                    return text.to_string();
                };

                substitutions += 1;
                let resolved_key = if content.contains("${") {
                    self.resolve_text(content, depth + 1)
                } else {
                    content.to_string()
                };

                if let Some(resolved) = self.resolve_key(&resolved_key, depth) {
                    if output.len() + resolved.len() > self.max_output_len {
                        warn!("Maven property output length limit hit resolving {text}");
                        return text.to_string();
                    }
                    output.extend_from_slice(resolved.as_bytes());
                } else {
                    let placeholder_bytes = &bytes[placeholder_start..=closing_index];
                    if output.len() + placeholder_bytes.len() > self.max_output_len {
                        warn!("Maven property output length limit hit resolving {text}");
                        return text.to_string();
                    }
                    output.extend_from_slice(placeholder_bytes);
                }

                index = closing_index + 1;
                continue;
            }

            if output.len() + 1 > self.max_output_len {
                warn!("Maven property output length limit hit resolving {text}");
                return text.to_string();
            }

            output.push(bytes[index]);
            index += 1;
        }

        String::from_utf8(output).unwrap_or_else(|_| text.to_string())
    }

    fn parse_placeholder_content<'a>(
        &self,
        text: &'a str,
        start_index: usize,
    ) -> Option<(&'a str, usize)> {
        let bytes = text.as_bytes();
        let mut index = start_index;
        let mut depth = 0;

        while index < bytes.len() {
            if bytes[index] == b'$' && index + 1 < bytes.len() && bytes[index + 1] == b'{' {
                depth += 1;
                index += 2;
                continue;
            }

            if bytes[index] == b'}' {
                if depth == 0 {
                    return Some((&text[start_index..index], index));
                }
                depth -= 1;
            }

            index += 1;
        }

        None
    }

    fn warn_once(&mut self, kind: &str, key: &str, message: String) {
        let token = format!("{kind}:{key}");
        if self.warned_keys.insert(token) {
            warn!("{message}");
        }
    }
}

pub(super) fn sanitize_template_directives(content: &str) -> Cow<'_, str> {
    if !content.contains("<%") {
        return Cow::Borrowed(content);
    }

    let mut sanitized = String::with_capacity(content.len());
    let mut remaining = content;

    while let Some(start) = remaining.find("<%") {
        let (before, after_start) = remaining.split_at(start);
        sanitized.push_str(before);

        let Some(end) = after_start.find("%>") else {
            return Cow::Borrowed(content);
        };

        let directive = &after_start[..end + 2];
        for ch in directive.chars() {
            if matches!(ch, '\n' | '\r') {
                sanitized.push(ch);
            } else {
                sanitized.push(' ');
            }
        }

        remaining = &after_start[end + 2..];
    }

    sanitized.push_str(remaining);
    Cow::Owned(sanitized)
}

pub(super) fn resolve_option(resolver: &mut PropertyResolver, value: &mut Option<String>) {
    if let Some(current) = value.clone() {
        *value = Some(resolver.resolve_text(&current, 0));
    }
}

pub(super) fn resolve_vec(resolver: &mut PropertyResolver, values: &mut [String]) {
    for value in values.iter_mut() {
        *value = resolver.resolve_text(value, 0);
    }
}

pub(super) fn resolve_dependency_data(
    resolver: &mut PropertyResolver,
    dependency: &mut MavenDependencyData,
) {
    resolve_option(resolver, &mut dependency.group_id);
    resolve_option(resolver, &mut dependency.artifact_id);
    resolve_option(resolver, &mut dependency.version);
    resolve_option(resolver, &mut dependency.classifier);
    resolve_option(resolver, &mut dependency.type_);
    resolve_option(resolver, &mut dependency.scope);
    resolve_option(resolver, &mut dependency.optional);
    resolve_option(resolver, &mut dependency.system_path);
    resolve_option(resolver, &mut dependency.message);
}

pub(super) struct MavenBuiltinPropertyInputs<'a> {
    pub(super) namespace: &'a Option<String>,
    pub(super) name: &'a Option<String>,
    pub(super) version: &'a Option<String>,
    pub(super) parent_group_id: &'a Option<String>,
    pub(super) parent_artifact_id: &'a Option<String>,
    pub(super) parent_version: &'a Option<String>,
    pub(super) project_name: &'a Option<String>,
    pub(super) project_packaging: &'a Option<String>,
}

pub(super) fn build_builtin_properties(
    inputs: MavenBuiltinPropertyInputs<'_>,
) -> HashMap<String, String> {
    let mut builtins = HashMap::new();
    let effective_group_id = inputs
        .namespace
        .clone()
        .or_else(|| inputs.parent_group_id.clone());
    let effective_version = inputs
        .version
        .clone()
        .or_else(|| inputs.parent_version.clone());

    if let Some(group_id) = effective_group_id.clone() {
        builtins.insert("project.groupId".to_string(), group_id.clone());
        builtins.insert("pom.groupId".to_string(), group_id);
    }

    if let Some(artifact_id) = inputs.name.clone() {
        builtins.insert("project.artifactId".to_string(), artifact_id.clone());
        builtins.insert("pom.artifactId".to_string(), artifact_id);
    }

    if let Some(ver) = effective_version.clone() {
        builtins.insert("project.version".to_string(), ver.clone());
        builtins.insert("pom.version".to_string(), ver);
    }

    if let Some(group_id) = inputs.parent_group_id.clone() {
        builtins.insert("project.parent.groupId".to_string(), group_id);
    }

    if let Some(artifact_id) = inputs.parent_artifact_id.clone() {
        builtins.insert("project.parent.artifactId".to_string(), artifact_id.clone());
        builtins.insert("pom.parent.artifactId".to_string(), artifact_id.clone());
        builtins.insert("parent.artifactId".to_string(), artifact_id);
    }

    if let Some(ver) = inputs.parent_version.clone() {
        builtins.insert("project.parent.version".to_string(), ver.clone());
        builtins.insert("pom.parent.version".to_string(), ver.clone());
        builtins.insert("parent.version".to_string(), ver);
    }

    if let Some(packaging) = inputs.project_packaging.clone() {
        builtins.insert("project.packaging".to_string(), packaging);
    }

    if let Some(name) = inputs.project_name.clone() {
        builtins.insert("project.name".to_string(), name);
    }

    builtins
}
