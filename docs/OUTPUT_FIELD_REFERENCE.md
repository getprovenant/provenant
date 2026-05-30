# Output Field Reference

> **⚠️ AUTO-GENERATED FILE** - Do not edit manually.
> This file is generated from semantic metadata stored in `src/output_schema/`.
> To update, run: `cargo run --manifest-path xtask/Cargo.toml --bin generate-output-field-reference`

This reference documents the public ScanCode-compatible output records and fields emitted from `src/output_schema/`. `src/output_schema/` remains the contract owner for the public output surface.

## `Output`

**Output location(s):** `top level`

Top-level ScanCode-compatible output object.

| JSON field                | Value shape     | Key presence                                      | Meaning                                                                       |
| ------------------------- | --------------- | ------------------------------------------------- | ----------------------------------------------------------------------------- |
| `summary`                 | `object`        | Emitted only when available.                      | Codebase-level rollup derived during post-processing.                         |
| `tallies`                 | `object`        | Emitted only when tally generation is enabled.    | Count-oriented rollup across the full scan result.                            |
| `tallies_of_key_files`    | `object`        | Emitted only when key-file tallies are available. | Tally rollup restricted to files treated as key files for summary scoring.    |
| `tallies_by_facet`        | `array<object>` | Emitted only when facet tallies were requested.   | Facet-scoped tally output for user-defined facet groupings.                   |
| `headers`                 | `array<object>` | Always emitted.                                   | Per-run metadata blocks describing tool/version/options and scan environment. |
| `packages`                | `array<object>` | Always emitted.                                   | Assembled top-level package records visible on the public output contract.    |
| `dependencies`            | `array<object>` | Always emitted.                                   | Top-level dependency records emitted after assembly and hoisting.             |
| `license_detections`      | `array<object>` | Always emitted.                                   | Top-level grouped license detections across the scanned codebase.             |
| `files`                   | `array<object>` | Always emitted.                                   | File and directory records with the main per-resource findings surface.       |
| `license_references`      | `array<object>` | Always emitted.                                   | Top-level unique license reference blocks for emitted detections.             |
| `license_rule_references` | `array<object>` | Always emitted.                                   | Top-level unique rule reference blocks for emitted detections.                |

## `OutputTopLevelDependency`

**Output location(s):** `dependencies[]`

Hoisted top-level dependency record emitted after assembly.

| JSON field              | Value shape       | Key presence    | Meaning                                                                                 |
| ----------------------- | ----------------- | --------------- | --------------------------------------------------------------------------------------- |
| `purl`                  | `string \| null`  | Always emitted. | Package URL for the top-level dependency row.                                           |
| `extracted_requirement` | `string \| null`  | Always emitted. | Raw requirement/version constraint text for the top-level dependency row.               |
| `scope`                 | `string \| null`  | Always emitted. | Datasource-specific dependency scope such as runtime/dev/test/build.                    |
| `is_runtime`            | `boolean \| null` | Always emitted. | Normalized runtime intent signal when it can be derived honestly.                       |
| `is_optional`           | `boolean \| null` | Always emitted. | Normalized optional-dependency signal when it can be derived honestly.                  |
| `is_pinned`             | `boolean \| null` | Always emitted. | Normalized version-pinning signal when it can be derived honestly.                      |
| `is_direct`             | `boolean \| null` | Always emitted. | Normalized direct-vs-transitive signal when it can be derived honestly.                 |
| `resolved_package`      | `object \| null`  | Always emitted. | Resolved package payload nested under the top-level dependency row.                     |
| `extra_data`            | `object`          | Always emitted. | Datasource-specific structured metadata preserved on the top-level dependency row.      |
| `dependency_uid`        | `string`          | Always emitted. | Stable identifier for the top-level dependency row on the public contract.              |
| `for_package_uid`       | `string \| null`  | Always emitted. | Package UID that owns this hoisted dependency row after assembly.                       |
| `datafile_path`         | `string`          | Always emitted. | Manifest or metadata file path that contributed this top-level dependency row.          |
| `datasource_id`         | `string`          | Always emitted. | Datasource identifier for the parser/input surface that contributed this row.           |
| `namespace`             | `string \| null`  | Always emitted. | Owning namespace lane preserved on the hoisted dependency row for output compatibility. |

## `OutputResolvedPackage`

**Output location(s):** `dependencies[].resolved_package`, `files[].package_data[].dependencies[].resolved_package`

Resolved package payload nested under dependency rows.

