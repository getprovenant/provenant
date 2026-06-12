// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use md5::{Digest as Md5Digest, Md5};
use sha1::Sha1;
use sha2::Sha256;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use crate::models::{GitSha1, Md5Digest as Md5DigestType, Sha1Digest, Sha256Digest};

pub fn calculate_sha1(content: &[u8]) -> Sha1Digest {
    let digest = Sha1::digest(content);
    Sha1Digest::from_bytes(digest.into())
}

pub fn calculate_md5(content: &[u8]) -> Md5DigestType {
    let digest = Md5::digest(content);
    Md5DigestType::from_bytes(digest.into())
}

pub fn calculate_sha256(content: &[u8]) -> Sha256Digest {
    let digest = Sha256::digest(content);
    Sha256Digest::from_bytes(digest.into())
}

pub fn calculate_sha1_git(content: &[u8]) -> GitSha1 {
    let mut payload = Vec::with_capacity(content.len() + 32);
    payload.extend_from_slice(format!("blob {}\0", content.len()).as_bytes());
    payload.extend_from_slice(content);
    let digest = Sha1::digest(&payload);
    GitSha1::from_bytes(digest.into())
}

/// Computes sha1, md5, sha256 and git-sha1 for an in-memory buffer in a single
/// chunked pass that updates every hasher per chunk, improving cache locality
/// over four independent full passes over the same bytes.
pub fn calculate_buffer_hashes(
    content: &[u8],
) -> (Sha1Digest, Md5DigestType, Sha256Digest, GitSha1) {
    let mut sha1 = Sha1::new();
    let mut md5 = Md5::new();
    let mut sha256 = Sha256::new();
    let mut git_sha1 = Sha1::new();

    git_sha1.update(format!("blob {}\0", content.len()).as_bytes());

    const CHUNK: usize = 64 * 1024;
    for chunk in content.chunks(CHUNK) {
        sha1.update(chunk);
        md5.update(chunk);
        sha256.update(chunk);
        git_sha1.update(chunk);
    }

    (
        Sha1Digest::from_bytes(sha1.finalize().into()),
        Md5DigestType::from_bytes(md5.finalize().into()),
        Sha256Digest::from_bytes(sha256.finalize().into()),
        GitSha1::from_bytes(git_sha1.finalize().into()),
    )
}

pub fn calculate_file_hashes(
    path: &Path,
    size: u64,
) -> io::Result<(Sha1Digest, Md5DigestType, Sha256Digest, GitSha1)> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut sha1 = Sha1::new();
    let mut md5 = Md5::new();
    let mut sha256 = Sha256::new();
    let mut git_sha1 = Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];

    git_sha1.update(format!("blob {}\0", size).as_bytes());

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        let chunk = &buffer[..read];
        sha1.update(chunk);
        md5.update(chunk);
        sha256.update(chunk);
        git_sha1.update(chunk);
    }

    Ok((
        Sha1Digest::from_bytes(sha1.finalize().into()),
        Md5DigestType::from_bytes(md5.finalize().into()),
        Sha256Digest::from_bytes(sha256.finalize().into()),
        GitSha1::from_bytes(git_sha1.finalize().into()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the fused buffer pass to the individual single-hash helpers so a
    /// future change to either path cannot silently diverge the `--info` output.
    #[test]
    fn fused_buffer_hashes_match_individual_helpers() {
        for content in [
            b"".as_slice(),
            b"hello world\n".as_slice(),
            &vec![0xABu8; 200 * 1024],
        ] {
            let (sha1, md5, sha256, sha1_git) = calculate_buffer_hashes(content);
            assert_eq!(sha1, calculate_sha1(content));
            assert_eq!(md5, calculate_md5(content));
            assert_eq!(sha256, calculate_sha256(content));
            assert_eq!(sha1_git, calculate_sha1_git(content));
        }
    }
}
