// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

struct ResolvedPropertyText {
    rendered: String,
    resolved_substitutions: usize,
}

fn is_safe_optional_property_omission(prev: Option<char>, next: Option<char>) -> bool {
    match (prev, next) {
        (Some(prev), None) => prev.is_ascii_alphanumeric(),
        (None, Some(next)) => next.is_ascii_alphanumeric(),
        _ => false,
    }
}

pub(crate) fn resolve_string_property_reference(
    value: &str,
    properties: &HashMap<String, String>,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut visiting = HashSet::new();
    let resolved = resolve_property_text(trimmed, properties, &mut visiting, 0)?;
    let rendered = resolved.rendered.trim();
    if rendered.is_empty() {
        None
    } else {
        Some(rendered.to_string())
    }
}

fn resolve_property_text(
    value: &str,
    properties: &HashMap<String, String>,
    visiting: &mut HashSet<String>,
    depth: usize,
) -> Option<ResolvedPropertyText> {
    if depth >= 10 {
        return None;
    }

    if !value.contains("$(") {
        return Some(ResolvedPropertyText {
            rendered: value.to_string(),
            resolved_substitutions: 0,
        });
    }

    let bytes = value.as_bytes();
    let mut index = 0;
    let mut rendered = String::with_capacity(value.len());
    let mut resolved_substitutions = 0;
    let mut had_property_reference = false;

    while index < bytes.len() {
        if bytes[index] == b'$' && index + 1 < bytes.len() && bytes[index + 1] == b'(' {
            had_property_reference = true;
            let start = index + 2;
            let relative_end = value[start..].find(')')?;
            let end = start + relative_end;
            let property_name = value[start..end].trim();
            if property_name.is_empty() || !visiting.insert(property_name.to_string()) {
                return None;
            }

            let raw_value = properties.get(property_name);
            let resolved = raw_value
                .and_then(|raw| resolve_property_text(raw, properties, visiting, depth + 1));
            visiting.remove(property_name);

            if let Some(resolved) = resolved {
                rendered.push_str(&resolved.rendered);
                resolved_substitutions += resolved.resolved_substitutions.max(1);
            } else if !is_safe_optional_property_omission(
                rendered.chars().last(),
                value[end + 1..].chars().next(),
            ) {
                return None;
            }

            index = end + 1;
            continue;
        }

        rendered.push(bytes[index] as char);
        index += 1;
    }

    if had_property_reference && resolved_substitutions == 0 {
        return None;
    }

    Some(ResolvedPropertyText {
        rendered,
        resolved_substitutions,
    })
}

pub(crate) fn resolve_bool_property_reference(
    value: Option<&str>,
    properties: &HashMap<String, String>,
) -> Option<bool> {
    let resolved = resolve_string_property_reference(value?, properties)?;
    Some(resolved.eq_ignore_ascii_case("true"))
}

pub(crate) fn resolve_optional_property_value(
    value: Option<&str>,
    properties: &HashMap<String, String>,
) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    if value.contains("$(") {
        resolve_string_property_reference(value, properties)
    } else {
        Some(value.to_string())
    }
}
