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
