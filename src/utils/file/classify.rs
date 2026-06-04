// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! File classification: mime type, binary/text/archive/media/script/source
//! flags, and a human-readable `file_type` label, mirroring ScanCode's
//! `file_info` surface.

use std::path::Path;

use file_format::{FileFormat, Kind as FileFormatKind};
use mime_guess::from_path;

use crate::utils::language::detect_language;

use super::encoding::{has_binary_control_chars, has_decodable_text, looks_like_textual_bytes};
use super::format_sniff::{
    ARCHIVE_EXTENSIONS, detect_file_format, format_based_file_type, is_known_binary_format,
    is_textual_format, is_zip_archive, looks_like_bzip2, looks_like_deb, looks_like_gzip,
    looks_like_pdf, looks_like_rpm, looks_like_squashfs, looks_like_xz,
    media_file_type_from_content, media_mime_from_content,
};
use super::path::{
    extension, is_c_like_source, is_java_like_source, is_makefile, is_plain_text, is_source_map,
    lower_extension, lower_file_name,
};

const JSON_VALIDATION_MAX_BYTES: usize = 4 * 1024 * 1024;
const BINARY_EXTENSIONS: &[&str] = &[
    "pyc", "pyo", "pgm", "pbm", "ppm", "mp3", "mp4", "mpeg", "mpg", "emf",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfoClassification {
    pub mime_type: String,
    pub file_type: String,
    pub programming_language: Option<String>,
    pub is_binary: bool,
    pub is_text: bool,
    pub is_archive: bool,
    pub is_media: bool,
    pub is_source: bool,
    pub is_script: bool,
}

pub fn classify_file_info(path: &Path, bytes: &[u8]) -> FileInfoClassification {
    let detected_format = detect_file_format(bytes);
    let detected_language = detect_language(path, bytes);
    let is_binary = detect_is_binary(path, bytes, detected_format, detected_language.as_deref());
    let is_text = !is_binary;
    let mime_type = detect_mime_type(path, bytes, detected_format, detected_language.as_deref());
    let is_archive = detect_is_archive(path, bytes, &mime_type, is_text, detected_format);
    let is_media = detect_is_media(path, bytes, &mime_type, detected_format);
    let is_script = detect_is_script(path, bytes, detected_language.as_deref(), is_text);
    let is_source = detect_is_source(path, detected_language.as_deref(), is_text, is_script);
    let programming_language = is_source.then(|| detected_language.clone()).flatten();
    let file_type = detect_file_type(
        path,
        bytes,
        detected_format,
        &mime_type,
        programming_language.as_deref(),
        is_binary,
        is_text,
        is_archive,
        is_media,
        is_script,
    );

    FileInfoClassification {
        mime_type,
        file_type,
        programming_language,
        is_binary,
        is_text,
        is_archive,
        is_media,
        is_source,
        is_script,
    }
}

pub fn detect_mime_type(
    path: &Path,
    bytes: &[u8],
    detected_format: FileFormat,
    programming_language: Option<&str>,
) -> String {
    if bytes.is_empty() {
        return "inode/x-empty".to_string();
    }

    if lower_extension(path).as_deref() == Some("json") {
        if let Some(is_binary) = json_binary_override(bytes) {
            if is_binary {
                return "application/octet-stream".to_string();
            }
            if has_valid_json_text(bytes) {
                return "application/json".to_string();
            }
            return "text/plain".to_string();
        }
        if has_valid_json_text(bytes) {
            return "application/json".to_string();
        }
        if has_decodable_text(bytes) && looks_like_textual_bytes(bytes) {
            return "text/plain".to_string();
        }
        return "application/octet-stream".to_string();
    }

    if is_zip_archive(bytes) {
        return detect_zip_like_mime(path);
    }

    if looks_like_deb(bytes, path) {
        return "application/vnd.debian.binary-package".to_string();
    }

    if looks_like_rpm(bytes, path) {
        return "application/x-rpm".to_string();
    }

    let guessed_mime = from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    let mime_type = match detected_format {
        FileFormat::Empty => "inode/x-empty".to_string(),
        FileFormat::PlainText => {
            if guessed_mime == "application/octet-stream" || guessed_mime.starts_with("video/") {
                "text/plain".to_string()
            } else {
                guessed_mime.clone()
            }
        }
        _ => {
            let detected_mime = detected_format.media_type();
            if detected_mime == "application/octet-stream"
                && guessed_mime != "application/octet-stream"
            {
                guessed_mime.clone()
            } else {
                detected_mime.to_string()
            }
        }
    };

    normalize_mime_type(path, bytes, programming_language, &mime_type)
}

pub(super) fn normalize_mime_type(
    path: &Path,
    bytes: &[u8],
    programming_language: Option<&str>,
    mime_type: &str,
) -> String {
    if should_prefer_text_mime(path, bytes, programming_language, mime_type) {
        return "text/plain".to_string();
    }

    mime_type.to_string()
}

fn should_prefer_text_mime(
    path: &Path,
    bytes: &[u8],
    programming_language: Option<&str>,
    mime_type: &str,
) -> bool {
    has_decodable_text(bytes)
        && looks_like_textual_bytes(bytes)
        && is_textual_source_candidate(path, programming_language)
        && (mime_type.starts_with("video/") || mime_type == "application/octet-stream")
}

fn has_valid_json_text(bytes: &[u8]) -> bool {
    if bytes.len() > JSON_VALIDATION_MAX_BYTES {
        return false;
    }

    serde_json::from_slice::<serde_json::Value>(bytes).is_ok()
        || super::encoding::decode_utf16_json_text(bytes)
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some()
}

fn is_wrapped_invalid_json_string_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0)
        && !bytes.contains(&0xFF)
        && bytes.starts_with(b"[\"")
        && bytes.ends_with(b"\"]")
        && bytes.len() >= 8
}