| JSON field                         | Value shape       | Key presence    | Meaning                                                                                    |
| ---------------------------------- | ----------------- | --------------- | ------------------------------------------------------------------------------------------ |
| `type`                             | `string`          | Always emitted. | Package ecosystem/type identifier on the resolved-package payload.                         |
| `namespace`                        | `string`          | Always emitted. | Resolved package namespace.                                                                |
| `name`                             | `string`          | Always emitted. | Resolved package name.                                                                     |
| `version`                          | `string`          | Always emitted. | Resolved package version.                                                                  |
| `qualifiers`                       | `object`          | Always emitted. | PURL-style qualifier key/value pairs on the resolved-package payload.                      |
| `subpath`                          | `string \| null`  | Always emitted. | Resolved package subpath, when available.                                                  |
| `primary_language`                 | `string \| null`  | Always emitted. | Primary language associated with the resolved package.                                     |
| `description`                      | `string \| null`  | Always emitted. | Resolved package description.                                                              |
| `release_date`                     | `string \| null`  | Always emitted. | Resolved package release date.                                                             |
| `parties`                          | `array<object>`   | Always emitted. | Party records attached to the resolved package.                                            |
| `keywords`                         | `array<string>`   | Always emitted. | Keywords attached to the resolved package.                                                 |
| `homepage_url`                     | `string \| null`  | Always emitted. | Resolved package homepage URL.                                                             |
| `download_url`                     | `string \| null`  | Always emitted. | Resolved package download URL.                                                             |
| `size`                             | `integer \| null` | Always emitted. | Resolved package size when known.                                                          |
| `sha1`                             | `string \| null`  | Always emitted. | Resolved package SHA-1 checksum when known.                                                |
| `md5`                              | `string \| null`  | Always emitted. | Resolved package MD5 checksum when known.                                                  |
| `sha256`                           | `string \| null`  | Always emitted. | Resolved package SHA-256 checksum when known.                                              |
| `sha512`                           | `string \| null`  | Always emitted. | Resolved package SHA-512 checksum when known.                                              |
| `bug_tracking_url`                 | `string \| null`  | Always emitted. | Resolved package bug-tracker URL.                                                          |
| `code_view_url`                    | `string \| null`  | Always emitted. | Resolved package code-browsing URL.                                                        |
| `vcs_url`                          | `string \| null`  | Always emitted. | Resolved package VCS URL.                                                                  |
| `copyright`                        | `string \| null`  | Always emitted. | Resolved package copyright string.                                                         |
| `holder`                           | `string \| null`  | Always emitted. | Resolved package holder string.                                                            |
| `declared_license_expression`      | `string \| null`  | Always emitted. | Primary declared license expression on the resolved-package payload.                       |
| `declared_license_expression_spdx` | `string \| null`  | Always emitted. | SPDX-form primary declared license expression on the resolved-package payload.             |
| `license_detections`               | `array<object>`   | Always emitted. | License detections attached to the resolved-package payload.                               |
| `other_license_expression`         | `string \| null`  | Always emitted. | Auxiliary non-primary license expression lane on the resolved-package payload.             |
| `other_license_expression_spdx`    | `string \| null`  | Always emitted. | SPDX-form auxiliary non-primary license expression lane on the resolved-package payload.   |
| `other_license_detections`         | `array<object>`   | Always emitted. | Auxiliary license detections attached to the resolved-package payload.                     |
| `extracted_license_statement`      | `string \| null`  | Always emitted. | Raw extracted license statement on the resolved-package payload.                           |
| `notice_text`                      | `string \| null`  | Always emitted. | Resolved package notice text.                                                              |
| `source_packages`                  | `array<string>`   | Always emitted. | Referenced source-package identifiers associated with the resolved package.                |
| `file_references`                  | `array<object>`   | Always emitted. | Referenced-file rows attached to the resolved package.                                     |
| `is_private`                       | `boolean`         | Always emitted. | Private/public signal on the resolved-package payload.                                     |
| `is_virtual`                       | `boolean`         | Always emitted. | Virtual/synthetic signal on the resolved-package payload.                                  |
| `extra_data`                       | `object`          | Always emitted. | Datasource-specific structured metadata preserved on the resolved-package payload.         |
| `dependencies`                     | `array<object>`   | Always emitted. | Dependency rows nested under the resolved-package payload.                                 |
| `repository_homepage_url`          | `string \| null`  | Always emitted. | Repository homepage URL for the resolved package.                                          |
| `repository_download_url`          | `string \| null`  | Always emitted. | Repository download URL for the resolved package.                                          |
| `api_data_url`                     | `string \| null`  | Always emitted. | API data URL for the resolved package.                                                     |
| `datasource_id`                    | `string \| null`  | Always emitted. | Single datasource identifier associated with the resolved-package payload, when available. |
| `purl`                             | `string \| null`  | Always emitted. | Package URL for the resolved package.                                                      |

## `OutputFileInfo`

**Output location(s):** `files[]`

File or directory record on the main per-resource output surface.

| JSON field                         | Value shape             | Key presence                                         | Meaning                                                                                                              |
| ---------------------------------- | ----------------------- | ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `path`                             | `string`                | Always emitted.                                      | Scan-root-relative path for the file or directory record.                                                            |
| `type`                             | `string`                | Always emitted.                                      | File-vs-directory type discriminator on the public output surface.                                                   |
| `name`                             | `string`                | Always emitted.                                      | Basename of the file or directory record.                                                                            |
| `base_name`                        | `string`                | Always emitted.                                      | Basename without extension.                                                                                          |
| `extension`                        | `string`                | Always emitted.                                      | Filename extension including the leading dot when one exists.                                                        |
| `size`                             | `integer`               | Always emitted.                                      | File size in bytes for files, or zero for synthetic/default directory rows.                                          |
| `date`                             | `string \| null`        | Emitted only on the file-info surface.               | File date metadata on the opt-in file-info surface.                                                                  |
| `sha1`                             | `string \| null`        | Emitted only on the file-info surface.               | SHA-1 checksum on the opt-in file-info surface.                                                                      |
| `md5`                              | `string \| null`        | Emitted only on the file-info surface.               | MD5 checksum on the opt-in file-info surface.                                                                        |
| `sha256`                           | `string \| null`        | Emitted only on the file-info surface.               | SHA-256 checksum on the opt-in file-info surface.                                                                    |
| `sha1_git`                         | `string \| null`        | Emitted only on the file-info surface.               | Git-style SHA-1 checksum on the opt-in file-info surface.                                                            |
| `mime_type`                        | `string \| null`        | Emitted only on the file-info surface.               | Detected MIME type on the opt-in file-info surface.                                                                  |
| `file_type`                        | `string \| null`        | Emitted only on the file-info surface.               | Additional file-type label on the opt-in file-info surface.                                                          |
| `programming_language`             | `string \| null`        | Emitted only on the file-info surface.               | Language hint derived by file classification rather than package parsing.                                            |
| `is_binary`                        | `boolean \| null`       | Emitted only on the file-info surface.               | Binary-file hint on the opt-in file-info surface.                                                                    |
| `is_text`                          | `boolean \| null`       | Emitted only on the file-info surface.               | Text-file hint on the opt-in file-info surface.                                                                      |
| `is_archive`                       | `boolean \| null`       | Emitted only on the file-info surface.               | Archive-file hint on the opt-in file-info surface.                                                                   |
| `is_media`                         | `boolean \| null`       | Emitted only on the file-info surface.               | Media-file hint on the opt-in file-info surface.                                                                     |
| `is_source`                        | `boolean \| null`       | Emitted only on the file-info surface.               | Source-file hint on the opt-in file-info surface.                                                                    |
| `is_script`                        | `boolean \| null`       | Emitted only on the file-info surface.               | Script-file hint on the opt-in file-info surface.                                                                    |
| `files_count`                      | `integer \| null`       | Emitted only on the file-info surface.               | Nested file count on directory/info records when available.                                                          |
| `dirs_count`                       | `integer \| null`       | Emitted only on the file-info surface.               | Nested directory count on directory/info records when available.                                                     |
| `size_count`                       | `integer \| null`       | Emitted only on the file-info surface.               | Aggregated nested size count on directory/info records when available.                                               |
| `package_data`                     | `array<object>`         | Always emitted.                                      | Raw parser-emitted package rows attached to this file record.                                                        |
| `detected_license_expression_spdx` | `string \| null`        | Always emitted.                                      | Primary SPDX-oriented file-level license expression after grouping detections and applying strict combination rules. |
| `license_detections`               | `array<object>`         | Always emitted.                                      | Grouped license detections attached to this file record.                                                             |
| `license_clues`                    | `array<object>`         | Emitted only when non-empty.                         | Low-confidence or clue-only matches that are surfaced separately from concrete detections.                           |
| `percentage_of_license_text`       | `number \| null`        | Emitted only when a percentage was computed.         | Approximate proportion of the file that participated in license text matches.                                        |
| `copyrights`                       | `array<object>`         | Always emitted.                                      | File-level copyright evidence records.                                                                               |
| `holders`                          | `array<object>`         | Always emitted.                                      | File-level holder evidence records.                                                                                  |
| `authors`                          | `array<object>`         | Always emitted.                                      | File-level author evidence records.                                                                                  |
| `emails`                           | `array<object>`         | Emitted only when non-empty.                         | File-level extracted email records.                                                                                  |
| `urls`                             | `array<object>`         | Always emitted.                                      | File-level extracted URL records.                                                                                    |
| `for_packages`                     | `array<string>`         | Always emitted.                                      | Package UIDs that this file record is attached to after assembly or file-reference resolution.                       |
| `scan_errors`                      | `array<string>`         | Always emitted.                                      | Per-file problems that did not prevent the overall scan from completing.                                             |
| `license_policy`                   | `array<object> \| null` | Emitted only when policy output is available.        | Policy decoration entries attached to the file’s license findings when policy evaluation ran.                        |
| `is_generated`                     | `boolean \| null`       | Emitted only when generated-code classification ran. | Generated-code signal on the info/classification surface.                                                            |
| `source_count`                     | `integer \| null`       | Emitted only when source counts are available.       | Count of nested source files on directory/info records when available.                                               |
| `is_legal`                         | `boolean`               | Emitted only when true.                              | Marks a file treated as legal-material evidence for key-file and summary logic.                                      |
| `is_manifest`                      | `boolean`               | Emitted only when true.                              | Marks a file treated as a manifest-like key file for package or summary logic.                                       |
| `is_readme`                        | `boolean`               | Emitted only when true.                              | Marks a file treated as README-style descriptive project metadata.                                                   |
| `is_top_level`                     | `boolean`               | Emitted only when true.                              | Marks a file treated as top-level for summary or package-root reasoning, even if filesystem depth differs.           |
| `is_key_file`                      | `boolean`               | Emitted only when true.                              | Marks a file that participates directly in summary and license clarity scoring.                                      |
| `is_referenced`                    | `boolean`               | Emitted only when true.                              | Marks a file whose content was followed as referenced evidence from another scanned file or package record.          |
| `is_community`                     | `boolean`               | Emitted only when true.                              | Marks a file treated as community-material evidence on the classification surface.                                   |
| `facets`                           | `array<string>`         | Emitted only when non-empty.                         | Facet labels attached by user-defined facet rules for tally grouping.                                                |
| `tallies`                          | `object \| null`        | Emitted only when file-level tallies were requested. | Per-file or per-directory tally block emitted by detailed tally workflows.                                           |

