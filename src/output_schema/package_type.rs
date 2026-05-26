// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputPackageType {
    About,
    Alpm,
    Alpine,
    Android,
    AndroidLib,
    Autotools,
    Axis2,
    Bazel,
    Bitbake,
    Bower,
    Buck,
    Cab,
    Cargo,
    Carthage,
    Chef,
    Chrome,
    Cocoapods,
    Composer,
    Conan,
    Conda,
    Cpan,
    Cran,
    Dart,
    Deb,
    Deno,
    Docker,
    Dmg,
    Ear,
    Freebsd,
    Gem,
    Generic,
    Github,
    Golang,
    Hackage,
    Haxe,
    Helm,
    Hex,
    Installshield,
    Julia,
    Ios,
    Iso,
    Ivy,
    Jar,
    #[serde(rename = "jboss-service")]
    JbossService,
    #[serde(rename = "linux-distro")]
    LinuxDistro,
    Maven,
    Meson,
    Meteor,
    Nix,
    Mozilla,
    Npm,
    Nsis,
    Nuget,
    Opam,
    Osgi,
    #[serde(rename = "pnpm-lock")]
    PnpmLock,
    Pubspec,
    Pypi,
    Pixi,
    Publiccode,
    Readme,
    Rpm,
    Shar,
    Squashfs,
    Swift,
    Vcpkg,
    War,
    Winexe,
    #[serde(rename = "windows-update")]
    WindowsUpdate,
}

impl From<crate::models::PackageType> for OutputPackageType {
    fn from(value: crate::models::PackageType) -> Self {
        match value {
            crate::models::PackageType::About => OutputPackageType::About,
            crate::models::PackageType::Alpm => OutputPackageType::Alpm,
            crate::models::PackageType::Alpine => OutputPackageType::Alpine,
            crate::models::PackageType::Android => OutputPackageType::Android,
            crate::models::PackageType::AndroidLib => OutputPackageType::AndroidLib,
            crate::models::PackageType::Autotools => OutputPackageType::Autotools,
            crate::models::PackageType::Axis2 => OutputPackageType::Axis2,
            crate::models::PackageType::Bazel => OutputPackageType::Bazel,
            crate::models::PackageType::Bitbake => OutputPackageType::Bitbake,
            crate::models::PackageType::Bower => OutputPackageType::Bower,
            crate::models::PackageType::Buck => OutputPackageType::Buck,
            crate::models::PackageType::Cab => OutputPackageType::Cab,
            crate::models::PackageType::Cargo => OutputPackageType::Cargo,
            crate::models::PackageType::Carthage => OutputPackageType::Carthage,
            crate::models::PackageType::Chef => OutputPackageType::Chef,
            crate::models::PackageType::Chrome => OutputPackageType::Chrome,
            crate::models::PackageType::Cocoapods => OutputPackageType::Cocoapods,
            crate::models::PackageType::Composer => OutputPackageType::Composer,
            crate::models::PackageType::Conan => OutputPackageType::Conan,
            crate::models::PackageType::Conda => OutputPackageType::Conda,
            crate::models::PackageType::Cpan => OutputPackageType::Cpan,
            crate::models::PackageType::Cran => OutputPackageType::Cran,
            crate::models::PackageType::Dart => OutputPackageType::Dart,
            crate::models::PackageType::Deb => OutputPackageType::Deb,
            crate::models::PackageType::Deno => OutputPackageType::Deno,
            crate::models::PackageType::Docker => OutputPackageType::Docker,
            crate::models::PackageType::Dmg => OutputPackageType::Dmg,
            crate::models::PackageType::Ear => OutputPackageType::Ear,
            crate::models::PackageType::Freebsd => OutputPackageType::Freebsd,
            crate::models::PackageType::Gem => OutputPackageType::Gem,
            crate::models::PackageType::Generic => OutputPackageType::Generic,
            crate::models::PackageType::Github => OutputPackageType::Github,
            crate::models::PackageType::Golang => OutputPackageType::Golang,
            crate::models::PackageType::Hackage => OutputPackageType::Hackage,
            crate::models::PackageType::Haxe => OutputPackageType::Haxe,
            crate::models::PackageType::Helm => OutputPackageType::Helm,
            crate::models::PackageType::Hex => OutputPackageType::Hex,
            crate::models::PackageType::Installshield => OutputPackageType::Installshield,
            crate::models::PackageType::Julia => OutputPackageType::Julia,
            crate::models::PackageType::Ios => OutputPackageType::Ios,
            crate::models::PackageType::Iso => OutputPackageType::Iso,
            crate::models::PackageType::Ivy => OutputPackageType::Ivy,
            crate::models::PackageType::Jar => OutputPackageType::Jar,
            crate::models::PackageType::JbossService => OutputPackageType::JbossService,
            crate::models::PackageType::LinuxDistro => OutputPackageType::LinuxDistro,
            crate::models::PackageType::Maven => OutputPackageType::Maven,
            crate::models::PackageType::Meson => OutputPackageType::Meson,
            crate::models::PackageType::Meteor => OutputPackageType::Meteor,
            crate::models::PackageType::Nix => OutputPackageType::Nix,
            crate::models::PackageType::Mozilla => OutputPackageType::Mozilla,
            crate::models::PackageType::Npm => OutputPackageType::Npm,
            crate::models::PackageType::Nsis => OutputPackageType::Nsis,
            crate::models::PackageType::Nuget => OutputPackageType::Nuget,
            crate::models::PackageType::Opam => OutputPackageType::Opam,
            crate::models::PackageType::Osgi => OutputPackageType::Osgi,
            crate::models::PackageType::PnpmLock => OutputPackageType::PnpmLock,
            crate::models::PackageType::Pubspec => OutputPackageType::Pubspec,
            crate::models::PackageType::Pypi => OutputPackageType::Pypi,
            crate::models::PackageType::Pixi => OutputPackageType::Pixi,
            crate::models::PackageType::Publiccode => OutputPackageType::Publiccode,
            crate::models::PackageType::Readme => OutputPackageType::Readme,
            crate::models::PackageType::Rpm => OutputPackageType::Rpm,
            crate::models::PackageType::Shar => OutputPackageType::Shar,
            crate::models::PackageType::Squashfs => OutputPackageType::Squashfs,
            crate::models::PackageType::Swift => OutputPackageType::Swift,
            crate::models::PackageType::Vcpkg => OutputPackageType::Vcpkg,
            crate::models::PackageType::War => OutputPackageType::War,
            crate::models::PackageType::Winexe => OutputPackageType::Winexe,
            crate::models::PackageType::WindowsUpdate => OutputPackageType::WindowsUpdate,
        }
    }
}