fn json_binary_override(bytes: &[u8]) -> Option<bool> {
    if has_valid_json_text(bytes) {
        return Some(false);
    }

    if bytes.contains(&0) {
        return Some(true);
    }

    if bytes.contains(&0xFF) && (bytes.len() <= 5 || bytes.len() > 1024) {
        return Some(true);
    }

    if is_wrapped_invalid_json_string_text(bytes) {
        return Some(false);
    }

    None
}

fn detect_is_binary(
    path: &Path,
    bytes: &[u8],
    detected_format: FileFormat,
    programming_language: Option<&str>,
) -> bool {
    if lower_extension(path).as_deref() == Some("json")
        && let Some(is_binary) = json_binary_override(bytes)
    {
        return is_binary;
    }

    if is_textual_format(detected_format) {
        return false;
    }

    if lower_extension(path)
        .as_deref()
        .is_some_and(|ext| BINARY_EXTENSIONS.contains(&ext))
    {
        return true;
    }

    if should_treat_binary_bytes_as_text(path, bytes, programming_language) {
        return false;
    }

    has_binary_control_chars(bytes)
        || is_known_binary_format(detected_format)
        || (matches!(detected_format, FileFormat::ArbitraryBinaryData)
            && !looks_like_textual_bytes(bytes))
}

fn should_treat_binary_bytes_as_text(
    path: &Path,
    bytes: &[u8],
    programming_language: Option<&str>,
) -> bool {
    has_decodable_text(bytes)
        && looks_like_textual_bytes(bytes)
        && (bytes.starts_with(b"#!") || is_textual_source_candidate(path, programming_language))
}

fn detect_is_archive(
    path: &Path,
    bytes: &[u8],
    mime_type: &str,
    is_text: bool,
    detected_format: FileFormat,
) -> bool {
    if is_text {
        return false;
    }

    lower_extension(path)
        .as_deref()
        .is_some_and(|ext| ARCHIVE_EXTENSIONS.contains(&ext))
        || matches!(
            detected_format.kind(),
            FileFormatKind::Archive | FileFormatKind::Compressed | FileFormatKind::Package
        )
        || is_zip_archive(bytes)
        || looks_like_gzip(bytes)
        || looks_like_bzip2(bytes)
        || looks_like_xz(bytes)
        || looks_like_deb(bytes, path)
        || looks_like_rpm(bytes, path)
        || looks_like_squashfs(bytes, path)
        || mime_type.contains("zip")
        || mime_type.contains("compressed")
        || mime_type.contains("tar")
        || mime_type.contains("x-rpm")
        || mime_type.contains("debian")
}

