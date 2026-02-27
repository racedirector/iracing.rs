//! Live telemetry to IBT CLI (Windows)
//!
//! Connects to a running iRacing instance and writes telemetry frames to an IBT.
//!
//! # Platform
//! This tool relies on `iracing_sdk::WindowsConnection`, so it is only usable on
//! Windows with iRacing shared memory available.
//!
//! # Usage
//!
//! ```text
//! live_to_ibt --output-path <OUTPUT_FILE.csv>
//! ```

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the output `.ibt` telemetry file.
    #[arg(short, long)]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

#[cfg(windows)]
fn run() -> Result<()> {
    info!("Opening iRacing connection");
    let mut connection =
        WindowsConnection::try_connect().context("Failed to connect to iRacing shared memory")?;

    if !connection.is_connected() {
        return Err(anyhow!("iRacing is not connected."));
    }

    // ------------------------------------------------------------
    // Create a projection of the variables you want from the
    // source connection
    // ------------------------------------------------------------
    let projection = FrameProjection::from_variable_names(
        reader.variables(),
        ["SessionTime", "Speed", "RPM", "OnPitRoad"],
    )?;

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live_to_csv is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );

    Err(anyhow!("live_to_csv is only supported on Windows"))
}
