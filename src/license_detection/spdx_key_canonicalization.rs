// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Build-time canonicalization of `LicenseRef-scancode-*` SPDX keys.
//!
//! Upstream ScanCode enforces an (arbitrary) "spdx_license_key must be 50
//! characters or less" lint in `licensedcode/models.py`. For licenses in the
//! ScanCode `LicenseRef-scancode-<key>` namespace this forced dozens of keys to
//! be squashed or truncated (for example
//! `LicenseRef-scancode-openssl-exception-lgpl3.0plus` instead of the canonical
//! `LicenseRef-scancode-openssl-exception-lgpl-3.0-plus`), see ScanCode PR
//! aboutcode-org/scancode-toolkit#5221.
//!
//! For the `LicenseRef-scancode-` namespace the SPDX key is by definition the
//! license `key` with the namespace prefix, so any deviation is a distortion,
//! not a semantic choice. Provenant applies no length limit, so this pass
//! restores the canonical form at index-build time and keeps the previous value
//! in `other_spdx_license_keys` for backward compatibility.
//!
//! A small set of licenses carry a typo in the license `key` itself while their
//! `spdx_license_key` is already correct. There, mirroring the key would
//! *regress* the correct SPDX key, so those keys are exempted from this pass.
//! The license key is left exactly as upstream has it (ScanCode parity), which
//! is harmless because renaming a license key is a foreign-identity change with
//! no backward-compatible alias mechanism, and the SPDX key users consume is
//! already right.

use crate::license_detection::models::LoadedLicense;

/// The ScanCode SPDX LicenseRef namespace prefix.
const SCANCODE_LICENSEREF_PREFIX: &str = "LicenseRef-scancode-";

/// License keys exempted from canonicalization because the upstream typo is in
/// the license `key`, not the SPDX key. Mirroring the key would regress an
/// already-correct `spdx_license_key`, so the entry is left untouched.
///
/// Each entry must be justified.
const SPDX_CANONICALIZATION_EXEMPT_KEYS: &[&str] = &[
    // "TCG" = Trusted Computing Group (see the license owner/holders). The key
    // misspells it as "tgc" while spdx_license_key already uses the correct
    // "tcg" (LicenseRef-scancode-tcg-spec-license-v2). Canonicalizing would
    // rewrite that correct SPDX key into the typo, so leave it as upstream has
    // it; the license key stays ScanCode-compatible and the SPDX key stays right.
    "tgc-spec-license-v2",
];

/// A single SPDX key that was canonicalized to mirror the license `key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpdxKeyCanonicalization {
    pub license_key: String,
    pub previous_spdx_license_key: String,
    pub canonical_spdx_license_key: String,
}

/// Summary of the changes applied by [`canonicalize_license_spdx_keys`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpdxKeyCanonicalizationReport {
    pub canonicalized_spdx_keys: Vec<SpdxKeyCanonicalization>,
}

impl SpdxKeyCanonicalizationReport {
    pub fn is_empty(&self) -> bool {
        self.canonicalized_spdx_keys.is_empty()
    }
}

