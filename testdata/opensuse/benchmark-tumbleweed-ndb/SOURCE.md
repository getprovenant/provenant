# Source

`var/lib/rpm/Packages.db` and `var/lib/rpm/Index.db` are the openSUSE RPM NDB
installed-package database, extracted from the openSUSE Tumbleweed container image
and archived here as a benchmark target fixture:

- Upstream image: `registry.opensuse.org/opensuse/tumbleweed`
  (multi-arch index digest `sha256:43b66646878b863e310e01eed91b00d6846597dc5de5724b1b1480590b747c80`,
  linux/amd64 manifest `sha256:8bc70ff82fd157a95162317f99eb7d6c85c9fa6ff01a73cb29c0036379904f8e`)
- Pulled on 2026-06-19 with:
  `skopeo copy --override-os linux --override-arch amd64 docker://registry.opensuse.org/opensuse/tumbleweed@sha256:43b66646878b863e310e01eed91b00d6846597dc5de5724b1b1480590b747c80 oci:tumbleweed:latest`
- Extracted the rootfs layers and copied `var/lib/rpm/Packages.db` plus `var/lib/rpm/Index.db`
- `Packages.db` sha256: `224594d00a53523d96115828a1aa915375db976987dadf43b34546ae29d5a9d2`
- `Index.db` sha256: `ddfaabd3104fb83bfb555f41d0ab4ba9ad8c22c44eeb6a405a4fb60884cc3476`

These two files are the NDB-format RPM database (note the `RpmP` magic) measured by
the `openSUSE Tumbleweed rpmdb NDB snapshot` row in
[`docs/BENCHMARKS.md`](../../../docs/BENCHMARKS.md). The bytes are committed because the
`tumbleweed` rolling tag is mutable and the original image is pruned over time, so the
upstream digest is not durably pullable. The database records package metadata
(names, versions, licenses, dependencies) for the permissively-licensed openSUSE base
system; it is committed here only as a test/benchmark fixture. See
[`../../PROVENANCE.md`](../../PROVENANCE.md).
