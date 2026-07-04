// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use crate::models::{DatasourceId, PackageType};
    use crate::parsers::{
        ClojureDepsEdnParser, ClojureProjectCljParser, PackageParser, capture_parser_diagnostics,
    };

    fn create_temp_file(file_name: &str, content: &str) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join(file_name);
        fs::write(&file_path, content).expect("Failed to write temp file");
        (temp_dir, file_path)
    }

    #[test]
    fn test_deps_edn_is_match() {
        assert!(ClojureDepsEdnParser::is_match(&PathBuf::from(
            "/repo/deps.edn"
        )));
        assert!(!ClojureDepsEdnParser::is_match(&PathBuf::from(
            "/repo/project.clj"
        )));
        assert!(!ClojureDepsEdnParser::is_match(&PathBuf::from(
            "/repo/deps.edn.bak"
        )));
    }

    #[test]
    fn test_project_clj_is_match() {
        assert!(ClojureProjectCljParser::is_match(&PathBuf::from(
            "/repo/project.clj"
        )));
        assert!(!ClojureProjectCljParser::is_match(&PathBuf::from(
            "/repo/deps.edn"
        )));
        assert!(!ClojureProjectCljParser::is_match(&PathBuf::from(
            "/repo/core.clj"
        )));
    }

    #[test]
    fn test_extract_from_deps_edn_with_top_level_and_alias_deps() {
        let content = r#"
{:paths ["src" "resources"]
 :deps {org.clojure/clojure {:mvn/version "1.12.0"}
        io.github.clojure/tools.build {:git/url "https://github.com/clojure/tools.build.git"
                                       :git/tag "v0.10.5"
                                       :git/sha "abc1234"}
        my.local/lib {:local/root "../lib"}
        exact/lib {:mvn/version "=1.5.0"}}
 :mvn/repos {"clojars" {:url "https://repo.clojars.org/"}}
 :aliases {:test {:extra-deps {lambdaisland/kaocha {:mvn/version "1.91.1392"}}}
           :build {:deps {io.github.clojure/tools.cli {:mvn/version "1.1.230"}}}}}
        "#;

        let (_temp_dir, path) = create_temp_file("deps.edn", content);
        let package_data = ClojureDepsEdnParser::extract_first_package(&path);

        assert_eq!(package_data.package_type, Some(PackageType::Maven));
        assert_eq!(package_data.primary_language.as_deref(), Some("Clojure"));
        assert_eq!(
            package_data.datasource_id,
            Some(DatasourceId::ClojureDepsEdn)
        );
        assert!(package_data.name.is_none());
        assert!(package_data.version.is_none());
        let extra_data = package_data
            .extra_data
            .as_ref()
            .expect("missing extra_data");
        assert!(extra_data.get("paths").is_some());
        assert_eq!(
            extra_data
                .get("mvn_repos")
                .and_then(|value| value.get("clojars"))
                .and_then(|value| value.get("url"))
                .and_then(|value| value.as_str()),
            Some("https://repo.clojars.org/")
        );
        assert!(extra_data.get("aliases").is_some());
        assert_eq!(package_data.dependencies.len(), 6);
    }

    #[test]
    fn test_extract_from_project_clj_literal_metadata_and_profile_dependencies() {
        let content = r#"
(defproject org.example/sample "1.0.0"
  :description "Sample project"
  :url "https://example.org/sample"
  :license {:name "Eclipse Public License"
            :url "https://www.eclipse.org/legal/epl-v10.html"}
  :scm {:url "https://github.com/example/sample"}
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [cheshire "5.12.0"]
                 ["ring/ring-core" "1.12.2" :classifier "tests"]]
  :profiles {:dev {:dependencies [[midje "1.10.10"]]}
             :provided {:dependencies [[javax.servlet/servlet-api "2.5"]]}
             :test {:dependencies [[lambdaisland/kaocha "1.91.1392"]]}})
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let package_data = ClojureProjectCljParser::extract_first_package(&path);

        assert_eq!(package_data.package_type, Some(PackageType::Maven));
        assert_eq!(package_data.primary_language.as_deref(), Some("Clojure"));
        assert_eq!(
            package_data.datasource_id,
            Some(DatasourceId::ClojureProjectClj)
        );
        assert_eq!(package_data.namespace.as_deref(), Some("org.example"));
        assert_eq!(package_data.name.as_deref(), Some("sample"));
        assert_eq!(package_data.version.as_deref(), Some("1.0.0"));
        assert_eq!(
            package_data.purl.as_deref(),
            Some("pkg:maven/org.example/sample@1.0.0")
        );
        assert_eq!(package_data.description.as_deref(), Some("Sample project"));
        assert_eq!(
            package_data.homepage_url.as_deref(),
            Some("https://example.org/sample")
        );
        assert_eq!(
            package_data.vcs_url.as_deref(),
            Some("https://github.com/example/sample")
        );
        assert_eq!(package_data.dependencies.len(), 6);
    }

    #[test]
    fn test_project_clj_skips_non_literal_constructs() {
        let content = r#"
(defproject org.example/dynamic "0.2.0"
  :description dynamic-description
  :url homepage-url
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [foo/bar dep-version]
                 ~(concat [] [])
                 [org.valid/lib "1.0.0"]]
  :profiles {:dev {:dependencies [[midje "1.10.10"]]}
             :test {:dependencies dynamic-test-deps}})
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let package_data = ClojureProjectCljParser::extract_first_package(&path);

        assert_eq!(package_data.namespace.as_deref(), Some("org.example"));
        assert_eq!(package_data.name.as_deref(), Some("dynamic"));
        assert_eq!(package_data.version.as_deref(), Some("0.2.0"));
        assert!(package_data.description.is_none());
        assert!(package_data.homepage_url.is_none());
        assert_eq!(package_data.dependencies.len(), 3);
        assert!(
            package_data
                .dependencies
                .iter()
                .any(|dep| dep.purl.as_deref() == Some("pkg:maven/midje/midje@1.10.10"))
        );
    }

    #[test]
    fn test_project_clj_resolves_dynamic_version_and_def_unquote_deps() {
        let content = r#"
(def netty-version "4.1.135.Final")

(defproject aleph (or (System/getenv "PROJECT_VERSION") "0.9.9")
  :description "Async communication"
  :dependencies [[org.clojure/clojure "1.12.5"]
                 [io.netty/netty-transport ~netty-version]
                 [io.netty/netty-handler ~netty-version :classifier "shaded"]])
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let package_data = ClojureProjectCljParser::extract_first_package(&path);

        // Bare-symbol name defaults its group; the `(or …)` version resolves to
        // its string-literal fallback rather than discarding the manifest.
        assert_eq!(package_data.namespace.as_deref(), Some("aleph"));
        assert_eq!(package_data.name.as_deref(), Some("aleph"));
        assert_eq!(package_data.version.as_deref(), Some("0.9.9"));
        assert_eq!(
            package_data.purl.as_deref(),
            Some("pkg:maven/aleph/aleph@0.9.9")
        );
        // `~netty-version` deps resolve from the file-local `def`.
        assert_eq!(package_data.dependencies.len(), 3);
        assert!(
            package_data
                .dependencies
                .iter()
                .any(|dep| dep.purl.as_deref()
                    == Some("pkg:maven/io.netty/netty-transport@4.1.135.Final"))
        );
        assert!(package_data.dependencies.iter().any(
            |dep| dep.purl.as_deref() == Some("pkg:maven/io.netty/netty-handler@4.1.135.Final")
        ));
    }

    #[test]
    fn test_project_clj_or_version_prefers_first_statically_known_value() {
        // `or` short-circuits to the first truthy value; an unresolvable
        // `(System/getenv …)` is skipped, and a bare `def` symbol resolves in
        // the evaluated version slot before any later literal fallback.
        let content = r#"
(def project-version "2.0.0")

(defproject org.example/orfirst (or (System/getenv "VERSION") project-version "1.0.0")
  :dependencies [[org.clojure/clojure "1.12.0"]])
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let package_data = ClojureProjectCljParser::extract_first_package(&path);

        assert_eq!(package_data.version.as_deref(), Some("2.0.0"));
        assert_eq!(
            package_data.purl.as_deref(),
            Some("pkg:maven/org.example/orfirst@2.0.0")
        );
    }

    #[test]
    fn test_project_clj_or_version_falls_through_to_first_literal() {
        let content = r#"
(defproject my/lib (or (System/getenv "VERSION") "1.0.0" "0.5.0")
  :dependencies [[org.clojure/clojure "1.12.0"]])
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let package_data = ClojureProjectCljParser::extract_first_package(&path);

        assert_eq!(package_data.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn test_project_clj_quoted_and_bare_dep_versions_are_not_resolved() {
        // Inside the quoted `:dependencies` body only a `~` unquote names a
        // `def` value; a `'`-quoted or bare symbol is literal data, so those
        // deps are dropped even though `(def dep-version …)` exists.
        let content = r#"
(def dep-version "1.2.3")

(defproject org.example/quoted "1.0.0"
  :dependencies [[org.clojure/clojure "1.12.0"]
                 [foo/bar 'dep-version]
                 [baz/qux dep-version]
                 [ok/unquoted ~dep-version]])
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let package_data = ClojureProjectCljParser::extract_first_package(&path);

        // clojure (literal) + ok/unquoted (~def) resolve; quoted/bare are dropped.
        assert_eq!(package_data.dependencies.len(), 2);
        assert!(
            package_data
                .dependencies
                .iter()
                .any(|dep| dep.purl.as_deref() == Some("pkg:maven/ok/unquoted@1.2.3"))
        );
        assert!(!package_data.dependencies.iter().any(|dep| {
            (dep.purl.as_deref().unwrap_or_default()).contains("foo/bar")
                || (dep.purl.as_deref().unwrap_or_default()).contains("baz/qux")
        }));
    }

    #[test]
    fn test_project_clj_unresolvable_version_degrades_without_dropping_manifest() {
        // A version that cannot be resolved statically must not discard the
        // recoverable name and dependencies; it is simply left unset.
        let content = r#"
(defproject org.example/degraded (str "1." (build-number))
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [foo/bar undefined-symbol]])
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let package_data = ClojureProjectCljParser::extract_first_package(&path);

        assert_eq!(package_data.namespace.as_deref(), Some("org.example"));
        assert_eq!(package_data.name.as_deref(), Some("degraded"));
        assert!(package_data.version.is_none());
        assert_eq!(
            package_data.purl.as_deref(),
            Some("pkg:maven/org.example/degraded")
        );
        // The literal dep is kept; the undefined-symbol version is still dropped.
        assert_eq!(package_data.dependencies.len(), 1);
        assert_eq!(
            package_data.dependencies[0].purl.as_deref(),
            Some("pkg:maven/org.clojure/clojure@1.11.1")
        );
    }

    #[test]
    fn test_graceful_error_handling_for_invalid_forms() {
        let (_temp_dir, deps_path) = create_temp_file("deps.edn", "{:deps {foo/bar");
        let deps_package = ClojureDepsEdnParser::extract_first_package(&deps_path);

        assert_eq!(deps_package.package_type, Some(PackageType::Maven));
        assert_eq!(
            deps_package.datasource_id,
            Some(DatasourceId::ClojureDepsEdn)
        );
        assert!(deps_package.dependencies.is_empty());

        let (_temp_dir, project_path) = create_temp_file("project.clj", "(defproject foo");
        let project_package = ClojureProjectCljParser::extract_first_package(&project_path);

        assert_eq!(project_package.package_type, Some(PackageType::Maven));
        assert_eq!(
            project_package.datasource_id,
            Some(DatasourceId::ClojureProjectClj)
        );
        assert!(project_package.name.is_none());
    }

    #[test]
    fn test_project_clj_without_defproject_falls_back_without_scan_errors() {
        let content = r#"
(ns leiningen.project)

(defn project [project & args]
  (if (seq args)
    (get-in project (mapv keyword args))
    project))
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let result = capture_parser_diagnostics(
            || ClojureProjectCljParser::extract_packages(&path),
            "ClojureProjectCljParser",
            &path,
            None,
        );

        assert!(result.scan_diagnostics.is_empty());
        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].package_type, Some(PackageType::Maven));
        assert_eq!(
            result.packages[0].datasource_id,
            Some(DatasourceId::ClojureProjectClj)
        );
        assert!(result.packages[0].name.is_none());
    }

    #[test]
    fn test_non_manifest_project_clj_with_reader_syntax_skips_without_scan_errors() {
        let content = r#"
(ns leiningen.core.project)

(def defaults
  {:uberjar-merge-with {#"\\.properties$" [slurp str spit]}})
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let result = capture_parser_diagnostics(
            || ClojureProjectCljParser::extract_packages(&path),
            "ClojureProjectCljParser",
            &path,
            None,
        );

        assert!(result.scan_diagnostics.is_empty());
        assert_eq!(result.packages.len(), 1);
        assert!(result.packages[0].name.is_none());
    }

    #[test]
    fn test_project_clj_template_falls_back_without_scan_errors() {
        let content = r#"
(defproject {{raw-name}} "0.1.0-SNAPSHOT"
  :description "FIXME: write description"
  :url "https://example.com/FIXME"
  :license {:name "EPL-2.0 OR GPL-2.0-or-later WITH Classpath-exception-2.0"
            :url "https://www.eclipse.org/legal/epl-2.0/"}
  :dependencies [[org.clojure/clojure "1.12.2"]])
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let result = capture_parser_diagnostics(
            || ClojureProjectCljParser::extract_packages(&path),
            "ClojureProjectCljParser",
            &path,
            None,
        );

        assert!(result.scan_diagnostics.is_empty());
        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].package_type, Some(PackageType::Maven));
        assert_eq!(
            result.packages[0].datasource_id,
            Some(DatasourceId::ClojureProjectClj)
        );
        assert!(result.packages[0].name.is_none());
        assert!(result.packages[0].dependencies.is_empty());
    }

    #[test]
    fn test_project_clj_accepts_regex_literals_in_ignored_fields() {
        let content = r#"
(defproject nomnomnom "0.5.0-SNAPSHOT"
  :dependencies [[org.clojure/clojure "1.8.0"]
                 [janino "2.5.15"]
                 [org.platypope/method-fn "0.1.0"]
                 [porcupine "0.0.4"]]
  :uberjar-exclusions [#"DUMMY"]
  :uberjar-merge-with {#"\.properties$" [slurp str spit]}
  :test-selectors {:default (fn [m] (not (:integration m)))
                   :integration :integration})
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let result = capture_parser_diagnostics(
            || ClojureProjectCljParser::extract_packages(&path),
            "ClojureProjectCljParser",
            &path,
            None,
        );

        assert!(result.scan_diagnostics.is_empty());
        assert_eq!(result.packages.len(), 1);

        let package_data = &result.packages[0];
        assert_eq!(package_data.name.as_deref(), Some("nomnomnom"));
        assert_eq!(package_data.version.as_deref(), Some("0.5.0-SNAPSHOT"));
        assert_eq!(package_data.dependencies.len(), 4);
    }

    #[test]
    fn test_project_clj_accepts_set_literals_in_ignored_fields() {
        let content = r#"
(defproject set-friendly "1.2.3"
  :dependencies [[org.clojure/clojure "1.12.0"]]
  :prep-tasks #{"compile" "test"}
  :profiles {:dev {:resource-paths #{"dev-resources" "test-resources"}}})
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let result = capture_parser_diagnostics(
            || ClojureProjectCljParser::extract_packages(&path),
            "ClojureProjectCljParser",
            &path,
            None,
        );

        assert!(result.scan_diagnostics.is_empty());
        assert_eq!(result.packages.len(), 1);

        let package_data = &result.packages[0];
        assert_eq!(package_data.name.as_deref(), Some("set-friendly"));
        assert_eq!(package_data.version.as_deref(), Some("1.2.3"));
        assert_eq!(package_data.dependencies.len(), 1);
    }

    #[test]
    fn test_project_clj_accepts_function_literals_in_ignored_fields() {
        let content = r#"
(defproject org.example/functions "1.0.0"
  :dependencies [[org.clojure/clojure "1.12.0"]]
  :manifest {"Class-Path" ~#(clojure.string/join
                             \space
                             ["lib/a.jar" "lib/b.jar"])}
  :filespecs [{:type :fn
               :fn (fn [p]
                     {:type :bytes
                      :path "git-log"
                      :bytes (:out p)})}])
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let result = capture_parser_diagnostics(
            || ClojureProjectCljParser::extract_packages(&path),
            "ClojureProjectCljParser",
            &path,
            None,
        );

        assert!(result.scan_diagnostics.is_empty());
        assert_eq!(result.packages.len(), 1);

        let package_data = &result.packages[0];
        assert_eq!(package_data.namespace.as_deref(), Some("org.example"));
        assert_eq!(package_data.name.as_deref(), Some("functions"));
        assert_eq!(package_data.version.as_deref(), Some("1.0.0"));
        assert_eq!(package_data.dependencies.len(), 1);
    }

    #[test]
    fn test_project_clj_accepts_var_quote_in_ignored_fields() {
        let content = r#"
(defproject org.example/var-quote "1.0.0"
  :dependencies [[org.clojure/clojure "1.12.0"]]
  :middleware [#'project/warn]
  :hooks [#'leiningen.test.helper/with-system-out-str])
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let result = capture_parser_diagnostics(
            || ClojureProjectCljParser::extract_packages(&path),
            "ClojureProjectCljParser",
            &path,
            None,
        );

        assert!(result.scan_diagnostics.is_empty());
        assert_eq!(result.packages.len(), 1);

        let package_data = &result.packages[0];
        assert_eq!(package_data.namespace.as_deref(), Some("org.example"));
        assert_eq!(package_data.name.as_deref(), Some("var-quote"));
        assert_eq!(package_data.version.as_deref(), Some("1.0.0"));
        assert_eq!(package_data.dependencies.len(), 1);
    }

    #[test]
    fn test_project_clj_tolerates_discard_before_closing_delimiter() {
        // A `#_` discard immediately before a closing bracket (as real manifests
        // such as aleph's `:jvm-opts` use) must not fail the whole-file parse.
        let content = r#"
(defproject org.example/discard "1.0.0"
  :dependencies [[org.clojure/clojure "1.12.0"]]
  :jvm-opts ^:replace ["-server"
                       "-Xmx2g"
                       #_"-XX:+PrintCompilation"
                       #_"-XX:+PrintInlining"]
  :global-vars {*warn-on-reflection* true})
        "#;

        let (_temp_dir, path) = create_temp_file("project.clj", content);
        let result = capture_parser_diagnostics(
            || ClojureProjectCljParser::extract_packages(&path),
            "ClojureProjectCljParser",
            &path,
            None,
        );

        assert!(result.scan_diagnostics.is_empty());
        assert_eq!(result.packages.len(), 1);

        let package_data = &result.packages[0];
        assert_eq!(package_data.namespace.as_deref(), Some("org.example"));
        assert_eq!(package_data.name.as_deref(), Some("discard"));
        assert_eq!(package_data.version.as_deref(), Some("1.0.0"));
        assert_eq!(package_data.dependencies.len(), 1);
    }

    #[test]
    fn test_deps_edn_splits_classifier_suffix_into_extra_data() {
        // tools.deps `artifact$classifier` syntax must yield a clean purl with
        // the classifier captured in extra_data, not mangled into the artifact.
        let content = r#"
{:deps {io.netty/netty-transport-native-epoll$linux-x86_64 {:mvn/version "4.1.135.Final"}}}
        "#;

        let (_temp_dir, path) = create_temp_file("deps.edn", content);
        let package_data = ClojureDepsEdnParser::extract_first_package(&path);

        assert_eq!(package_data.dependencies.len(), 1);
        let dep = &package_data.dependencies[0];
        assert_eq!(
            dep.purl.as_deref(),
            Some("pkg:maven/io.netty/netty-transport-native-epoll@4.1.135.Final")
        );
        assert_eq!(
            dep.extra_data
                .as_ref()
                .and_then(|data| data.get("classifier"))
                .and_then(|value| value.as_str()),
            Some("linux-x86_64")
        );
    }

    #[test]
    fn test_deps_edn_allows_commas_as_whitespace() {
        let content = r#"
{:paths ["src", "resources"],
 :deps {org.clojure/clojure {:mvn/version "1.12.0"},
        cheshire {:mvn/version "5.12.0"}}}
        "#;

        let (_temp_dir, path) = create_temp_file("deps.edn", content);
        let package_data = ClojureDepsEdnParser::extract_first_package(&path);

        assert_eq!(package_data.dependencies.len(), 2);
        assert!(
            package_data
                .dependencies
                .iter()
                .any(|dep| dep.purl.as_deref() == Some("pkg:maven/org.clojure/clojure@1.12.0"))
        );
        assert!(
            package_data
                .dependencies
                .iter()
                .any(|dep| dep.purl.as_deref() == Some("pkg:maven/cheshire/cheshire@5.12.0"))
        );
    }

    #[test]
    fn test_deps_edn_unsupported_dispatch_macro_falls_back_safely() {
        let content = r#"
{:deps {foo/bar {:mvn/version "1.0.0"}}
 :aliases {:test #=(eval {:extra-deps {baz/qux {:mvn/version "2.0.0"}}})}}
        "#;

        let (_temp_dir, path) = create_temp_file("deps.edn", content);
        let package_data = ClojureDepsEdnParser::extract_first_package(&path);

        assert_eq!(package_data.package_type, Some(PackageType::Maven));
        assert_eq!(
            package_data.datasource_id,
            Some(DatasourceId::ClojureDepsEdn)
        );
        assert!(package_data.dependencies.is_empty());
    }
}
