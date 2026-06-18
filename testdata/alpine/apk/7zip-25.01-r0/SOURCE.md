# Source

`7zip-25.01-r0.apk` is the upstream Alpine Linux binary package, archived here as a
benchmark target fixture:

- Upstream: `https://dl-cdn.alpinelinux.org/alpine/v3.23/main/x86_64/7zip-25.01-r0.apk`
- Version: `7zip 25.01-r0` (Alpine v3.23 `main`)
- Declared license (from the package `.PKGINFO`): **LGPL-2.0-only** (redistributable)
- sha256: `6602ccb86033f4132f7c20e7a551908e14631d6d40b00fb3e0e00ae8914ab405`

The bytes are committed because Alpine's `main` repository keeps only the current
build per branch: when a newer 7zip revision ships, the exact `25.01-r0` `.apk` is
pruned from `v3.23/main`, so the upstream URL is not durable. This is the artifact
measured by the `7zip 25.01-r0 .apk` row in [`docs/BENCHMARKS.md`](../../../../docs/BENCHMARKS.md).
Included solely as a test/benchmark fixture. See [`../../../PROVENANCE.md`](../../../PROVENANCE.md).
