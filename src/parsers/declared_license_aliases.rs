// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Declared-license name aliases.
//!
//! Maps bare / informal license names (e.g. `"Apache"`, `"PSF"`) that are not
//! valid SPDX expressions to a canonical license key, for use **only** when
//! normalizing a package's declared license field. These aliases are never
//! consulted during file-content license detection and are not part of the
//! license index, so they cannot produce file-content false positives.
//!
//! The list — and the justification for every entry — lives in the
//! checked-in config at
//! `resources/license_detection/declared_license_aliases.toml`, intentionally
//! out of the code so the curation stays transparent and reviewable. This
//! module only loads and indexes it.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

const DECLARED_LICENSE_ALIASES_TEXT: &str =
    include_str!("../../resources/license_detection/declared_license_aliases.toml");

#[derive(Debug, Deserialize)]
struct AliasFile {
    #[serde(default)]
    alias: Vec<AliasEntry>,
}

#[derive(Debug, Deserialize)]
struct AliasEntry {
    names: Vec<String>,
    expression: String,
    // `reason` is intentionally not deserialized: it is required documentation
    // in the TOML (enforced by a test), but the runtime only needs the mapping.
}

/// Lower-cased declared name -> canonical license key.
static DECLARED_LICENSE_ALIASES: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let parsed: AliasFile = toml::from_str(DECLARED_LICENSE_ALIASES_TEXT)
        .expect("declared_license_aliases.toml must be valid TOML");
    let mut map = HashMap::new();
    for entry in parsed.alias {
        for name in entry.names {
            map.insert(name.trim().to_ascii_lowercase(), entry.expression.clone());
        }
    }
    map
});

/// Resolve a declared license statement to a canonical license key via the
/// curated alias table, or `None` if it is not an aliased name.
///
/// Matching is case-insensitive on the trimmed statement; only exact bare-name
/// matches resolve (this is not a substring or fuzzy match).
pub(crate) fn declared_license_alias(statement: &str) -> Option<&'static str> {
    let key = statement.trim().to_ascii_lowercase();
    DECLARED_LICENSE_ALIASES.get(&key).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_indexes_aliases() {
        assert_eq!(declared_license_alias("apache"), Some("apache-2.0"));
        assert_eq!(declared_license_alias("Apache"), Some("apache-2.0"));
        assert_eq!(declared_license_alias("  APACHE  "), Some("apache-2.0"));
        assert_eq!(declared_license_alias("boost"), Some("boost-1.0"));
        assert_eq!(declared_license_alias("PSF"), Some("python"));
        assert_eq!(declared_license_alias("Python"), Some("python"));
    }

    #[test]
    fn unaliased_names_return_none() {
        // Deliberately-excluded ambiguous names must not resolve here.
        assert_eq!(declared_license_alias("LGPL"), None);
        assert_eq!(declared_license_alias("MPL"), None);
        assert_eq!(declared_license_alias("EPL"), None);
        // Not a bare alias name.
        assert_eq!(declared_license_alias("Apache License, Version 2.0"), None);
        assert_eq!(declared_license_alias(""), None);
    }

    #[test]
    fn every_alias_entry_has_a_nonempty_reason() {
        // Enforce the transparency contract: each curated mapping must justify
        // itself. Parsed generically so the runtime struct can ignore `reason`.
        let value: toml::Value =
            toml::from_str(DECLARED_LICENSE_ALIASES_TEXT).expect("aliases TOML parses");
        let aliases = value
            .get("alias")
            .and_then(toml::Value::as_array)
            .expect("at least one [[alias]] entry");
        assert!(!aliases.is_empty());
        for entry in aliases {
            let names = entry.get("names").and_then(toml::Value::as_array);
            assert!(
                names.is_some_and(|n| !n.is_empty()),
                "every alias needs at least one name: {entry:?}"
            );
            assert!(
                entry
                    .get("expression")
                    .and_then(toml::Value::as_str)
                    .is_some(),
                "every alias needs an expression: {entry:?}"
            );
            let reason = entry.get("reason").and_then(toml::Value::as_str);
            assert!(
                reason.is_some_and(|r| r.trim().len() >= 20),
                "every alias needs a substantive `reason`: {entry:?}"
            );
        }
    }
}