## `OutputAuthor`

**Output location(s):** `files[].authors[]`

File-level author evidence record.

| JSON field   | Value shape | Key presence    | Meaning                                        |
| ------------ | ----------- | --------------- | ---------------------------------------------- |
| `author`     | `string`    | Always emitted. | Extracted author string.                       |
| `start_line` | `integer`   | Always emitted. | First line where the author evidence appeared. |
| `end_line`   | `integer`   | Always emitted. | Last line where the author evidence appeared.  |

## `OutputCopyright`

**Output location(s):** `files[].copyrights[]`

File-level copyright evidence record.

| JSON field   | Value shape | Key presence    | Meaning                                                                             |
| ------------ | ----------- | --------------- | ----------------------------------------------------------------------------------- |
| `copyright`  | `string`    | Always emitted. | Rendered copyright string, using native or ScanCode compatibility mode as selected. |
| `start_line` | `integer`   | Always emitted. | First line where the copyright evidence appeared.                                   |
| `end_line`   | `integer`   | Always emitted. | Last line where the copyright evidence appeared.                                    |

## `OutputEmail`

**Output location(s):** `files[].emails[]`

File-level email evidence record.

| JSON field   | Value shape | Key presence    | Meaning                                       |
| ------------ | ----------- | --------------- | --------------------------------------------- |
| `email`      | `string`    | Always emitted. | Extracted email address.                      |
| `start_line` | `integer`   | Always emitted. | First line where the email evidence appeared. |
| `end_line`   | `integer`   | Always emitted. | Last line where the email evidence appeared.  |

## `OutputHolder`

**Output location(s):** `files[].holders[]`

File-level holder evidence record.

| JSON field   | Value shape | Key presence    | Meaning                                        |
| ------------ | ----------- | --------------- | ---------------------------------------------- |
| `holder`     | `string`    | Always emitted. | Extracted copyright holder string.             |
| `start_line` | `integer`   | Always emitted. | First line where the holder evidence appeared. |
| `end_line`   | `integer`   | Always emitted. | Last line where the holder evidence appeared.  |

## `OutputMatch`

**Output location(s):** `files[].license_clues[]`, `files[].license_detections[].matches[]`, `license_detections[].reference_matches[]`

Match record used for clue output, grouped detections, and top-level representative references.

| JSON field                 | Value shape             | Key presence                 | Meaning                                                            |
| -------------------------- | ----------------------- | ---------------------------- | ------------------------------------------------------------------ |
| `license_expression`       | `string`                | Always emitted.              | License expression assigned to this individual match.              |
| `license_expression_spdx`  | `string`                | Always emitted.              | SPDX-form expression assigned to this individual match.            |
| `from_file`                | `string \| null`        | Always emitted.              | Origin file path associated with the match record.                 |
| `start_line`               | `integer`               | Always emitted.              | First line covered by the match.                                   |
| `end_line`                 | `integer`               | Always emitted.              | Last line covered by the match.                                    |
| `matcher`                  | `string \| null`        | Emitted only when available. | Matcher kind that produced the match.                              |
| `score`                    | `number`                | Always emitted.              | Match score on the public output scale.                            |
| `matched_length`           | `integer \| null`       | Emitted only when available. | Matched token/text length when tracked.                            |
| `match_coverage`           | `number \| null`        | Emitted only when available. | Coverage ratio for the match when tracked.                         |
| `rule_relevance`           | `integer \| null`       | Emitted only when available. | Rule relevance score when tracked.                                 |
| `rule_identifier`          | `string \| null`        | Emitted only when available. | Identifier of the matched rule when available.                     |
| `rule_url`                 | `string \| null`        | Always emitted.              | Rule URL when available.                                           |
| `matched_text`             | `string \| null`        | Emitted only when available. | Matched text payload when text output is enabled.                  |
| `matched_text_diagnostics` | `string \| null`        | Emitted only when available. | Diagnostic rendering of the matched text when enabled.             |
| `referenced_filenames`     | `array<string> \| null` | Emitted only when available. | Referenced filenames captured on the matched rule when applicable. |

## `OutputLicenseDetection`

**Output location(s):** `files[].license_detections[]`, `files[].package_data[].license_detections[]`, `packages[].license_detections[]`, `dependencies[].resolved_package.license_detections[]`

Grouped license detection record used on file, package_data, package, and resolved-package surfaces.

