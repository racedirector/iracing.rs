use anyhow::Result;
use clap::Parser;
use csv::Writer;
use iracing_sdk::{
    AdapterValidation, FieldExtraction, FrameAdapter, LayoutProvider, provider::Provider,
    providers::ibt::IbtProvider,
};
use std::{fs, path::PathBuf};
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
/// This struct defines the output layout written per frame.
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

impl FrameAdapter for Row {
    fn validate_layout(
        layout: &std::sync::Arc<iracing_sdk::TelemetryLayout>,
    ) -> iracing_sdk::Result<AdapterValidation> {
        let extraction_plan = vec![
            FieldExtraction::Required(
                AdapterValidation::resolve::<f32>(layout, "LapDist", true)?
                    .expect("required field"),
            ),
            FieldExtraction::Required(
                AdapterValidation::resolve::<f32>(layout, "LapDistPct", true)?
                    .expect("required field"),
            ),
            FieldExtraction::Required(
                AdapterValidation::resolve::<f64>(layout, "Lat", true)?.expect("required field"),
            ),
            FieldExtraction::Required(
                AdapterValidation::resolve::<f64>(layout, "Lon", true)?.expect("required field"),
            ),
            FieldExtraction::Required(
                AdapterValidation::resolve::<f32>(layout, "Alt", true)?.expect("required field"),
            ),
            FieldExtraction::Required(
                AdapterValidation::resolve::<bool>(layout, "OnPitRoad", true)?
                    .expect("required field"),
            ),
            FieldExtraction::Required(
                AdapterValidation::resolve::<bool>(layout, "PlayerCarInPitStall", true)?
                    .expect("required field"),
            ),
        ];
        Ok(AdapterValidation::new(
            std::sync::Arc::clone(layout),
            extraction_plan,
        ))
    }

    fn adapt(packet: &iracing_sdk::FramePacket, validation: &AdapterValidation) -> Self {
        validation
            .ensure_packet(packet)
            .expect("adapter layout mismatch");
        Self {
            lap_distance_meters: validation.fetch_or_default::<f32>(packet, 0),
            lap_distance_percentage: validation.fetch_or_default::<f32>(packet, 1),
            latitude: validation.fetch_or_default::<f64>(packet, 2),
            longitude: validation.fetch_or_default::<f64>(packet, 3),
            altitude: validation.fetch_or_default::<f32>(packet, 4),
            is_on_pit_road: validation.fetch_or_default::<bool>(packet, 5),
            is_in_pit_box: validation.fetch_or_default::<bool>(packet, 6),
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
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

    tracing::info!(path = %ibt_path.display(), "Opening IBT file");

    let mut ibt_provider = IbtProvider::open(&ibt_path)?;

    // ------------------------------------------------------------
    // Write session string to output path.
    // ------------------------------------------------------------
    if let Some(yml_output) = yml_output_path {
        tracing::info!("Parsing session information...");
        if let Some(session) = ibt_provider.session_yaml(0).await? {
            fs::write(&yml_output, session)?;
            tracing::info!(session_output_path = %yml_output.display(), "Session information written.")
        }
    }

    let mut writer = Writer::from_path(&csv_output_path).expect("Could not create CSV output");

    tracing::info!("Parsing frames from IBT provider");

    let layout = ibt_provider.layout();
    let shared_validation = Row::validate_layout(layout)?;
    while let Some(packet) = ibt_provider.next_frame().await? {
        let frame = Row::adapt(&packet, &shared_validation);
        // Serialize row to CSV.
        writer.serialize(frame)?;
    }

    writer.flush()?;
    tracing::info!(output_path = %csv_output_path.display(), "Finished processing frames");

    Ok(())
}
