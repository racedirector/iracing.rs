#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use iracing_sdk::IRacingTelemetryFrame;
#[cfg(windows)]
use std::path::PathBuf;

#[cfg(windows)]
#[derive(Parser, Debug)]
struct Args {
    /// Path where the output CSV should be written.
    #[arg(short, long)]
    csv_output_path: PathBuf,
}

/// CSV row representation of positional telemetry.
///
/// This struct defines the output schema written per frame.
#[cfg(windows)]
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
    latitude: f64,

    /// Longitude in decimal degrees. Unit: deg.
    #[field_name = "Lon"]
    longitude: f64,

    /// Altitude. Unit: m.
    #[field_name = "Alt"]
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
async fn main() -> anyhow::Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    #[cfg(not(windows))]
    {
        tracing::warn!(
            "live-position example is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
        );
        Err(anyhow::anyhow!(
            "live-position example is only supported on Windows"
        ))
    }

    #[cfg(windows)]
    {
        use csv::Writer;
        use futures::StreamExt;
        use iracing_sdk::{LiveConnection, UpdateRate};

        // ------------------------------------------------------------
        // Parse CLI arguments
        // ------------------------------------------------------------
        let Args { csv_output_path } = Args::parse();

        tracing::info!("Opening Live iRacing connection");

        // ------------------------------------------------------------
        // Open telemetry connection and CSV writer
        // ------------------------------------------------------------
        let connection = LiveConnection::builder().build()?;
        let mut stream = connection.subscribe::<Row>(UpdateRate::Native);
        let mut writer = Writer::from_path(&csv_output_path)?;

        while let Some(frame) = stream.next().await {
            writer.serialize(frame)?;
        }

        writer.flush()?;

        Ok(())
    }
}
