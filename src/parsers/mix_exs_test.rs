// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use crate::models::{DatasourceId, Dependency, PackageType};

use super::PackageParser;
use super::mix_exs::MixExsParser;

fn dep<'a>(deps: &'a [Dependency], purl: &str) -> &'a Dependency {
    deps.iter()
        .find(|d| d.purl.as_deref() == Some(purl))
        .unwrap_or_else(|| panic!("expected dependency {purl}, found: {:?}", purls(deps)))
}

fn purls(deps: &[Dependency]) -> Vec<Option<String>> {
    deps.iter().map(|d| d.purl.clone()).collect()
}

#[test]
fn test_mix_exs_is_match() {
    assert!(MixExsParser::is_match(&PathBuf::from("/tmp/mix.exs")));
    assert!(!MixExsParser::is_match(&PathBuf::from("/tmp/mix.lock")));
    assert!(!MixExsParser::is_match(&PathBuf::from("/tmp/mix.ex")));
}

#[test]
fn test_parse_mix_exs_basic_identity() {
    let path = PathBuf::from("testdata/hex/basic/mix.exs");
    let package_data = MixExsParser::extract_first_package(&path);

    assert_eq!(package_data.package_type, Some(PackageType::Hex));
    assert_eq!(package_data.primary_language.as_deref(), Some("Elixir"));
    assert_eq!(package_data.datasource_id, Some(DatasourceId::HexMixExs));
    // version resolved through `version: @version` → `@version "1.2.3"`.
    assert_eq!(package_data.name.as_deref(), Some("my_app"));
    assert_eq!(package_data.version.as_deref(), Some("1.2.3"));
    assert_eq!(package_data.purl.as_deref(), Some("pkg:hex/my_app@1.2.3"));
}

#[test]
fn test_parse_mix_exs_dependencies_and_scopes() {
    let path = PathBuf::from("testdata/hex/basic/mix.exs");
    let deps = MixExsParser::extract_first_package(&path).dependencies;

    // The dynamic `System.get_env(...)` dep contributes name-only (no version);
    // all literal deps are present.
    let phoenix = dep(&deps, "pkg:hex/phoenix");
    assert_eq!(phoenix.extracted_requirement.as_deref(), Some("~> 1.7.0"));
    assert_eq!(phoenix.is_direct, Some(true));
    assert_eq!(phoenix.is_optional, None);
    assert_eq!(phoenix.scope, None);

    let ecto = dep(&deps, "pkg:hex/ecto");
    assert_eq!(ecto.extracted_requirement.as_deref(), Some(">= 3.0.0"));
    assert_eq!(ecto.scope.as_deref(), Some("test"));

    let jason = dep(&deps, "pkg:hex/jason");
    assert_eq!(jason.extracted_requirement.as_deref(), Some("~> 1.4"));
    assert_eq!(jason.is_optional, Some(true));

    let credo = dep(&deps, "pkg:hex/credo");
    assert_eq!(credo.extracted_requirement.as_deref(), Some("~> 1.7"));
    assert_eq!(credo.scope.as_deref(), Some("dev,test"));

    // `github:` dependency has no string-literal version requirement.
    let plug_cowboy = dep(&deps, "pkg:hex/plug_cowboy");
    assert_eq!(plug_cowboy.extracted_requirement, None);
    assert_eq!(plug_cowboy.is_direct, Some(true));

    // Dynamic version expression is skipped (name-only, no requirement).
    let dynamic = dep(&deps, "pkg:hex/dynamic_dep");
    assert_eq!(dynamic.extracted_requirement, None);

    assert_eq!(deps.len(), 6);
}

#[test]
fn test_parse_mix_exs_literal_version_string() {
    let content = r#"
defmodule App.MixProject do
  use Mix.Project

  def project do
    [app: :app, version: "0.9.0", deps: deps()]
  end

  defp deps do
    [{:foo, "~> 2.0"}]
  end
end
"#;
    let package = super::mix_exs::parse_mix_exs_for_test(content);
    assert_eq!(package.name.as_deref(), Some("app"));
    assert_eq!(package.version.as_deref(), Some("0.9.0"));
    assert_eq!(package.purl.as_deref(), Some("pkg:hex/app@0.9.0"));
    assert_eq!(package.dependencies.len(), 1);
}

