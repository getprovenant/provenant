# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0
#
# Minimal runtime image that packages a pre-built provenant release binary.
#
# Built by .github/workflows/release.yml from the linux release artifacts, so the
# published image ships the exact same binary as the GitHub release — no in-container
# recompile, and multi-arch (amd64/arm64) needs no QEMU emulation (COPY only).
#
# The binary is fully static: SQLite is statically linked (rusqlite "bundled"), TLS is
# rustls (no OpenSSL), libc is statically linked musl, and the license index is embedded.
# So distroless/static — no libc, just CA certificates and /etc scaffolding — is all it
# needs at runtime.
#
# To build locally, stage a statically-linked binary for your arch first, e.g.:
#   cargo build --release --target aarch64-unknown-linux-musl
#   mkdir -p dist/arm64 && cp target/aarch64-unknown-linux-musl/release/provenant dist/arm64/
#   docker build --build-arg TARGETARCH=arm64 -t provenant .
FROM gcr.io/distroless/static-debian12@sha256:22fd79fd75eab2372585b44517f8a094349938919dc613aafc37e4bdc9967c82

# TARGETARCH is set automatically by BuildKit per target platform (amd64, arm64).
ARG TARGETARCH
COPY dist/${TARGETARCH}/provenant /usr/local/bin/provenant

ENTRYPOINT ["/usr/local/bin/provenant"]
