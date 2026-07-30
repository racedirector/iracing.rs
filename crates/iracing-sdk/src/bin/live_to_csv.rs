//! Live telemetry to CSV CLI (Windows)
//!
//! Connects to a running iRacing instance and writes telemetry frames to a CSV.
//!
//! This binary mirrors `ibt_to_csv` CSV formatting:
//! - Columns are deterministic and expanded for array variables
//! - Every telemetry variable in the live schema is written per frame
//!
//! # Platform
//! This tool relies on `iracing_sdk::WindowsConnection`, so it is only usable on
//! Windows with iRacing shared memory available.
//!
//! # Usage
//!
//! ```text
//! live_to_csv --output-path <OUTPUT_FILE.csv>
//! ```

#[cfg(any(windows, test))]
mod csv_telemetry_writer;

#[cfg(windows)]
use anyhow::Context;
use anyhow::{Result, anyhow};
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use futures::StreamExt;
#[cfg(windows)]
use iracing_sdk::{DynamicFrame, LiveConnection, WindowsConnection, providers::live::LiveProvider};
#[cfg(windows)]
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
use csv_telemetry_writer::CsvTelemetryWriter;

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the telemetry CSV should be written.
    #[arg(short, long)]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn run() -> Result<()> {
    let Args { output_path } = Args::parse();

    tracing::info!("Opening iRacing connection");
    let connection =
        WindowsConnection::try_connect().context("Failed to connect to iRacing shared memory")?;

    if !connection.is_connected() {
        return Err(anyhow!("iRacing is not connected."));
    }

    // Sort the variables for extraction by the offset of each variable info
    let mut variables = connection.get_variables();
    if variables.is_empty() {
        return Err(anyhow!(
            "No telemetry variables were available from the live connection"
        ));
    }

    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    // Create a provider for actual extraction
    let provider = LiveProvider::builder()
        .with_connection(connection)
        .build()?;

    let connection = LiveConnection::builder().with_provider(provider).build()?;
    let mut stream = connection.subscribe::<DynamicFrame>(iracing_sdk::UpdateRate::Native);

    tracing::info!(path = %output_path.display(), "Creating CSV output");
    let mut writer = CsvTelemetryWriter::builder()
        .with_output_path(&output_path)
        .with_variables(variables)
        .build()?;

    tracing::info!(
        variable_count = writer.variable_count(),
        column_count = writer.column_count(),
        "Starting live CSV export"
    );

    let mut frame_count = 0usize;
    while let Some(frame) = stream.next().await {
        writer.write_telemetry(&frame)?;
        frame_count += 1;
    }

    // Clean up
    writer.flush()?;
    tracing::info!(frames_exported = frame_count, "Finished live CSV export");

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live_to_csv is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("live_to_csv is only supported on Windows"))
}
