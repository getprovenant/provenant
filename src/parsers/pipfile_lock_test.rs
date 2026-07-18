// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use crate::parsers::{PackageParser, PipfileLockParser};

    #[test]
    fn test_pipfile_lock_with_develop_dependencies() {
        use std::fs;
        use tempfile::tempdir;

        let content = r#"{
    "_meta": {
        "hash": {"sha256": "test-hash"},
        "pipfile-spec": 6
    },
    "default": {
        "requests": {
            "hashes": ["sha256:abc123"],
            "version": "==2.28.0"
        }
    },
    "develop": {
        "pytest": {
            "hashes": ["sha256:def456"],
            "version": "==7.2.0"
        },
        "black": {
            "hashes": ["sha256:ghi789"],
            "version": "==23.1.0"
        }
    }
}"#;

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("Pipfile.lock");
        fs::write(&file_path, content).unwrap();

        let package_data = PipfileLockParser::extract_first_package(&file_path);

        assert_eq!(package_data.dependencies.len(), 3);

        let default_deps: Vec<_> = package_data
            .dependencies
            .iter()
            .filter(|dep| dep.scope.as_deref() == Some("install"))
            .collect();
        assert_eq!(default_deps.len(), 1);
        assert_eq!(default_deps[0].is_runtime, Some(true));
        // The default/develop section split is provable, but Pipfile.lock's sections are
        // the full flattened closure (direct + transitive), so neither `is_direct` nor
        // `is_optional` can be asserted from the lock alone.
        assert_eq!(default_deps[0].is_direct, None);
        assert_eq!(default_deps[0].is_optional, None);

        let develop_deps: Vec<_> = package_data
            .dependencies
            .iter()
            .filter(|dep| dep.scope.as_deref() == Some("develop"))
            .collect();
        assert_eq!(develop_deps.len(), 2);
        for dep in develop_deps {
            assert_eq!(dep.scope, Some("develop".to_string()));
            assert_eq!(dep.is_runtime, Some(false));
            assert_eq!(dep.is_direct, None);
            assert_eq!(dep.is_optional, None);
        }
    }

    #[test]
    fn test_pipfile_lock_surfaces_pinned_hashes_as_hash_options() {
        use serde_json::json;
        use std::fs;
        use tempfile::tempdir;

        let content = r#"{
    "_meta": {"hash": {"sha256": "test-hash"}, "pipfile-spec": 6},
    "default": {
        "requests": {
            "hashes": ["sha256:abc123", "sha256:def456"],
            "version": "==2.28.0"
        },
        "no-hash-pkg": {
            "version": "==1.0.0"
        }
    }
}"#;

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("Pipfile.lock");
        fs::write(&file_path, content).unwrap();

        let package_data = PipfileLockParser::extract_first_package(&file_path);

        let requests = package_data
            .dependencies
            .iter()
            .find(|dep| dep.purl.as_deref() == Some("pkg:pypi/requests@2.28.0"))
            .expect("requests dependency");
        let hash_options = requests
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("hash_options"))
            .expect("hash_options present");
        assert_eq!(
            hash_options,
            &json!(["sha256:abc123", "sha256:def456"]),
            "pinned artifact hashes should be surfaced as hash_options"
        );

        // A dependency without pinned hashes carries no hash_options noise.
        let no_hash = package_data
            .dependencies
            .iter()
            .find(|dep| dep.purl.as_deref() == Some("pkg:pypi/no-hash-pkg@1.0.0"))
            .expect("no-hash-pkg dependency");
        assert!(
            no_hash
                .extra_data
                .as_ref()
                .is_none_or(|extra| !extra.contains_key("hash_options")),
            "dependencies without pinned hashes should not get hash_options"
        );
    }
}
