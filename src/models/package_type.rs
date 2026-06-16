// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

//! Package type identifiers for package parsers.
//!
//! Each variant uniquely identifies the package ecosystem/registry type.
//! These are used in Package URL (purl) type fields and in the JSON output
//! as the `"type"` field of package data.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Package ecosystem/registry type identifier.
///
/// Identifies the package manager or ecosystem a package belongs to
/// (e.g., npm, PyPI, Maven, Cargo). Used as the `"type"` field in
/// ScanCode Toolkit-compatible JSON output.
///
/// This enum includes both standard purl types and ScanCode-specific types
/// for file format recognizers (e.g., `Jar`, `War`) and metadata sources
/// (e.g., `About`, `Readme`). For the official list of standardized purl types, see:
/// <https://github.com/package-url/purl-spec/blob/main/purl-types-index.json>
///
/// # Serialization
///
/// Variants serialize as PascalCase in the cache/spill format (e.g., `JbossService`).
/// For JSON output, use `as_str()` / `Display` which returns lowercase/kebab-case
/// strings matching the Python ScanCode Toolkit values (e.g., `jboss-service`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PackageType {
    About,
    Alpm,
    Apk,
    Android,
    AndroidLib,
    Autotools,
    Axis2,
    Bazel,
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
    Huggingface,
    Installshield,
    Julia,
    Ios,
    Iso,
    Ivy,
    Jar,
    JbossService,
    LinuxDistro,
    Maven,
    Meson,
    Meteor,
    Nix,
    Mozilla,
    Npm,
    Nsis,
    Nuget,
    Oci,
    Opam,
    Osgi,
    PnpmLock,
    Pub,
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
    WindowsUpdate,
    Yocto,
}

impl PackageType {
    /// Returns the string representation of this package type.
    ///
    /// This matches the serialized form used in JSON output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::About => "about",
            Self::Alpm => "alpm",
            Self::Apk => "apk",
            Self::Android => "android",
            Self::AndroidLib => "android_lib",
            Self::Autotools => "autotools",
            Self::Axis2 => "axis2",
            Self::Bazel => "bazel",
            Self::Bower => "bower",
            Self::Buck => "buck",
            Self::Cab => "cab",
            Self::Cargo => "cargo",
            Self::Carthage => "carthage",
            Self::Chef => "chef",
            Self::Chrome => "chrome",
            Self::Cocoapods => "cocoapods",
            Self::Composer => "composer",
            Self::Conan => "conan",
            Self::Conda => "conda",
            Self::Cpan => "cpan",
            Self::Cran => "cran",
            Self::Deb => "deb",
            Self::Deno => "deno",
            Self::Docker => "docker",
            Self::Dmg => "dmg",
            Self::Ear => "ear",
            Self::Freebsd => "freebsd",
            Self::Gem => "gem",
            Self::Generic => "generic",
            Self::Github => "github",
            Self::Golang => "golang",
            Self::Hackage => "hackage",
            Self::Haxe => "haxe",
            Self::Helm => "helm",
            Self::Hex => "hex",
            Self::Huggingface => "huggingface",
            Self::Installshield => "installshield",
            Self::Julia => "julia",
            Self::Ios => "ios",
            Self::Iso => "iso",
            Self::Ivy => "ivy",
            Self::Jar => "jar",
            Self::JbossService => "jboss-service",
            Self::LinuxDistro => "linux-distro",
            Self::Maven => "maven",
            Self::Meson => "meson",
            Self::Meteor => "meteor",
            Self::Nix => "nix",
            Self::Mozilla => "mozilla",
            Self::Npm => "npm",
            Self::Nsis => "nsis",
            Self::Nuget => "nuget",
            Self::Oci => "oci",
            Self::Opam => "opam",
            Self::Osgi => "osgi",
            Self::PnpmLock => "pnpm-lock",
            Self::Pub => "pub",
            Self::Pypi => "pypi",
            Self::Pixi => "pixi",
            Self::Publiccode => "publiccode",
            Self::Readme => "readme",
            Self::Rpm => "rpm",
            Self::Shar => "shar",
            Self::Squashfs => "squashfs",
            Self::Swift => "swift",
            Self::Vcpkg => "vcpkg",
            Self::War => "war",
            Self::Winexe => "winexe",
            Self::WindowsUpdate => "windows-update",
            Self::Yocto => "yocto",
        }
    }
}

