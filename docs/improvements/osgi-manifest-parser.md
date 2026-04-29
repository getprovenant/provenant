# OSGi Manifest Parser: Bundle Metadata Extraction

## Summary

**✨ New Feature**: the Python reference only reaches OSGi manifest handling during assembly-oriented flows, while Rust can recognize OSGi bundles during ordinary file scanning and extract useful bundle metadata directly from the manifest.

When a manifest is clearly both an OSGi bundle and a Maven-identifiable artifact, Rust now keeps the richer OSGi extraction while preferring the more interoperable Maven primary package identity. Pure OSGi manifests without strong Maven coordinates still emit an OSGi package identity.

## Reference limitation

In the Python reference, OSGi handling is effectively assembly-oriented. That means a manifest can exist in the scanned tree without producing the package data users would expect from a direct file scan.

## Rust behavior

Rust detects OSGi bundles from `META-INF/MANIFEST.MF` files when OSGi-specific headers are present, most importantly `Bundle-SymbolicName`.

When OSGi headers are present, Rust always parses the OSGi-specific metadata surface. The emitted primary package then follows one of two paths:

- if the manifest also proves a strong Maven identity, Rust emits a single Maven package row and preserves the OSGi bundle identity alongside the richer OSGi dependency extraction
- otherwise Rust emits an OSGi package row using `Bundle-SymbolicName` and `Bundle-Version`

When a bundle is recognized, Rust can extract:

- bundle identity from `Bundle-SymbolicName` and `Bundle-Version`
- human-facing description fields from `Bundle-Name` and `Bundle-Description`
- vendor information from `Bundle-Vendor`
- homepage information from `Bundle-DocURL`
- declared license information from `Bundle-License`
- dependency edges from `Import-Package` and `Require-Bundle`

OSGi version ranges are preserved in the extracted dependency requirements instead of being flattened into looser text.

Optional OSGi dependencies are now treated more truthfully too:

- `Require-Bundle` already preserved `resolution:=optional`
- `Import-Package` now also honors `resolution:=optional`, marking those dependencies as optional instead of emitting them as required runtime imports while still preserving that they are runtime package-wiring dependencies

## Why this matters

- **Automatic detection**: OSGi bundles are no longer invisible during regular scans
- **Better mixed-manifest interoperability**: manifests that also prove Maven coordinates can keep a Maven primary identity without dropping OSGi bundle metadata or dependency edges
- **Better bundle metadata**: vendor, license, and homepage data can flow straight from the manifest into package output
- **Richer dependency visibility**: imported packages and required bundles show up as structured dependency edges, including truthful optional import semantics

## Reference

- [OSGi Core Specification](https://docs.osgi.org/specification/osgi.core/7.0.0/framework.module.html)
