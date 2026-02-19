use anyhow::Result;
use clap::Parser;
use csv::Writer;
use iracing_sdk::IRacingSDKError;
use iracing_sdk_adapter::{
    AdapterValidation, FieldExtraction, FrameAdapter, IbtProvider, Provider,
};
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

impl FrameAdapter for Row {
    fn validate_schema(
        schema: &iracing_sdk::VariableSchema,
    ) -> iracing_sdk_adapter::Result<AdapterValidation> {
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

    fn adapt(packet: &iracing_sdk_adapter::FramePacket, validation: &AdapterValidation) -> Self {
        let lap_distance_meters = validation.fetch_or_default::<f32>(packet, "LapDist");
        let lap_distance_percentage = validation.fetch_or_default::<f32>(packet, "LapDistPct");
        let latitude = validation.fetch_or_default::<f64>(packet, "Lat");
        let longitude = validation.fetch_or_default::<f64>(packet, "Lon");
        let altitude = validation.fetch_or_default::<f32>(packet, "Alt");
        let is_on_pit_road = validation.fetch_or_default::<bool>(packet, "OnPitRoad");
        let is_in_pit_box = validation.fetch_or_default::<bool>(packet, "PlayerCarInPitStall");

        Self {
            lap_distance_meters,
            lap_distance_percentage,
            latitude,
            longitude,
            altitude,
            is_in_pit_box,
            is_on_pit_road,
        }
    }
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
    let schema = ibt_provider.schema();

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

    let shared_validation = Row::validate_schema(&schema)?;
    while let Some(packet) = ibt_provider.next_frame()? {
        let frame = Row::adapt(&packet, &shared_validation);
        // Serialize row to CSV.
        writer.serialize(frame)?;
    }

    writer.flush()?;
    info!(output_path = %csv_output_path.display(), "Finished processing frames");

    Ok(())
}
