// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Central, per-type Package-URL (PURL) normalization.
//!
//! The `packageurl` crate percent-encodes components and lowercases a few
//! hard-coded names, but does not apply the full per-type case/canonicalization
//! rules from the [purl-spec](https://github.com/package-url/purl-spec), and
//! never touches the namespace. Without a central layer each parser would have
//! to reimplement these rules, so the same package could get different PURLs
//! from different datasources (e.g. `typing_extensions` vs `typing-extensions`),
//! breaking dedup and registry/vuln-database lookups.
//!
//! Every emitted PURL passes through [`normalize_purl`]: in-memory before a
//! [`crate::models::Package`] derives its `package_uid` (the dedup key), and at
//! the `src/output_schema` boundary for package, package-data, dependency, and
//! resolved-package PURLs.
//!
//! This layer covers the case/name-canonicalization rules. Structural per-type
//! fixes that need parser knowledge (type remapping, moving a value between
//! namespace/qualifier/subpath, synthesizing a required qualifier) stay with the
//! owning parser. Unlisted types are returned unchanged, preserving the
//! case-sensitive types (npm, maven, cargo, gem, …).

use std::str::FromStr;

use packageurl::PackageUrl;

/// Normalize a PURL string according to its type's spec rules.
///
/// Per-type case/canonicalization rules are applied to the namespace and name;
/// version, qualifiers, and subpath are preserved. Unparsable input and types
/// with no rule are returned unchanged, so already-canonical PURLs never churn.
pub fn normalize_purl(purl: &str) -> String {
    let Ok(parsed) = PackageUrl::from_str(purl) else {
        return purl.to_string();
    };

    let (new_namespace, new_name): (Option<String>, String) = match parsed.ty() {
        // PEP 503: lowercase, then collapse runs of `-_.` to a single `-` (the
        // crate only handles `_`). Namespace is prohibited for pypi.
        "pypi" => (None, normalize_pypi_name(parsed.name())),

        // Lowercase namespace + name. The crate lowercases some of these names
        // but never the namespace.
        "composer" | "hex" | "github" | "gitlab" | "bitbucket" => (
            parsed.namespace().map(str::to_ascii_lowercase),
            parsed.name().to_ascii_lowercase(),
        ),

        // golang's spec is self-contradictory and acknowledged-broken; the
        // decided direction (purl-spec#308) is to lowercase only the host
        // segment and preserve path-part case (e.g. keep `github.com/Azure/…`).
        // The crate force-lowercases the whole golang namespace at parse time,
        // so `parsed` has already lost the case — edit the raw string instead.
        "golang" => return lowercase_first_path_segment(purl, "pkg:golang/"),

        _ => return purl.to_string(),
    };

    rebuild(purl, &parsed, new_namespace, new_name)
}

/// Re-emit `parsed` with a replaced namespace/name, preserving the rest.
///
/// Falls back to the original string if the rebuilt PURL cannot be constructed.
fn rebuild(
    original: &str,
    parsed: &PackageUrl<'_>,
    namespace: Option<String>,
    name: String,
) -> String {
    let Ok(mut rebuilt) = PackageUrl::new(parsed.ty().to_string(), name) else {
        return original.to_string();
    };

    if let Some(namespace) = namespace.filter(|value| !value.is_empty())
        && rebuilt.with_namespace(namespace).is_err()
    {
        return original.to_string();
    }

    if let Some(version) = parsed.version()
        && rebuilt.with_version(version.to_string()).is_err()
    {
        return original.to_string();
    }

    for (key, value) in parsed.qualifiers() {
        if rebuilt
            .add_qualifier(key.to_string(), value.to_string())
            .is_err()
        {
            return original.to_string();
        }
    }

    if let Some(subpath) = parsed.subpath()
        && rebuilt.with_subpath(subpath.to_string()).is_err()
    {
        return original.to_string();
    }

    rebuilt.to_string()
}

/// Apply the PEP 503 normalized distribution name rule: lowercase, then collapse
/// every run of `-`, `_`, or `.` into a single `-`.
fn normalize_pypi_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let mut normalized = String::with_capacity(lower.len());
    let mut last_was_separator = false;

    for ch in lower.chars() {
        if matches!(ch, '-' | '_' | '.') {
            if !last_was_separator {
                normalized.push('-');
                last_was_separator = true;
            }
        } else {
            normalized.push(ch);
            last_was_separator = false;
        }
    }

    normalized
}

