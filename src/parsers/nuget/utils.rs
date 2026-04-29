// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

pub(super) fn resolve_string_property_reference(
    value: &str,
    properties: &HashMap<String, String>,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut visiting = HashSet::new();
    resolve_property_text(trimmed, properties, &mut visiting, 0)
}

fn resolve_property_text(
    value: &str,
    properties: &HashMap<String, String>,
    visiting: &mut HashSet<String>,
    depth: usize,
) -> Option<String> {
    if depth >= 10 {
        return None;
    }

    if !value.contains("$(") {
        return Some(value.to_string());
    }

    let bytes = value.as_bytes();
    let mut index = 0;
    let mut rendered = String::with_capacity(value.len());

    while index < bytes.len() {
        if bytes[index] == b'$' && index + 1 < bytes.len() && bytes[index + 1] == b'(' {
            let start = index + 2;
            let relative_end = value[start..].find(')')?;
            let end = start + relative_end;
            let property_name = value[start..end].trim();
            if property_name.is_empty() || !visiting.insert(property_name.to_string()) {
                return None;
            }

            let raw_value = properties.get(property_name)?;
            let resolved = resolve_property_text(raw_value, properties, visiting, depth + 1)?;
            visiting.remove(property_name);
            rendered.push_str(&resolved);
            index = end + 1;
            continue;
        }

        rendered.push(bytes[index] as char);
        index += 1;
    }

    Some(rendered)
}

pub(super) fn resolve_bool_property_reference(
    value: Option<&str>,
    properties: &HashMap<String, String>,
) -> Option<bool> {
    let resolved = resolve_string_property_reference(value?, properties)?;
    Some(resolved.eq_ignore_ascii_case("true"))
}

pub(super) fn resolve_optional_property_value(
    value: Option<&str>,
    properties: &HashMap<String, String>,
) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    if value.starts_with("$(") && value.ends_with(')') {
        resolve_string_property_reference(value, properties)
    } else {
        Some(value.to_string())
    }
}
