// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputDatasourceId {
    AboutFile,
    Readme,
    EtcOsRelease,

    AlpineApkArchive,
    AlpineApkbuild,
    AlpineInstalledDb,

    ArchAurinfo,
    ArchPkginfo,
    ArchSrcinfo,

    AndroidAab,
    AndroidAarLibrary,
    AndroidApk,
    AndroidManifestXml,
    AndroidSoongMetadata,

    Axis2Mar,
    Axis2ModuleXml,

    AutotoolsConfigure,

    BazelBuild,
    BazelModule,

    BitbakeRecipe,
    BitbakeRecipeAppend,

    BowerJson,

    BuckFile,
    BuckMetadata,

    BunLock,
    BunLockb,

    CarthageCartfile,
    CarthageCartfileResolved,

    CargoLock,
    CargoToml,
    RustBinary,
    WindowsExecutable,

    ChefCookbookMetadataJson,
    ChefCookbookMetadataRb,

    CitationCff,

    CocoapodsPodfile,
    CocoapodsPodfileLock,
    CocoapodsPodspec,
    CocoapodsPodspecJson,

    #[serde(rename = "conan_conandata_yml")]
    ConanConanDataYml,
    #[serde(rename = "conan_conanfile_py")]
    ConanConanFilePy,
    #[serde(rename = "conan_conanfile_txt")]
    ConanConanFileTxt,
    ConanLock,

    CondaYaml,
    CondaMetaJson,
    CondaMetaYaml,

    ClojureDepsEdn,
    ClojureProjectClj,

    CpanDistIni,
    CpanMakefile,
    CpanManifest,
    CpanMetaJson,
    CpanMetaYml,

    CranDescription,

    PubspecLock,
    PubspecYaml,

    DebianControlExtractedDeb,
    DebianControlInSource,
    DebianCopyright,
    DebianCopyrightInSource,
    DebianCopyrightInPackage,
    DebianCopyrightStandalone,
    DebianDeb,
    DebianSourceMetadataTarball,
    DebianDistrolessInstalledDb,
    DebianInstalledFilesList,
    #[serde(rename = "debian_installed_md5sums")]
    DebianInstalledMd5Sums,
    DebianInstalledStatusDb,
    #[serde(rename = "debian_md5sums_in_extracted_deb")]
    DebianMd5SumsInExtractedDeb,
    DebianOriginalSourceTarball,
    DebianSourceControlDsc,

    DenoJson,
    DenoLock,

    Dockerfile,

    ErlangOtpAppSrc,
    RebarConfig,
    RebarLock,

    FreebsdCompactManifest,

    Godeps,
    GoBinary,
    GoMod,
    GoModGraph,
    GoSum,
    GoWork,

    HackageCabal,
    HackageCabalProject,
    HackageStackYaml,

    BuildGradle,
    GradleLockfile,
    GradleModule,

    HaxelibJson,

    HelmChartLock,
    HelmChartYaml,

    HexMixLock,

    JuliaProjectToml,
    JuliaManifestToml,

    AntIvyXml,
    JavaEarApplicationXml,
    JavaEarArchive,
    JavaJar,
    JavaJarManifest,
    JavaOsgiManifest,
    JavaWarArchive,
    JavaWarWebXml,
    JbossSar,
    JbossServiceXml,

    MavenPom,
    MavenPomProperties,
    MesonBuild,

    SbtBuildSbt,

    MicrosoftCabinet,
    MicrosoftUpdateManifestMum,

    AppleDmg,
    ChromeCrx,
    IosIpa,
    MozillaXpi,

    MeteorPackage,

    NixDefaultNix,
    NixFlakeLock,
    NixFlakeNix,

    NpmPackageJson,
    NpmPackageLockJson,

    NugetCsproj,
    NugetDepsJson,
    NugetDirectoryBuildProps,
    NugetDirectoryPackagesProps,
    NugetNupkg,
    NugetProjectJson,
    NugetProjectLockJson,
    NugetPackagesConfig,
    NugetPackagesLock,
    #[serde(alias = "nuget_nupsec")]
    NugetNuspec,
    NugetVbproj,
    NugetFsproj,

    OpamFile,

    PhpComposerJson,
    PhpComposerLock,

    PnpmLockYaml,
    PnpmWorkspaceYaml,

    Pipfile,
    PipfileLock,
    PipRequirements,
    PixiLock,
    PixiToml,
    PypiPipOriginJson,
    PypiEgg,
    PypiEggPkginfo,
    PypiEditableEggPkginfo,
    PypiInspectDeplock,
    PypiJson,
    PypiPoetryLock,
    PypiPoetryPyprojectToml,
    PypiSdist,
    PypiPylockToml,
    PypiPyprojectToml,
    PypiSdistPkginfo,
    PypiSetupCfg,
    PypiSetupPy,
    PypiUvLock,
    PypiWheel,
    PypiWheelMetadata,

    RpmArchive,
    RpmInstalledDatabaseBdb,
    RpmInstalledDatabaseNdb,
    RpmInstalledDatabaseSqlite,
    RpmMarinerManifest,
    RpmPackageLicenses,
    #[serde(alias = "rpm_spefile")]
    RpmSpecfile,
    RpmYumdb,

    Gemfile,
    GemfileExtracted,
    GemfileLock,
    GemfileLockExtracted,
    GemArchive,
    GemArchiveExtracted,
    Gemspec,
    GemspecExtracted,
    GemGemspecInstalledSpecifications,

    InstallshieldInstaller,
    IsoDiskImage,
    NsisInstaller,
    SharShellArchive,
    SquashfsDiskImage,

    SwiftPackageManifestJson,
    SwiftPackageResolved,
    SwiftPackageShowDependencies,

    PubliccodeYaml,

    VcpkgJson,

    YarnLock,
    YarnLockV1,
    YarnLockV2,
    YarnPnpCjs,

    Gitmodules,
}