| JSON field                | Value shape      | Key presence                 | Meaning                                                                          |
| ------------------------- | ---------------- | ---------------------------- | -------------------------------------------------------------------------------- |
| `license_expression`      | `string`         | Always emitted.              | Grouped license expression for the detection block.                              |
| `license_expression_spdx` | `string`         | Always emitted.              | SPDX-form expression for the detection block.                                    |
| `matches`                 | `array<object>`  | Always emitted.              | Match records that contributed to this grouped file- or package-level detection. |
| `detection_log`           | `array<string>`  | Emitted only when non-empty. | Notes explaining post-processing or grouping decisions for this detection.       |
| `identifier`              | `string \| null` | Always emitted.              | Stable detection identifier when available.                                      |

## `OutputLicensePolicyEntry`

**Output location(s):** `files[].license_policy[]`

Policy decoration entry attached to file-level license-policy output.

| JSON field    | Value shape | Key presence    | Meaning                                              |
| ------------- | ----------- | --------------- | ---------------------------------------------------- |
| `license_key` | `string`    | Always emitted. | License key this policy entry applies to.            |
| `label`       | `string`    | Always emitted. | Human-facing policy label for the license.           |
| `color_code`  | `string`    | Always emitted. | Color code used by policy-aware outputs or UIs.      |
| `icon`        | `string`    | Always emitted. | Icon identifier used by policy-aware outputs or UIs. |

## `OutputPackageData`

**Output location(s):** `files[].package_data[]`

Raw parser-emitted package record attached to a specific file.

| JSON field                         | Value shape       | Key presence    | Meaning                                                                                  |
| ---------------------------------- | ----------------- | --------------- | ---------------------------------------------------------------------------------------- |
| `type`                             | `string \| null`  | Always emitted. | Package ecosystem/type identifier on a file-local package_data row.                      |
| `namespace`                        | `string \| null`  | Always emitted. | Package namespace on the file-local package_data row.                                    |
| `name`                             | `string \| null`  | Always emitted. | Package name on the file-local package_data row.                                         |
| `version`                          | `string \| null`  | Always emitted. | Package version on the file-local package_data row.                                      |
| `qualifiers`                       | `object`          | Always emitted. | PURL-style qualifier key/value pairs on a file-local package_data row.                   |
| `subpath`                          | `string \| null`  | Always emitted. | Package subpath on the file-local package_data row.                                      |
| `primary_language`                 | `string \| null`  | Always emitted. | Primary language associated with the file-local package_data row.                        |
| `description`                      | `string \| null`  | Always emitted. | Package description on the file-local package_data row.                                  |
| `release_date`                     | `string \| null`  | Always emitted. | Release date on the file-local package_data row.                                         |
| `parties`                          | `array<object>`   | Always emitted. | Party records attached to the file-local package_data row.                               |
| `keywords`                         | `array<string>`   | Always emitted. | Keywords attached to the file-local package_data row.                                    |
| `homepage_url`                     | `string \| null`  | Always emitted. | Homepage URL on the file-local package_data row.                                         |
| `download_url`                     | `string \| null`  | Always emitted. | Download URL on the file-local package_data row.                                         |
| `size`                             | `integer \| null` | Always emitted. | Package size on the file-local package_data row when known.                              |
| `sha1`                             | `string \| null`  | Always emitted. | SHA-1 checksum on the file-local package_data row when known.                            |
| `md5`                              | `string \| null`  | Always emitted. | MD5 checksum on the file-local package_data row when known.                              |
| `sha256`                           | `string \| null`  | Always emitted. | SHA-256 checksum on the file-local package_data row when known.                          |
| `sha512`                           | `string \| null`  | Always emitted. | SHA-512 checksum on the file-local package_data row when known.                          |
| `bug_tracking_url`                 | `string \| null`  | Always emitted. | Bug-tracker URL on the file-local package_data row.                                      |
| `code_view_url`                    | `string \| null`  | Always emitted. | Code-view URL on the file-local package_data row.                                        |
| `vcs_url`                          | `string \| null`  | Always emitted. | VCS URL on the file-local package_data row.                                              |
| `copyright`                        | `string \| null`  | Always emitted. | Copyright string on the file-local package_data row.                                     |
| `holder`                           | `string \| null`  | Always emitted. | Holder string on the file-local package_data row.                                        |
| `declared_license_expression`      | `string \| null`  | Always emitted. | Primary declared license expression on the file-local package_data row.                  |
| `declared_license_expression_spdx` | `string \| null`  | Always emitted. | SPDX-form primary declared license expression on the file-local package_data row.        |
| `license_detections`               | `array<object>`   | Always emitted. | Structured license detections attached to the raw parser-emitted package_data row.       |
| `other_license_expression`         | `string \| null`  | Always emitted. | Auxiliary non-primary license expression lane for the package_data row.                  |
| `other_license_expression_spdx`    | `string \| null`  | Always emitted. | SPDX-form auxiliary non-primary license expression lane for the package_data row.        |
| `other_license_detections`         | `array<object>`   | Always emitted. | Detections associated with the auxiliary or non-primary license lane.                    |
| `extracted_license_statement`      | `string \| null`  | Always emitted. | Raw extracted license statement on the package_data row.                                 |
| `notice_text`                      | `string \| null`  | Always emitted. | Notice text on the package_data row.                                                     |
| `source_packages`                  | `array<string>`   | Always emitted. | Referenced source-package identifiers preserved on the raw package_data row.             |
| `file_references`                  | `array<object>`   | Always emitted. | File-reference hints emitted by parsers for later resolution or ownership assignment.    |
| `is_private`                       | `boolean`         | Always emitted. | Private/public package signal on the raw parser-emitted row.                             |
| `is_virtual`                       | `boolean`         | Always emitted. | Virtual/synthetic package signal on the raw parser-emitted row.                          |
| `extra_data`                       | `object`          | Always emitted. | Datasource-specific structured metadata preserved without promotion into core fields.    |
| `dependencies`                     | `array<object>`   | Always emitted. | Raw dependency rows emitted directly by the parser before top-level assembly.            |
| `repository_homepage_url`          | `string \| null`  | Always emitted. | Repository homepage URL on the file-local package_data row.                              |
| `repository_download_url`          | `string \| null`  | Always emitted. | Repository download URL on the file-local package_data row.                              |
| `api_data_url`                     | `string \| null`  | Always emitted. | API data URL on the file-local package_data row.                                         |
| `datasource_id`                    | `string \| null`  | Always emitted. | Single datasource identifier for the parser surface that produced this package_data row. |
| `purl`                             | `string \| null`  | Always emitted. | Package URL on the file-local package_data row.                                          |

## `OutputDependency`

**Output location(s):** `files[].package_data[].dependencies[]`

Raw dependency row preserved on parser-emitted package data.

