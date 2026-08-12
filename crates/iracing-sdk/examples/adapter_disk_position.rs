use anyhow::Result;
use clap::Parser;
use csv::Writer;
use iracing_sdk::{
    AdapterValidation, FieldExtraction, FrameAdapter, IRacingSDKError, SchemaProvider,
    provider::Provider, providers::ibt::IbtProvider,
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

impl FrameAdapter for Row {
    fn validate_schema(
        schema: &iracing_sdk::VariableSchema,
    ) -> iracing_sdk::Result<AdapterValidation> {
        let mut extraction_plan = Vec::new();

        let lap_distance_meters_info =
            schema
                .get_variable("LapDist")
                .ok_or_else(|| IRacingSDKError::Parse {
                    context: "Field validation".to_string(),
                    details: "Missing required field 'LapDist'".to_string(),
                })?;

        extraction_plan.push(FieldExtraction::Required {
            name: "LapDist".to_string(),
            var_info: lap_distance_meters_info.clone(),
        });

        let lap_distance_percentage_info =
            schema
                .get_variable("LapDistPct")
                .ok_or_else(|| IRacingSDKError::Parse {
                    context: "Field validation".to_string(),
                    details: "Missing required field 'LapDistPct'".to_string(),
                })?;

        extraction_plan.push(FieldExtraction::Required {
            name: "LapDistPct".to_string(),
            var_info: lap_distance_percentage_info.clone(),
        });

        let latitude_info = schema
            .get_variable("Lat")
            .ok_or_else(|| IRacingSDKError::Parse {
                context: "Field validation".to_string(),
                details: "Missing required field 'Lat'".to_string(),
            })?;

        extraction_plan.push(FieldExtraction::Required {
            name: "Lat".to_string(),
            var_info: latitude_info.clone(),
        });

        let longitude_info = schema
            .get_variable("Lon")
            .ok_or_else(|| IRacingSDKError::Parse {
                context: "Field validation".to_string(),
                details: "Missing required field 'Lon'".to_string(),
            })?;

        extraction_plan.push(FieldExtraction::Required {
            name: "Lon".to_string(),
            var_info: longitude_info.clone(),
        });

        let altitude_info = schema
            .get_variable("Alt")
            .ok_or_else(|| IRacingSDKError::Parse {
                context: "Field validation".to_string(),
                details: "Missing required field 'Alt'".to_string(),
            })?;

        extraction_plan.push(FieldExtraction::Required {
            name: "Alt".to_string(),
            var_info: altitude_info.clone(),
        });

        let is_on_pit_road_info =
            schema
                .get_variable("OnPitRoad")
                .ok_or_else(|| IRacingSDKError::Parse {
                    context: "Field validation".to_string(),
                    details: "Missing required field 'OnPitRoad'".to_string(),
                })?;

        extraction_plan.push(FieldExtraction::Required {
            name: "OnPitRoad".to_string(),
            var_info: is_on_pit_road_info.clone(),
        });

        let is_in_pit_box_info =
            schema
                .get_variable("PlayerCarInPitStall")
                .ok_or_else(|| IRacingSDKError::Parse {
                    context: "Field validation".to_string(),
                    details: "Missing required field 'PlayerCarInPitStall'".to_string(),
                })?;

        extraction_plan.push(FieldExtraction::Required {
            name: "PlayerCarInPitStall".to_string(),
            var_info: is_in_pit_box_info.clone(),
        });

        Ok(AdapterValidation::new(extraction_plan))
    }

    fn adapt(packet: &iracing_sdk::FramePacket, validation: &AdapterValidation) -> Self {
        Self {
            lap_distance_meters: validation.fetch_or_default::<f32>(packet, "LapDist"),
            lap_distance_percentage: validation.fetch_or_default::<f32>(packet, "LapDistPct"),
            latitude: validation.fetch_or_default::<f64>(packet, "Lat"),
            longitude: validation.fetch_or_default::<f64>(packet, "Lon"),
            altitude: validation.fetch_or_default::<f32>(packet, "Alt"),
            is_on_pit_road: validation.fetch_or_default::<bool>(packet, "OnPitRoad"),
            is_in_pit_box: validation.fetch_or_default::<bool>(packet, "PlayerCarInPitStall"),
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
            fs::write(&yml_output, session.to_string())?;
            tracing::info!(session_output_path = %yml_output.display(), "Session information written.")
        }
    }

    let mut writer = Writer::from_path(&csv_output_path).expect("Could not create CSV output");

    tracing::info!(
        total_frames = ibt_provider.total_frames(),
        "Parsing frames from IBT provider"
    );

    let schema = ibt_provider.schema();
    let shared_validation = Row::validate_schema(schema)?;
    while let Some(packet) = ibt_provider.next_frame().await? {
        let frame = Row::adapt(&packet, &shared_validation);
        // Serialize row to CSV.
        writer.serialize(frame)?;
    }

    writer.flush()?;
    tracing::info!(output_path = %csv_output_path.display(), "Finished processing frames");

    Ok(())
}
