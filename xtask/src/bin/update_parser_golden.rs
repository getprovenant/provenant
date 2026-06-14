// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use provenant::golden_maintenance::run_prettier;
use provenant::output_schema::OutputPackageData;
use provenant::parsers;

#[derive(Parser, Debug)]
#[command(
    name = "update-parser-golden",
    about = "Generate parser golden expected JSON from fixture input"
)]
struct Args {
    #[arg(short, long, help = "List all available parser types")]
    list: bool,

    #[arg(
        required_unless_present = "list",
        help = "Parser struct name (for example: NpmParser, DebianDebParser)"
    )]
    parser_type: Option<String>,

    #[arg(
        required_unless_present = "list",
        help = "Path to package manifest fixture input file"
    )]
    input_file: Option<PathBuf>,

    #[arg(
        required_unless_present = "list",
        help = "Path to write expected JSON output"
    )]
    output_file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.list {
        print_available_parsers();
        return Ok(());
    }

    let parser_type = args
        .parser_type
        .as_deref()
        .context("missing required argument: parser_type")?;
    let input_file = args
        .input_file
        .as_ref()
        .context("missing required argument: input_file")?;
    let output_file = args
        .output_file
        .as_ref()
        .context("missing required argument: output_file")?;

    let package_data = match parsers::parse_by_type_name(parser_type, input_file) {
        Some(data) => data,
        None => {
            anyhow::bail!(
                "unknown parser type: {parser_type}. Run with --list to see available parser types"
            );
        }
    };

    // Serialize through `OutputPackageData` (the public output schema), which is
    // exactly what the golden comparators compare against. Serializing the raw
    // internal `PackageData` instead produced a different shape (e.g. field
    // `package_type` vs the schema's `type`), generating fixtures that did not
    // match how the goldens are actually checked.
    let output: OutputPackageData = (&package_data).into();
    let json = serde_json::to_string_pretty(&vec![output]).context("failed to serialize JSON")?;
    fs::write(output_file, json)
        .with_context(|| format!("failed to write output file: {}", output_file.display()))?;

    run_prettier(std::slice::from_ref(output_file))?;

    println!("✅ Generated: {}", output_file.display());

    Ok(())
}

fn print_available_parsers() {
    println!("Available parser types:");
    println!();

    let mut parsers = parsers::list_parser_types();
    parsers.sort();

    for (i, parser) in parsers.iter().enumerate() {
        println!("  {:<3} {}", format!("{}.", i + 1), parser);
    }

    println!();
    println!("Total: {} parsers", parsers.len());
}