| JSON field              | Value shape       | Key presence    | Meaning                                                                               |
| ----------------------- | ----------------- | --------------- | ------------------------------------------------------------------------------------- |
| `purl`                  | `string \| null`  | Always emitted. | Package URL for the dependency row.                                                   |
| `extracted_requirement` | `string \| null`  | Always emitted. | Raw requirement/version constraint text extracted from the manifest or lockfile.      |
| `scope`                 | `string \| null`  | Always emitted. | Datasource-specific dependency scope such as runtime/dev/test/build.                  |
| `is_runtime`            | `boolean \| null` | Always emitted. | Normalized runtime intent signal when it can be derived honestly.                     |
| `is_optional`           | `boolean \| null` | Always emitted. | Normalized optional-dependency signal when it can be derived honestly.                |
| `is_pinned`             | `boolean \| null` | Always emitted. | Normalized version-pinning signal when it can be derived honestly.                    |
| `is_direct`             | `boolean \| null` | Always emitted. | Normalized direct-vs-transitive signal when it can be derived honestly.               |
| `resolved_package`      | `object \| null`  | Always emitted. | Resolved package identity/details nested directly under the dependency row.           |
| `extra_data`            | `object`          | Always emitted. | Datasource-specific dependency metadata preserved without promotion into core fields. |

## `OutputFileReference`

**Output location(s):** `files[].package_data[].file_references[]`, `dependencies[].resolved_package.file_references[]`

Referenced-file record used on package-related surfaces.

| JSON field   | Value shape       | Key presence                 | Meaning                                                               |
| ------------ | ----------------- | ---------------------------- | --------------------------------------------------------------------- |
| `path`       | `string`          | Always emitted.              | Referenced file path preserved on the package/file-reference surface. |
| `size`       | `integer \| null` | Emitted only when available. | Referenced file size when known.                                      |
| `sha1`       | `string \| null`  | Always emitted.              | Referenced file SHA-1 checksum when known.                            |
| `md5`        | `string \| null`  | Always emitted.              | Referenced file MD5 checksum when known.                              |
| `sha256`     | `string \| null`  | Always emitted.              | Referenced file SHA-256 checksum when known.                          |
| `sha512`     | `string \| null`  | Always emitted.              | Referenced file SHA-512 checksum when known.                          |
| `extra_data` | `object`          | Always emitted.              | Additional metadata preserved on the referenced-file row.             |

## `OutputFileType`

**Output location(s):** `files[].type`

Serialized file-type enum used by file records.

This record has no nested fields on the public output surface.

## `OutputURL`

**Output location(s):** `files[].urls[]`

File-level URL evidence record.

| JSON field   | Value shape | Key presence    | Meaning                                     |
| ------------ | ----------- | --------------- | ------------------------------------------- |
| `url`        | `string`    | Always emitted. | Extracted URL string.                       |
| `start_line` | `integer`   | Always emitted. | First line where the URL evidence appeared. |
| `end_line`   | `integer`   | Always emitted. | Last line where the URL evidence appeared.  |

## `OutputHeader`

**Output location(s):** `headers[]`

Per-run metadata block for one scan invocation.

| JSON field              | Value shape     | Key presence    | Meaning                                                                                |
| ----------------------- | --------------- | --------------- | -------------------------------------------------------------------------------------- |
| `tool_name`             | `string`        | Always emitted. | Scanner tool name recorded for the run.                                                |
| `tool_version`          | `string`        | Always emitted. | Scanner version recorded for the run.                                                  |
| `options`               | `object`        | Always emitted. | Serialized scan options so downstream readers can interpret how the run was performed. |
| `notice`                | `string`        | Always emitted. | Run-level legal or attribution notice emitted by the scanner.                          |
| `start_timestamp`       | `string`        | Always emitted. | Run start timestamp.                                                                   |
| `end_timestamp`         | `string`        | Always emitted. | Run end timestamp.                                                                     |
| `output_format_version` | `string`        | Always emitted. | Version of the public output format contract used by this run.                         |
| `duration`              | `number`        | Always emitted. | Wall-clock scan duration recorded for the run.                                         |
| `errors`                | `array<string>` | Always emitted. | Run-level errors recorded in the header rather than on a specific file.                |
| `warnings`              | `array<string>` | Always emitted. | Run-level warnings recorded in the header rather than on a specific file.              |
| `extra_data`            | `object`        | Always emitted. | Scanner-owned counts and provenance metadata that augment the header contract.         |

## `OutputExtraData`

**Output location(s):** `headers[].extra_data`

Scanner-owned counts and provenance metadata nested under a header block.

| JSON field                  | Value shape | Key presence                                     | Meaning                                                                                       |
| --------------------------- | ----------- | ------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| `spdx_license_list_version` | `string`    | Always emitted.                                  | The SPDX license list version associated with the effective license data used for the run.    |
| `files_count`               | `integer`   | Always emitted.                                  | Count of file records seen by the scan pipeline.                                              |
| `directories_count`         | `integer`   | Always emitted.                                  | Count of directory records seen by the scan pipeline.                                         |
| `excluded_count`            | `integer`   | Always emitted.                                  | Count of paths excluded before file processing completed.                                     |
| `license_index_provenance`  | `object`    | Emitted only when index provenance is available. | Provenance metadata for the effective embedded or custom license index used during detection. |

## `OutputLicenseIndexProvenance`

**Output location(s):** `headers[].extra_data.license_index_provenance`

Provenance block for the effective license index used by the scan.

| JSON field                      | Value shape     | Key presence                 | Meaning                                                                          |
| ------------------------------- | --------------- | ---------------------------- | -------------------------------------------------------------------------------- |
| `source`                        | `string`        | Always emitted.              | Source lane for the effective license index, such as embedded or custom dataset. |
| `dataset_fingerprint`           | `string`        | Always emitted.              | Stable fingerprint for the effective license dataset contents.                   |
| `ignored_rules`                 | `array<string>` | Emitted only when non-empty. | Rule identifiers excluded while building the effective index.                    |
| `ignored_licenses`              | `array<string>` | Emitted only when non-empty. | License keys excluded while building the effective index.                        |
| `ignored_rules_due_to_licenses` | `array<string>` | Emitted only when non-empty. | Rules excluded indirectly because their owning licenses were excluded.           |
| `added_rules`                   | `array<string>` | Emitted only when non-empty. | Rule identifiers added by local overlays or custom dataset input.                |
| `replaced_rules`                | `array<string>` | Emitted only when non-empty. | Rule identifiers replaced by local overlays or custom dataset input.             |
| `added_licenses`                | `array<string>` | Emitted only when non-empty. | License keys added by local overlays or custom dataset input.                    |
| `replaced_licenses`             | `array<string>` | Emitted only when non-empty. | License keys replaced by local overlays or custom dataset input.                 |

