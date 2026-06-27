// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use super::super::scan_test_utils::{assert_dependency_present, scan_and_assemble};
    use crate::models::DatasourceId;

    /// A directory holding `mix.exs` + `mix.lock` must assemble into exactly one
    /// `pkg:hex/<app>` package that owns both the `mix.exs` direct deps and the
    /// `mix.lock` locked deps (`for_package_uid` set, not null). This is the
    /// contract that eliminates the previously orphaned `mix.lock` dependencies.
    #[test]
    fn test_mix_exs_and_lock_assemble_into_one_owned_package() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::write(
            temp_dir.path().join("mix.exs"),
            r#"defmodule MyApp.MixProject do
  use Mix.Project

  @version "1.2.3"

  def project do
    [app: :my_app, version: @version, deps: deps()]
  end

  defp deps do
    [
      {:phoenix, "~> 1.7.0"},
      {:ecto, ">= 3.0.0", only: :test}
    ]
  end
end
"#,
        )
        .expect("write mix.exs");
        std::fs::write(
            temp_dir.path().join("mix.lock"),
            "%{\n  \"phoenix\": {:hex, :phoenix, \"1.7.10\", \"abc\", [:mix], [], \"hexpm\", \"def\"},\n  \"ecto\": {:hex, :ecto, \"3.10.0\", \"ghi\", [:mix], [], \"hexpm\", \"jkl\"}\n}\n",
        )
        .expect("write mix.lock");

        let (files, result) = scan_and_assemble(temp_dir.path());

        // Exactly one package, formed from the mix.exs identity.
        assert_eq!(result.packages.len(), 1, "expected one assembled package");
        let package = &result.packages[0];
        assert_eq!(package.purl.as_deref(), Some("pkg:hex/my_app@1.2.3"));

        // Both datafiles belong to the package.
        assert!(
            package
                .datafile_paths
                .iter()
                .any(|p| p.ends_with("mix.exs"))
        );
        assert!(
            package
                .datafile_paths
                .iter()
                .any(|p| p.ends_with("mix.lock"))
        );

        // The mix.exs direct deps and the mix.lock locked deps are all owned by
        // the package — none orphaned.
        assert!(!result.dependencies.is_empty());
        assert!(
            result
                .dependencies
                .iter()
                .all(|d| d.for_package_uid.as_ref() == Some(&package.package_uid)),
            "all dependencies must be owned by the package, found orphans: {:?}",
            result
                .dependencies
                .iter()
                .map(|d| (d.purl.clone(), d.for_package_uid.clone()))
                .collect::<Vec<_>>()
        );

        assert_dependency_present(&result.dependencies, "pkg:hex/phoenix", "mix.exs");
        assert_dependency_present(&result.dependencies, "pkg:hex/ecto", "mix.exs");
        assert_dependency_present(&result.dependencies, "pkg:hex/phoenix@1.7.10", "mix.lock");
        assert_dependency_present(&result.dependencies, "pkg:hex/ecto@3.10.0", "mix.lock");

        // Both files link back to the assembled package.
        for suffix in ["mix.exs", "mix.lock"] {
            let file = files
                .iter()
                .find(|f| f.path.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} should be scanned"));
            assert!(
                file.for_packages
                    .iter()
                    .any(|uid| uid == &package.package_uid),
                "{suffix} should link to the package"
            );
        }
    }

    /// A standalone `mix.lock` with no sibling `mix.exs` yields no package; its
    /// deps stay hoisted (orphaned), which is the accepted fallback.
    #[test]
    fn test_standalone_mix_lock_yields_no_package() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::write(
            temp_dir.path().join("mix.lock"),
            "%{\n  \"phoenix\": {:hex, :phoenix, \"1.7.10\", \"abc\", [:mix], [], \"hexpm\", \"def\"}\n}\n",
        )
        .expect("write mix.lock");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        assert!(result.packages.is_empty());
        assert!(
            result
                .dependencies
                .iter()
                .all(|d| d.for_package_uid.is_none())
        );
        assert_dependency_present(&result.dependencies, "pkg:hex/phoenix@1.7.10", "mix.lock");
    }

    /// A standalone `mix.exs` (no lockfile) still promotes to a package owning
    /// its direct deps.
    #[test]
    fn test_standalone_mix_exs_promotes_to_package() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::write(
            temp_dir.path().join("mix.exs"),
            r#"defmodule App.MixProject do
  use Mix.Project

  def project do
    [app: :solo, version: "0.1.0", deps: deps()]
  end

  defp deps do
    [{:jason, "~> 1.4"}]
  end
end
"#,
        )
        .expect("write mix.exs");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        let package = result
            .packages
            .iter()
            .find(|p| p.purl.as_deref() == Some("pkg:hex/solo@0.1.0"))
            .expect("mix.exs should promote to a package");
        assert_eq!(package.datasource_ids, vec![DatasourceId::HexMixExs]);

        let dep = result
            .dependencies
            .iter()
            .find(|d| d.purl.as_deref() == Some("pkg:hex/jason"))
            .expect("jason dep should be present");
        assert_eq!(dep.for_package_uid.as_ref(), Some(&package.package_uid));
    }
}
