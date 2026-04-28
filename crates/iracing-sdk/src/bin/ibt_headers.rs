//! IBT header dump CLI
//!
//! Prints the parsed IBT main header and disk sub-header for a provided `.ibt` file.
//!
//! This binary is intended for:
//! - Quick inspection of `.ibt` metadata
//! - Verifying header parsing logic
//! - Producing reproducible debug output for support
//!
//! # Behavior
//! - Opens the `.ibt` file from disk
//! - Parses the main header (`IbtHeader`)
//! - Parses the disk sub-header (`IbtDiskSubHeader`)
//! - Prints both structures using pretty `Debug` formatting
//!
//! # Logging
//! Uses `tracing` + `tracing_subscriber`.
//!
//! - Defaults to `trace` level
//! - Override with `RUST_LOG`, e.g. `RUST_LOG=info`
//!
//! # Usage
//!
//! ```text
//! ibt-headers --ibt-path <PATH_TO_FILE.ibt>
//! ```
//!
//! Short flag is also supported:
//!
//! ```text
//! ibt-headers -i <PATH_TO_FILE.ibt>
//! ```
//!
//! # Example
//!
//! ```bash
//! cargo run -p iracing-sdk --bin ibt_headers -- \
//!   --ibt-path "/path/to/telemetry.ibt"
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use iracing_sdk::ibt::format::{IbtDiskSubHeader, IbtHeader};
use std::{fs::File, io::BufReader, path::PathBuf};
use tracing_subscriber::EnvFilter;

/// CLI arguments for the IBT header dump.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments.
    // ------------------------------------------------------------
    let Args { ibt_path } = Args::parse();

    let file = File::open(&ibt_path)
        .with_context(|| format!("Opening IBT file {}", ibt_path.display()))?;
    let mut reader = BufReader::new(file);

    tracing::info!("Parsing IBT header");
    let header = IbtHeader::parse_from_reader(&mut reader)
        .with_context(|| format!("Parsing IBT header from {}", ibt_path.display()))?;
    println!("{:#?}", header);

    tracing::info!("Parsing IBT disk sub-header");
    let disk_header = IbtDiskSubHeader::parse_from_reader_with_header(&mut reader, &header)
        .with_context(|| format!("Parsing IBT disk sub-header from {}", ibt_path.display()))?;

    println!("{:#?}", disk_header);

    Ok(())
}