## `OutputSystemEnvironment`

**Output location(s):** `headers[].extra_data.system_environment`

Recorded environment metadata for the scan runtime.

| JSON field         | Value shape | Key presence    | Meaning                                                  |
| ------------------ | ----------- | --------------- | -------------------------------------------------------- |
| `operating_system` | `string`    | Always emitted. | Operating-system name recorded for the scan environment. |
| `cpu_architecture` | `string`    | Always emitted. | CPU architecture recorded for the scan environment.      |
| `platform`         | `string`    | Always emitted. | Platform family recorded for the scan environment.       |
| `platform_version` | `string`    | Always emitted. | Platform version recorded for the scan environment.      |
| `rust_version`     | `string`    | Always emitted. | Rust toolchain version used by the scanner binary.       |

## `OutputTopLevelLicenseDetection`

**Output location(s):** `license_detections[]`

Grouped top-level license detection block across the scanned codebase.

| JSON field                | Value shape     | Key presence                 | Meaning                                                                                   |
| ------------------------- | --------------- | ---------------------------- | ----------------------------------------------------------------------------------------- |
| `identifier`              | `string`        | Always emitted.              | Stable grouped-detection identifier for this top-level license detection block.           |
| `license_expression`      | `string`        | Always emitted.              | Grouped license expression for the top-level detection block.                             |
| `license_expression_spdx` | `string`        | Always emitted.              | SPDX-form grouped license expression for the top-level detection block.                   |
| `detection_count`         | `integer`       | Always emitted.              | Number of file-level detections that contributed to this grouped top-level block.         |
| `detection_log`           | `array<string>` | Emitted only when non-empty. | Grouping-time notes that explain why this top-level detection looks the way it does.      |
| `reference_matches`       | `array<object>` | Always emitted.              | Representative match/reference records retained on the top-level grouped detection block. |

## `OutputLicenseReference`

**Output location(s):** `license_references[]`

Top-level license reference record describing one emitted license key and its reference metadata.

| JSON field                | Value shape       | Key presence                 | Meaning                                                                                       |
| ------------------------- | ----------------- | ---------------------------- | --------------------------------------------------------------------------------------------- |
| `key`                     | `string \| null`  | Emitted only when available. | Primary ScanCode-style license key when one is available for the reference block.             |
| `language`                | `string \| null`  | Emitted only when available. | Language tag for the referenced license text when the license reference is language-specific. |
| `name`                    | `string`          | Always emitted.              | Canonical human-facing name of the referenced license.                                        |
| `short_name`              | `string`          | Always emitted.              | Short display name of the referenced license.                                                 |
| `owner`                   | `string \| null`  | Emitted only when available. | Owning organization or steward of the referenced license, when captured.                      |
| `homepage_url`            | `string \| null`  | Emitted only when available. | Homepage URL for the referenced license.                                                      |
| `spdx_license_key`        | `string`          | Always emitted.              | Primary SPDX license key associated with the referenced license.                              |
| `other_spdx_license_keys` | `array<string>`   | Emitted only when non-empty. | Additional SPDX license keys associated with the same referenced license.                     |
| `osi_license_key`         | `string \| null`  | Emitted only when available. | OSI license key when the referenced license is recognized by OSI.                             |
| `text_urls`               | `array<string>`   | Emitted only when non-empty. | URLs to known license text sources for this referenced license.                               |
| `osi_url`                 | `string \| null`  | Emitted only when available. | OSI detail page URL for the referenced license.                                               |
| `faq_url`                 | `string \| null`  | Emitted only when available. | FAQ URL for the referenced license.                                                           |
| `other_urls`              | `array<string>`   | Emitted only when non-empty. | Additional URLs associated with the referenced license.                                       |
| `category`                | `string \| null`  | Emitted only when available. | License category label, when the license data classifies it.                                  |
| `is_exception`            | `boolean`         | Always emitted.              | Whether the referenced license is an exception rather than a standalone license.              |
| `is_unknown`              | `boolean`         | Always emitted.              | Whether the referenced license is treated as an unknown or placeholder license.               |
| `is_generic`              | `boolean`         | Always emitted.              | Whether the referenced license is generic rather than a specific named license.               |
| `notes`                   | `string \| null`  | Emitted only when available. | Additional notes carried by the referenced license record.                                    |
| `minimum_coverage`        | `integer \| null` | Emitted only when available. | Minimum coverage threshold associated with the referenced license, when specified.            |
| `standard_notice`         | `string \| null`  | Emitted only when available. | Standard notice text associated with the referenced license.                                  |
| `ignorable_copyrights`    | `array<string>`   | Emitted only when non-empty. | Copyright strings considered ignorable for this referenced license.                           |
| `ignorable_holders`       | `array<string>`   | Emitted only when non-empty. | Holder strings considered ignorable for this referenced license.                              |
| `ignorable_authors`       | `array<string>`   | Emitted only when non-empty. | Author strings considered ignorable for this referenced license.                              |
| `ignorable_urls`          | `array<string>`   | Emitted only when non-empty. | URL strings considered ignorable for this referenced license.                                 |
| `ignorable_emails`        | `array<string>`   | Emitted only when non-empty. | Email strings considered ignorable for this referenced license.                               |
| `scancode_url`            | `string \| null`  | Emitted only when available. | ScanCode reference URL for this license, when available.                                      |
| `licensedb_url`           | `string \| null`  | Emitted only when available. | LicenseDB URL for this license, when available.                                               |
| `spdx_url`                | `string \| null`  | Emitted only when available. | SPDX reference URL for this license, when available.                                          |
| `text`                    | `string`          | Always emitted.              | Canonical license text payload preserved on the reference block.                              |

## `OutputLicenseRuleReference`

**Output location(s):** `license_rule_references[]`

Top-level license-rule reference record describing one emitted rule and its reference metadata.

