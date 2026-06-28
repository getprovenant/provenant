// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::collections::{HashMap, HashSet};

use crate::models::{DatasourceId, FileInfo, LicenseDetection, Package};
use crate::utils::path::parent_dir;
use crate::utils::spdx::combine_license_expressions;

use super::PackageIx;
use super::classification::is_legal_file;
use super::output_indexes::OutputIndexes;
use super::summary_helpers::unique;

/// Packages synthesized purely from a dependency lockfile's resolved entries
/// (currently Swift `Package.resolved`) describe remote third-party packages, not a
/// package anchored by a local manifest. A co-located `LICENSE`/`README` therefore
/// belongs to the enclosing repository, not to these dependency records, so key-file
/// copyright/holder must not be promoted onto them (matching ScanCode, which leaves
/// them unset). The primary package retains a manifest datasource alongside the
/// resolved one, so it is not affected by this guard.
fn is_resolved_dependency_record(package: &Package) -> bool {
    !package.datasource_ids.is_empty()
        && package
            .datasource_ids
            .iter()
            .all(|id| matches!(id, DatasourceId::SwiftPackageResolved))
}

pub(super) fn promote_package_metadata_from_key_files(
    files: &[FileInfo],
    packages: &mut [Package],
    indexes: &OutputIndexes,
) {
    for (idx, package) in packages.iter_mut().enumerate() {
        if is_resolved_dependency_record(package) {
            continue;
        }

        let Some(key_file_indices) = indexes.key_file_indices_for_package(PackageIx(idx)) else {
            continue;
        };

        if package.copyright.is_none() {
            package.copyright = key_file_indices
                .iter()
                .filter_map(|index| files.get(index.0))
                .flat_map(|file| file.copyrights.iter())
                .map(|copyright| copyright.normalized_text().to_string())
                .next();
        }

        if package.holder.is_none() {
            let promoted_holders = unique(
                &key_file_indices
                    .iter()
                    .filter_map(|index| files.get(index.0))
                    .flat_map(|file| file.holders.iter())
                    .map(|holder| holder.holder.clone())
                    .collect::<Vec<_>>(),
            );
            if promoted_holders.len() == 1 {
                package.holder = promoted_holders.into_iter().next();
            }
        }
    }
}