fn detect_is_media(
    path: &Path,
    bytes: &[u8],
    mime_type: &str,
    detected_format: FileFormat,
) -> bool {
    media_mime_from_content(bytes).is_some()
        || matches!(
            detected_format.kind(),
            FileFormatKind::Audio | FileFormatKind::Image | FileFormatKind::Video
        )
        || mime_type.starts_with("image/")
        || mime_type.starts_with("audio/")
        || mime_type.starts_with("video/")
        || (mime_type == "application/octet-stream"
            && lower_extension(path).as_deref() == Some("tga")
            && !has_binary_control_chars(bytes))
}

fn detect_is_script(
    path: &Path,
    bytes: &[u8],
    programming_language: Option<&str>,
    is_text: bool,
) -> bool {
    if !is_text || is_makefile(path) {
        return false;
    }

    bytes.starts_with(b"#!")
        || lower_extension(path).as_deref().is_some_and(|ext| {
            matches!(
                ext,
                "sh" | "bash" | "zsh" | "fish" | "ksh" | "ps1" | "psm1" | "psd1" | "awk"
            )
        })
        || matches!(
            programming_language,
            Some(
                "Shell"
                    | "Bash"
                    | "Zsh"
                    | "Fish"
                    | "Ksh"
                    | "Python"
                    | "Ruby"
                    | "Perl"
                    | "PHP"
                    | "PowerShell"
                    | "Awk"
            )
        )
}

fn detect_is_source(
    path: &Path,
    programming_language: Option<&str>,
    is_text: bool,
    is_script: bool,
) -> bool {
    if !is_text || is_plain_text(path) || is_makefile(path) || is_source_map(path) {
        return false;
    }

    if is_c_like_source(path) || is_java_like_source(path) {
        return true;
    }

    programming_language.is_some() || is_script
}

// The arguments are individually computed classification inputs threaded down
// from `classify_file_info`. Bundling them into a struct would only couple the
// two functions more tightly without improving clarity; the flag pipeline is
// intentionally kept flat here.
#[allow(clippy::too_many_arguments)]
fn detect_file_type(
    path: &Path,
    bytes: &[u8],
    detected_format: FileFormat,
    mime_type: &str,
    programming_language: Option<&str>,
    is_binary: bool,
    is_text: bool,
    is_archive: bool,
    is_media: bool,
    is_script: bool,
) -> String {
    if bytes.is_empty() {
        return "empty".to_string();
    }

    if looks_like_pdf(bytes) {
        return "PDF document".to_string();
    }

    if let Some(file_type) = media_file_type_from_content(bytes) {
        return file_type.to_string();
    }

    if is_archive {
        return archive_file_type(path, bytes, detected_format);
    }

    if is_script {
        return script_file_type(programming_language, bytes);
    }

    if is_text {
        if lower_extension(path).as_deref() == Some("json") {
            if has_valid_json_text(bytes) {
                return "JSON text data".to_string();
            }
            return text_file_type(bytes);
        }
        if lower_extension(path).as_deref() == Some("xml") {
            return "XML text data".to_string();
        }
        if matches!(lower_extension(path).as_deref(), Some("yaml" | "yml")) {
            return "YAML text data".to_string();
        }
        if lower_extension(path).as_deref() == Some("toml") {
            return "TOML text data".to_string();
        }
        if matches!(
            lower_extension(path).as_deref(),
            Some("ini" | "cfg" | "conf")
        ) {
            return "INI text data".to_string();
        }
        if matches!(lower_file_name(path).as_str(), ".gitmodules" | ".gitconfig") {
            return "Git configuration text".to_string();
        }
        if matches!(lower_extension(path).as_deref(), Some("md" | "markdown")) {
            return text_file_type(bytes);
        }
        if programming_language.is_some() && !is_media {
            return source_file_type(programming_language, bytes);
        }
        return text_file_type(bytes);
    }

    if let Some(file_type) = format_based_file_type(detected_format) {
        return file_type;
    }

    if is_binary && mime_type == "application/octet-stream" {
        return "data".to_string();
    }

    mime_type.to_string()
}

