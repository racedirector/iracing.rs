//! Convert IBT telemetry to newline-delimited JSON.
//!
//! Opens an iRacing `.ibt` recording and writes every telemetry frame as one
//! compact JSON object per line.
//!
//! # Usage
//!
//! ```text
//! ibt-to-json --ibt-path <INPUT.ibt> --output-path <OUTPUT.jsonl>
//! ```

mod json_telemetry_writer;

use anyhow::{Context, Result};
use clap::Parser;
use futures::StreamExt;
use iracing_sdk::{DynamicFrame, IbtConnection, SchemaProvider, VariableInfo};
use json_telemetry_writer::{DynamicFrameSnapshot, JsonTelemetryWriter};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the telemetry JSONL stream should be written.
    #[arg(short, long)]
    output_path: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args {
        ibt_path,
        output_path,
    } = Args::parse();

    tracing::info!(path = %ibt_path.display(), "Opening IBT file");
    let connection = IbtConnection::builder()
        .with_path(&ibt_path)
        .build()
        .await
        .context("Failed to open IBT telemetry file")?;

    let mut variables: Vec<VariableInfo> = connection.variables();
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    tracing::info!(path = %output_path.display(), "Creating JSONL output");
    let mut writer = JsonTelemetryWriter::from_path(&output_path)?;
    let mut frames = Box::pin(connection.subscribe::<DynamicFrame>()?);
    connection.start()?;

    tracing::info!(variable_count = variables.len(), "Starting IBT JSON export");
    let mut frame_count = 0usize;
    while let Some(frame) = frames.next().await {
        let snapshot = DynamicFrameSnapshot::from_frame(&frame, &variables)?;
        writer.write_snapshot(&snapshot)?;
        frame_count += 1;

        if frame_count.is_multiple_of(10_000) {
            tracing::debug!(frames_exported = frame_count, "IBT JSON export progress");
        }
    }

    writer.flush()?;
    tracing::info!(frames_exported = frame_count, "Finished IBT JSON export");
    Ok(())
}
