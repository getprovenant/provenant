// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// Verify the checked-in file instead of rewriting it.
    #[arg(long)]
    check: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let output_path = PathBuf::from("docs/openapi/provenant-serve.openapi.json");
    let json = serde_json::to_string_pretty(&provenant::serve_api::openapi_document())?;
    let expected = format!("{json}\n");

    if args.check {
        let existing = fs::read_to_string(&output_path)
            .with_context(|| format!("failed to read {}", output_path.display()))?;
        if existing != expected {
            return Err(anyhow!(
                "{} is out of date; run generate-serve-openapi",
                output_path.display()
            ));
        }
        return Ok(());
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&output_path, expected)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    Ok(())
}