#[test]
fn test_parse_mix_exs_dynamic_version_skipped() {
    // An interpolated/dynamic version must not be guessed.
    let content = r##"
defmodule App.MixProject do
  use Mix.Project

  def project do
    [app: :app, version: "#{@base}.0", deps: []]
  end
end
"##;
    let package = super::mix_exs::parse_mix_exs_for_test(content);
    assert_eq!(package.name.as_deref(), Some("app"));
    assert_eq!(package.version, None);
    assert_eq!(package.purl.as_deref(), Some("pkg:hex/app"));
}

#[test]
fn test_parse_mix_exs_computed_string_prefix_is_skipped() {
    // A string literal that is only the prefix of a computed expression must be
    // rejected, not emitted as a partial version/requirement.
    let content = r##"
defmodule App.MixProject do
  use Mix.Project

  @suffix "0"

  def project do
    [app: :app, version: "1." <> @suffix, deps: deps()]
  end

  defp deps do
    [
      {:phoenix, "~> " <> phoenix_version()},
      {:plug, "~> 1.15"}
    ]
  end
end
"##;
    let package = super::mix_exs::parse_mix_exs_for_test(content);
    assert_eq!(package.name.as_deref(), Some("app"));
    // `"1." <> @suffix` is computed → no version guessed.
    assert_eq!(package.version, None);

    let phoenix = package
        .dependencies
        .iter()
        .find(|d| d.purl.as_deref() == Some("pkg:hex/phoenix"))
        .expect("phoenix dep should be present (name-only)");
    // The computed requirement prefix `"~> "` must not leak.
    assert_eq!(phoenix.extracted_requirement, None);
    let plug = package
        .dependencies
        .iter()
        .find(|d| d.purl.as_deref() == Some("pkg:hex/plug"))
        .expect("plug dep should be present");
    assert_eq!(plug.extracted_requirement.as_deref(), Some("~> 1.15"));
}

#[test]
fn test_parse_mix_exs_missing_project_is_empty_but_typed() {
    let content = "defmodule App do\n  def hello, do: :world\nend\n";
    let package = super::mix_exs::parse_mix_exs_for_test(content);
    assert_eq!(package.datasource_id, Some(DatasourceId::HexMixExs));
    assert_eq!(package.name, None);
    assert_eq!(package.version, None);
    assert!(package.purl.is_none());
    assert!(package.dependencies.is_empty());
}

#[test]
fn test_parse_mix_exs_umbrella_root_apps_path_and_apps() {
    let content = r#"
defmodule MyUmbrella.MixProject do
  use Mix.Project

  def project do
    [
      apps_path: "apps",
      apps: [:kv, :kv_server],
      version: "0.1.0",
      deps: deps()
    ]
  end

  defp deps do
    [{:meck, "~> 1.0", only: :test}]
  end
end
"#;
    let package = super::mix_exs::parse_mix_exs_for_test(content);
    // An umbrella root commonly has no `app:` field, so no purl is formed for it.
    assert_eq!(package.name, None);
    let extra_data = package.extra_data.expect("apps_path marker expected");
    assert_eq!(
        extra_data.get("apps_path").and_then(|v| v.as_str()),
        Some("apps")
    );
    let apps = extra_data
        .get("apps")
        .and_then(|v| v.as_array())
        .expect("apps allow-list expected");
    let apps: Vec<&str> = apps.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(apps, vec!["kv", "kv_server"]);
}

#[test]
fn test_parse_mix_exs_umbrella_root_without_apps_filter() {
    let content = r#"
defmodule MyUmbrella.MixProject do
  use Mix.Project

  def project do
    [apps_path: "apps", version: "0.1.0", deps: []]
  end
end
"#;
    let package = super::mix_exs::parse_mix_exs_for_test(content);
    let extra_data = package.extra_data.expect("apps_path marker expected");
    assert_eq!(
        extra_data.get("apps_path").and_then(|v| v.as_str()),
        Some("apps")
    );
    assert!(!extra_data.contains_key("apps"));
}

#[test]
fn test_parse_mix_exs_non_umbrella_manifest_has_no_apps_path_marker() {
    let path = PathBuf::from("testdata/hex/basic/mix.exs");
    let package_data = MixExsParser::extract_first_package(&path);
    assert!(package_data.extra_data.is_none());
}

