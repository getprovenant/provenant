# Source

`+COMPACT_MANIFEST` is the FreeBSD pkg compact manifest extracted from the upstream
binary package, archived here as a benchmark target fixture:

- Upstream package: `https://pkg.freebsd.org/FreeBSD:14:amd64/latest/All/Hashed/bash-5.3.15~a16f84ceed.pkg`
  (`.pkg` sha256 `a16f84ceed90dc9e8186482cdb20888735109020897464df899b744430715062`)
- Version: `bash 5.3.15` (FreeBSD `FreeBSD:14:amd64` `latest`)
- Declared license (from the manifest `licenses`): **GPLv3+**
- Extracted with: `tar -xf bash-5.3.15~a16f84ceed.pkg +COMPACT_MANIFEST`
- `+COMPACT_MANIFEST` sha256: `88cea96a283ac83febc35fc3254e69dec49c846a1634852b25f4e36f1ea9851e`

The bytes are committed because the FreeBSD `latest` repository keeps only the current
build per branch, so the upstream `.pkg` URL is not durable. This is the single-file
input measured by the `FreeBSD bash 5.3.15 +COMPACT_MANIFEST` row in
[`docs/BENCHMARKS.md`](../../../docs/BENCHMARKS.md). Included solely as a test/benchmark
fixture. See [`../../PROVENANCE.md`](../../PROVENANCE.md).