| JSON field                            | Value shape       | Key presence                 | Meaning                                                                                    |
| ------------------------------------- | ----------------- | ---------------------------- | ------------------------------------------------------------------------------------------ |
| `identifier`                          | `string`          | Always emitted.              | Stable identifier of the referenced license rule.                                          |
| `license_expression`                  | `string`          | Always emitted.              | License expression associated with the referenced rule.                                    |
| `is_license_text`                     | `boolean`         | Always emitted.              | Whether the rule is classified as license-text evidence.                                   |
| `is_license_notice`                   | `boolean`         | Always emitted.              | Whether the rule is classified as license-notice evidence.                                 |
| `is_license_reference`                | `boolean`         | Always emitted.              | Whether the rule is classified as license-reference evidence.                              |
| `is_license_tag`                      | `boolean`         | Always emitted.              | Whether the rule is classified as license-tag evidence.                                    |
| `is_license_clue`                     | `boolean`         | Always emitted.              | Whether the rule is classified as clue-only evidence.                                      |
| `is_license_intro`                    | `boolean`         | Always emitted.              | Whether the rule is classified as introductory license wording.                            |
| `language`                            | `string \| null`  | Emitted only when available. | Language tag for the referenced rule when the rule is language-specific.                   |
| `rule_url`                            | `string \| null`  | Emitted only when available. | Reference URL for the rule, when available.                                                |
| `is_required_phrase`                  | `boolean`         | Always emitted.              | Whether the rule is a required-phrase rule.                                                |
| `skip_for_required_phrase_generation` | `boolean`         | Always emitted.              | Whether this rule should be excluded when deriving required-phrase rules automatically.    |
| `replaced_by`                         | `array<string>`   | Emitted only when non-empty. | Rule identifiers that supersede this rule.                                                 |
| `is_continuous`                       | `boolean`         | Always emitted.              | Whether the rule expects continuous text rather than discontinuous matches.                |
| `is_synthetic`                        | `boolean`         | Always emitted.              | Whether the rule was synthesized rather than sourced directly from curated reference text. |
| `is_from_license`                     | `boolean`         | Always emitted.              | Whether the rule was derived directly from a license text record.                          |
| `length`                              | `integer`         | Always emitted.              | Rule length on the public rule-reference surface.                                          |
| `relevance`                           | `integer \| null` | Emitted only when available. | Rule relevance score, when present.                                                        |
| `minimum_coverage`                    | `integer \| null` | Emitted only when available. | Minimum coverage threshold associated with the rule, when specified.                       |
| `referenced_filenames`                | `array<string>`   | Emitted only when non-empty. | Referenced filenames attached to the rule metadata.                                        |
| `notes`                               | `string \| null`  | Emitted only when available. | Additional notes carried by the rule reference.                                            |
| `ignorable_copyrights`                | `array<string>`   | Emitted only when non-empty. | Copyright strings considered ignorable for this rule.                                      |
| `ignorable_holders`                   | `array<string>`   | Emitted only when non-empty. | Holder strings considered ignorable for this rule.                                         |
| `ignorable_authors`                   | `array<string>`   | Emitted only when non-empty. | Author strings considered ignorable for this rule.                                         |
| `ignorable_urls`                      | `array<string>`   | Emitted only when non-empty. | URL strings considered ignorable for this rule.                                            |
| `ignorable_emails`                    | `array<string>`   | Emitted only when non-empty. | Email strings considered ignorable for this rule.                                          |
| `text`                                | `string \| null`  | Emitted only when available. | Canonical rule text payload when the reference includes it.                                |

## `OutputPackage`

**Output location(s):** `packages[]`

Assembled top-level package record on the public output contract.

| JSON field                         | Value shape       | Key presence    | Meaning                                                                                                                                        |
| ---------------------------------- | ----------------- | --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `type`                             | `string \| null`  | Always emitted. | Package ecosystem/type identifier on the public ScanCode-compatible surface.                                                                   |
| `namespace`                        | `string \| null`  | Always emitted. | Package namespace on the public package surface.                                                                                               |
| `name`                             | `string \| null`  | Always emitted. | Package name on the public package surface.                                                                                                    |
| `version`                          | `string \| null`  | Always emitted. | Package version on the public package surface.                                                                                                 |
| `qualifiers`                       | `object`          | Always emitted. | PURL-style qualifier key/value pairs. Empty object when qualifiers are absent.                                                                 |
| `subpath`                          | `string \| null`  | Always emitted. | Package subpath on the public package surface.                                                                                                 |
| `primary_language`                 | `string \| null`  | Always emitted. | Primary language associated with the package.                                                                                                  |
| `description`                      | `string \| null`  | Always emitted. | Package description.                                                                                                                           |
| `release_date`                     | `string \| null`  | Always emitted. | Package release date.                                                                                                                          |
| `parties`                          | `array<object>`   | Always emitted. | Party records attached to the package.                                                                                                         |
| `keywords`                         | `array<string>`   | Always emitted. | Keywords attached to the package.                                                                                                              |
| `homepage_url`                     | `string \| null`  | Always emitted. | Package homepage URL.                                                                                                                          |
| `download_url`                     | `string \| null`  | Always emitted. | Package download URL.                                                                                                                          |
| `size`                             | `integer \| null` | Always emitted. | Package size when known.                                                                                                                       |
| `sha1`                             | `string \| null`  | Always emitted. | Package SHA-1 checksum when known.                                                                                                             |
| `md5`                              | `string \| null`  | Always emitted. | Package MD5 checksum when known.                                                                                                               |
| `sha256`                           | `string \| null`  | Always emitted. | Package SHA-256 checksum when known.                                                                                                           |
| `sha512`                           | `string \| null`  | Always emitted. | Package SHA-512 checksum when known.                                                                                                           |
| `bug_tracking_url`                 | `string \| null`  | Always emitted. | Package bug-tracker URL.                                                                                                                       |
| `code_view_url`                    | `string \| null`  | Always emitted. | Package code-view URL.                                                                                                                         |
| `vcs_url`                          | `string \| null`  | Always emitted. | Package VCS URL.                                                                                                                               |
| `copyright`                        | `string \| null`  | Always emitted. | Package copyright string.                                                                                                                      |
| `holder`                           | `string \| null`  | Always emitted. | Package holder string.                                                                                                                         |
| `declared_license_expression`      | `string \| null`  | Always emitted. | Primary declared license expression on the package record.                                                                                     |
| `declared_license_expression_spdx` | `string \| null`  | Always emitted. | SPDX-form primary declared license expression on the package record.                                                                           |
| `license_detections`               | `array<object>`   | Always emitted. | Structured declared or extracted license detections attached to the package record.                                                            |
| `other_license_expression`         | `string \| null`  | Always emitted. | Non-primary declared license text normalized into an auxiliary expression lane.                                                                |
| `other_license_expression_spdx`    | `string \| null`  | Always emitted. | SPDX-form auxiliary non-primary declared license expression on the package record.                                                             |
| `other_license_detections`         | `array<object>`   | Always emitted. | Detections associated with the auxiliary or non-primary license lane.                                                                          |
| `extracted_license_statement`      | `string \| null`  | Always emitted. | Raw extracted license statement on the package record.                                                                                         |
| `notice_text`                      | `string \| null`  | Always emitted. | Package notice text.                                                                                                                           |
| `source_packages`                  | `array<string>`   | Always emitted. | Referenced source-package package URLs or identifiers associated with this package.                                                            |
| `is_private`                       | `boolean`         | Always emitted. | Package-level private/public signal when the parser or datasource can state it confidently.                                                    |
| `is_virtual`                       | `boolean`         | Always emitted. | Marks package records that represent virtual or synthetic package identities rather than concrete deliverables.                                |
| `extra_data`                       | `object`          | Always emitted. | Datasource-specific structured metadata preserved without promoting it into the core package contract. Empty object when extra data is absent. |
| `repository_homepage_url`          | `string \| null`  | Always emitted. | Repository homepage URL for the package.                                                                                                       |
| `repository_download_url`          | `string \| null`  | Always emitted. | Repository download URL for the package.                                                                                                       |
| `api_data_url`                     | `string \| null`  | Always emitted. | API data URL for the package.                                                                                                                  |
| `purl`                             | `string \| null`  | Always emitted. | Package URL for the package record.                                                                                                            |
| `package_uid`                      | `string`          | Always emitted. | Stable package identifier used internally and on output links such as `for_packages`.                                                          |
| `datafile_paths`                   | `array<string>`   | Always emitted. | Manifest or metadata file paths that contributed to this assembled package record.                                                             |
| `datasource_ids`                   | `array<string>`   | Always emitted. | Datasource identifiers that explain which parser/input surfaces contributed to this package record.                                            |

