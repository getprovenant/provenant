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
/// - **Single unambiguous result**: promoted only when the legal files resolve to
///   exactly one distinct declared expression; conflicting results are left unset
///   rather than guessing an `AND`/`OR` combination.
/// - **Provenance preserved**: the legal file's `license_detections` are copied
///   onto the package, keeping each detection's `from_file` so consumers can tell a
///   co-hosted-file-derived license from a manifest-declared one.
pub(super) fn promote_package_declared_license_from_legal_files(
    files: &[FileInfo],
    packages: &mut [Package],
) {
    // Detections carried by legal files, grouped by their containing directory.
    let mut detections_by_dir: HashMap<String, Vec<LicenseDetection>> = HashMap::new();
    for file in files.iter().filter(|file| is_legal_file(file)) {
        if file.license_detections.is_empty() {
            continue;
        }
        detections_by_dir
            .entry(parent_dir(&file.path).to_string())
            .or_default()
            .extend(file.license_detections.iter().cloned());
    }
    if detections_by_dir.is_empty() {
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

        let detections: Vec<LicenseDetection> = package_anchor_dirs(package)
            .into_iter()
            .filter(|dir| packages_per_dir.get(dir).copied() == Some(1))
            .filter_map(|dir| detections_by_dir.get(&dir))
            .flatten()
            .cloned()
            .collect();
        if detections.is_empty() {
            continue;
        }

        // Promote only when the legal files agree on a single declared expression.
        let distinct_expressions = unique(
            &detections
                .iter()
                .map(|detection| detection.license_expression.clone())
                .collect::<Vec<_>>(),
        );
        let [expression] = distinct_expressions.as_slice() else {
            continue;
        };

        package.declared_license_expression = Some(expression.clone());
        package.declared_license_expression_spdx = combine_license_expressions(
            detections
                .iter()
                .map(|detection| detection.license_expression_spdx.clone())
                .filter(|spdx| !spdx.is_empty()),
        );
        package.license_detections = detections;
    }
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
    use crate::models::PackageData;

    fn package_with_datasources(datasources: &[DatasourceId]) -> Package {
        let mut package =
            Package::from_package_data(&PackageData::default(), "Package.resolved".to_string());
        package.datasource_ids = datasources.to_vec();
        package
    }

    fn legal(path: &str, expression: &str, spdx: &str) -> FileInfo {
        let mut file = crate::post_processing::test_utils::file(path);
        file.license_detections = vec![LicenseDetection {
            license_expression: expression.to_string(),
            license_expression_spdx: spdx.to_string(),
            matches: Vec::new(),
            detection_log: Vec::new(),
            identifier: String::new(),
        }];
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
