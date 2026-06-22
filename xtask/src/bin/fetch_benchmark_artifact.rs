// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Fetch a benchmark artifact into the gitignored local cache, verified by sha256.
//!
//! Benchmark rows for downloadable package artifacts (`.apk`, `.deb`, `.xpi`,
//! `.pkg.tar.zst`, release snapshots, …) are reproduced from their durable upstream
//! sources rather than by committing binaries. This tool reads a committed manifest
//! (`xtask/benchmark-artifacts.json`), downloads each artifact from its pinned URL,
//! verifies the sha256, runs its preparation recipe, and prints the prepared path to
//! feed into `compare-outputs --target-path`.
//!
//! ```sh
//! cargo run --manifest-path xtask/Cargo.toml --bin fetch-benchmark-artifact -- \
//!   --id python-construct-pkginfo --print-path
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Parser;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Parser)]
#[command(about = "Fetch and prepare a benchmark artifact from its pinned upstream source")]
struct Args {
    /// Artifact id from the manifest.
    #[arg(long, conflicts_with = "all")]
    id: Option<String>,
    /// Fetch and prepare every artifact in the manifest.
    #[arg(long)]
    all: bool,
    /// Manifest path.
    #[arg(long, default_value = "xtask/benchmark-artifacts.json")]
    manifest: PathBuf,
    /// Print only the prepared scan path (for piping into compare-outputs --target-path).
    #[arg(long)]
    print_path: bool,
    /// Re-download even if a verified copy is already cached.
    #[arg(long)]
    force: bool,
}

#[derive(Deserialize)]
struct Manifest {
    artifacts: Vec<Artifact>,
}