## `OutputDatasourceId`

**Output location(s):** `packages[].datasource_ids[]`, `dependencies[].datasource_id`, `files[].package_data[].datasource_id`

Serialized datasource-id string newtype used by package and dependency records.

This record has no nested fields on the public output surface.

## `OutputParty`

**Output location(s):** `packages[].parties[]`, `files[].package_data[].parties[]`, `dependencies[].resolved_package.parties[]`

Party record used on package and resolved-package surfaces.

| JSON field         | Value shape      | Key presence                 | Meaning                                                                          |
| ------------------ | ---------------- | ---------------------------- | -------------------------------------------------------------------------------- |
| `type`             | `string \| null` | Always emitted.              | Normalized party type such as person or organization.                            |
| `role`             | `string \| null` | Always emitted.              | Role of the party on the package or metadata record.                             |
| `name`             | `string \| null` | Always emitted.              | Human-readable party name.                                                       |
| `email`            | `string \| null` | Always emitted.              | Party email address.                                                             |
| `url`              | `string \| null` | Always emitted.              | Party homepage or profile URL.                                                   |
| `organization`     | `string \| null` | Emitted only when available. | Owning organization for the party, when captured separately from the party name. |
| `organization_url` | `string \| null` | Emitted only when available. | Owning organization URL for the party.                                           |
| `timezone`         | `string \| null` | Emitted only when available. | Timezone associated with the party metadata, when available.                     |

## `OutputPackageType`

**Output location(s):** `packages[].type`, `files[].package_data[].type`

Serialized package-type string newtype used by package-related records.

This record has no nested fields on the public output surface.

## `OutputSummary`

**Output location(s):** `summary`

Optional codebase-level rollup emitted by summary/classification workflows.

| JSON field                    | Value shape     | Key presence                                                    | Meaning                                                                                                               |
| ----------------------------- | --------------- | --------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `declared_license_expression` | `string`        | Emitted only when the summary can derive a declared expression. | Best summary-level declared license rollup derived from key files and assembled package data.                         |
| `license_clarity_score`       | `object`        | Emitted only when clarity scoring is available.                 | Structured clarity signal explaining how complete and trustworthy the summary-level licensing evidence looks.         |
| `other_license_expressions`   | `array<object>` | Emitted only when non-empty.                                    | Secondary license expressions that contributed to the summary but were not chosen as the primary declared expression. |
| `other_holders`               | `array<object>` | Emitted only when non-empty.                                    | Secondary holders that contributed to the summary but were not chosen as the primary holder.                          |
| `other_languages`             | `array<object>` | Emitted only when non-empty.                                    | Secondary languages that contributed to the summary but were not chosen as the primary language.                      |

## `OutputLicenseClarityScore`

**Output location(s):** `summary.license_clarity_score`

Structured license-clarity scoring payload on the summary surface.

| JSON field                       | Value shape | Key presence    | Meaning                                                                            |
| -------------------------------- | ----------- | --------------- | ---------------------------------------------------------------------------------- |
| `score`                          | `integer`   | Always emitted. | Overall clarity score for the summary-level licensing evidence.                    |
| `declared_license`               | `boolean`   | Always emitted. | Whether clear declared-license evidence was found.                                 |
| `identification_precision`       | `boolean`   | Always emitted. | Whether the detected licensing evidence is precise rather than vague or generic.   |
| `has_license_text`               | `boolean`   | Always emitted. | Whether substantive license-text evidence was found.                               |
| `declared_copyrights`            | `boolean`   | Always emitted. | Whether declared copyright evidence was found in the key-file set.                 |
| `conflicting_license_categories` | `boolean`   | Always emitted. | Whether the evidence contains conflicting license-category signals.                |
| `ambiguous_compound_licensing`   | `boolean`   | Always emitted. | Whether the evidence suggests a compound license situation that remains ambiguous. |

## `OutputTallyEntry`

**Output location(s):** `summary.other_license_expressions[]`, `summary.other_holders[]`, `summary.other_languages[]`, `tallies.*[]`

Single tally bucket entry used throughout summary and tally outputs.

| JSON field | Value shape      | Key presence    | Meaning                                               |
| ---------- | ---------------- | --------------- | ----------------------------------------------------- |
| `value`    | `string \| null` | Always emitted. | Bucket value represented by the tally row.            |
| `count`    | `integer`        | Always emitted. | Number of occurrences counted into this tally bucket. |

## `OutputTallies`

**Output location(s):** `tallies`, `tallies_of_key_files`, `files[].tallies`, `tallies_by_facet[].tallies`

Tally block used on top-level, key-file, facet, and file-level tally surfaces.

| JSON field                    | Value shape     | Key presence                 | Meaning                                                    |
| ----------------------------- | --------------- | ---------------------------- | ---------------------------------------------------------- |
| `detected_license_expression` | `array<object>` | Emitted only when non-empty. | Tally entries for file-level detected license expressions. |
| `copyrights`                  | `array<object>` | Emitted only when non-empty. | Tally entries for copyright strings.                       |
| `holders`                     | `array<object>` | Emitted only when non-empty. | Tally entries for copyright holders.                       |
| `authors`                     | `array<object>` | Emitted only when non-empty. | Tally entries for author strings.                          |
| `programming_language`        | `array<object>` | Emitted only when non-empty. | Tally entries for detected programming-language hints.     |

## `OutputFacetTallies`

**Output location(s):** `tallies_by_facet[]`

Facet-specific tally wrapper for one user-defined facet label.

| JSON field | Value shape | Key presence    | Meaning                                   |
| ---------- | ----------- | --------------- | ----------------------------------------- |
| `facet`    | `string`    | Always emitted. | Facet label for this grouped tally block. |
| `tallies`  | `object`    | Always emitted. | Tally payload for this single facet.      |
