# Font fixture provenance

- `SyntheticVariableNameRunon.ttf` — synthetic, hand-built SFNT containing only a
  `name` table. It is **not** a copy of any shipped font binary. It reproduces the
  variable-font name-table packing (designer, vendor URL, and description records
  stored contiguously in UTF-16 storage with no separators) that caused run-on URL
  false positives such as `bulenkovhttps://www.jetbrains.comThis`. The embedded
  strings mimic JetBrains Mono name records for regression realism; JetBrains Mono
  itself is OFL-1.1.

Other font fixtures in this directory are upstream font binaries used as parser
fixtures; see their respective upstream projects for licensing.
