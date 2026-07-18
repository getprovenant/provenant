// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use crate::models::{DatasourceId, PackageType};
    use crate::parsers::{DenoLockParser, PackageParser};

    #[test]
    fn test_is_match() {
        assert!(DenoLockParser::is_match(Path::new("deno.lock")));
        assert!(!DenoLockParser::is_match(Path::new("package-lock.json")));
    }

    #[test]
    fn test_extract_from_deno_lock_v5() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("deno.lock");
        fs::write(&file_path, sample_deno_lock()).unwrap();

        let package_data = DenoLockParser::extract_first_package(&file_path);

        assert_eq!(package_data.package_type, Some(PackageType::Deno));
        assert_eq!(package_data.primary_language.as_deref(), Some("TypeScript"));
        assert_eq!(package_data.datasource_id, Some(DatasourceId::DenoLock));
        assert_eq!(
            package_data
                .extra_data
                .as_ref()
                .and_then(|extra| extra.get("version"))
                .and_then(|value| value.as_str()),
            Some("5")
        );

        let direct_assert = package_data
            .dependencies
            .iter()
            .find(|dep| dep.purl.as_deref() == Some("pkg:generic/jsr.io/%40std/assert@1.0.19"))
            .unwrap();
        assert_eq!(direct_assert.is_direct, Some(true));
        assert_eq!(direct_assert.is_runtime, None);
        assert_eq!(direct_assert.is_pinned, Some(true));

        let transitive_internal = package_data
            .dependencies
            .iter()
            .find(|dep| dep.purl.as_deref() == Some("pkg:generic/jsr.io/%40std/internal@1.0.12"))
            .unwrap();
        assert_eq!(transitive_internal.is_direct, Some(false));
        assert_eq!(transitive_internal.is_runtime, None);

        let chalk = package_data
            .dependencies
            .iter()
            .find(|dep| dep.extracted_requirement.as_deref() == Some("npm:chalk@5"))
            .unwrap();
        assert_eq!(chalk.is_direct, Some(true));
        assert_eq!(chalk.is_runtime, None);
        assert_eq!(chalk.purl.as_deref(), Some("pkg:npm/chalk@5.6.2"));

        let remote = package_data
            .dependencies
            .iter()
            .find(|dep| {
                dep.extracted_requirement.as_deref() == Some("https://deno.land/x/oak/mod.ts")
            })
            .unwrap();
        assert_eq!(remote.is_runtime, None);
        assert_eq!(remote.is_direct, None);
        assert!(remote.resolved_package.is_some());
    }

    // deno.lock v2/v3 (Deno 1.18-1.40) nest npm deps under npm.{specifiers,packages}.
    #[test]
    fn test_extract_from_deno_lock_v3_nested_npm() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("deno.lock");
        fs::write(
            &file_path,
            r#"{
  "version": "3",
  "npm": {
    "specifiers": { "chalk@5": "chalk@5.6.2" },
    "packages": {
      "chalk@5.6.2": { "integrity": "sha512-aaaa", "dependencies": { "ansi-styles": "ansi-styles@6.2.1" } },
      "ansi-styles@6.2.1": { "integrity": "sha512-bbbb", "dependencies": {} }
    }
  },
  "remote": {}
}"#,
        )
        .unwrap();

        let package_data = DenoLockParser::extract_first_package(&file_path);

        assert_eq!(package_data.datasource_id, Some(DatasourceId::DenoLock));
        assert_eq!(package_data.dependencies.len(), 2);
        let chalk = package_data
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:npm/chalk@5.6.2"))
            .expect("chalk dependency");
        assert_eq!(chalk.is_direct, Some(true));
        // A directly-referenced package keeps its requested specifier as the requirement.
        assert_eq!(chalk.extracted_requirement.as_deref(), Some("chalk@5"));
        // v2/v3 encode nested dependencies as an object; the resolved key still surfaces.
        let chalk_resolved = chalk
            .resolved_package
            .as_ref()
            .expect("chalk resolved package");
        assert!(
            chalk_resolved
                .dependencies
                .iter()
                .any(|d| d.purl.as_deref() == Some("pkg:npm/ansi-styles@6.2.1")),
            "object-shaped nested dependencies should be parsed"
        );
        let ansi = package_data
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:npm/ansi-styles@6.2.1"))
            .expect("transitive ansi-styles dependency");
        assert_eq!(ansi.is_direct, Some(false));
    }

    // deno.lock v4 (Deno 1.45+) uses the same flat layout as v5.
    #[test]
    fn test_extract_from_deno_lock_v4_flat() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("deno.lock");
        fs::write(
            &file_path,
            r#"{
  "version": "4",
  "specifiers": { "npm:chalk@5": "5.6.2" },
  "npm": { "chalk@5.6.2": { "integrity": "sha512-aaaa" } },
  "workspace": { "dependencies": ["npm:chalk@5"] }
}"#,
        )
        .unwrap();

        let package_data = DenoLockParser::extract_first_package(&file_path);

        assert_eq!(package_data.datasource_id, Some(DatasourceId::DenoLock));
        assert_eq!(package_data.dependencies.len(), 1);
        assert_eq!(
            package_data.dependencies[0].purl.as_deref(),
            Some("pkg:npm/chalk@5.6.2")
        );
    }

    // An unrecognized/newer version must not silently drop everything; it falls back to
    // the latest known (flat) layout.
    #[test]
    fn test_extract_from_deno_lock_future_version_falls_back_to_flat() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("deno.lock");
        fs::write(
            &file_path,
            r#"{
  "version": "99",
  "specifiers": { "npm:chalk@5": "5.6.2" },
  "npm": { "chalk@5.6.2": { "integrity": "sha512-aaaa" } }
}"#,
        )
        .unwrap();

        let package_data = DenoLockParser::extract_first_package(&file_path);

        assert_eq!(package_data.datasource_id, Some(DatasourceId::DenoLock));
        assert_eq!(package_data.dependencies.len(), 1);
        assert_eq!(
            package_data.dependencies[0].purl.as_deref(),
            Some("pkg:npm/chalk@5.6.2")
        );
    }

    // deno.lock v1 (Deno 1.0-1.17) predates npm/jsr support: remote ESM imports only, so
    // no registry dependencies, but it must still be recognized (not error out).
    #[test]
    fn test_extract_from_deno_lock_v1_remote_only() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("deno.lock");
        fs::write(
            &file_path,
            r#"{
  "version": "1",
  "remote": {
    "https://deno.land/std@0.100.0/fmt/colors.ts": "abc0000000000000000000000000000000000000000000000000000000000abc"
  }
}"#,
        )
        .unwrap();

        let package_data = DenoLockParser::extract_first_package(&file_path);

        assert_eq!(package_data.datasource_id, Some(DatasourceId::DenoLock));
        assert!(package_data.dependencies.is_empty());
    }

    fn sample_deno_lock() -> &'static str {
        r#"{
  "version": "5",
  "specifiers": {
    "jsr:@std/assert@1": "1.0.19",
    "npm:chalk@5": "5.6.2"
  },
  "jsr": {
    "@std/assert@1.0.19": {
      "integrity": "sha256-qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=",
      "dependencies": ["jsr:@std/internal"]
    },
    "@std/internal@1.0.12": {
      "integrity": "sha256-u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7s="
    }
  },
  "npm": {
    "chalk@5.6.2": {
      "integrity": "sha512-zMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzA=="
    }
  },
  "redirects": {
    "https://deno.land/x/oak/mod.ts": "https://deno.land/x/oak@v17.2.0/mod.ts"
  },
  "remote": {
    "https://deno.land/x/oak@v17.2.0/mod.ts": "ddd00000000000000000000000000000000000000000000000000000000000dd"
  },
  "workspace": {
    "dependencies": [
      "jsr:@std/assert@1",
      "npm:chalk@5"
    ]
  }
}"#
    }
}
