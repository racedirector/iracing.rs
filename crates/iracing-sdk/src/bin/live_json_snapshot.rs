//! Capture one live telemetry frame as JSONL (Windows).
//!
//! Connects to a running iRacing instance, waits for one native-rate telemetry
//! frame, writes it as a compact JSON object followed by a newline, and exits.
//!
//! # Usage
//!
//! ```text
//! live-json-snapshot --output-path <OUTPUT_FILE.jsonl>
//! ```

#[cfg(any(windows, test))]
mod json_telemetry_writer;

use anyhow::{Result, anyhow};
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use futures::StreamExt;
#[cfg(windows)]
use iracing_sdk::providers::live::LiveProvider;
#[cfg(windows)]
use json_telemetry_writer::{DynamicFrameSnapshot, JsonTelemetryWriter};
#[cfg(windows)]
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the telemetry JSONL snapshot should be written.
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
    use iracing_sdk::{
        DynamicFrame, LiveConnection, SchemaProvider, UpdateRate, WindowsConnection,
    };
    use std::{thread, time::Duration};

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

    let mut frames = Box::pin(connection.subscribe::<DynamicFrame>(UpdateRate::Native)?);
    tracing::info!(path = %output_path.display(), "Waiting for one live telemetry frame");
    let frame = frames
        .next()
        .await
        .ok_or_else(|| anyhow!("Live telemetry ended before a frame was received"))?;
    let snapshot = DynamicFrameSnapshot::from_frame(&frame, &variables)?;

    let mut writer = JsonTelemetryWriter::from_path(&output_path)?;
    writer.write_record(&snapshot)?;
    writer.flush()?;

    tracing::info!(
        path = %output_path.display(),
        tick_count = frame.tick_count(),
        variable_count = variables.len(),
        "Wrote live telemetry JSONL snapshot"
    );
    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live-json-snapshot is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("live-json-snapshot is only supported on Windows"))
}