/// Promotes a co-hosted legal file's detected license onto a package that declares
/// no license of its own.
///
/// This is the bounded post-assembly pass adopted in
/// [ADR 0010](../../docs/adr/0010-package-license-from-cohosted-files.md). Many
/// package formats carry no declared-license field (Go `go.mod`, autotools, Swift,
/// Bazel/Gradle sub-modules), so the only declared-license signal for such a
/// package is a `LICENSE`/`COPYING`/`NOTICE` file in its directory. ScanCode
/// attaches that license via `add_license_from_file`; this pass does the same under
/// strict guards:
///
/// - **Genuine absence only**: the package has no `declared_license_expression`,
///   `declared_license_expression_spdx`, or `extracted_license_statement` (the
///   manifest neither declared nor referenced a license).
/// - **Legal files only**: sourced from `is_legal_file` files in the package's own
///   directory — never `README`/source files.
/// - **Same-directory, sole-package only**: a legal file is adopted only when it
///   sits in a directory the package is anchored in (one of its `datafile_paths`)
///   *and* that directory anchors no other package. This bounds attribution to the
///   package's own directory and never smears a license across sibling packages or
///   down into nested sub-packages. (`for_packages` is not used here: it is only
///   populated by ecosystem-specific resource-assign passes, so it is empty for
///   exactly the formats this pass targets — Go, autotools, Swift, Bazel.)
/// - **Single legal file, or agreeing files**: promoted from a *single* legal file —
///   which may legitimately carry a compound license (e.g. a repo-root `LICENSE`
///   detecting `mpl-2.0 AND bsl-1.1`) — or when *multiple* legal files resolve to one
///   shared expression. The promoted expression is built from the file's own
///   `license_detections` (whose `license_expression`/`_spdx` are reliably key and
///   SPDX form), not from the file's `detected_license_expression` string, which can
///   already be SPDX-rendered when this pass runs. *Multiple* legal files that resolve
///   to *differing* expressions (e.g. dual `LICENSE-APACHE` + `LICENSE-MIT`) are left
///   unset rather than guessing an `AND`/`OR` combination. See ADR 0010 "Single legal
///   file with a compound license vs. multiple disagreeing files".
/// - **Provenance preserved**: the legal file's `license_detections` are copied
///   onto the package, keeping each detection's `from_file` so consumers can tell a
///   co-hosted-file-derived license from a manifest-declared one.
pub(super) fn promote_package_declared_license_from_legal_files(
    files: &[FileInfo],
    packages: &mut [Package],
) {
    // Legal files (with their detections), grouped by their containing directory.
    let mut legal_files_by_dir: HashMap<String, Vec<&FileInfo>> = HashMap::new();
    for file in files.iter().filter(|file| is_legal_file(file)) {
        if file.license_detections.is_empty() {
            continue;
        }
        legal_files_by_dir
            .entry(parent_dir(&file.path).to_string())
            .or_default()
            .push(file);
    }
    if legal_files_by_dir.is_empty() {
        return;
    }

    // How many distinct packages are anchored (have a datafile) in each directory.
    let mut packages_per_dir: HashMap<String, usize> = HashMap::new();
    for package in packages.iter() {
        for dir in package_anchor_dirs(package) {
            *packages_per_dir.entry(dir).or_insert(0) += 1;
        }
    }

    for package in packages.iter_mut() {
        // Only fill a genuine absence; never override a manifest-declared or
        // manifest-referenced license.
        if package.declared_license_expression.is_some()
            || package.declared_license_expression_spdx.is_some()
            || package.extracted_license_statement.is_some()
        {
            continue;
        }
        // Remote dependency records (e.g. `Package.resolved`) are not anchored by a
        // local manifest, so a co-located legal file belongs to the enclosing repo.
        if is_resolved_dependency_record(package) {
            continue;
        }

        let source_legal_files: Vec<&FileInfo> = package_anchor_dirs(package)
            .into_iter()
            .filter(|dir| packages_per_dir.get(dir).copied() == Some(1))
            .filter_map(|dir| legal_files_by_dir.get(&dir))
            .flatten()
            .copied()
            .collect();
        if source_legal_files.is_empty() {
            continue;
        }

        // The legal files' agreed expression carries the authoritative operator
        // structure — crucially an `OR` choice, which must not be tightened to `AND`.
        // Abstain when separate legal files disagree (e.g. dual `LICENSE-APACHE` +
        // `LICENSE-MIT`).
        let Some(file_expression) = single_declared_expression(&source_legal_files) else {
            continue;
        };

        let detections: Vec<LicenseDetection> = source_legal_files
            .iter()
            .flat_map(|file| file.license_detections.iter())
            .cloned()
            .collect();

        // Re-render the agreed expression into key and SPDX forms from the per-detection
        // key<->SPDX correspondence, preserving its `AND`/`OR`/`WITH` structure, then
        // canonically order operands to match ScanCode. The file's own
        // `detected_license_expression` string is NOT trusted verbatim for the key field:
        // it can be SPDX-rendered when this pass runs, which previously leaked
        // `BUSL-1.1 AND MPL-2.0` into the key field with a null SPDX field. The SPDX field
        // is rendered strictly — left unset (never key-form text) if any operand lacks an
        // SPDX id, e.g. a custom/unmapped license.
        let (token_to_key, token_to_spdx) = detection_token_maps(&detections);
        let Some(declared_key_expression) =
            render_license_expression(&file_expression, &token_to_key, false)
                .and_then(|key_form| combine_license_expressions([key_form]))
        else {
            continue;
        };
        package.declared_license_expression_spdx =
            render_license_expression(&file_expression, &token_to_spdx, true)
                .and_then(|spdx_form| combine_license_expressions([spdx_form]));
        package.declared_license_expression = Some(declared_key_expression);
        package.license_detections = detections;
    }
}

/// The unambiguous declared expression to promote from a package's co-located legal
/// files, or `None` when the result would be ambiguous.
///
/// - A *single* legal file contributes its own `detected_license_expression` verbatim
///   (the authoritative combined form, preserving its `AND`/`OR` structure rather than
///   re-combining its detections).
/// - *Multiple* legal files contribute only when they all resolve to the same
///   expression; otherwise the directory is genuinely dual-licensed and is left unset.
fn single_declared_expression(legal_files: &[&FileInfo]) -> Option<String> {
    let distinct_expressions = unique(
        &legal_files
            .iter()
            .filter_map(|file| file.detected_license_expression.clone())
            .collect::<Vec<_>>(),
    );
    let [expression] = distinct_expressions.as_slice() else {
        return None;
    };
    Some(expression.clone())
}