/// Canonicalize `LicenseRef-scancode-*` SPDX keys in place so the suffix mirrors
/// each license `key`.
///
/// This is a build-time curation step: it runs when the embedded license index
/// artifact is generated, so the corrected values are baked into the artifact
/// and flow through the SPDX mapping, license references, and SPDX output.
pub fn canonicalize_license_spdx_keys(
    licenses: &mut [LoadedLicense],
) -> SpdxKeyCanonicalizationReport {
    let mut report = SpdxKeyCanonicalizationReport::default();

    for license in licenses.iter_mut() {
        if SPDX_CANONICALIZATION_EXEMPT_KEYS.contains(&license.key.as_str()) {
            continue;
        }

        let Some(spdx) = license.spdx_license_key.as_deref() else {
            continue;
        };
        if !spdx.starts_with(SCANCODE_LICENSEREF_PREFIX) {
            continue;
        }

        let canonical = format!("{SCANCODE_LICENSEREF_PREFIX}{}", license.key);
        if spdx == canonical {
            continue;
        }

        let previous = spdx.to_string();

        // Drop the canonical form from the alias list to avoid duplicating the
        // new primary, and preserve the previous primary as a backward-compatible
        // alias.
        license
            .other_spdx_license_keys
            .retain(|k| k != &canonical && k != &previous);
        license.other_spdx_license_keys.push(previous.clone());

        license.spdx_license_key = Some(canonical.clone());
        report
            .canonicalized_spdx_keys
            .push(SpdxKeyCanonicalization {
                license_key: license.key.clone(),
                previous_spdx_license_key: previous,
                canonical_spdx_license_key: canonical,
            });
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn license(key: &str, spdx: Option<&str>, other: &[&str]) -> LoadedLicense {
        LoadedLicense {
            key: key.to_string(),
            spdx_license_key: spdx.map(str::to_string),
            other_spdx_license_keys: other.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn promotes_canonical_form_and_demotes_squashed_primary() {
        // The openssl-exception-lgpl-3.0-plus case from ScanCode PR #5221.
        let mut licenses = vec![license(
            "openssl-exception-lgpl-3.0-plus",
            Some("LicenseRef-scancode-openssl-exception-lgpl3.0plus"),
            &["LicenseRef-scancode-openssl-exception-lgpl-3.0-plus"],
        )];

        let report = canonicalize_license_spdx_keys(&mut licenses);

        assert_eq!(
            licenses[0].spdx_license_key.as_deref(),
            Some("LicenseRef-scancode-openssl-exception-lgpl-3.0-plus")
        );
        assert_eq!(
            licenses[0].other_spdx_license_keys,
            vec!["LicenseRef-scancode-openssl-exception-lgpl3.0plus".to_string()]
        );
        assert_eq!(report.canonicalized_spdx_keys.len(), 1);
    }

    #[test]
    fn adds_previous_primary_when_canonical_not_already_present() {
        // Truncation case (gradle-enterprise-sla-2022-11-08): canonical is not in
        // the alias list yet.
        let mut licenses = vec![license(
            "gradle-enterprise-sla-2022-11-08",
            Some("LicenseRef-scancode-gradle-enterprise-sla-2022-11-"),
            &[],
        )];

        canonicalize_license_spdx_keys(&mut licenses);

        assert_eq!(
            licenses[0].spdx_license_key.as_deref(),
            Some("LicenseRef-scancode-gradle-enterprise-sla-2022-11-08")
        );
        assert_eq!(
            licenses[0].other_spdx_license_keys,
            vec!["LicenseRef-scancode-gradle-enterprise-sla-2022-11-".to_string()]
        );
    }

    #[test]
    fn exempts_key_typo_so_correct_spdx_key_is_not_regressed() {
        // tgc-spec-license-v2: the key is the typo, the SPDX key is already
        // correct. The pass must leave both fields untouched (no rename, no SPDX
        // regression).
        let mut licenses = vec![license(
            "tgc-spec-license-v2",
            Some("LicenseRef-scancode-tcg-spec-license-v2"),
            &[],
        )];

        let report = canonicalize_license_spdx_keys(&mut licenses);

        assert_eq!(licenses[0].key, "tgc-spec-license-v2");
        assert_eq!(
            licenses[0].spdx_license_key.as_deref(),
            Some("LicenseRef-scancode-tcg-spec-license-v2"),
            "correct SPDX key must be preserved, not regressed to the typo"
        );
        assert!(licenses[0].other_spdx_license_keys.is_empty());
        assert!(report.is_empty());
    }

    #[test]
    fn leaves_canonical_and_real_spdx_keys_untouched() {
        let mut licenses = vec![
            license("mit", Some("MIT"), &[]),
            license(
                "some-ref",
                Some("LicenseRef-scancode-some-ref"),
                &["LicenseRef-scancode-old-alias"],
            ),
            license("no-spdx", None, &[]),
        ];

        let report = canonicalize_license_spdx_keys(&mut licenses);

        assert!(report.is_empty());
        assert_eq!(licenses[0].spdx_license_key.as_deref(), Some("MIT"));
        assert_eq!(
            licenses[1].other_spdx_license_keys,
            vec!["LicenseRef-scancode-old-alias".to_string()]
        );
    }
}
