use anyhow::{Result, anyhow};
use clap::Parser;
use csv::Writer;
use iracing_sdk_adapter::{AdapterValidation, DynamicFrame, FrameAdapter, IbtProvider, Provider};
use std::{fs, path::PathBuf};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    ibt_path: PathBuf,
    #[arg(short, long)]
    csv_output_path: PathBuf,
    #[arg(short, long)]
    yml_output_path: Option<PathBuf>,
}

/// CSV row representation of positional telemetry.
///
/// This struct defines the output schema written per frame.
#[derive(serde::Serialize)]
struct Row {
    /// Distance traveled around the lap (meters).
    lap_distance_meters: f32,

    /// Lap distance expressed as a percentage (0.0 - 1.0).
    lap_distance_percentage: f32,

    /// GPS latitude.
    latitude: f64,

    /// GPS longitude.
    longitude: f64,

    /// Altitude above sea level (meters).
    altitude: f32,

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
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args {
        ibt_path,
        csv_output_path,
        yml_output_path,
    } = Args::parse();

    info!(path = %ibt_path.display(), "Opening IBT file");

    let mut ibt_provider =
        IbtProvider::from_path(&ibt_path).expect("Failed to initialize IBT provider");

    // ------------------------------------------------------------
    // Write session string to output path.
    // ------------------------------------------------------------
    if let Some(yml_output) = yml_output_path {
        info!("Parsing session information...");
        if let Some(session) = ibt_provider.session_yaml(0)? {
            fs::write(&yml_output, session)?;
            info!(session_output_path = %yml_output.display(), "Session information written.")
        }
    }

    let mut writer = Writer::from_path(&csv_output_path).expect("Could not create CSV output");

    info!(
        total_frames = ibt_provider.total_frames(),
        "Parsing frames from IBT provider"
    );

    let shared_validation = AdapterValidation::new(vec![]);
    while let Some(packet) = ibt_provider.next_frame()? {
        let frame = DynamicFrame::adapt(&packet, &shared_validation);
        let lap_distance_meters = frame.get("LapDist").unwrap();
        let lap_distance_percentage = frame.get("LapDistPct").unwrap();
        let latitude = frame.get("Lat").unwrap();
        let longitude = frame.get("Lon").unwrap();
        let altitude = frame.get("Alt").unwrap();
        let is_on_pit_road = frame.get("OnPitRoad").unwrap();
        let is_in_pit_box = frame.get("PlayerCarInPitStall").unwrap();

        if let Some(_) = frame.get::<f32>("ThisFieldWill Never Exist") {
            return Err(anyhow!("This will never happen"));
        }

        // Serialize row to CSV.
        writer.serialize(Row {
            lap_distance_meters,
            lap_distance_percentage,
            latitude,
            longitude,
            altitude,
            is_in_pit_box,
            is_on_pit_road,
        })?;
    }

    writer.flush()?;
    info!(output_path = %csv_output_path.display(), "Finished processing frames");

    Ok(())
}
