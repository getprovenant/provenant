# SPDX-FileCopyrightText: Provenant contributors
# SPDX-License-Identifier: Apache-2.0
#
# Minimal runtime image that packages a pre-built provenant release binary.
#
# Built by .github/workflows/release.yml from the linux release artifacts, so the
# published image ships the exact same binary as the GitHub release — no in-container
# recompile, and multi-arch (amd64/arm64) needs no QEMU emulation (COPY only).
#
# The binary is self-contained: SQLite is statically linked (rusqlite "bundled"), TLS is
# rustls (no OpenSSL), and the license index is embedded. So distroless/cc — glibc,
# libgcc, and CA certificates — is all it needs at runtime.
#
# To build locally, stage the binary for your arch first, e.g.:
#   cargo build --release
#   mkdir -p dist/amd64 && cp target/release/provenant dist/amd64/
#   docker build --build-arg TARGETARCH=amd64 -t provenant .
FROM gcr.io/distroless/cc-debian12@sha256:a90cf0f046efb32466b38b0972fef3a95e7c580e392e79ff1b7ac08c15fed0bc

# TARGETARCH is set automatically by BuildKit per target platform (amd64, arm64).
ARG TARGETARCH
COPY dist/${TARGETARCH}/provenant /usr/local/bin/provenant

ENTRYPOINT ["/usr/local/bin/provenant"]
