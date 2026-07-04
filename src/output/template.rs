// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::fs;
use std::io::{self, Write};

use minijinja::Environment;
use serde_json::{Map, Value, json};

use crate::output_schema::Output;

use super::OutputWriteConfig;
use super::shared::io_other;

pub(crate) fn write_custom_template(
    output: &Output,
    writer: &mut dyn Write,
    config: &OutputWriteConfig,
) -> io::Result<()> {
    let template_path = config.custom_template.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "--custom-template path is required for custom template output",
        )
    })?;

    let template = fs::read_to_string(template_path)?;
    let output_value = serde_json::to_value(output).map_err(io_other)?;

    // Provenant-native context is the default and mirrors the JSON output
    // schema. `scancode` exposes a ScanCode-Toolkit-compatible reshape so
    // templates written against ScanCode's `--custom-template` contract can be
    // ported by referencing the `scancode` namespace.
    let context = json!({
        "output": output_value,
        "headers": output.headers,
        "files": output.files,
        "packages": output.packages,
        "dependencies": output.dependencies,
        "scancode": scancode_context(output)?,
    });

    let rendered = Environment::new()
        .render_str(&template, &context)
        .map_err(io_other)?;
    writer.write_all(rendered.as_bytes())
}

/// Build the ScanCode-Toolkit-compatible template context.
///
/// Mirrors ScanCode's `formattedcode.output_html.generate_output`: `files` is a
/// path-keyed reshape into `license_copyright`, `infos`, and `package_data`,
/// exposed alongside the tool `version` and the `license_references` list.
fn scancode_context(output: &Output) -> io::Result<Value> {
    let mut license_copyright = Map::new();
    let mut infos = Map::new();
    let mut package_data = Map::new();

    for file in &output.files {
        let path = &file.path;

        let mut entries: Vec<Value> = Vec::new();
        for copyright in &file.copyrights {
            entries.push(json!({
                "start": copyright.start_line,
                "end": copyright.end_line,
                "what": "copyright",
                "value": copyright.copyright,
            }));
        }
        for detection in &file.license_detections {
            for m in &detection.matches {
                entries.push(json!({
                    "start": m.start_line,
                    "end": m.end_line,
                    "what": "license",
                    "value": m.license_expression,
                }));
            }
        }
        entries.sort_by_key(|entry| entry["start"].as_u64().unwrap_or(0));
        // ScanCode inserts `license_copyright` only for files that have at least
        // one entry (its `if results:`), so this map is intentionally sparse.
        if !entries.is_empty() {
            license_copyright.insert(path.clone(), Value::Array(entries));
        }

        // `infos` and `package_data` are dense (one entry per scanned file):
        // `infos` carries every serialized file field except the ones broken
        // out into `license_copyright` and `package_data`, and `package_data`
        // is `[]` for files without packages, matching ScanCode.
        if let Value::Object(mut fields) = serde_json::to_value(file).map_err(io_other)? {
            fields.remove("license_detections");
            fields.remove("package_data");
            fields.remove("copyrights");
            infos.insert(path.clone(), Value::Object(fields));
        }

        package_data.insert(
            path.clone(),
            serde_json::to_value(&file.package_data).map_err(io_other)?,
        );
    }

    Ok(json!({
        "files": {
            "license_copyright": Value::Object(license_copyright),
            "infos": Value::Object(infos),
            "package_data": Value::Object(package_data),
        },
        "license_references": output.license_references,
        "version": output.headers.first().map(|header| header.tool_version.as_str()).unwrap_or_default(),
    }))
}
