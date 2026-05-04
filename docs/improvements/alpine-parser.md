# Alpine Parser: Beyond-Parity Improvements

## Summary

Provenant's Alpine parser improves on the Python reference in six concrete ways:

1. **🐛 Bug Fix**: SHA1 checksums correctly decoded (Python always returns `null`)
2. **✨ New Feature**: Provider field extraction (Python explicitly ignores this field)
3. **✨ New Feature**: Static APKBUILD recipe parsing
4. **🐛 Bug Fix**: Alpine commit metadata now produces `git+https://...` VCS URLs
5. **🐛 Bug Fix**: Packages with no files are still detected and retained
6. **🔍 Enhanced**: APKBUILD dependency families and non-APKBUILD license normalization now match Alpine package semantics more closely

## Improvement 1: SHA1 Checksum Decoding (Bug Fix)

### Python Implementation (Broken)

**Code** (line 211):

```python
def get_checksums(checksum_field):
    """
    Return a mapping of checksums from an Alpine `checksum_field` value string.

    For example:
        Q1/xzW3F4RLfZtPxzivPuWWYTt3A=
    >>> get_checksums('Q1/xzW3F4RLfZtPxzivPuWWYTt3A=')
    {'sha1': None}  # Always returns None!
    """
    # FIXME: the checksum is base64-encoded, needs decoding
    return dict(sha1=checksum_field or None)
```

**Problem**: The checksum is Q1-prefixed base64-encoded data that Python never decodes, always returning `null`.

### Our Rust Implementation (Fixed)

**Code**:

```rust
fn decode_checksum(checksum: &str) -> Option<String> {
    // Format: Q1<base64-encoded SHA1>
    // Example: "Q1/xzW3F4RLfZtPxzivPuWWYTt3A=" → "435ff1cd6dc5e112df66d3f1ce2bcfb965984eddc0"

    if !checksum.starts_with("Q1") {
        return None;
    }

    // Decode base64 (skip "Q1" prefix)
    let decoded = general_purpose::STANDARD.decode(&checksum[2..]).ok()?;

    // Convert to hex string
    Some(hex::encode(decoded))
}
```

### Coverage

Coverage verifies that installed-database file references retain decoded SHA1 checksums instead of collapsing to missing hash data.

### Impact

**Critical for**: Package integrity verification, vulnerability scanning, SBOM accuracy

**Before (Python)**:

```json
{
  "file_references": [{ "path": "sbin/apk", "sha1": null }]
}
```

**After (Rust)**:

```json
{
  "file_references": [{ "path": "sbin/apk", "sha1": "435ff1cd6dc5e112df66d3f1ce2bcfb965984eddc0" }]
}
```

## Improvement 2: Provider Field Extraction (New Feature)

### Python Implementation (TODO)

**Code** (line 87-90):

```python
# Ignored per-package fields that are documented but not used yet
# p: provider_name - provides this command or library
#     e.g., p:cmd:busybox p:/bin/sh so:libcrypto.so.1.1
```

**Comment**: Python explicitly documents but ignores the `p:` provider field.

### Our Rust Implementation (Complete)

**Code**:

```rust
fn extract_providers(line: &str) -> Vec<String> {
    // Provider field format: p:cmd:busybox p:/bin/sh so:libcrypto.so.1.1
    // Multiple providers space-separated

    if let Some(providers_str) = line.strip_prefix("p:") {
        providers_str
            .split_whitespace()
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![]
    }
}
```

**Storage**: Stored in `extra_data.providers` array

```json
{
  "extra_data": {
    "providers": ["cmd:busybox", "cmd:sh", "/bin/sh"]
  }
}
```

### Coverage

Coverage verifies that provider metadata is preserved as structured package data rather than being silently ignored.

### Impact

**Useful for**:

- Understanding what commands/libraries a package provides
- Resolving virtual package dependencies
- Conflict detection (multiple packages providing same command)
- Package replacement analysis

### Why Python Doesn't Extract This

**Quote from Python source** (line 88):

> "Ignored per-package fields that are documented but not used yet"

**Our Decision**: Implement it anyway. It's documented in Alpine's format spec, and the data is valuable for SBOM completeness.

## Improvement 3: APKBUILD Recipe Parsing

### Python Reference Status

The Python reference already parses `APKBUILD`, but Provenant extends that support with broader static dependency-family extraction and more consistent declared-license handling across Alpine inputs.

### Our Rust Implementation

Rust now parses checked-in `APKBUILD` recipes statically, without executing shell code.

Implemented coverage includes:

- `pkgname`, `pkgver`, `pkgrel` → package identity and `pkgver-rpkgrel`
- `pkgdesc`
- `url`
- `license`
- `depends`, `depends_dev`, `makedepends`, `makedepends_build`, `makedepends_host`, `checkdepends`
- `source`
- `sha512sums`, `sha256sums`, `md5sums`
- variable expansion for the upstream fixture forms we need now:
  - `${pkgver//./-}`
  - `${pkgver//./_}`
  - `${var::8}`

### Why This Matters

This keeps the parser inside the project's security-first boundary: it still does **not** execute shell or evaluate arbitrary shell functions.

Rust now also emits APKBUILD dependency families as structured dependencies:

- `depends` → runtime package dependencies
- `makedepends`, `makedepends_build`, `makedepends_host` → build-time dependencies
- `checkdepends` → test/check dependencies
- `depends_dev` → development-subpackage runtime dependencies, preserved under their own native scope

## Improvement 4: HTTPS VCS URL Generation

Python does not consistently preserve HTTPS commit-style Alpine VCS URLs from installed-db metadata, while Rust emits them directly.

### Before

- no `vcs_url` from Alpine installed-db commit field

### After

- `c:<commit>` now becomes:
  - `git+https://git.alpinelinux.org/aports/commit/?id={commit}`

## Improvement 5: Fileless Package Retention

Rust preserves packages with no file references as packages rather than dropping them.

This matters for packages like `libc-utils` and for APKBUILD “dummy package” patterns such as `linux-firmware`’s `none()` subpackage.

## Improvement 6: Dependency and license parity across Alpine surfaces

Rust now keeps Alpine package metadata more consistent across all supported Alpine inputs:

- `APKBUILD` recipes emit declared dependency families instead of only source/checksum metadata
- installed-db records now normalize trustworthy license statements into declared license fields, not just raw text
- `.apk` archives now normalize the same trustworthy license statements into declared license fields too

This removes an odd asymmetry where only APKBUILD enjoyed normalized license metadata even though package archives and installed-db records carry the same package-declared license values.

## Coverage

Coverage includes SHA1 decoding, provider extraction, APKBUILD metadata parsing, APKBUILD dependency families, raw matched-text preservation for `custom:multiple`, fileless package retention, HTTPS VCS URL synthesis, and non-APKBUILD license normalization.

## References

### Alpine Documentation

- [Alpine Package Format](https://github.com/alpinelinux/apk-tools/blob/master/doc/apk-v2.5.scd)
- [Installed Database Format](https://github.com/alpinelinux/apk-tools/blob/master/doc/apk-package.5.scd)

SHA1 decoding and provider extraction are both part of the parser behavior described above.