impl AsRef<str> for PackageType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for PackageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PackageType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "about" => Ok(Self::About),
            "alpm" => Ok(Self::Alpm),
            "apk" => Ok(Self::Apk),
            "android" => Ok(Self::Android),
            "android_lib" => Ok(Self::AndroidLib),
            "autotools" => Ok(Self::Autotools),
            "axis2" => Ok(Self::Axis2),
            "bazel" => Ok(Self::Bazel),
            "bower" => Ok(Self::Bower),
            "buck" => Ok(Self::Buck),
            "cab" => Ok(Self::Cab),
            "cargo" => Ok(Self::Cargo),
            "carthage" => Ok(Self::Carthage),
            "chef" => Ok(Self::Chef),
            "chrome" => Ok(Self::Chrome),
            "cocoapods" => Ok(Self::Cocoapods),
            "composer" => Ok(Self::Composer),
            "conan" => Ok(Self::Conan),
            "conda" => Ok(Self::Conda),
            "cpan" => Ok(Self::Cpan),
            "cran" => Ok(Self::Cran),
            "deb" => Ok(Self::Deb),
            "deno" => Ok(Self::Deno),
            "docker" => Ok(Self::Docker),
            "dmg" => Ok(Self::Dmg),
            "ear" => Ok(Self::Ear),
            "freebsd" => Ok(Self::Freebsd),
            "gem" => Ok(Self::Gem),
            "generic" => Ok(Self::Generic),
            "github" => Ok(Self::Github),
            "golang" => Ok(Self::Golang),
            "hackage" => Ok(Self::Hackage),
            "haxe" => Ok(Self::Haxe),
            "helm" => Ok(Self::Helm),
            "hex" => Ok(Self::Hex),
            "huggingface" => Ok(Self::Huggingface),
            "installshield" => Ok(Self::Installshield),
            "julia" => Ok(Self::Julia),
            "ios" => Ok(Self::Ios),
            "iso" => Ok(Self::Iso),
            "ivy" => Ok(Self::Ivy),
            "jar" => Ok(Self::Jar),
            "jboss-service" => Ok(Self::JbossService),
            "linux-distro" => Ok(Self::LinuxDistro),
            "maven" => Ok(Self::Maven),
            "meson" => Ok(Self::Meson),
            "meteor" => Ok(Self::Meteor),
            "nix" => Ok(Self::Nix),
            "mozilla" => Ok(Self::Mozilla),
            "npm" => Ok(Self::Npm),
            "nsis" => Ok(Self::Nsis),
            "nuget" => Ok(Self::Nuget),
            "oci" => Ok(Self::Oci),
            "opam" => Ok(Self::Opam),
            "osgi" => Ok(Self::Osgi),
            "pnpm-lock" => Ok(Self::PnpmLock),
            "pub" => Ok(Self::Pub),
            "pypi" => Ok(Self::Pypi),
            "pixi" => Ok(Self::Pixi),
            "publiccode" => Ok(Self::Publiccode),
            "readme" => Ok(Self::Readme),
            "rpm" => Ok(Self::Rpm),
            "shar" => Ok(Self::Shar),
            "squashfs" => Ok(Self::Squashfs),
            "swift" => Ok(Self::Swift),
            "vcpkg" => Ok(Self::Vcpkg),
            "war" => Ok(Self::War),
            "winexe" => Ok(Self::Winexe),
            "windows-update" => Ok(Self::WindowsUpdate),
            "yocto" => Ok(Self::Yocto),
            _ => Err(format!("unknown package type: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization() {
        let pt = PackageType::Npm;
        let json = serde_json::to_string(&pt).unwrap();
        assert_eq!(json, r#""Npm""#);
    }

    #[test]
    fn test_deserialization() {
        let json = r#""Npm""#;
        let pt: PackageType = serde_json::from_str(json).unwrap();
        assert_eq!(pt, PackageType::Npm);
    }

    #[test]
    fn test_as_str() {
        assert_eq!(PackageType::Npm.as_str(), "npm");
        assert_eq!(PackageType::Cargo.as_str(), "cargo");
        assert_eq!(PackageType::Pypi.as_str(), "pypi");
        assert_eq!(PackageType::Alpm.as_str(), "alpm");
        assert_eq!(PackageType::Vcpkg.as_str(), "vcpkg");
        assert_eq!(PackageType::Hackage.as_str(), "hackage");
        assert_eq!(PackageType::Hex.as_str(), "hex");
    }

    #[test]
    fn test_display() {
        assert_eq!(PackageType::Npm.to_string(), "npm");
    }

    #[test]
    fn test_as_ref() {
        let pt = PackageType::Npm;
        let s: &str = pt.as_ref();
        assert_eq!(s, "npm");
    }

    #[test]
    fn test_kebab_case_variants() {
        assert_eq!(PackageType::JbossService.as_str(), "jboss-service");
        assert_eq!(PackageType::LinuxDistro.as_str(), "linux-distro");
        assert_eq!(PackageType::PnpmLock.as_str(), "pnpm-lock");
        assert_eq!(PackageType::Winexe.as_str(), "winexe");
        assert_eq!(PackageType::WindowsUpdate.as_str(), "windows-update");

        let json = serde_json::to_string(&PackageType::JbossService).unwrap();
        assert_eq!(json, r#""JbossService""#);

        let json = serde_json::to_string(&PackageType::LinuxDistro).unwrap();
        assert_eq!(json, r#""LinuxDistro""#);

        let json = serde_json::to_string(&PackageType::PnpmLock).unwrap();
        assert_eq!(json, r#""PnpmLock""#);

        let json = serde_json::to_string(&PackageType::Winexe).unwrap();
        assert_eq!(json, r#""Winexe""#);

        let json = serde_json::to_string(&PackageType::WindowsUpdate).unwrap();
        assert_eq!(json, r#""WindowsUpdate""#);
    }

    #[test]
    fn test_snake_case_variant() {
        assert_eq!(PackageType::AndroidLib.as_str(), "android_lib");

        let json = serde_json::to_string(&PackageType::AndroidLib).unwrap();
        assert_eq!(json, r#""AndroidLib""#);
    }

    #[test]
    fn test_deserialization_kebab_case() {
        let pt: PackageType = serde_json::from_str(r#""JbossService""#).unwrap();
        assert_eq!(pt, PackageType::JbossService);

        let pt: PackageType = serde_json::from_str(r#""LinuxDistro""#).unwrap();
        assert_eq!(pt, PackageType::LinuxDistro);

        let pt: PackageType = serde_json::from_str(r#""PnpmLock""#).unwrap();
        assert_eq!(pt, PackageType::PnpmLock);

        let pt: PackageType = serde_json::from_str(r#""Winexe""#).unwrap();
        assert_eq!(pt, PackageType::Winexe);

        let pt: PackageType = serde_json::from_str(r#""WindowsUpdate""#).unwrap();
        assert_eq!(pt, PackageType::WindowsUpdate);
    }
}