impl From<crate::models::DatasourceId> for OutputDatasourceId {
    fn from(value: crate::models::DatasourceId) -> Self {
        match value {
            crate::models::DatasourceId::AboutFile => OutputDatasourceId::AboutFile,
            crate::models::DatasourceId::Readme => OutputDatasourceId::Readme,
            crate::models::DatasourceId::EtcOsRelease => OutputDatasourceId::EtcOsRelease,

            crate::models::DatasourceId::AlpineApkArchive => OutputDatasourceId::AlpineApkArchive,
            crate::models::DatasourceId::AlpineApkbuild => OutputDatasourceId::AlpineApkbuild,
            crate::models::DatasourceId::AlpineInstalledDb => OutputDatasourceId::AlpineInstalledDb,

            crate::models::DatasourceId::ArchAurinfo => OutputDatasourceId::ArchAurinfo,
            crate::models::DatasourceId::ArchPkginfo => OutputDatasourceId::ArchPkginfo,
            crate::models::DatasourceId::ArchSrcinfo => OutputDatasourceId::ArchSrcinfo,

            crate::models::DatasourceId::AndroidAab => OutputDatasourceId::AndroidAab,
            crate::models::DatasourceId::AndroidAarLibrary => OutputDatasourceId::AndroidAarLibrary,
            crate::models::DatasourceId::AndroidApk => OutputDatasourceId::AndroidApk,
            crate::models::DatasourceId::AndroidManifestXml => {
                OutputDatasourceId::AndroidManifestXml
            }
            crate::models::DatasourceId::AndroidSoongMetadata => {
                OutputDatasourceId::AndroidSoongMetadata
            }

            crate::models::DatasourceId::Axis2Mar => OutputDatasourceId::Axis2Mar,
            crate::models::DatasourceId::Axis2ModuleXml => OutputDatasourceId::Axis2ModuleXml,

            crate::models::DatasourceId::AutotoolsConfigure => {
                OutputDatasourceId::AutotoolsConfigure
            }

            crate::models::DatasourceId::BazelBuild => OutputDatasourceId::BazelBuild,
            crate::models::DatasourceId::BazelModule => OutputDatasourceId::BazelModule,

            crate::models::DatasourceId::BitbakeRecipe => OutputDatasourceId::BitbakeRecipe,
            crate::models::DatasourceId::BitbakeRecipeAppend => {
                OutputDatasourceId::BitbakeRecipeAppend
            }

            crate::models::DatasourceId::BowerJson => OutputDatasourceId::BowerJson,

            crate::models::DatasourceId::BuckFile => OutputDatasourceId::BuckFile,
            crate::models::DatasourceId::BuckMetadata => OutputDatasourceId::BuckMetadata,

            crate::models::DatasourceId::BunLock => OutputDatasourceId::BunLock,
            crate::models::DatasourceId::BunLockb => OutputDatasourceId::BunLockb,

            crate::models::DatasourceId::CarthageCartfile => OutputDatasourceId::CarthageCartfile,
            crate::models::DatasourceId::CarthageCartfileResolved => {
                OutputDatasourceId::CarthageCartfileResolved
            }

            crate::models::DatasourceId::CargoLock => OutputDatasourceId::CargoLock,
            crate::models::DatasourceId::CargoToml => OutputDatasourceId::CargoToml,
            crate::models::DatasourceId::RustBinary => OutputDatasourceId::RustBinary,
            crate::models::DatasourceId::WindowsExecutable => OutputDatasourceId::WindowsExecutable,

            crate::models::DatasourceId::ChefCookbookMetadataJson => {
                OutputDatasourceId::ChefCookbookMetadataJson
            }
            crate::models::DatasourceId::ChefCookbookMetadataRb => {
                OutputDatasourceId::ChefCookbookMetadataRb
            }

            crate::models::DatasourceId::CitationCff => OutputDatasourceId::CitationCff,

            crate::models::DatasourceId::CocoapodsPodfile => OutputDatasourceId::CocoapodsPodfile,
            crate::models::DatasourceId::CocoapodsPodfileLock => {
                OutputDatasourceId::CocoapodsPodfileLock
            }
            crate::models::DatasourceId::CocoapodsPodspec => OutputDatasourceId::CocoapodsPodspec,
            crate::models::DatasourceId::CocoapodsPodspecJson => {
                OutputDatasourceId::CocoapodsPodspecJson
            }

            crate::models::DatasourceId::ConanConanDataYml => OutputDatasourceId::ConanConanDataYml,
            crate::models::DatasourceId::ConanConanFilePy => OutputDatasourceId::ConanConanFilePy,
            crate::models::DatasourceId::ConanConanFileTxt => OutputDatasourceId::ConanConanFileTxt,
            crate::models::DatasourceId::ConanLock => OutputDatasourceId::ConanLock,

            crate::models::DatasourceId::CondaYaml => OutputDatasourceId::CondaYaml,
            crate::models::DatasourceId::CondaMetaJson => OutputDatasourceId::CondaMetaJson,
            crate::models::DatasourceId::CondaMetaYaml => OutputDatasourceId::CondaMetaYaml,

            crate::models::DatasourceId::ClojureDepsEdn => OutputDatasourceId::ClojureDepsEdn,
            crate::models::DatasourceId::ClojureProjectClj => OutputDatasourceId::ClojureProjectClj,

            crate::models::DatasourceId::CpanDistIni => OutputDatasourceId::CpanDistIni,
            crate::models::DatasourceId::CpanMakefile => OutputDatasourceId::CpanMakefile,
            crate::models::DatasourceId::CpanManifest => OutputDatasourceId::CpanManifest,
            crate::models::DatasourceId::CpanMetaJson => OutputDatasourceId::CpanMetaJson,
            crate::models::DatasourceId::CpanMetaYml => OutputDatasourceId::CpanMetaYml,

            crate::models::DatasourceId::CranDescription => OutputDatasourceId::CranDescription,

            crate::models::DatasourceId::PubspecLock => OutputDatasourceId::PubspecLock,
            crate::models::DatasourceId::PubspecYaml => OutputDatasourceId::PubspecYaml,

            crate::models::DatasourceId::DebianControlExtractedDeb => {
                OutputDatasourceId::DebianControlExtractedDeb
            }
            crate::models::DatasourceId::DebianControlInSource => {
                OutputDatasourceId::DebianControlInSource
            }
            crate::models::DatasourceId::DebianCopyright => OutputDatasourceId::DebianCopyright,
            crate::models::DatasourceId::DebianCopyrightInSource => {
                OutputDatasourceId::DebianCopyrightInSource
            }
            crate::models::DatasourceId::DebianCopyrightInPackage => {
                OutputDatasourceId::DebianCopyrightInPackage
            }
            crate::models::DatasourceId::DebianCopyrightStandalone => {
                OutputDatasourceId::DebianCopyrightStandalone
            }
            crate::models::DatasourceId::DebianDeb => OutputDatasourceId::DebianDeb,
            crate::models::DatasourceId::DebianSourceMetadataTarball => {
                OutputDatasourceId::DebianSourceMetadataTarball
            }
            crate::models::DatasourceId::DebianDistrolessInstalledDb => {
                OutputDatasourceId::DebianDistrolessInstalledDb
            }
            crate::models::DatasourceId::DebianInstalledFilesList => {
                OutputDatasourceId::DebianInstalledFilesList
            }
            crate::models::DatasourceId::DebianInstalledMd5Sums => {
                OutputDatasourceId::DebianInstalledMd5Sums
            }
            crate::models::DatasourceId::DebianInstalledStatusDb => {
                OutputDatasourceId::DebianInstalledStatusDb
            }
            crate::models::DatasourceId::DebianMd5SumsInExtractedDeb => {
                OutputDatasourceId::DebianMd5SumsInExtractedDeb
            }
            crate::models::DatasourceId::DebianOriginalSourceTarball => {
                OutputDatasourceId::DebianOriginalSourceTarball
            }
            crate::models::DatasourceId::DebianSourceControlDsc => {
                OutputDatasourceId::DebianSourceControlDsc
            }

            crate::models::DatasourceId::DenoJson => OutputDatasourceId::DenoJson,
            crate::models::DatasourceId::DenoLock => OutputDatasourceId::DenoLock,

            crate::models::DatasourceId::Dockerfile => OutputDatasourceId::Dockerfile,

            crate::models::DatasourceId::ErlangOtpAppSrc => OutputDatasourceId::ErlangOtpAppSrc,
            crate::models::DatasourceId::RebarConfig => OutputDatasourceId::RebarConfig,
            crate::models::DatasourceId::RebarLock => OutputDatasourceId::RebarLock,

            crate::models::DatasourceId::FreebsdCompactManifest => {
                OutputDatasourceId::FreebsdCompactManifest
            }

            crate::models::DatasourceId::Godeps => OutputDatasourceId::Godeps,
            crate::models::DatasourceId::GoBinary => OutputDatasourceId::GoBinary,
            crate::models::DatasourceId::GoMod => OutputDatasourceId::GoMod,
            crate::models::DatasourceId::GoModGraph => OutputDatasourceId::GoModGraph,
            crate::models::DatasourceId::GoSum => OutputDatasourceId::GoSum,
            crate::models::DatasourceId::GoWork => OutputDatasourceId::GoWork,

            crate::models::DatasourceId::HackageCabal => OutputDatasourceId::HackageCabal,
            crate::models::DatasourceId::HackageCabalProject => {
                OutputDatasourceId::HackageCabalProject
            }
            crate::models::DatasourceId::HackageStackYaml => OutputDatasourceId::HackageStackYaml,

            crate::models::DatasourceId::BuildGradle => OutputDatasourceId::BuildGradle,
            crate::models::DatasourceId::GradleLockfile => OutputDatasourceId::GradleLockfile,
            crate::models::DatasourceId::GradleModule => OutputDatasourceId::GradleModule,

            crate::models::DatasourceId::HaxelibJson => OutputDatasourceId::HaxelibJson,

            crate::models::DatasourceId::HelmChartLock => OutputDatasourceId::HelmChartLock,
            crate::models::DatasourceId::HelmChartYaml => OutputDatasourceId::HelmChartYaml,

            crate::models::DatasourceId::HexMixLock => OutputDatasourceId::HexMixLock,

            crate::models::DatasourceId::JuliaProjectToml => OutputDatasourceId::JuliaProjectToml,
            crate::models::DatasourceId::JuliaManifestToml => OutputDatasourceId::JuliaManifestToml,

            crate::models::DatasourceId::AntIvyXml => OutputDatasourceId::AntIvyXml,
            crate::models::DatasourceId::JavaEarApplicationXml => {
                OutputDatasourceId::JavaEarApplicationXml
            }
            crate::models::DatasourceId::JavaEarArchive => OutputDatasourceId::JavaEarArchive,
            crate::models::DatasourceId::JavaJar => OutputDatasourceId::JavaJar,
            crate::models::DatasourceId::JavaJarManifest => OutputDatasourceId::JavaJarManifest,
            crate::models::DatasourceId::JavaOsgiManifest => OutputDatasourceId::JavaOsgiManifest,
            crate::models::DatasourceId::JavaWarArchive => OutputDatasourceId::JavaWarArchive,
            crate::models::DatasourceId::JavaWarWebXml => OutputDatasourceId::JavaWarWebXml,
            crate::models::DatasourceId::JbossSar => OutputDatasourceId::JbossSar,
            crate::models::DatasourceId::JbossServiceXml => OutputDatasourceId::JbossServiceXml,

            crate::models::DatasourceId::MavenPom => OutputDatasourceId::MavenPom,
            crate::models::DatasourceId::MavenPomProperties => {
                OutputDatasourceId::MavenPomProperties
            }
            crate::models::DatasourceId::MesonBuild => OutputDatasourceId::MesonBuild,

            crate::models::DatasourceId::SbtBuildSbt => OutputDatasourceId::SbtBuildSbt,

            crate::models::DatasourceId::MicrosoftCabinet => OutputDatasourceId::MicrosoftCabinet,
            crate::models::DatasourceId::MicrosoftUpdateManifestMum => {
                OutputDatasourceId::MicrosoftUpdateManifestMum
            }

            crate::models::DatasourceId::AppleDmg => OutputDatasourceId::AppleDmg,
            crate::models::DatasourceId::ChromeCrx => OutputDatasourceId::ChromeCrx,
            crate::models::DatasourceId::IosIpa => OutputDatasourceId::IosIpa,
            crate::models::DatasourceId::MozillaXpi => OutputDatasourceId::MozillaXpi,

            crate::models::DatasourceId::MeteorPackage => OutputDatasourceId::MeteorPackage,

            crate::models::DatasourceId::NixDefaultNix => OutputDatasourceId::NixDefaultNix,
            crate::models::DatasourceId::NixFlakeLock => OutputDatasourceId::NixFlakeLock,
            crate::models::DatasourceId::NixFlakeNix => OutputDatasourceId::NixFlakeNix,

            crate::models::DatasourceId::NpmPackageJson => OutputDatasourceId::NpmPackageJson,
            crate::models::DatasourceId::NpmPackageLockJson => {
                OutputDatasourceId::NpmPackageLockJson
            }

            crate::models::DatasourceId::NugetCsproj => OutputDatasourceId::NugetCsproj,
            crate::models::DatasourceId::NugetDepsJson => OutputDatasourceId::NugetDepsJson,
            crate::models::DatasourceId::NugetDirectoryBuildProps => {
                OutputDatasourceId::NugetDirectoryBuildProps
            }
            crate::models::DatasourceId::NugetDirectoryPackagesProps => {
                OutputDatasourceId::NugetDirectoryPackagesProps
            }
            crate::models::DatasourceId::NugetNupkg => OutputDatasourceId::NugetNupkg,
            crate::models::DatasourceId::NugetProjectJson => OutputDatasourceId::NugetProjectJson,
            crate::models::DatasourceId::NugetProjectLockJson => {
                OutputDatasourceId::NugetProjectLockJson
            }
            crate::models::DatasourceId::NugetPackagesConfig => {
                OutputDatasourceId::NugetPackagesConfig
            }
            crate::models::DatasourceId::NugetPackagesLock => OutputDatasourceId::NugetPackagesLock,
            crate::models::DatasourceId::NugetNuspec => OutputDatasourceId::NugetNuspec,
            crate::models::DatasourceId::NugetVbproj => OutputDatasourceId::NugetVbproj,
            crate::models::DatasourceId::NugetFsproj => OutputDatasourceId::NugetFsproj,

            crate::models::DatasourceId::OpamFile => OutputDatasourceId::OpamFile,

            crate::models::DatasourceId::PhpComposerJson => OutputDatasourceId::PhpComposerJson,
            crate::models::DatasourceId::PhpComposerLock => OutputDatasourceId::PhpComposerLock,

            crate::models::DatasourceId::PnpmLockYaml => OutputDatasourceId::PnpmLockYaml,
            crate::models::DatasourceId::PnpmWorkspaceYaml => OutputDatasourceId::PnpmWorkspaceYaml,

            crate::models::DatasourceId::Pipfile => OutputDatasourceId::Pipfile,
            crate::models::DatasourceId::PipfileLock => OutputDatasourceId::PipfileLock,
            crate::models::DatasourceId::PipRequirements => OutputDatasourceId::PipRequirements,
            crate::models::DatasourceId::PixiLock => OutputDatasourceId::PixiLock,
            crate::models::DatasourceId::PixiToml => OutputDatasourceId::PixiToml,
            crate::models::DatasourceId::PypiPipOriginJson => OutputDatasourceId::PypiPipOriginJson,
            crate::models::DatasourceId::PypiEgg => OutputDatasourceId::PypiEgg,
            crate::models::DatasourceId::PypiEggPkginfo => OutputDatasourceId::PypiEggPkginfo,
            crate::models::DatasourceId::PypiEditableEggPkginfo => {
                OutputDatasourceId::PypiEditableEggPkginfo
            }
            crate::models::DatasourceId::PypiInspectDeplock => {
                OutputDatasourceId::PypiInspectDeplock
            }
            crate::models::DatasourceId::PypiJson => OutputDatasourceId::PypiJson,
            crate::models::DatasourceId::PypiPoetryLock => OutputDatasourceId::PypiPoetryLock,
            crate::models::DatasourceId::PypiPoetryPyprojectToml => {
                OutputDatasourceId::PypiPoetryPyprojectToml
            }
            crate::models::DatasourceId::PypiSdist => OutputDatasourceId::PypiSdist,
            crate::models::DatasourceId::PypiPylockToml => OutputDatasourceId::PypiPylockToml,
            crate::models::DatasourceId::PypiPyprojectToml => OutputDatasourceId::PypiPyprojectToml,
            crate::models::DatasourceId::PypiSdistPkginfo => OutputDatasourceId::PypiSdistPkginfo,
            crate::models::DatasourceId::PypiSetupCfg => OutputDatasourceId::PypiSetupCfg,
            crate::models::DatasourceId::PypiSetupPy => OutputDatasourceId::PypiSetupPy,
            crate::models::DatasourceId::PypiUvLock => OutputDatasourceId::PypiUvLock,
            crate::models::DatasourceId::PypiWheel => OutputDatasourceId::PypiWheel,
            crate::models::DatasourceId::PypiWheelMetadata => OutputDatasourceId::PypiWheelMetadata,

            crate::models::DatasourceId::RpmArchive => OutputDatasourceId::RpmArchive,
            crate::models::DatasourceId::RpmInstalledDatabaseBdb => {
                OutputDatasourceId::RpmInstalledDatabaseBdb
            }
            crate::models::DatasourceId::RpmInstalledDatabaseNdb => {
                OutputDatasourceId::RpmInstalledDatabaseNdb
            }
            crate::models::DatasourceId::RpmInstalledDatabaseSqlite => {
                OutputDatasourceId::RpmInstalledDatabaseSqlite
            }
            crate::models::DatasourceId::RpmMarinerManifest => {
                OutputDatasourceId::RpmMarinerManifest
            }
            crate::models::DatasourceId::RpmPackageLicenses => {
                OutputDatasourceId::RpmPackageLicenses
            }
            crate::models::DatasourceId::RpmSpecfile => OutputDatasourceId::RpmSpecfile,
            crate::models::DatasourceId::RpmYumdb => OutputDatasourceId::RpmYumdb,

            crate::models::DatasourceId::Gemfile => OutputDatasourceId::Gemfile,
            crate::models::DatasourceId::GemfileExtracted => OutputDatasourceId::GemfileExtracted,
            crate::models::DatasourceId::GemfileLock => OutputDatasourceId::GemfileLock,
            crate::models::DatasourceId::GemfileLockExtracted => {
                OutputDatasourceId::GemfileLockExtracted
            }
            crate::models::DatasourceId::GemArchive => OutputDatasourceId::GemArchive,
            crate::models::DatasourceId::GemArchiveExtracted => {
                OutputDatasourceId::GemArchiveExtracted
            }
            crate::models::DatasourceId::Gemspec => OutputDatasourceId::Gemspec,
            crate::models::DatasourceId::GemspecExtracted => OutputDatasourceId::GemspecExtracted,
            crate::models::DatasourceId::GemGemspecInstalledSpecifications => {
                OutputDatasourceId::GemGemspecInstalledSpecifications
            }

            crate::models::DatasourceId::InstallshieldInstaller => {
                OutputDatasourceId::InstallshieldInstaller
            }
            crate::models::DatasourceId::IsoDiskImage => OutputDatasourceId::IsoDiskImage,
            crate::models::DatasourceId::NsisInstaller => OutputDatasourceId::NsisInstaller,
            crate::models::DatasourceId::SharShellArchive => OutputDatasourceId::SharShellArchive,
            crate::models::DatasourceId::SquashfsDiskImage => OutputDatasourceId::SquashfsDiskImage,

            crate::models::DatasourceId::SwiftPackageManifestJson => {
                OutputDatasourceId::SwiftPackageManifestJson
            }
            crate::models::DatasourceId::SwiftPackageResolved => {
                OutputDatasourceId::SwiftPackageResolved
            }
            crate::models::DatasourceId::SwiftPackageShowDependencies => {
                OutputDatasourceId::SwiftPackageShowDependencies
            }

            crate::models::DatasourceId::PubliccodeYaml => OutputDatasourceId::PubliccodeYaml,

            crate::models::DatasourceId::VcpkgJson => OutputDatasourceId::VcpkgJson,

            crate::models::DatasourceId::YarnLock => OutputDatasourceId::YarnLock,
            crate::models::DatasourceId::YarnLockV1 => OutputDatasourceId::YarnLockV1,
            crate::models::DatasourceId::YarnLockV2 => OutputDatasourceId::YarnLockV2,
            crate::models::DatasourceId::YarnPnpCjs => OutputDatasourceId::YarnPnpCjs,

            crate::models::DatasourceId::Gitmodules => OutputDatasourceId::Gitmodules,
        }
    }
}