impl From<&crate::models::PackageType> for OutputPackageType {
    fn from(value: &crate::models::PackageType) -> Self {
        (*value).into()
    }
}

impl TryFrom<OutputPackageType> for crate::models::PackageType {
    type Error = String;
    fn try_from(value: OutputPackageType) -> Result<Self, Self::Error> {
        match value {
            OutputPackageType::About => Ok(crate::models::PackageType::About),
            OutputPackageType::Alpm => Ok(crate::models::PackageType::Alpm),
            OutputPackageType::Alpine => Ok(crate::models::PackageType::Alpine),
            OutputPackageType::Android => Ok(crate::models::PackageType::Android),
            OutputPackageType::AndroidLib => Ok(crate::models::PackageType::AndroidLib),
            OutputPackageType::Autotools => Ok(crate::models::PackageType::Autotools),
            OutputPackageType::Axis2 => Ok(crate::models::PackageType::Axis2),
            OutputPackageType::Bazel => Ok(crate::models::PackageType::Bazel),
            OutputPackageType::Bitbake => Ok(crate::models::PackageType::Bitbake),
            OutputPackageType::Bower => Ok(crate::models::PackageType::Bower),
            OutputPackageType::Buck => Ok(crate::models::PackageType::Buck),
            OutputPackageType::Cab => Ok(crate::models::PackageType::Cab),
            OutputPackageType::Cargo => Ok(crate::models::PackageType::Cargo),
            OutputPackageType::Carthage => Ok(crate::models::PackageType::Carthage),
            OutputPackageType::Chef => Ok(crate::models::PackageType::Chef),
            OutputPackageType::Chrome => Ok(crate::models::PackageType::Chrome),
            OutputPackageType::Cocoapods => Ok(crate::models::PackageType::Cocoapods),
            OutputPackageType::Composer => Ok(crate::models::PackageType::Composer),
            OutputPackageType::Conan => Ok(crate::models::PackageType::Conan),
            OutputPackageType::Conda => Ok(crate::models::PackageType::Conda),
            OutputPackageType::Cpan => Ok(crate::models::PackageType::Cpan),
            OutputPackageType::Cran => Ok(crate::models::PackageType::Cran),
            OutputPackageType::Dart => Ok(crate::models::PackageType::Dart),
            OutputPackageType::Deb => Ok(crate::models::PackageType::Deb),
            OutputPackageType::Deno => Ok(crate::models::PackageType::Deno),
            OutputPackageType::Docker => Ok(crate::models::PackageType::Docker),
            OutputPackageType::Dmg => Ok(crate::models::PackageType::Dmg),
            OutputPackageType::Ear => Ok(crate::models::PackageType::Ear),
            OutputPackageType::Freebsd => Ok(crate::models::PackageType::Freebsd),
            OutputPackageType::Gem => Ok(crate::models::PackageType::Gem),
            OutputPackageType::Generic => Ok(crate::models::PackageType::Generic),
            OutputPackageType::Github => Ok(crate::models::PackageType::Github),
            OutputPackageType::Golang => Ok(crate::models::PackageType::Golang),
            OutputPackageType::Hackage => Ok(crate::models::PackageType::Hackage),
            OutputPackageType::Haxe => Ok(crate::models::PackageType::Haxe),
            OutputPackageType::Helm => Ok(crate::models::PackageType::Helm),
            OutputPackageType::Hex => Ok(crate::models::PackageType::Hex),
            OutputPackageType::Installshield => Ok(crate::models::PackageType::Installshield),
            OutputPackageType::Julia => Ok(crate::models::PackageType::Julia),
            OutputPackageType::Ios => Ok(crate::models::PackageType::Ios),
            OutputPackageType::Iso => Ok(crate::models::PackageType::Iso),
            OutputPackageType::Ivy => Ok(crate::models::PackageType::Ivy),
            OutputPackageType::Jar => Ok(crate::models::PackageType::Jar),
            OutputPackageType::JbossService => Ok(crate::models::PackageType::JbossService),
            OutputPackageType::LinuxDistro => Ok(crate::models::PackageType::LinuxDistro),
            OutputPackageType::Maven => Ok(crate::models::PackageType::Maven),
            OutputPackageType::Meson => Ok(crate::models::PackageType::Meson),
            OutputPackageType::Meteor => Ok(crate::models::PackageType::Meteor),
            OutputPackageType::Nix => Ok(crate::models::PackageType::Nix),
            OutputPackageType::Mozilla => Ok(crate::models::PackageType::Mozilla),
            OutputPackageType::Npm => Ok(crate::models::PackageType::Npm),
            OutputPackageType::Nsis => Ok(crate::models::PackageType::Nsis),
            OutputPackageType::Nuget => Ok(crate::models::PackageType::Nuget),
            OutputPackageType::Opam => Ok(crate::models::PackageType::Opam),
            OutputPackageType::Osgi => Ok(crate::models::PackageType::Osgi),
            OutputPackageType::PnpmLock => Ok(crate::models::PackageType::PnpmLock),
            OutputPackageType::Pubspec => Ok(crate::models::PackageType::Pubspec),
            OutputPackageType::Pypi => Ok(crate::models::PackageType::Pypi),
            OutputPackageType::Pixi => Ok(crate::models::PackageType::Pixi),
            OutputPackageType::Publiccode => Ok(crate::models::PackageType::Publiccode),
            OutputPackageType::Readme => Ok(crate::models::PackageType::Readme),
            OutputPackageType::Rpm => Ok(crate::models::PackageType::Rpm),
            OutputPackageType::Shar => Ok(crate::models::PackageType::Shar),
            OutputPackageType::Squashfs => Ok(crate::models::PackageType::Squashfs),
            OutputPackageType::Swift => Ok(crate::models::PackageType::Swift),
            OutputPackageType::Vcpkg => Ok(crate::models::PackageType::Vcpkg),
            OutputPackageType::War => Ok(crate::models::PackageType::War),
            OutputPackageType::Winexe => Ok(crate::models::PackageType::Winexe),
            OutputPackageType::WindowsUpdate => Ok(crate::models::PackageType::WindowsUpdate),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_case_renames() {
        assert_eq!(
            serde_json::to_string(&OutputPackageType::JbossService).unwrap(),
            r#""jboss-service""#
        );
        assert_eq!(
            serde_json::to_string(&OutputPackageType::LinuxDistro).unwrap(),
            r#""linux-distro""#
        );
        assert_eq!(
            serde_json::to_string(&OutputPackageType::PnpmLock).unwrap(),
            r#""pnpm-lock""#
        );
        assert_eq!(
            serde_json::to_string(&OutputPackageType::WindowsUpdate).unwrap(),
            r#""windows-update""#
        );
    }

    #[test]
    fn snake_case_variants() {
        assert_eq!(
            serde_json::to_string(&OutputPackageType::AndroidLib).unwrap(),
            r#""android_lib""#
        );
    }

    #[test]
    fn simple_lowercase_variants() {
        assert_eq!(
            serde_json::to_string(&OutputPackageType::Npm).unwrap(),
            r#""npm""#
        );
    }

    #[test]
    fn roundtrip_from_model() {
        let model = crate::models::PackageType::JbossService;
        let output = OutputPackageType::from(model);
        assert_eq!(output, OutputPackageType::JbossService);
        let back = crate::models::PackageType::try_from(output).unwrap();
        assert_eq!(back, model);
    }
}