fn is_textual_source_candidate(path: &Path, programming_language: Option<&str>) -> bool {
    if matches!(programming_language, Some(language) if is_source_like_language(language)) {
        return true;
    }

    if matches!(
        lower_file_name(path).as_str(),
        "dockerfile"
            | "containerfile"
            | "containerfile.core"
            | "apkbuild"
            | "podfile"
            | "jamfile"
            | "jamroot"
            | "meson.build"
            | "build"
            | "workspace"
            | "buck"
            | "default.nix"
            | "flake.nix"
            | "shell.nix"
    ) {
        return true;
    }

    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "rs" | "py"
                    | "js"
                    | "mjs"
                    | "cjs"
                    | "jsx"
                    | "ts"
                    | "mts"
                    | "cts"
                    | "tsx"
                    | "c"
                    | "cpp"
                    | "cc"
                    | "cxx"
                    | "h"
                    | "hpp"
                    | "m"
                    | "mm"
                    | "s"
                    | "asm"
                    | "java"
                    | "go"
                    | "rb"
                    | "php"
                    | "pl"
                    | "swift"
                    | "sh"
                    | "bash"
                    | "zsh"
                    | "fish"
                    | "ksh"
                    | "ps1"
                    | "psm1"
                    | "psd1"
                    | "awk"
                    | "kt"
                    | "kts"
                    | "dart"
                    | "scala"
                    | "groovy"
                    | "gradle"
                    | "gvy"
                    | "gy"
                    | "gsh"
                    | "cs"
                    | "fs"
                    | "fsx"
                    | "r"
                    | "lua"
                    | "jl"
                    | "ex"
                    | "exs"
                    | "clj"
                    | "cljs"
                    | "cljc"
                    | "hs"
                    | "erl"
                    | "nix"
                    | "zig"
                    | "bzl"
                    | "bazel"
                    | "star"
                    | "sky"
                    | "ml"
                    | "mli"
                    | "tex"
            )
        })
}

fn is_source_like_language(language: &str) -> bool {
    matches!(
        language,
        "Rust"
            | "Python"
            | "JavaScript"
            | "TypeScript"
            | "JavaScript/TypeScript"
            | "C"
            | "C++"
            | "Objective-C"
            | "Objective-C++"
            | "GAS"
            | "Java"
            | "Go"
            | "Ruby"
            | "PHP"
            | "Perl"
            | "Swift"
            | "Shell"
            | "PowerShell"
            | "Awk"
            | "Kotlin"
            | "Dart"
            | "Scala"
            | "C#"
            | "F#"
            | "R"
            | "Lua"
            | "Julia"
            | "Elixir"
            | "Clojure"
            | "Haskell"
            | "Erlang"
            | "Groovy"
            | "Nix"
            | "Zig"
            | "Starlark"
            | "OCaml"
            | "Meson"
            | "TeX"
            | "Dockerfile"
            | "Makefile"
            | "Jamfile"
    )
}

fn detect_zip_like_mime(path: &Path) -> String {
    match extension(path).map(|ext| ext.to_ascii_lowercase()) {
        Some(ext) if ext == "apk" => "application/vnd.android.package-archive".to_string(),
        Some(ext) if matches!(ext.as_str(), "jar" | "war" | "ear") => {
            "application/java-archive".to_string()
        }
        _ => "application/zip".to_string(),
    }
}

fn archive_file_type(path: &Path, bytes: &[u8], detected_format: FileFormat) -> String {
    if looks_like_deb(bytes, path) {
        "debian binary package (format 2.0)".to_string()
    } else if looks_like_rpm(bytes, path) {
        "RPM package".to_string()
    } else if looks_like_squashfs(bytes, path) {
        "Squashfs filesystem".to_string()
    } else if looks_like_gzip(bytes) {
        "gzip compressed data".to_string()
    } else if looks_like_bzip2(bytes) {
        "bzip2 compressed data".to_string()
    } else if looks_like_xz(bytes) {
        "XZ compressed data".to_string()
    } else if is_zip_archive(bytes) {
        "Zip archive data".to_string()
    } else if lower_extension(path).as_deref() == Some("gem") {
        "POSIX tar archive".to_string()
    } else if let Some(file_type) = format_based_file_type(detected_format) {
        file_type
    } else {
        "archive data".to_string()
    }
}

