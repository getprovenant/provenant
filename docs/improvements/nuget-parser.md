# NuGet Parser Improvements

## Summary

Rust now goes beyond the released Python ScanCode NuGet support in six concrete ways:

1. parses additional NuGet and Visual Studio manifests (legacy `project.json`, `project.lock.json`, and PackageReference project files)
2. parses `.deps.json` runtime dependency graphs from built .NET outputs
3. preserves modern nuspec license hints (`license_type`, `license_file`) instead of collapsing everything to deprecated `licenseUrl` fallbacks
4. reads archive-backed license file contents from `.nupkg` files when the nuspec points at a packaged license file
5. parses NuGet Central Package Management files (`Directory.Packages.props`) and statically backfills nearest-ancestor central versions into versionless project dependencies, including composed property-backed `PackageVersion` expressions, bounded repository-local `.props` imports, and bounded literal `VersionOverride` support when explicitly enabled
6. adds bounded `Directory.Build.props` participation so CPM-relevant properties can flow into central versions and project overrides without full MSBuild evaluation

## Python Status

- Released ScanCode handles `.nuspec`, `.nupkg`, and `packages.lock.json`, but not legacy `project.json`, `project.lock.json`, PackageReference project files, standalone `Directory.Packages.props`, or `.deps.json` runtime graphs.
- Upstream enhancement issues explicitly ask for these extra manifests and modern nuspec/license improvements.
- Python also keeps NuGet party `type` empty and does not extract packaged license file contents from `.nupkg` archives.

## Rust Improvements

### Extra manifest support

- Legacy `project.json` manifests now extract package metadata plus direct and framework-specific dependencies.
- `project.lock.json` now extracts dependency groups from `projectFileDependencyGroups`.
- PackageReference `.csproj`, `.vbproj`, and `.fsproj` files now extract package metadata and `<PackageReference>` dependencies.
- `Directory.Packages.props` now extracts central `PackageVersion` declarations as dependency metadata, including `Condition`, central-package-management feature flags, and composed property-backed version expressions such as `$(VersionPrefix)-$(VersionSuffix)` when the bounded property chain is statically known.
- `Directory.Build.props` now extracts bounded literal property maps and bounded parent-import metadata relevant to CPM.
- Assembly now backfills versionless PackageReference dependencies from the nearest ancestor `Directory.Packages.props`, can merge bounded explicit parent `Directory.Packages.props` imports, can consume bounded `Directory.Build.props` property maps, can ingest bounded repository-local `.props` imports addressed directly or through `$(MSBuildThisFileDirectory)`-anchored paths, and can prefer literal project-file `VersionOverride` values when CPM overrides are statically enabled.
- `.deps.json` now extracts runtime-target-aware resolved dependency graphs from built .NET outputs.

### Static CPM backfill with bounded parent imports and build-props participation

Rust now uses `Directory.Packages.props` in two truthful ways: it still parses the file as standalone NuGet metadata, and it now applies its central versions to versionless PackageReference dependencies during assembly when a narrow static match is available.

This CPM slice intentionally:

- extracts `<PackageVersion Include="..." Version="..." />` and `<PackageVersion Update="..." Version="..." />`
- preserves `Condition` metadata on central package-version entries
- preserves central flags such as `ManagePackageVersionsCentrally`, `CentralPackageTransitivePinningEnabled`, and `CentralPackageVersionOverrideEnabled`
- backfills versionless `PackageReference` entries from the nearest ancestor `Directory.Packages.props` when central package management is enabled and there is exactly one matching central entry
- preserves literal `VersionOverride` metadata on `PackageReference` entries in project files
- supports the documented explicit parent-import pattern for `Directory.Packages.props`, including bounded static handling of `GetPathOfFileAbove(Directory.Packages.props, $(MSBuildThisFileDirectory)..)`
- supports bounded repository-local `.props` imports reachable from `Directory.Packages.props` / `Directory.Build.props`, including `$(MSBuildThisFileDirectory)`-anchored paths, when the resolved file stays under the same scan root
- supports bounded nearest-ancestor and bounded parent-import participation for `Directory.Build.props`
- resolves literal and composed property-backed `PackageVersion` expressions such as `$(SomeVersion)` and `$(VersionPrefix)-$(VersionSuffix)` when the bounded import chain statically proves the needed properties
- resolves literal project-file `VersionOverride="$(SomeProperty)"` values when that property is statically known in the project file
- prefers a literal `PackageReference VersionOverride` over the nearest central package version only when CPM overrides are statically enabled and exactly one matching central package entry exists
- prefers explicit project-file versions over central backfill
- treats conditioned central versions as applicable only when the project dependency carries the exact same raw condition string
- does **not** evaluate full MSBuild semantics
- does **not** participate in `Directory.Build.targets` or broader non-CPM import graphs
- does **not** evaluate wildcard or non-literal import targets
- does **not** implement broader MSBuild condition/property resolution

That keeps the implementation truthful: it improves real CPM dependency recovery without pretending that full MSBuild-driven central-version resolution already works.

### Modern nuspec metadata

- NuGet author/owner parties now record `type = person`.
- Nuspec `<license type="expression">...` and `<license type="file">...` now preserve their modern type hints in `extra_data`.
- File-based nuspec licenses keep the real file reference instead of falling back to deprecated `licenseUrl` placeholders.
- Repository `branch` and `commit` attributes are preserved in `extra_data` when present.

### Archive-backed license extraction

- When a `.nupkg` nuspec declares `<license type="file">LICENSE.txt</license>`, Rust now reads that packaged file and stores its contents as the extracted license statement.
- This gives downstream license analysis a real license text source instead of only the placeholder filename.

## Coverage

Coverage spans modern and legacy NuGet manifests, CPM backfill and import bounds, build-props participation, version overrides, archive-backed license extraction, and assembly behavior for the supported inputs.