/// Builds case-insensitive token maps from a set of detections: license-key token →
/// canonical key form, and license-key token → SPDX id. Each detection's
/// `license_expression` and `license_expression_spdx` share the same operator
/// structure, so their license tokens align positionally. Both the key and the SPDX
/// spelling of every token are indexed, so an expression in *either* form resolves.
fn detection_token_maps(
    detections: &[LicenseDetection],
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut token_to_key: HashMap<String, String> = HashMap::new();
    let mut token_to_spdx: HashMap<String, String> = HashMap::new();
    for detection in detections {
        let pairs = std::iter::once((
            detection.license_expression.as_str(),
            detection.license_expression_spdx.as_str(),
        ))
        .chain(detection.matches.iter().map(|match_item| {
            (
                match_item.license_expression.as_str(),
                match_item.license_expression_spdx.as_str(),
            )
        }));
        for (key_expression, spdx_expression) in pairs {
            let keys = license_tokens(key_expression);
            let spdxes = license_tokens(spdx_expression);
            if keys.len() != spdxes.len() {
                continue;
            }
            for (key, spdx) in keys.into_iter().zip(spdxes) {
                if key.is_empty() || spdx.is_empty() {
                    continue;
                }
                token_to_key
                    .entry(key.to_ascii_lowercase())
                    .or_insert_with(|| key.to_string());
                token_to_key
                    .entry(spdx.to_ascii_lowercase())
                    .or_insert_with(|| key.to_string());
                token_to_spdx
                    .entry(key.to_ascii_lowercase())
                    .or_insert_with(|| spdx.to_string());
                token_to_spdx
                    .entry(spdx.to_ascii_lowercase())
                    .or_insert_with(|| spdx.to_string());
            }
        }
    }
    (token_to_key, token_to_spdx)
}

/// Re-renders a license expression by mapping each license-key token (case-insensitively)
/// through `token_map`, leaving `AND`/`OR`/`WITH` operators and parentheses untouched so
/// the operator structure is preserved. With `strict`, returns `None` if any license
/// token is unmapped — so an SPDX rendering that cannot fully resolve yields an absent
/// field rather than leaking key-form text. Without `strict`, an unmapped token passes
/// through unchanged.
fn render_license_expression(
    expression: &str,
    token_map: &HashMap<String, String>,
    strict: bool,
) -> Option<String> {
    let mut rendered = String::with_capacity(expression.len());
    for token in tokenize_license_expression(expression) {
        match token {
            ExpressionToken::Operator(text) => rendered.push_str(text),
            ExpressionToken::License(key) => match token_map.get(&key.to_ascii_lowercase()) {
                Some(mapped) => rendered.push_str(mapped),
                None if strict => return None,
                None => rendered.push_str(key),
            },
        }
    }
    Some(rendered)
}

enum ExpressionToken<'a> {
    /// Operators, parentheses, and whitespace — emitted verbatim.
    Operator(&'a str),
    /// A license-key token to be mapped through a token map.
    License(&'a str),
}

/// Splits a license expression into license-key tokens and the operator/punctuation
/// runs between them, preserving every character so the input can be reconstructed.
fn tokenize_license_expression(expression: &str) -> Vec<ExpressionToken<'_>> {
    let is_license_char = |c: char| c.is_alphanumeric() || matches!(c, '-' | '.' | '_' | '+' | ':');
    let mut tokens = Vec::new();
    let mut rest = expression;
    while !rest.is_empty() {
        let boundary = rest.find(is_license_char).unwrap_or(rest.len());
        if boundary > 0 {
            tokens.push(ExpressionToken::Operator(&rest[..boundary]));
            rest = &rest[boundary..];
            continue;
        }
        let end = rest
            .find(|c: char| !is_license_char(c))
            .unwrap_or(rest.len());
        let word = &rest[..end];
        if is_expression_operator(word) {
            tokens.push(ExpressionToken::Operator(word));
        } else {
            tokens.push(ExpressionToken::License(word));
        }
        rest = &rest[end..];
    }
    tokens
}

fn is_expression_operator(word: &str) -> bool {
    matches!(word.to_ascii_uppercase().as_str(), "AND" | "OR" | "WITH")
}

/// The license-key tokens of an expression in order, dropping operators and punctuation.
fn license_tokens(expression: &str) -> Vec<&str> {
    tokenize_license_expression(expression)
        .into_iter()
        .filter_map(|token| match token {
            ExpressionToken::License(license) => Some(license),
            ExpressionToken::Operator(_) => None,
        })
        .collect()
}

