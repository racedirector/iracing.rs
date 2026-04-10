use anyhow::{anyhow, Result};
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use iracing_sdk::IRacingSDKError;
#[cfg(windows)]
use iracing_sdk::{AdapterValidation, FieldExtraction, FrameAdapter, LiveProvider};
#[cfg(windows)]
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    csv_output_path: PathBuf,
    // #[arg(short, long)]
    // yml_output_path: Option<PathBuf>,
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

#[cfg(windows)]
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
            // Uncomment these to demonstrate when fields are missing.
            // latitude: validation.fetch_or_default::<f64>(packet, "Lat"),
            // longitude: validation.fetch_or_default::<f64>(packet, "Lon"),
            // altitude: validation.fetch_or_default::<f32>(packet, "Alt"),
            is_on_pit_road: validation.fetch_or_default::<bool>(packet, "OnPitRoad"),
            is_in_pit_box: validation.fetch_or_default::<bool>(packet, "PlayerCarInPitStall"),
        }
    }
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
    use csv::Writer;
    use iracing_sdk::Provider;
    use tracing::info;

    let Args { csv_output_path } = Args::parse();

    let mut live_provider = LiveProvider::new().expect("Could not create LiveProvider");
    let schema = live_provider.schema();
    let mut writer = Writer::from_path(&csv_output_path).expect("Could not create CSV output");

    info!("Parsing frames from live connection");

    let shared_validation = Row::validate_schema(&schema)?;
    while let Some(packet) = live_provider.next_frame()? {
        let frame = Row::adapt(&packet, &shared_validation);
        writer.serialize(frame)?;
    }

    writer.flush()?;
    info!(output_path = %csv_output_path.display(), "Finished processing frames");

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
