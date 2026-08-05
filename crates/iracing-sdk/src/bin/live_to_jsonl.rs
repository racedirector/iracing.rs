//! Live telemetry to JSONL CLI (Windows).
//!
//! Connects to a running iRacing instance and writes each native-rate telemetry
//! frame as one compact JSON object per line until the live stream ends.
//!
//! # Usage
//!
//! ```text
//! live-to-jsonl --output-path <OUTPUT_FILE.jsonl>
//! ```

#[cfg(any(windows, test))]
mod json_telemetry_writer;

use anyhow::{Result, anyhow};
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use futures::StreamExt;
#[cfg(windows)]
use iracing_sdk::{
    DynamicFrame, LiveConnection, SchemaProvider, UpdateRate, WindowsConnection,
    providers::live::LiveProvider,
};
#[cfg(windows)]
use json_telemetry_writer::{DynamicFrameSnapshot, JsonTelemetryWriter};
#[cfg(windows)]
use std::{path::PathBuf, thread, time::Duration};
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the telemetry JSONL stream should be written.
    #[arg(short, long)]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn run() -> Result<()> {
    let Args { output_path } = Args::parse();

    tracing::info!("Opening iRacing connection...");
    let windows_connection = loop {
        match WindowsConnection::try_connect() {
            Ok(connection) if connection.is_connected() => break connection,
            Ok(_) => {
                tracing::debug!("Shared memory opened but telemetry is not connected yet");
            }
            Err(error) => {
                tracing::debug!(%error, "Waiting for iRacing shared memory");
            }
        }

        thread::sleep(Duration::from_secs(1));
    };

    let connection = LiveConnection::builder()
        .with_provider(
            LiveProvider::builder()
                .with_connection(windows_connection)
                .build()?,
        )
        .build()?;

    let mut variables = connection.variables();
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

    tracing::info!(path = %output_path.display(), "Creating JSONL output");
    let mut writer = JsonTelemetryWriter::from_path(&output_path)?;
    let mut frames = Box::pin(connection.subscribe::<DynamicFrame>(UpdateRate::Native)?);

    tracing::info!(
        variable_count = variables.len(),
        "Starting live JSONL export"
    );
    let mut frame_count = 0usize;
    while let Some(frame) = frames.next().await {
        let snapshot = DynamicFrameSnapshot::from_frame(&frame, &variables)?;
        writer.write_record(&snapshot)?;
        frame_count += 1;

        if frame_count.is_multiple_of(10_000) {
            tracing::debug!(frames_exported = frame_count, "JSONL export progress");
        }
    }

    writer.flush()?;
    tracing::info!(frames_exported = frame_count, "Finished live JSONL export");
    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live-to-jsonl is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("live-to-jsonl is only supported on Windows"))
}