/// The distinct directories a package is anchored in (the parent directory of each
/// of its datafiles).
fn package_anchor_dirs(package: &Package) -> HashSet<String> {
    package
        .datafile_paths
        .iter()
        .map(|datafile| parent_dir(datafile).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_detection::MatcherKind;
    use crate::models::{LineNumber, Match, MatchScore, PackageData};

    fn package_with_datasources(datasources: &[DatasourceId]) -> Package {
        let mut package =
            Package::from_package_data(&PackageData::default(), "Package.resolved".to_string());
        package.datasource_ids = datasources.to_vec();
        package
    }

    /// A match whose `from_file` records the legal file it came from, matching the
    /// provenance real detections carry (see `FileInfo` license-detection plumbing).
    fn match_from(from_file: &str, expression: &str, spdx: &str) -> Match {
        Match {
            license_expression: expression.to_string(),
            license_expression_spdx: spdx.to_string(),
            from_file: Some(from_file.to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
            matcher: MatcherKind::default(),
            score: MatchScore::MAX,
            matched_length: Some(1),
            match_coverage: Some(100.0),
            rule_relevance: Some(100),
            rule_identifier: String::new(),
            rule_url: None,
            matched_text: None,
            matched_text_diagnostics: None,
            referenced_filenames: None,
        }
    }

    fn detection(path: &str, expression: &str, spdx: &str) -> LicenseDetection {
        LicenseDetection {
            license_expression: expression.to_string(),
            license_expression_spdx: spdx.to_string(),
            matches: vec![match_from(path, expression, spdx)],
            detection_log: Vec::new(),
            identifier: String::new(),
        }
    }

    fn legal(path: &str, expression: &str, spdx: &str) -> FileInfo {
        let mut file = crate::post_processing::test_utils::file(path);
        file.license_detections = vec![detection(path, expression, spdx)];
        file.detected_license_expression = Some(expression.to_string());
        file
    }

    /// A single legal file carrying a *compound* license: several detections that all
    /// originate from the same file. `combined` is the file's own
    /// `detected_license_expression` (the authoritative combined form, e.g.
    /// `bsl-1.1 AND mpl-2.0` or `mit OR apache-2.0`), which need not be the
    /// `AND`-combination of the individual detections.
    fn legal_compound(path: &str, combined: &str, expressions: &[(&str, &str)]) -> FileInfo {
        let mut file = crate::post_processing::test_utils::file(path);
        file.license_detections = expressions
            .iter()
            .map(|(expression, spdx)| detection(path, expression, spdx))
            .collect();
        file.detected_license_expression = Some(combined.to_string());
        file
    }

    fn pkg(uid: &str, datafile: &str) -> Package {
        crate::post_processing::test_utils::package(uid, datafile)
    }

    #[test]
    fn promotes_single_colocated_legal_file() {
        let files = vec![legal("proj/LICENSE", "apache-2.0", "Apache-2.0")];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("apache-2.0")
        );
        assert_eq!(
            packages[0].declared_license_expression_spdx.as_deref(),
            Some("Apache-2.0")
        );
        assert_eq!(packages[0].license_detections.len(), 1);
    }

    #[test]
    fn does_not_override_manifest_declared_or_referenced_license() {
        let files = vec![legal("proj/LICENSE", "apache-2.0", "Apache-2.0")];
        let mut declared = pkg("a", "proj/go.mod");
        declared.declared_license_expression = Some("mit".to_string());
        declared.declared_license_expression_spdx = Some("MIT".to_string());
        let mut extracted_only = pkg("b", "other/go.mod");
        extracted_only.extracted_license_statement = Some("see LICENSE".to_string());
        let files2 = vec![legal("other/LICENSE", "apache-2.0", "Apache-2.0")];

        promote_package_declared_license_from_legal_files(
            &files,
            std::slice::from_mut(&mut declared),
        );
        promote_package_declared_license_from_legal_files(
            &files2,
            std::slice::from_mut(&mut extracted_only),
        );

        assert_eq!(declared.declared_license_expression.as_deref(), Some("mit"));
        assert_eq!(
            extracted_only.declared_license_expression, None,
            "an extracted-but-unresolved statement still blocks co-hosted promotion"
        );
    }

    #[test]
    fn skips_directory_hosting_multiple_packages() {
        let files = vec![legal("proj/LICENSE", "apache-2.0", "Apache-2.0")];
        let mut packages = vec![pkg("a", "proj/go.mod"), pkg("b", "proj/pom.xml")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(packages[0].declared_license_expression, None);
        assert_eq!(packages[1].declared_license_expression, None);
    }

    #[test]
    fn does_not_smear_root_license_into_nested_subpackage() {
        let files = vec![legal("proj/LICENSE", "apache-2.0", "Apache-2.0")];
        let mut packages = vec![pkg("root", "proj/go.mod"), pkg("sub", "proj/sub/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("apache-2.0")
        );
        assert_eq!(
            packages[1].declared_license_expression, None,
            "nested sub-package must not inherit the root LICENSE"
        );
    }

    #[test]
    fn each_package_gets_its_own_colocated_license() {
        let files = vec![
            legal("proj/LICENSE", "apache-2.0", "Apache-2.0"),
            legal("proj/sub/LICENSE", "mit", "MIT"),
        ];
        let mut packages = vec![pkg("root", "proj/go.mod"), pkg("sub", "proj/sub/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("apache-2.0")
        );
        assert_eq!(
            packages[1].declared_license_expression.as_deref(),
            Some("mit")
        );
    }

    #[test]
    fn abstains_when_colocated_legal_files_disagree() {
        let files = vec![
            legal("proj/LICENSE-APACHE", "apache-2.0", "Apache-2.0"),
            legal("proj/LICENSE-MIT", "mit", "MIT"),
        ];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression, None,
            "conflicting co-hosted licenses are left unset rather than guessed"
        );
    }

    #[test]
    fn promotes_single_file_compound_license() {
        // One legal file (terraform's repo-root `LICENSE`) legitimately detects a
        // compound license. Its own combined expression must be promoted rather than
        // abstained on as if it were separate disagreeing files.
        let files = vec![legal_compound(
            "proj/LICENSE",
            // The file's own detected expression in detection order; promotion should
            // canonically reorder it to match ScanCode.
            "mpl-2.0 AND bsl-1.1",
            &[("mpl-2.0", "MPL-2.0"), ("bsl-1.1", "BUSL-1.1")],
        )];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("bsl-1.1 AND mpl-2.0"),
            "a single legal file's compound license is promoted as its own canonically ordered expression"
        );
        assert_eq!(
            packages[0].declared_license_expression_spdx.as_deref(),
            Some("BUSL-1.1 AND MPL-2.0")
        );
        assert_eq!(packages[0].license_detections.len(), 2);
    }

    #[test]
    fn promotes_key_form_even_when_file_expression_is_spdx() {
        // Regression guard: a legal file's own `detected_license_expression` can be in
        // SPDX form when this pass runs, while its detections still carry ScanCode keys.
        // The promoted key field must be derived from the detections (ScanCode keys),
        // with a matching SPDX field — never the file's SPDX string in the key field
        // with a null SPDX field.
        let files = vec![legal_compound(
            "proj/LICENSE",
            "BUSL-1.1 AND MPL-2.0", // file expression already rendered in SPDX form
            &[("mpl-2.0", "MPL-2.0"), ("bsl-1.1", "BUSL-1.1")],
        )];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("bsl-1.1 AND mpl-2.0"),
            "key field must be ScanCode keys from the detections, not the file's SPDX string"
        );
        assert_eq!(
            packages[0].declared_license_expression_spdx.as_deref(),
            Some("BUSL-1.1 AND MPL-2.0"),
            "SPDX field must be populated and match the key expression's operands"
        );
    }

    #[test]
    fn preserves_file_level_or_even_with_separate_detections() {
        // The legal file's own expression is an `OR` choice while its licenses are
        // present as separate detections. The agreed file-level `OR` must be preserved,
        // NOT tightened to a stricter `AND` by re-combining the detections.
        let files = vec![legal_compound(
            "proj/LICENSE",
            "apache-2.0 OR mit",
            &[("apache-2.0", "Apache-2.0"), ("mit", "MIT")],
        )];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("apache-2.0 OR mit"),
            "a file-level OR must not be turned into AND even when its licenses are separate detections"
        );
        assert_eq!(
            packages[0].declared_license_expression_spdx.as_deref(),
            Some("Apache-2.0 OR MIT")
        );
    }

    #[test]
    fn leaves_spdx_unset_when_a_license_has_no_spdx_id() {
        // A co-hosted legal file with a custom/unmapped license (no SPDX id) must still
        // get a key-form declared expression, but its SPDX field must be left absent —
        // never filled with key-form text.
        let files = vec![legal("proj/LICENSE", "custom-vendor-license", "")];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("custom-vendor-license"),
            "the key field is still promoted for an unmapped license"
        );
        assert_eq!(
            packages[0].declared_license_expression_spdx, None,
            "the SPDX field is absent rather than carrying key-form text for an unmapped license"
        );
    }

    #[test]
    fn promotes_single_file_alternative_license_without_forcing_and() {
        // A legal file whose detection is an `OR` choice must be promoted verbatim,
        // NOT silently tightened into a stricter `AND`. An `OR` license arises from a
        // single rule, so it is one detection carrying the `OR` expression.
        let files = vec![legal(
            "proj/LICENSE",
            "apache-2.0 OR mit",
            "Apache-2.0 OR MIT",
        )];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("apache-2.0 OR mit"),
            "an OR-shaped single-file expression must not be turned into an AND"
        );
        assert_eq!(
            packages[0].declared_license_expression_spdx.as_deref(),
            Some("Apache-2.0 OR MIT"),
            "the SPDX field must carry the same OR operator as the key expression"
        );
    }

    #[test]
    fn promotes_single_file_with_exception_keeps_spdx() {
        // A single detection whose expression is a `WITH` exception must still produce
        // a matching SPDX field: the per-operand key->SPDX mapping resolves `gpl-2.0`
        // and `classpath-exception-2.0` individually even though the detection records
        // them as one compound expression.
        let files = vec![legal(
            "proj/LICENSE",
            "gpl-2.0 WITH classpath-exception-2.0",
            "GPL-2.0 WITH Classpath-exception-2.0",
        )];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("gpl-2.0 WITH classpath-exception-2.0")
        );
        assert_eq!(
            packages[0].declared_license_expression_spdx.as_deref(),
            Some("GPL-2.0 WITH Classpath-exception-2.0"),
            "a compound WITH expression must not drop its SPDX field"
        );
    }

    #[test]
    fn abstains_when_multiple_files_disagree_even_with_provenance() {
        // Two *separate* legal files disagreeing remains ambiguous: unlike a single
        // compound file, their union must not be guessed into an `AND` expression.
        let files = vec![
            legal("proj/LICENSE-APACHE", "apache-2.0", "Apache-2.0"),
            legal("proj/LICENSE-MIT", "mit", "MIT"),
        ];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression, None,
            "two disagreeing legal files stay ambiguous even though each match has a from_file"
        );
    }

    #[test]
    fn promotes_when_multiple_files_agree() {
        // Two separate legal files that resolve to the *same* expression are not
        // ambiguous, so promotion still proceeds.
        let files = vec![
            legal("proj/LICENSE", "apache-2.0", "Apache-2.0"),
            legal("proj/COPYING", "apache-2.0", "Apache-2.0"),
        ];
        let mut packages = vec![pkg("a", "proj/go.mod")];

        promote_package_declared_license_from_legal_files(&files, &mut packages);

        assert_eq!(
            packages[0].declared_license_expression.as_deref(),
            Some("apache-2.0")
        );
    }

    #[test]
    fn skips_resolved_dependency_records() {
        let files = vec![legal("proj/LICENSE", "apache-2.0", "Apache-2.0")];
        let mut package = pkg("a", "proj/Package.resolved");
        package.datasource_ids = vec![DatasourceId::SwiftPackageResolved];

        promote_package_declared_license_from_legal_files(
            &files,
            std::slice::from_mut(&mut package),
        );

        assert_eq!(package.declared_license_expression, None);
    }

    #[test]
    fn resolved_dependency_record_detected_only_when_resolved_only() {
        // A package built purely from `Package.resolved` is a dependency record.
        assert!(is_resolved_dependency_record(&package_with_datasources(&[
            DatasourceId::SwiftPackageResolved
        ])));
        // The primary package retains its manifest datasource, so it still receives
        // co-located key-file metadata.
        assert!(!is_resolved_dependency_record(&package_with_datasources(
            &[
                DatasourceId::SwiftPackageManifestJson,
                DatasourceId::SwiftPackageResolved,
            ]
        )));
        assert!(!is_resolved_dependency_record(&package_with_datasources(
            &[DatasourceId::SwiftPackageManifestJson]
        )));
        // No datasource at all must not be treated as a dependency record.
        assert!(!is_resolved_dependency_record(&package_with_datasources(
            &[]
        )));
    }
}
