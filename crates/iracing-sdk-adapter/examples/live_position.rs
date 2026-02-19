use anyhow::{Result, anyhow};
#[cfg(windows)]
use clap::Parser;
use iracing_sdk_adapter::{AdapterValidation, DynamicFrame, FrameAdapter, LiveProvider, Provider};
#[cfg(windows)]
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    csv_output_path: PathBuf,

    #[arg(short, long)]
    yml_output_path: Option<PathBuf>,
}

/// CSV row representation of positional telemetry.
///
/// This struct defines the output schema written per frame.
#[cfg(windows)]
#[derive(serde::Serialize)]
struct Row {
    /// Distance traveled around the lap (meters).
    lap_distance_meters: f32,

    /// Lap distance expressed as a percentage (0.0 - 1.0).
    lap_distance_percentage: f32,

    /// Whether the car is currently on pit road.
    is_on_pit_road: bool,

    /// Whether the car is currently in its pit stall.
    is_in_pit_box: bool,
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
    let Args {
        csv_output_path,
        yml_output_path,
    } = Args::parse();

    let mut live_provider = LiveProvider::new().expect("Could not create LiveProvider");

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live-position example is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!(
        "live-position example is only supported on Windows"
    ))
}
