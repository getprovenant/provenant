# Source

`installed` is the Alpine APK installed-package database (`lib/apk/db/installed`)
extracted from the Alpine 3.23.3 minirootfs, archived here as a benchmark target
fixture:

- Upstream tarball: `https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-minirootfs-3.23.3-x86_64.tar.gz`
  (tarball sha256 `42d0e6d8de5521e7bf92e075e032b5690c1d948fa9775efa32a51a38b25460fb`)
- Extracted with: `tar -xzf alpine-minirootfs-3.23.3-x86_64.tar.gz lib/apk/db/installed`
- `installed` sha256: `fc396ebdcc1666c9277fb01a5ac513920cfcd2f02cadd1c5f676ec8ea6fe155c`

This is the single-file input measured by the `Alpine 3.23.3 installed DB snapshot`
row in [`docs/BENCHMARKS.md`](../../../docs/BENCHMARKS.md). The Alpine APK database
records package metadata (names, versions, licenses, maintainers) for the
permissively-licensed Alpine base system; it is committed here only as a test/benchmark
fixture. See [`../../PROVENANCE.md`](../../PROVENANCE.md).
