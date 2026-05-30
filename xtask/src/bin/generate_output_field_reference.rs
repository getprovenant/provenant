// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use provenant::golden_maintenance::run_prettier;
use provenant::output_schema::{OutputFieldDoc, OutputTypeDoc, documented_output_types};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    check: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let output_path = PathBuf::from("docs/OUTPUT_FIELD_REFERENCE.md");
    let output = format_markdown(&generate_markdown())?;

    if args.check {
        match fs::read_to_string(&output_path) {
            Ok(existing) => {
                if normalize_markdown_for_compare(&existing)
                    == normalize_markdown_for_compare(&output)
                {
                    println!("✓ {} is up to date", output_path.display());
                    return Ok(());
                }

                if let Some((line_number, existing_line, generated_line)) =
                    first_normalized_diff(&existing, &output)
                {
                    eprintln!("first normalized diff at line {}", line_number);
                    eprintln!("existing : {}", existing_line);
                    eprintln!("generated: {}", generated_line);
                }

                eprintln!("✗ {} is out of date", output_path.display());
                eprintln!(
                    "Run: cargo run --manifest-path xtask/Cargo.toml --bin generate-output-field-reference"
                );
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("✗ Failed to read {}: {}", output_path.display(), e);
                std::process::exit(1);
            }
        }
    }

    fs::write(&output_path, &output)?;
    println!("✓ Generated docs/OUTPUT_FIELD_REFERENCE.md");
    Ok(())
}

fn format_markdown(output: &str) -> Result<String> {
    let temp_path = std::env::temp_dir().join(format!(
        "provenant-output-field-reference-{}.md",
        std::process::id()
    ));
    fs::write(&temp_path, output)?;
    run_prettier(std::slice::from_ref(&temp_path))?;
    let formatted = fs::read_to_string(&temp_path)?;
    let _ = fs::remove_file(&temp_path);
    Ok(formatted)
}

fn generate_markdown() -> String {
    let mut docs = documented_output_types().to_vec();
    docs.sort_by(|a, b| {
        a.json_paths[0]
            .cmp(b.json_paths[0])
            .then(a.type_name.cmp(b.type_name))
    });

    let mut output = String::new();
    output.push_str("# Output Field Reference\n\n");
    output.push_str(
        "> **⚠️ AUTO-GENERATED FILE** - Do not edit manually.\n> This file is generated from semantic metadata stored in `src/output_schema/`.\n> To update, run: `cargo run --manifest-path xtask/Cargo.toml --bin generate-output-field-reference`\n\n",
    );
    output.push_str("This reference documents the public ScanCode-compatible output records and fields emitted from `src/output_schema/`. `src/output_schema/` remains the contract owner for the public output surface.\n\n");

    for type_doc in &docs {
        write_type_section(&mut output, type_doc);
    }

    output
}

fn write_type_section(output: &mut String, type_doc: &OutputTypeDoc) {
    output.push_str(&format!("## `{}`\n\n", type_doc.type_name));
    output.push_str(&format!(
        "**Output location(s):** `{}`\n\n",
        type_doc
            .json_paths
            .iter()
            .map(|path| display_location(path))
            .collect::<Vec<_>>()
            .join("`, `")
    ));
    output.push_str(type_doc.summary);
    output.push_str("\n\n");

    if type_doc.fields.is_empty() {
        output.push_str("This record has no nested fields on the public output surface.\n\n");
        return;
    }

    output.push_str("| JSON field | Value shape | Key presence | Meaning |\n");
    output.push_str("| --- | --- | --- | --- |\n");
    for field in type_doc.fields {
        write_field_row(output, field);
    }
    output.push('\n');
}

fn display_location(path: &str) -> String {
    if path == "$" {
        return "top level".to_string();
    }
    path.trim_start_matches("$.").to_string()
}

fn write_field_row(output: &mut String, field: &OutputFieldDoc) {
    output.push_str(&format!(
        "| `{}` | `{}` | {} | {} |\n",
        field.json_name,
        escape_pipes(field.value_shape),
        escape_pipes(field.presence),
        escape_pipes(field.meaning)
    ));
}

fn escape_pipes(text: &str) -> String {
    text.replace('|', "\\|")
}

fn normalize_markdown_for_compare(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('|') && trimmed.ends_with('|') {
                let cells: Vec<String> = trimmed
                    .trim_matches('|')
                    .split('|')
                    .map(|cell| normalize_markdown_text(cell.trim()))
                    .collect();

                let is_separator_row = cells
                    .iter()
                    .all(|cell| !cell.is_empty() && cell.chars().all(|c| c == '-' || c == ':'));

                if is_separator_row {
                    return format!("| {} |", vec!["---"; cells.len()].join(" | "));
                }

                format!("| {} |", cells.join(" | "))
            } else {
                normalize_markdown_text(trimmed)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_markdown_text(input: &str) -> String {
    let collapsed = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut normalized = String::with_capacity(collapsed.len());
    let mut chars = collapsed.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek().is_some_and(|next| next.is_ascii_punctuation()) {
            continue;
        }
        normalized.push(ch);
    }

    normalized
}

fn first_normalized_diff(existing: &str, generated: &str) -> Option<(usize, String, String)> {
    let existing_lines = normalize_markdown_for_compare(existing)
        .lines()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let generated_lines = normalize_markdown_for_compare(generated)
        .lines()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let max_len = existing_lines.len().max(generated_lines.len());

    (0..max_len).find_map(|index| {
        let existing_line = existing_lines.get(index).cloned().unwrap_or_default();
        let generated_line = generated_lines.get(index).cloned().unwrap_or_default();
        (existing_line != generated_line).then_some((index + 1, existing_line, generated_line))
    })
}

#[cfg(test)]
mod tests {
    use super::{display_location, generate_markdown};

    #[test]
    fn generated_markdown_contains_core_sections() {
        let markdown = generate_markdown();

        assert!(markdown.contains("# Output Field Reference"));
        assert!(markdown.contains("## `OutputFileInfo`"));
        assert!(markdown.contains("`is_referenced`"));
        assert!(markdown.contains("`files[]`"));
        assert!(!markdown.contains("## Included records"));
        assert!(!markdown.contains("**Notes:**"));
    }

    #[test]
    fn display_location_removes_jsonpath_root_marker() {
        assert_eq!(display_location("$"), "top level");
        assert_eq!(display_location("$.files[]"), "files[]");
        assert_eq!(
            display_location("$.headers[].extra_data"),
            "headers[].extra_data"
        );
    }
}
