use anyhow::Result;
use clap::Parser;
use csv::Writer;
use futures::StreamExt;
use iracing_sdk::{IRacingTelemetryFrame, IbtConnection};
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the output CSV should be written.
    #[arg(short, long)]
    csv_output_path: PathBuf,
}

/// CSV row representation of positional telemetry.
///
/// This struct defines the output schema written per frame.
#[derive(IRacingTelemetryFrame, Debug, Clone, Copy, serde::Serialize)]
struct Row {
    /// Distance traveled around the lap (meters).
    #[field_name = "LapDist"]
    #[fail_if_missing]
    lap_distance_meters: f32,

    /// Lap distance expressed as a percentage (0.0 - 1.0).
    #[field_name = "LapDistPct"]
    #[fail_if_missing]
    lap_distance_percentage: f32,

    /// !!!: iRacing uses EPSG:3857 for coordinates.
    /// Latitude in decimal degrees. Unit: deg.
    #[field_name = "Lat"]
    #[fail_if_missing]
    latitude: f64,

    /// Longitude in decimal degrees. Unit: deg.
    #[field_name = "Lon"]
    #[fail_if_missing]
    longitude: f64,

    /// Altitude. Unit: m.
    #[field_name = "Alt"]
    #[fail_if_missing]
    altitude: f32,

    /// Whether the car is currently on pit road.
    #[field_name = "OnPitRoad"]
    #[fail_if_missing]
    is_on_pit_road: bool,

    /// Whether the car is currently in its pit stall.
    #[field_name = "PlayerCarInPitStall"]
    #[fail_if_missing]
    is_in_pit_box: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments
    // ------------------------------------------------------------
    let Args {
        ibt_path,
        csv_output_path,
    } = Args::parse();

    tracing::info!(path = %ibt_path.display(), "Opening IBT file");

    // ------------------------------------------------------------
    // Open telemetry reader and CSV writer
    // ------------------------------------------------------------
    let connection = IbtConnection::builder().with_path(ibt_path).build().await?;
    let mut stream = connection.subscribe::<Row>(iracing_sdk::UpdateRate::Native);
    let mut writer = Writer::from_path(&csv_output_path)?;

    while let Some(frame) = stream.next().await {
        writer.serialize(frame)?;
    }

    writer.flush()?;

    Ok(())
}