#[test]
fn test_parse_mix_exs_in_umbrella_dependency_has_no_hex_purl() {
    let content = r#"
defmodule KVServer.MixProject do
  use Mix.Project

  def project do
    [app: :kv_server, version: "0.1.0", deps: deps()]
  end

  defp deps do
    [
      {:kv, in_umbrella: true},
      {:phoenix, "~> 1.7.0"}
    ]
  end
end
"#;
    let package = super::mix_exs::parse_mix_exs_for_test(content);
    let kv = package
        .dependencies
        .iter()
        .find(|d| {
            d.extra_data
                .as_ref()
                .and_then(|extra| extra.get("app"))
                .and_then(|v| v.as_str())
                == Some("kv")
        })
        .expect("in_umbrella dep should be present");
    // No fabricated unversioned `pkg:hex/kv` purl for an internal sibling app.
    assert_eq!(kv.purl, None);
    assert_eq!(kv.extracted_requirement, None);
    assert_eq!(
        kv.extra_data
            .as_ref()
            .and_then(|extra| extra.get("in_umbrella"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );

    // A regular Hex dependency alongside it is unaffected.
    let phoenix = dep(&package.dependencies, "pkg:hex/phoenix");
    assert_eq!(phoenix.extracted_requirement.as_deref(), Some("~> 1.7.0"));
    assert!(
        phoenix
            .extra_data
            .as_ref()
            .and_then(|extra| extra.get("in_umbrella"))
            .is_none()
    );
}

#[test]
fn test_parse_mix_exs_def_deps_supported() {
    let content = r#"
defmodule App.MixProject do
  use Mix.Project

  def project do
    [app: :app, version: "1.0.0", deps: deps()]
  end

  # public def deps (not defp) is also supported
  def deps do
    [{:bar, "1.0"}]
  end
end
"#;
    let package = super::mix_exs::parse_mix_exs_for_test(content);
    assert_eq!(package.dependencies.len(), 1);
    assert_eq!(package.dependencies[0].purl.as_deref(), Some("pkg:hex/bar"));
    assert_eq!(
        package.dependencies[0].extracted_requirement.as_deref(),
        Some("1.0")
    );
}

fn deps_of(deps_body: &str) -> Vec<Dependency> {
    let content = format!(
        r#"
defmodule Demo.MixProject do
  use Mix.Project

  def project do
    [app: :demo, version: "0.1.0", deps: deps()]
  end

  defp deps do
{deps_body}
  end

  defp aliases do
    [setup: ["deps.get"]]
  end
end
"#
    );
    super::mix_exs::parse_mix_exs_for_test(&content).dependencies
}

#[test]
fn test_parse_mix_exs_deps_after_leading_statements() {
    let deps = deps_of(
        r#"    IO.puts("Resolving deps: see https://example.com")
    Logger.warning("Missing: thing. Read more: https://docs.example.com/x")
    [{:jason, "~> 1.4"}]"#,
    );
    assert_eq!(purls(&deps), vec![Some("pkg:hex/jason".to_string())]);
    assert_eq!(deps[0].extracted_requirement.as_deref(), Some("~> 1.4"));
}

#[test]
fn test_parse_mix_exs_deps_after_leading_comments_and_semicolons() {
    let deps = deps_of(
        r#"    # Pinned for compatibility; see "upgrade notes".
    _ = :ok; :ok
    [
      # the JSON library
      {:jason, "~> 1.4"},
      {:plug, "~> 1.15", only: :test}
    ]"#,
    );
    assert_eq!(
        purls(&deps),
        vec![
            Some("pkg:hex/jason".to_string()),
            Some("pkg:hex/plug".to_string())
        ]
    );
    assert_eq!(dep(&deps, "pkg:hex/plug").scope.as_deref(), Some("test"));
}

#[test]
fn test_parse_mix_exs_deps_bound_variable_returned() {
    let deps = deps_of(
        r#"    deps = [
      {:jason, "~> 1.4"},
      {:ecto, ">= 3.0.0"}
    ]

    deps"#,
    );
    assert_eq!(
        purls(&deps),
        vec![
            Some("pkg:hex/jason".to_string()),
            Some("pkg:hex/ecto".to_string())
        ]
    );
}

#[test]
fn test_parse_mix_exs_deps_bound_variable_in_conditional_return() {
    // Only deps returned by every branch are emitted; the dev-only extra is skipped.
    let deps = deps_of(
        r#"    deps = [{:jason, "~> 1.4"}]
    if Mix.env() == :dev, do: deps ++ [{:credo, "~> 1.7"}], else: deps"#,
    );
    assert_eq!(purls(&deps), vec![Some("pkg:hex/jason".to_string())]);

    let deps = deps_of(
        r#"    deps = [{:jason, "~> 1.4"}]

    if Mix.env() == :dev do
      deps ++ [{:credo, "~> 1.7"}]
    else
      deps
    end"#,
    );
    assert_eq!(purls(&deps), vec![Some("pkg:hex/jason".to_string())]);
}

#[test]
fn test_parse_mix_exs_deps_leading_list_with_concat_continuation() {
    let deps = deps_of(
        r#"    [{:jason, "~> 1.4"}]
    ++ optional_deps()"#,
    );
    assert_eq!(purls(&deps), vec![Some("pkg:hex/jason".to_string())]);
}

#[test]
fn test_parse_mix_exs_deps_ignore_tuples_in_leading_statements() {
    let deps = deps_of(
        r#"    IO.puts("Note: {:fake, \"~> 1.0\"} deprecated")
    IO.inspect([{:also_fake, "1.0"}])
    unused = [{:unused, "1.0"}]
    [{:jason, "~> 1.4"}]"#,
    );
    assert_eq!(purls(&deps), vec![Some("pkg:hex/jason".to_string())]);
}

#[test]
fn test_parse_mix_exs_deps_reassignment_to_non_literal_invalidates_binding() {
    let deps = deps_of(
        r#"    deps = [{:old, "1.0"}]
    deps = load_deps()
    deps"#,
    );
    assert!(deps.is_empty(), "unexpected deps: {:?}", purls(&deps));
}

#[test]
fn test_parse_mix_exs_deps_reassignment_with_concat_is_traced() {
    let deps = deps_of(
        r#"    deps = [{:jason, "~> 1.4"}]
    deps = deps ++ [{:ecto, ">= 3.0.0"}]
    deps"#,
    );
    assert_eq!(
        purls(&deps),
        vec![
            Some("pkg:hex/jason".to_string()),
            Some("pkg:hex/ecto".to_string())
        ]
    );
}

#[test]
fn test_parse_mix_exs_deps_wrapped_or_condition_only_variable_is_unknown() {
    for tail in [
        "Enum.reject(deps, fn _ -> true end)",
        "deps |> Enum.reject(&(elem(&1, 0) == :jason))",
        "deps -- [{:jason, \"1.4\"}]",
        "if length(deps) > 0, do: fetch_deps(), else: []",
        "if deps == [] do\n      fetch_deps()\n    else\n      other_deps()\n    end",
    ] {
        let deps = deps_of(&format!("    deps = [{{:jason, \"1.4\"}}]\n    {tail}"));
        assert!(
            deps.is_empty(),
            "{tail}: unexpected deps {:?}",
            purls(&deps)
        );
    }
}

#[test]
fn test_parse_mix_exs_deps_branch_intersection_ignores_tuple_formatting() {
    let deps = deps_of(
        r#"    if Mix.env() == :dev do
      [{:jason, "~> 1.4"}, {:credo, "~> 1.7"}, {:ecto, "~> 3.0"}]
    else
      [{:jason,"~> 1.4"}, {:ecto, "~> 3.1"}]
    end"#,
    );
    assert_eq!(purls(&deps), vec![Some("pkg:hex/jason".to_string())]);
}

#[test]
fn test_parse_mix_exs_deps_self_concatenation_is_bounded() {
    let mut body = String::from("    deps = [{:a, \"1.0\"}, {:b, \"1.0\"}]\n");
    for _ in 0..64 {
        body.push_str("    deps = deps ++ deps\n");
    }
    body.push_str("    deps");

    let started = std::time::Instant::now();
    let deps = deps_of(&body);
    assert_eq!(deps.len(), super::mix_exs::MAX_DEPENDENCIES);
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

#[test]
fn test_parse_mix_exs_deps_unresolvable_return_is_empty() {
    let deps = deps_of(
        r#"    IO.puts("{:fake, \"1.0\"}")
    base_deps() ++ extra_deps()"#,
    );
    assert!(deps.is_empty(), "unexpected deps: {:?}", purls(&deps));
}