fn script_file_type(programming_language: Option<&str>, bytes: &[u8]) -> String {
    let suffix = text_executable_label(bytes);

    match programming_language {
        Some("Python") => format!("python script, {suffix}"),
        Some("Ruby") => format!("ruby script, {suffix}"),
        Some("Perl") => format!("perl script, {suffix}"),
        Some("PHP") => format!("php script, {suffix}"),
        Some("Shell") => format!("shell script, {suffix}"),
        Some("Bash") => format!("bash script, {suffix}"),
        Some("Zsh") => format!("zsh script, {suffix}"),
        Some("Fish") => format!("fish script, {suffix}"),
        Some("Ksh") => format!("ksh script, {suffix}"),
        Some("JavaScript") => format!("javascript script, {suffix}"),
        Some("TypeScript") => format!("typescript script, {suffix}"),
        Some("PowerShell") => format!("powershell script, {suffix}"),
        Some("Awk") => format!("awk script, {suffix}"),
        _ => format!("script, {suffix}"),
    }
}

fn source_file_type(programming_language: Option<&str>, bytes: &[u8]) -> String {
    let suffix = text_label(bytes);
    match programming_language {
        Some("C") => format!("C source, {suffix}"),
        Some("C++") => format!("C++ source, {suffix}"),
        Some("Java") => format!("Java source, {suffix}"),
        Some("C#") => format!("C# source, {suffix}"),
        Some("F#") => format!("F# source, {suffix}"),
        Some("Go") => format!("Go source, {suffix}"),
        Some("Rust") => format!("Rust source, {suffix}"),
        Some("Starlark") => format!("Starlark source, {suffix}"),
        Some("CMake") => format!("CMake source, {suffix}"),
        Some("Meson") => format!("Meson source, {suffix}"),
        Some("Nix") => format!("Nix source, {suffix}"),
        Some("Groovy") => format!("Groovy source, {suffix}"),
        Some("Makefile") => format!("Makefile source, {suffix}"),
        Some("Dockerfile") => format!("Dockerfile source, {suffix}"),
        Some("Jamfile") => format!("Jamfile source, {suffix}"),
        Some("Batchfile") => format!("Batchfile source, {suffix}"),
        Some(language) => format!("{language} source, {suffix}"),
        None => text_file_type(bytes),
    }
}

fn text_file_type(bytes: &[u8]) -> String {
    text_label(bytes).to_string()
}

fn text_label(bytes: &[u8]) -> &'static str {
    if std::str::from_utf8(bytes).is_ok() {
        if bytes.contains(&b'\n') {
            "UTF-8 Unicode text"
        } else {
            "UTF-8 Unicode text, with no line terminators"
        }
    } else if bytes.contains(&b'\n') {
        "text"
    } else {
        "text, with no line terminators"
    }
}

fn text_executable_label(bytes: &[u8]) -> &'static str {
    if std::str::from_utf8(bytes).is_ok() {
        if bytes.contains(&b'\n') {
            "UTF-8 Unicode text executable"
        } else {
            "UTF-8 Unicode text executable, with no line terminators"
        }
    } else if bytes.contains(&b'\n') {
        "text executable"
    } else {
        "text executable, with no line terminators"
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::normalize_mime_type;

    #[test]
    fn test_normalize_mime_type_prefers_text_for_textual_video_guess() {
        assert_eq!(
            normalize_mime_type(
                Path::new("main.ts"),
                b"export const answer = 42;\n",
                Some("TypeScript"),
                "video/mp2t",
            ),
            "text/plain"
        );
    }

    #[test]
    fn test_normalize_mime_type_prefers_text_for_octet_stream_source_guess() {
        assert_eq!(
            normalize_mime_type(
                Path::new("main.js"),
                b"console.log('hello');\n",
                Some("JavaScript"),
                "application/octet-stream",
            ),
            "text/plain"
        );
    }

    #[test]
    fn test_normalize_mime_type_preserves_binary_video_guess() {
        assert_eq!(
            normalize_mime_type(
                Path::new("main.ts"),
                &[0, 159, 146, 150, 0, 1, 2, 3],
                Some("TypeScript"),
                "video/mp2t",
            ),
            "video/mp2t"
        );
    }

    #[test]
    fn test_normalize_mime_type_preserves_short_binary_octet_stream_guess() {
        assert_eq!(
            normalize_mime_type(
                Path::new("main.ts"),
                &[0, 159, 146, 150],
                Some("TypeScript"),
                "application/octet-stream",
            ),
            "application/octet-stream"
        );
    }
}