impl From<&crate::models::DatasourceId> for OutputDatasourceId {
    fn from(value: &crate::models::DatasourceId) -> Self {
        (*value).into()
    }
}

impl TryFrom<OutputDatasourceId> for crate::models::DatasourceId {
    type Error = String;
    fn try_from(value: OutputDatasourceId) -> Result<Self, Self::Error> {
        match value {
            OutputDatasourceId::AboutFile => Ok(crate::models::DatasourceId::AboutFile),
            OutputDatasourceId::Readme => Ok(crate::models::DatasourceId::Readme),
            OutputDatasourceId::EtcOsRelease => Ok(crate::models::DatasourceId::EtcOsRelease),

            OutputDatasourceId::AlpineApkArchive => {
                Ok(crate::models::DatasourceId::AlpineApkArchive)
            }
            OutputDatasourceId::AlpineApkbuild => Ok(crate::models::DatasourceId::AlpineApkbuild),
            OutputDatasourceId::AlpineInstalledDb => {
                Ok(crate::models::DatasourceId::AlpineInstalledDb)
            }

            OutputDatasourceId::ArchAurinfo => Ok(crate::models::DatasourceId::ArchAurinfo),
            OutputDatasourceId::ArchPkginfo => Ok(crate::models::DatasourceId::ArchPkginfo),
            OutputDatasourceId::ArchSrcinfo => Ok(crate::models::DatasourceId::ArchSrcinfo),

            OutputDatasourceId::AndroidAab => Ok(crate::models::DatasourceId::AndroidAab),
            OutputDatasourceId::AndroidAarLibrary => {
                Ok(crate::models::DatasourceId::AndroidAarLibrary)
            }
            OutputDatasourceId::AndroidApk => Ok(crate::models::DatasourceId::AndroidApk),
            OutputDatasourceId::AndroidManifestXml => {
                Ok(crate::models::DatasourceId::AndroidManifestXml)
            }
            OutputDatasourceId::AndroidSoongMetadata => {
                Ok(crate::models::DatasourceId::AndroidSoongMetadata)
            }

            OutputDatasourceId::Axis2Mar => Ok(crate::models::DatasourceId::Axis2Mar),
            OutputDatasourceId::Axis2ModuleXml => Ok(crate::models::DatasourceId::Axis2ModuleXml),

            OutputDatasourceId::AutotoolsConfigure => {
                Ok(crate::models::DatasourceId::AutotoolsConfigure)
            }

            OutputDatasourceId::BazelBuild => Ok(crate::models::DatasourceId::BazelBuild),
            OutputDatasourceId::BazelModule => Ok(crate::models::DatasourceId::BazelModule),

            OutputDatasourceId::BitbakeRecipe => Ok(crate::models::DatasourceId::BitbakeRecipe),
            OutputDatasourceId::BitbakeRecipeAppend => {
                Ok(crate::models::DatasourceId::BitbakeRecipeAppend)
            }

            OutputDatasourceId::BowerJson => Ok(crate::models::DatasourceId::BowerJson),

            OutputDatasourceId::BuckFile => Ok(crate::models::DatasourceId::BuckFile),
            OutputDatasourceId::BuckMetadata => Ok(crate::models::DatasourceId::BuckMetadata),

            OutputDatasourceId::BunLock => Ok(crate::models::DatasourceId::BunLock),
            OutputDatasourceId::BunLockb => Ok(crate::models::DatasourceId::BunLockb),

            OutputDatasourceId::CarthageCartfile => {
                Ok(crate::models::DatasourceId::CarthageCartfile)
            }
            OutputDatasourceId::CarthageCartfileResolved => {
                Ok(crate::models::DatasourceId::CarthageCartfileResolved)
            }

            OutputDatasourceId::CargoLock => Ok(crate::models::DatasourceId::CargoLock),
            OutputDatasourceId::CargoToml => Ok(crate::models::DatasourceId::CargoToml),
            OutputDatasourceId::RustBinary => Ok(crate::models::DatasourceId::RustBinary),
            OutputDatasourceId::WindowsExecutable => {
                Ok(crate::models::DatasourceId::WindowsExecutable)
            }

            OutputDatasourceId::ChefCookbookMetadataJson => {
                Ok(crate::models::DatasourceId::ChefCookbookMetadataJson)
            }
            OutputDatasourceId::ChefCookbookMetadataRb => {
                Ok(crate::models::DatasourceId::ChefCookbookMetadataRb)
            }

            OutputDatasourceId::CitationCff => Ok(crate::models::DatasourceId::CitationCff),

            OutputDatasourceId::CocoapodsPodfile => {
                Ok(crate::models::DatasourceId::CocoapodsPodfile)
            }
            OutputDatasourceId::CocoapodsPodfileLock => {
                Ok(crate::models::DatasourceId::CocoapodsPodfileLock)
            }
            OutputDatasourceId::CocoapodsPodspec => {
                Ok(crate::models::DatasourceId::CocoapodsPodspec)
            }
            OutputDatasourceId::CocoapodsPodspecJson => {
                Ok(crate::models::DatasourceId::CocoapodsPodspecJson)
            }

            OutputDatasourceId::ConanConanDataYml => {
                Ok(crate::models::DatasourceId::ConanConanDataYml)
            }
            OutputDatasourceId::ConanConanFilePy => {
                Ok(crate::models::DatasourceId::ConanConanFilePy)
            }
            OutputDatasourceId::ConanConanFileTxt => {
                Ok(crate::models::DatasourceId::ConanConanFileTxt)
            }
            OutputDatasourceId::ConanLock => Ok(crate::models::DatasourceId::ConanLock),

            OutputDatasourceId::CondaYaml => Ok(crate::models::DatasourceId::CondaYaml),
            OutputDatasourceId::CondaMetaJson => Ok(crate::models::DatasourceId::CondaMetaJson),
            OutputDatasourceId::CondaMetaYaml => Ok(crate::models::DatasourceId::CondaMetaYaml),

            OutputDatasourceId::ClojureDepsEdn => Ok(crate::models::DatasourceId::ClojureDepsEdn),
            OutputDatasourceId::ClojureProjectClj => {
                Ok(crate::models::DatasourceId::ClojureProjectClj)
            }

            OutputDatasourceId::CpanDistIni => Ok(crate::models::DatasourceId::CpanDistIni),
            OutputDatasourceId::CpanMakefile => Ok(crate::models::DatasourceId::CpanMakefile),
            OutputDatasourceId::CpanManifest => Ok(crate::models::DatasourceId::CpanManifest),
            OutputDatasourceId::CpanMetaJson => Ok(crate::models::DatasourceId::CpanMetaJson),
            OutputDatasourceId::CpanMetaYml => Ok(crate::models::DatasourceId::CpanMetaYml),

            OutputDatasourceId::CranDescription => Ok(crate::models::DatasourceId::CranDescription),

            OutputDatasourceId::PubspecLock => Ok(crate::models::DatasourceId::PubspecLock),
            OutputDatasourceId::PubspecYaml => Ok(crate::models::DatasourceId::PubspecYaml),

            OutputDatasourceId::DebianControlExtractedDeb => {
                Ok(crate::models::DatasourceId::DebianControlExtractedDeb)
            }
            OutputDatasourceId::DebianControlInSource => {
                Ok(crate::models::DatasourceId::DebianControlInSource)
            }
            OutputDatasourceId::DebianCopyright => Ok(crate::models::DatasourceId::DebianCopyright),
            OutputDatasourceId::DebianCopyrightInSource => {
                Ok(crate::models::DatasourceId::DebianCopyrightInSource)
            }
            OutputDatasourceId::DebianCopyrightInPackage => {
                Ok(crate::models::DatasourceId::DebianCopyrightInPackage)
            }
            OutputDatasourceId::DebianCopyrightStandalone => {
                Ok(crate::models::DatasourceId::DebianCopyrightStandalone)
            }
            OutputDatasourceId::DebianDeb => Ok(crate::models::DatasourceId::DebianDeb),
            OutputDatasourceId::DebianSourceMetadataTarball => {
                Ok(crate::models::DatasourceId::DebianSourceMetadataTarball)
            }
            OutputDatasourceId::DebianDistrolessInstalledDb => {
                Ok(crate::models::DatasourceId::DebianDistrolessInstalledDb)
            }
            OutputDatasourceId::DebianInstalledFilesList => {
                Ok(crate::models::DatasourceId::DebianInstalledFilesList)
            }
            OutputDatasourceId::DebianInstalledMd5Sums => {
                Ok(crate::models::DatasourceId::DebianInstalledMd5Sums)
            }
            OutputDatasourceId::DebianInstalledStatusDb => {
                Ok(crate::models::DatasourceId::DebianInstalledStatusDb)
            }
            OutputDatasourceId::DebianMd5SumsInExtractedDeb => {
                Ok(crate::models::DatasourceId::DebianMd5SumsInExtractedDeb)
            }
            OutputDatasourceId::DebianOriginalSourceTarball => {
                Ok(crate::models::DatasourceId::DebianOriginalSourceTarball)
            }
            OutputDatasourceId::DebianSourceControlDsc => {
                Ok(crate::models::DatasourceId::DebianSourceControlDsc)
            }

            OutputDatasourceId::DenoJson => Ok(crate::models::DatasourceId::DenoJson),
            OutputDatasourceId::DenoLock => Ok(crate::models::DatasourceId::DenoLock),

            OutputDatasourceId::Dockerfile => Ok(crate::models::DatasourceId::Dockerfile),

            OutputDatasourceId::ErlangOtpAppSrc => Ok(crate::models::DatasourceId::ErlangOtpAppSrc),
            OutputDatasourceId::RebarConfig => Ok(crate::models::DatasourceId::RebarConfig),
            OutputDatasourceId::RebarLock => Ok(crate::models::DatasourceId::RebarLock),

            OutputDatasourceId::FreebsdCompactManifest => {
                Ok(crate::models::DatasourceId::FreebsdCompactManifest)
            }

            OutputDatasourceId::Godeps => Ok(crate::models::DatasourceId::Godeps),
            OutputDatasourceId::GoBinary => Ok(crate::models::DatasourceId::GoBinary),
            OutputDatasourceId::GoMod => Ok(crate::models::DatasourceId::GoMod),
            OutputDatasourceId::GoModGraph => Ok(crate::models::DatasourceId::GoModGraph),
            OutputDatasourceId::GoSum => Ok(crate::models::DatasourceId::GoSum),
            OutputDatasourceId::GoWork => Ok(crate::models::DatasourceId::GoWork),

            OutputDatasourceId::HackageCabal => Ok(crate::models::DatasourceId::HackageCabal),
            OutputDatasourceId::HackageCabalProject => {
                Ok(crate::models::DatasourceId::HackageCabalProject)
            }
            OutputDatasourceId::HackageStackYaml => {
                Ok(crate::models::DatasourceId::HackageStackYaml)
            }

            OutputDatasourceId::BuildGradle => Ok(crate::models::DatasourceId::BuildGradle),
            OutputDatasourceId::GradleLockfile => Ok(crate::models::DatasourceId::GradleLockfile),
            OutputDatasourceId::GradleModule => Ok(crate::models::DatasourceId::GradleModule),

            OutputDatasourceId::HaxelibJson => Ok(crate::models::DatasourceId::HaxelibJson),

            OutputDatasourceId::HelmChartLock => Ok(crate::models::DatasourceId::HelmChartLock),
            OutputDatasourceId::HelmChartYaml => Ok(crate::models::DatasourceId::HelmChartYaml),

            OutputDatasourceId::HexMixLock => Ok(crate::models::DatasourceId::HexMixLock),

            OutputDatasourceId::JuliaProjectToml => {
                Ok(crate::models::DatasourceId::JuliaProjectToml)
            }
            OutputDatasourceId::JuliaManifestToml => {
                Ok(crate::models::DatasourceId::JuliaManifestToml)
            }

            OutputDatasourceId::AntIvyXml => Ok(crate::models::DatasourceId::AntIvyXml),
            OutputDatasourceId::JavaEarApplicationXml => {
                Ok(crate::models::DatasourceId::JavaEarApplicationXml)
            }
            OutputDatasourceId::JavaEarArchive => Ok(crate::models::DatasourceId::JavaEarArchive),
            OutputDatasourceId::JavaJar => Ok(crate::models::DatasourceId::JavaJar),
            OutputDatasourceId::JavaJarManifest => Ok(crate::models::DatasourceId::JavaJarManifest),
            OutputDatasourceId::JavaOsgiManifest => {
                Ok(crate::models::DatasourceId::JavaOsgiManifest)
            }
            OutputDatasourceId::JavaWarArchive => Ok(crate::models::DatasourceId::JavaWarArchive),
            OutputDatasourceId::JavaWarWebXml => Ok(crate::models::DatasourceId::JavaWarWebXml),
            OutputDatasourceId::JbossSar => Ok(crate::models::DatasourceId::JbossSar),
            OutputDatasourceId::JbossServiceXml => Ok(crate::models::DatasourceId::JbossServiceXml),

            OutputDatasourceId::MavenPom => Ok(crate::models::DatasourceId::MavenPom),
            OutputDatasourceId::MavenPomProperties => {
                Ok(crate::models::DatasourceId::MavenPomProperties)
            }
            OutputDatasourceId::MesonBuild => Ok(crate::models::DatasourceId::MesonBuild),

            OutputDatasourceId::SbtBuildSbt => Ok(crate::models::DatasourceId::SbtBuildSbt),

            OutputDatasourceId::MicrosoftCabinet => {
                Ok(crate::models::DatasourceId::MicrosoftCabinet)
            }
            OutputDatasourceId::MicrosoftUpdateManifestMum => {
                Ok(crate::models::DatasourceId::MicrosoftUpdateManifestMum)
            }

            OutputDatasourceId::AppleDmg => Ok(crate::models::DatasourceId::AppleDmg),
            OutputDatasourceId::ChromeCrx => Ok(crate::models::DatasourceId::ChromeCrx),
            OutputDatasourceId::IosIpa => Ok(crate::models::DatasourceId::IosIpa),
            OutputDatasourceId::MozillaXpi => Ok(crate::models::DatasourceId::MozillaXpi),

            OutputDatasourceId::MeteorPackage => Ok(crate::models::DatasourceId::MeteorPackage),

            OutputDatasourceId::NixDefaultNix => Ok(crate::models::DatasourceId::NixDefaultNix),
            OutputDatasourceId::NixFlakeLock => Ok(crate::models::DatasourceId::NixFlakeLock),
            OutputDatasourceId::NixFlakeNix => Ok(crate::models::DatasourceId::NixFlakeNix),

            OutputDatasourceId::NpmPackageJson => Ok(crate::models::DatasourceId::NpmPackageJson),
            OutputDatasourceId::NpmPackageLockJson => {
                Ok(crate::models::DatasourceId::NpmPackageLockJson)
            }

            OutputDatasourceId::NugetCsproj => Ok(crate::models::DatasourceId::NugetCsproj),
            OutputDatasourceId::NugetDepsJson => Ok(crate::models::DatasourceId::NugetDepsJson),
            OutputDatasourceId::NugetDirectoryBuildProps => {
                Ok(crate::models::DatasourceId::NugetDirectoryBuildProps)
            }
            OutputDatasourceId::NugetDirectoryPackagesProps => {
                Ok(crate::models::DatasourceId::NugetDirectoryPackagesProps)
            }
            OutputDatasourceId::NugetNupkg => Ok(crate::models::DatasourceId::NugetNupkg),
            OutputDatasourceId::NugetProjectJson => {
                Ok(crate::models::DatasourceId::NugetProjectJson)
            }
            OutputDatasourceId::NugetProjectLockJson => {
                Ok(crate::models::DatasourceId::NugetProjectLockJson)
            }
            OutputDatasourceId::NugetPackagesConfig => {
                Ok(crate::models::DatasourceId::NugetPackagesConfig)
            }
            OutputDatasourceId::NugetPackagesLock => {
                Ok(crate::models::DatasourceId::NugetPackagesLock)
            }
            OutputDatasourceId::NugetNuspec => Ok(crate::models::DatasourceId::NugetNuspec),
            OutputDatasourceId::NugetVbproj => Ok(crate::models::DatasourceId::NugetVbproj),
            OutputDatasourceId::NugetFsproj => Ok(crate::models::DatasourceId::NugetFsproj),

            OutputDatasourceId::OpamFile => Ok(crate::models::DatasourceId::OpamFile),

            OutputDatasourceId::PhpComposerJson => Ok(crate::models::DatasourceId::PhpComposerJson),
            OutputDatasourceId::PhpComposerLock => Ok(crate::models::DatasourceId::PhpComposerLock),

            OutputDatasourceId::PnpmLockYaml => Ok(crate::models::DatasourceId::PnpmLockYaml),
            OutputDatasourceId::PnpmWorkspaceYaml => {
                Ok(crate::models::DatasourceId::PnpmWorkspaceYaml)
            }

            OutputDatasourceId::Pipfile => Ok(crate::models::DatasourceId::Pipfile),
            OutputDatasourceId::PipfileLock => Ok(crate::models::DatasourceId::PipfileLock),
            OutputDatasourceId::PipRequirements => Ok(crate::models::DatasourceId::PipRequirements),
            OutputDatasourceId::PixiLock => Ok(crate::models::DatasourceId::PixiLock),
            OutputDatasourceId::PixiToml => Ok(crate::models::DatasourceId::PixiToml),
            OutputDatasourceId::PypiPipOriginJson => {
                Ok(crate::models::DatasourceId::PypiPipOriginJson)
            }
            OutputDatasourceId::PypiEgg => Ok(crate::models::DatasourceId::PypiEgg),
            OutputDatasourceId::PypiEggPkginfo => Ok(crate::models::DatasourceId::PypiEggPkginfo),
            OutputDatasourceId::PypiEditableEggPkginfo => {
                Ok(crate::models::DatasourceId::PypiEditableEggPkginfo)
            }
            OutputDatasourceId::PypiInspectDeplock => {
                Ok(crate::models::DatasourceId::PypiInspectDeplock)
            }
            OutputDatasourceId::PypiJson => Ok(crate::models::DatasourceId::PypiJson),
            OutputDatasourceId::PypiPoetryLock => Ok(crate::models::DatasourceId::PypiPoetryLock),
            OutputDatasourceId::PypiPoetryPyprojectToml => {
                Ok(crate::models::DatasourceId::PypiPoetryPyprojectToml)
            }
            OutputDatasourceId::PypiSdist => Ok(crate::models::DatasourceId::PypiSdist),
            OutputDatasourceId::PypiPylockToml => Ok(crate::models::DatasourceId::PypiPylockToml),
            OutputDatasourceId::PypiPyprojectToml => {
                Ok(crate::models::DatasourceId::PypiPyprojectToml)
            }
            OutputDatasourceId::PypiSdistPkginfo => {
                Ok(crate::models::DatasourceId::PypiSdistPkginfo)
            }
            OutputDatasourceId::PypiSetupCfg => Ok(crate::models::DatasourceId::PypiSetupCfg),
            OutputDatasourceId::PypiSetupPy => Ok(crate::models::DatasourceId::PypiSetupPy),
            OutputDatasourceId::PypiUvLock => Ok(crate::models::DatasourceId::PypiUvLock),
            OutputDatasourceId::PypiWheel => Ok(crate::models::DatasourceId::PypiWheel),
            OutputDatasourceId::PypiWheelMetadata => {
                Ok(crate::models::DatasourceId::PypiWheelMetadata)
            }

            OutputDatasourceId::RpmArchive => Ok(crate::models::DatasourceId::RpmArchive),
            OutputDatasourceId::RpmInstalledDatabaseBdb => {
                Ok(crate::models::DatasourceId::RpmInstalledDatabaseBdb)
            }
            OutputDatasourceId::RpmInstalledDatabaseNdb => {
                Ok(crate::models::DatasourceId::RpmInstalledDatabaseNdb)
            }
            OutputDatasourceId::RpmInstalledDatabaseSqlite => {
                Ok(crate::models::DatasourceId::RpmInstalledDatabaseSqlite)
            }
            OutputDatasourceId::RpmMarinerManifest => {
                Ok(crate::models::DatasourceId::RpmMarinerManifest)
            }
            OutputDatasourceId::RpmPackageLicenses => {
                Ok(crate::models::DatasourceId::RpmPackageLicenses)
            }
            OutputDatasourceId::RpmSpecfile => Ok(crate::models::DatasourceId::RpmSpecfile),
            OutputDatasourceId::RpmYumdb => Ok(crate::models::DatasourceId::RpmYumdb),

            OutputDatasourceId::Gemfile => Ok(crate::models::DatasourceId::Gemfile),
            OutputDatasourceId::GemfileExtracted => {
                Ok(crate::models::DatasourceId::GemfileExtracted)
            }
            OutputDatasourceId::GemfileLock => Ok(crate::models::DatasourceId::GemfileLock),
            OutputDatasourceId::GemfileLockExtracted => {
                Ok(crate::models::DatasourceId::GemfileLockExtracted)
            }
            OutputDatasourceId::GemArchive => Ok(crate::models::DatasourceId::GemArchive),
            OutputDatasourceId::GemArchiveExtracted => {
                Ok(crate::models::DatasourceId::GemArchiveExtracted)
            }
            OutputDatasourceId::Gemspec => Ok(crate::models::DatasourceId::Gemspec),
            OutputDatasourceId::GemspecExtracted => {
                Ok(crate::models::DatasourceId::GemspecExtracted)
            }
            OutputDatasourceId::GemGemspecInstalledSpecifications => {
                Ok(crate::models::DatasourceId::GemGemspecInstalledSpecifications)
            }

            OutputDatasourceId::InstallshieldInstaller => {
                Ok(crate::models::DatasourceId::InstallshieldInstaller)
            }
            OutputDatasourceId::IsoDiskImage => Ok(crate::models::DatasourceId::IsoDiskImage),
            OutputDatasourceId::NsisInstaller => Ok(crate::models::DatasourceId::NsisInstaller),
            OutputDatasourceId::SharShellArchive => {
                Ok(crate::models::DatasourceId::SharShellArchive)
            }
            OutputDatasourceId::SquashfsDiskImage => {
                Ok(crate::models::DatasourceId::SquashfsDiskImage)
            }

            OutputDatasourceId::SwiftPackageManifestJson => {
                Ok(crate::models::DatasourceId::SwiftPackageManifestJson)
            }
            OutputDatasourceId::SwiftPackageResolved => {
                Ok(crate::models::DatasourceId::SwiftPackageResolved)
            }
            OutputDatasourceId::SwiftPackageShowDependencies => {
                Ok(crate::models::DatasourceId::SwiftPackageShowDependencies)
            }

            OutputDatasourceId::PubliccodeYaml => Ok(crate::models::DatasourceId::PubliccodeYaml),

            OutputDatasourceId::VcpkgJson => Ok(crate::models::DatasourceId::VcpkgJson),

            OutputDatasourceId::YarnLock => Ok(crate::models::DatasourceId::YarnLock),
            OutputDatasourceId::YarnLockV1 => Ok(crate::models::DatasourceId::YarnLockV1),
            OutputDatasourceId::YarnLockV2 => Ok(crate::models::DatasourceId::YarnLockV2),
            OutputDatasourceId::YarnPnpCjs => Ok(crate::models::DatasourceId::YarnPnpCjs),

            OutputDatasourceId::Gitmodules => Ok(crate::models::DatasourceId::Gitmodules),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conan_renames() {
        assert_eq!(
            serde_json::to_string(&OutputDatasourceId::ConanConanDataYml).unwrap(),
            r#""conan_conandata_yml""#
        );
        assert_eq!(
            serde_json::to_string(&OutputDatasourceId::ConanConanFilePy).unwrap(),
            r#""conan_conanfile_py""#
        );
        assert_eq!(
            serde_json::to_string(&OutputDatasourceId::ConanConanFileTxt).unwrap(),
            r#""conan_conanfile_txt""#
        );
    }

    #[test]
    fn debian_md5sums_renames() {
        assert_eq!(
            serde_json::to_string(&OutputDatasourceId::DebianInstalledMd5Sums).unwrap(),
            r#""debian_installed_md5sums""#
        );
        assert_eq!(
            serde_json::to_string(&OutputDatasourceId::DebianMd5SumsInExtractedDeb).unwrap(),
            r#""debian_md5sums_in_extracted_deb""#
        );
    }

    #[test]
    fn nuget_nuspec_alias() {
        let id: OutputDatasourceId = serde_json::from_str(r#""nuget_nupsec""#).unwrap();
        assert_eq!(id, OutputDatasourceId::NugetNuspec);
    }

    #[test]
    fn rpm_specfile_alias() {
        let id: OutputDatasourceId = serde_json::from_str(r#""rpm_spefile""#).unwrap();
        assert_eq!(id, OutputDatasourceId::RpmSpecfile);
    }

    #[test]
    fn roundtrip_from_model() {
        let model = crate::models::DatasourceId::ConanConanDataYml;
        let output = OutputDatasourceId::from(model);
        assert_eq!(output, OutputDatasourceId::ConanConanDataYml);
        let back = crate::models::DatasourceId::try_from(output).unwrap();
        assert_eq!(back, model);
    }

    #[test]
    fn typical_snake_case() {
        assert_eq!(
            serde_json::to_string(&OutputDatasourceId::NpmPackageJson).unwrap(),
            r#""npm_package_json""#
        );
        assert_eq!(
            serde_json::to_string(&OutputDatasourceId::NugetNuspec).unwrap(),
            r#""nuget_nuspec""#
        );
    }
}