#[derive(Deserialize)]
struct Artifact {
    id: String,
    /// Human-readable description (mirrors the BENCHMARKS.md row label).
    #[allow(dead_code)]
    description: String,
    /// Primary download URL. Mutually exclusive with `urls`.
    url: Option<String>,
    /// Override the cached download filename when the URL basename is opaque
    /// (e.g. a content-addressed snapshot hash) and the parser needs a real
    /// extension to match (`.deb`, …).
    filename: Option<String>,
    /// Expected sha256 of the primary download.
    sha256: Option<String>,
    prep: Prep,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Prep {
    /// The downloaded file is itself the scan target (`.apk`, `.deb`, `.xpi`).
    Raw,
    /// Extract a single member from a tar archive (gz/zst auto-detected).
    TarMember { member: String },
    /// Extract the whole tar archive into a directory.
    TarAll,
    /// Unzip the whole archive into a directory.
    ZipAll,
    /// Extract only the listed members (curated "release snapshot" subset). Archive
    /// type is detected from the download extension (`.zip` → unzip, else tar).
    SelectMembers { members: Vec<String> },
    /// Pull a container image (may be digest-pinned) and extract the listed rootfs
    /// paths into a directory (e.g. `var/lib/dpkg` for an installed-package database).
    DockerExport { image: String, paths: Vec<String> },
}

fn main() -> Result<()> {
    let args = Args::parse();
    let manifest: Manifest = serde_json::from_str(
        &fs::read_to_string(&args.manifest)
            .with_context(|| format!("reading manifest {}", args.manifest.display()))?,
    )
    .context("parsing benchmark-artifacts manifest")?;

    let selected: Vec<&Artifact> = if args.all {
        manifest.artifacts.iter().collect()
    } else {
        let id = args.id.as_deref().context("specify --id <id> or --all")?;
        vec![
            manifest
                .artifacts
                .iter()
                .find(|artifact| artifact.id == id)
                .with_context(|| format!("no artifact with id `{id}` in manifest"))?,
        ]
    };

    for artifact in selected {
        let target = prepare(artifact, args.force)?;
        if args.print_path {
            println!("{}", target.display());
        } else {
            eprintln!("[{}] prepared at {}", artifact.id, target.display());
        }
    }

    Ok(())
}

fn cache_dir(id: &str) -> PathBuf {
    PathBuf::from(".provenant/benchmark-artifacts").join(id)
}

fn prepare(artifact: &Artifact, force: bool) -> Result<PathBuf> {
    let dir = cache_dir(&artifact.id);
    fs::create_dir_all(&dir).with_context(|| format!("creating cache dir {}", dir.display()))?;

    if let Prep::DockerExport { image, paths } = &artifact.prep {
        let out = dir.join("rootfs");
        if !force && out.exists() {
            return Ok(out);
        }
        let _ = fs::remove_dir_all(&out);
        fs::create_dir_all(&out)?;
        let tar = dir.join("rootfs.tar");
        // `docker create` resolves the (possibly digest-pinned) image, pulling if needed.
        let create = Command::new("docker")
            .args(["create", image])
            .output()
            .with_context(|| format!("docker create {image}"))?;
        if !create.status.success() {
            bail!(
                "docker create {image} failed: {}",
                String::from_utf8_lossy(&create.stderr).trim()
            );
        }
        let cid = String::from_utf8(create.stdout)?.trim().to_string();
        if cid.is_empty() {
            bail!(
                "docker create {image} produced no container id (is the image/digest available?)"
            );
        }
        let export = run(Command::new("docker")
            .arg("export")
            .arg("-o")
            .arg(&tar)
            .arg(&cid));
        let _ = Command::new("docker").args(["rm", &cid]).status();
        export?;
        run(Command::new("tar")
            .arg("-xf")
            .arg(&tar)
            .arg("-C")
            .arg(&out)
            .args(paths))?;
        return Ok(out);
    }

    let url = artifact.url.as_deref().context("artifact requires `url`")?;
    let download_name = artifact.filename.as_deref().unwrap_or_else(|| {
        url.rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("download")
    });
    let download_path = dir.join(download_name);

    if force || !download_path.exists() {
        download(url, &download_path)?;
    }

    let digest = sha256_hex(&download_path)?;
    match &artifact.sha256 {
        Some(expected) if expected.eq_ignore_ascii_case(&digest) => {}
        Some(expected) => bail!(
            "sha256 mismatch for {}: manifest {expected}, downloaded {digest}",
            artifact.id
        ),
        None => eprintln!(
            "[{}] no sha256 pinned in manifest; downloaded sha256 = {digest} (add it to the manifest)",
            artifact.id
        ),
    }

    match &artifact.prep {
        Prep::Raw => Ok(download_path),
        Prep::TarMember { member } => {
            let out = dir.join("extracted");
            fs::create_dir_all(&out)?;
            run(Command::new("tar")
                .arg("-xf")
                .arg(&download_path)
                .arg("-C")
                .arg(&out)
                .arg(member))?;
            Ok(out.join(member))
        }
        Prep::TarAll => {
            let out = dir.join("extracted");
            let _ = fs::remove_dir_all(&out);
            fs::create_dir_all(&out)?;
            run(Command::new("tar")
                .arg("-xf")
                .arg(&download_path)
                .arg("-C")
                .arg(&out))?;
            Ok(out)
        }
        Prep::ZipAll => {
            let out = dir.join("extracted");
            let _ = fs::remove_dir_all(&out);
            fs::create_dir_all(&out)?;
            run(Command::new("unzip")
                .arg("-oq")
                .arg(&download_path)
                .arg("-d")
                .arg(&out))?;
            Ok(out)
        }
        Prep::SelectMembers { members } => {
            let out = dir.join("extracted");
            let _ = fs::remove_dir_all(&out);
            fs::create_dir_all(&out)?;
            let is_zip = download_name.ends_with(".zip") || download_name.ends_with(".xpi");
            if is_zip {
                run(Command::new("unzip")
                    .arg("-oq")
                    .arg(&download_path)
                    .args(members)
                    .arg("-d")
                    .arg(&out))?;
            } else {
                run(Command::new("tar")
                    .arg("-xf")
                    .arg(&download_path)
                    .arg("-C")
                    .arg(&out)
                    .args(members))?;
            }
            Ok(out)
        }
        Prep::DockerExport { .. } => unreachable!("handled above"),
    }
}

fn download(url: &str, dest: &Path) -> Result<()> {
    eprintln!("downloading {url}");
    run(Command::new("curl")
        .arg("--fail")
        .arg("--location")
        .arg("--silent")
        .arg("--show-error")
        .arg("--output")
        .arg(dest)
        .arg(url))
    .with_context(|| format!("downloading {url}"))
}

fn run(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("spawning {command:?}"))?;
    if !status.success() {
        bail!("command {command:?} failed with {status}");
    }
    Ok(())
}

fn sha256_hex(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex_encode(&hasher.finalize()))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
