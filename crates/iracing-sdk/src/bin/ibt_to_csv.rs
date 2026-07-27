//! IBT to CSV CLI
//!
//! Parses an `.ibt` telemetry file and writes the output to a CSV.
//!
//! This binary is intended for:
//! - Converting an `.ibt` file into a standalone CSV file for sharing.
//!
//! # Behavior
//! - Opens the `.ibt` file using [`iracing_sdk::ibt::IbtReader`]
//! - Streams frame packets through an [`iracing_sdk::providers::ibt::IbtProvider`]
//! - Reads the selected variables from each packet and writes them to a CSV
//!
//! # Logging
//! Uses `tracing` + `tracing_subscriber`.
//!
//! - Defaults to `info` level
//! - Override with `RUST_LOG`, e.g. `RUST_LOG=info`
//!
//! # Usage
//!
//! ```text
//! ibt_to_csv --ibt-path <PATH_TO_FILE.ibt> --output-path <OUTPUT_FILE.csv>
//! ```
//!
//! # Examples
//!
//! Parse an `.ibt` file and write CSV:
//!
//! ```bash
//! cargo run -p iracing-sdk --bin ibt_to_csv -- \
//!   --ibt-path "C:\path\to\telemetry.ibt" \
//!   --output-path "C:\path\to\telemetry.csv"
//! ```
//!
//! Reduce log noise:
//!
//! ```bash
//! RUST_LOG=info cargo run -p iracing-sdk --bin ibt_to_csv -- \
//!   -i "./telemetry.ibt" \
//!   -o "./telemetry.csv"
//! ```
//!
//! # Exit codes / errors
//! - Returns an error if the `.ibt` cannot be opened or read
//! - Returns an error if the output file cannot be written
//!

mod csv_telemetry_writer;

use anyhow::{Context, Result};
use clap::Parser;
use iracing_sdk::{VariableInfo, ibt::IbtReader, provider::Provider, providers::ibt::IbtProvider};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use csv_telemetry_writer::CsvTelemetryWriter;

/// CLI arguments for the disk session parser.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the session CSV should be written.
    #[arg(short, long)]
    output_path: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to INFO unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments.
    // ------------------------------------------------------------
    let Args {
        ibt_path,
        output_path,
    } = Args::parse();

    // ------------------------------------------------------------
    // Open telemetry reader.
    // ------------------------------------------------------------
    tracing::info!(path = %ibt_path.display(), "Opening IBT file");
    let reader = IbtReader::open(&ibt_path).context("Failed to open IBT file")?;

    // Clone and sort variables for deterministic column ordering.
    let mut variables: Vec<VariableInfo> = reader.variables().variables.values().cloned().collect();
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut provider = IbtProvider::from_reader(reader);

    tracing::info!(path = %output_path.display(), "Creating CSV output");
    let mut writer = CsvTelemetryWriter::builder()
        .with_output_path(&output_path)
        .with_variables(variables)
        .build()?;

    tracing::info!(
        variable_count = writer.variable_count(),
        column_count = writer.column_count(),
        "Starting CSV export"
    );

    // ------------------------------------------------------------
    // Frame streaming
    // ------------------------------------------------------------
    let mut frame_count = 0usize;

    while let Some(packet) = provider.next_frame().await? {
        writer.write_telemetry(&packet)?;
        frame_count += 1;

        if frame_count.is_multiple_of(10_000) {
            tracing::debug!(frames_exported = frame_count, "CSV export progress");
        }
    }

    writer.flush()?;
    tracing::info!(frames_exported = frame_count, "Finished CSV export");

    Ok(())
}