/// Lowercase the first path segment that follows `prefix` in a PURL string,
/// stopping at the next path separator or component delimiter (`/ @ ? #`).
///
/// Used for golang's host-only lowercasing: it edits the raw string so the
/// remaining path parts, version, qualifiers, and subpath survive untouched.
/// Returns the input unchanged if it does not start with `prefix`.
fn lowercase_first_path_segment(purl: &str, prefix: &str) -> String {
    let Some(rest) = purl.strip_prefix(prefix) else {
        return purl.to_string();
    };
    let end = rest.find(['/', '@', '?', '#']).unwrap_or(rest.len());
    format!(
        "{prefix}{}{}",
        rest[..end].to_ascii_lowercase(),
        &rest[end..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec-rule matrix: representative PURL per type asserted against the
    /// canonical form. Guards every parser against drift.
    #[test]
    fn normalize_purl_matrix() {
        let cases = [
            // pypi: full PEP 503 (lowercase + collapse `-_.` runs).
            (
                "pkg:pypi/typing_extensions@4.0.0",
                "pkg:pypi/typing-extensions@4.0.0",
            ),
            ("pkg:pypi/Django@4.2", "pkg:pypi/django@4.2"),
            ("pkg:pypi/zope.interface@5.0", "pkg:pypi/zope-interface@5.0"),
            ("pkg:pypi/foo__bar@1.0", "pkg:pypi/foo-bar@1.0"),
            // composer: lowercase vendor namespace + name.
            (
                "pkg:composer/Monolog/Monolog@2.0",
                "pkg:composer/monolog/monolog@2.0",
            ),
            // hex: lowercase namespace + name.
            ("pkg:hex/Phoenix@1.7.0", "pkg:hex/phoenix@1.7.0"),
            // github / gitlab / bitbucket: lowercase namespace + name.
            (
                "pkg:github/Package-Url/purl-Spec@1.0",
                "pkg:github/package-url/purl-spec@1.0",
            ),
            ("pkg:gitlab/FooBar/Baz@2.0", "pkg:gitlab/foobar/baz@2.0"),
            (
                "pkg:bitbucket/Birkenfeld/Pygments@2.0",
                "pkg:bitbucket/birkenfeld/pygments@2.0",
            ),
            // golang: lowercase host only, preserve path-part case.
            (
                "pkg:golang/github.com/Azure/azure-sdk-for-go@1.0",
                "pkg:golang/github.com/Azure/azure-sdk-for-go@1.0",
            ),
            (
                "pkg:golang/GitHub.com/Azure/azure-sdk-for-go@1.0",
                "pkg:golang/github.com/Azure/azure-sdk-for-go@1.0",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(normalize_purl(input), expected, "input: {input}");
        }
    }

    /// Case-sensitive and custom types must be returned byte-for-byte unchanged.
    #[test]
    fn normalize_purl_preserves_case_sensitive_types() {
        let untouched = [
            // npm grandfathers mixed-case names.
            "pkg:npm/%40angular/Core@13.0.0",
            "pkg:maven/com.Example/MyLib@1.0",
            "pkg:cargo/Serde@1.0",
            "pkg:gem/RSpec@3.0",
            // Unknown / custom type with no registered spec.
            "pkg:bower/SomeLib@1.0",
        ];

        for purl in untouched {
            assert_eq!(normalize_purl(purl), purl, "input: {purl}");
        }
    }

    #[test]
    fn normalize_purl_is_idempotent() {
        let inputs = [
            "pkg:pypi/typing_extensions@4.0.0",
            "pkg:composer/Monolog/Monolog@2.0",
            "pkg:golang/GitHub.com/Azure/azure-sdk-for-go@1.0",
            "pkg:github/Foo/Bar",
        ];

        for input in inputs {
            let once = normalize_purl(input);
            let twice = normalize_purl(&once);
            assert_eq!(once, twice, "not idempotent for {input}");
        }
    }

    #[test]
    fn normalize_purl_preserves_qualifiers_and_subpath() {
        // pypi name changes, but qualifiers and subpath survive the round-trip.
        assert_eq!(
            normalize_purl("pkg:pypi/typing_extensions@4.0?arch=any#sub/path"),
            "pkg:pypi/typing-extensions@4.0?arch=any#sub/path",
        );
    }

    #[test]
    fn normalize_purl_returns_unparsable_input_unchanged() {
        assert_eq!(normalize_purl("not-a-purl"), "not-a-purl");
        assert_eq!(normalize_purl(""), "");
    }

    #[test]
    fn normalize_purl_handles_pypi_without_version() {
        assert_eq!(
            normalize_purl("pkg:pypi/typing_extensions"),
            "pkg:pypi/typing-extensions",
        );
    }

    /// A golang PURL with no namespace (single-segment module path): the whole
    /// `rest` slice is lowercased, which is the correct host-only rule when the
    /// module name itself is the host (e.g. the standard library placeholder).
    #[test]
    fn normalize_purl_golang_no_namespace() {
        assert_eq!(
            normalize_purl("pkg:golang/Std@go1.21"),
            "pkg:golang/std@go1.21",
        );
        // Already lowercase — unchanged.
        assert_eq!(
            normalize_purl("pkg:golang/std@go1.21"),
            "pkg:golang/std@go1.21",
        );
    }

    /// Qualifiers and subpath on a golang PURL must survive the raw-string edit.
    #[test]
    fn normalize_purl_golang_preserves_qualifiers_and_subpath() {
        assert_eq!(
            normalize_purl(
                "pkg:golang/GITHUB.COM/Azure/pkg@1.0?vcs_url=https://github.com/Azure/pkg#sub/path"
            ),
            "pkg:golang/github.com/Azure/pkg@1.0?vcs_url=https://github.com/Azure/pkg#sub/path",
        );
    }

    /// A golang PURL whose type prefix is not all-lowercase is returned unchanged
    /// because `lowercase_first_path_segment` relies on the crate serializer
    /// always emitting a lowercase type; all PURL-generating paths in this code
    /// base go through that serializer so this edge is never triggered in practice.
    #[test]
    fn normalize_purl_golang_mixed_case_type_unchanged() {
        // The crate's serializer always lowercases the type, so "pkg:Golang/"
        // never appears in practice. The function documents this no-op contract.
        assert_eq!(
            normalize_purl("pkg:Golang/GITHUB.COM/Azure/pkg@1.0"),
            "pkg:Golang/GITHUB.COM/Azure/pkg@1.0",
        );
    }
}
