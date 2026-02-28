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
//! live-to-ibt --output-path <OUTPUT_FILE.ibt>
//! ```

use anyhow::{Context, Result, anyhow};
use clap::Parser;
#[cfg(windows)]
use iracing_sdk::{
    FrameProjection, IbtWriteOptions, IbtWriter, VariableSchema, WaitResult, WindowsConnection,
};
use std::path::PathBuf;
#[cfg(windows)]
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
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
    let Args { output_path } = Args::parse();

    info!("Opening iRacing connection");
    let mut connection =
        WindowsConnection::try_connect().context("Failed to connect to iRacing shared memory")?;

    if !connection.is_connected() {
        return Err(anyhow!("iRacing is not connected."));
    }

    // Build schema from variables
    let variables: Vec<_> = connection.get_variables();
    let mut variable_map = std::collections::HashMap::new();

    for var_info in variables {
        variable_map.insert(var_info.name.clone(), var_info);
    }

    let frame_size = connection.header().buf_len as usize;
    let schema = VariableSchema::new(variable_map, frame_size)?;

    // ------------------------------------------------------------
    // Create a projection of the variables you want from the
    // source connection
    // ------------------------------------------------------------
    let projection = FrameProjection::from_variable_names(
        &schema,
        [
            "CarIdxBestLapNum",
            "CarIdxBestLapTime",
            "CarIdxClass",
            "CarIdxClassPosition",
            "CarIdxEstTime",
            "CarIdxF2Time",
            "CarIdxFastRepairsUsed",
            "CarIdxGear",
            "CarIdxLap",
            "CarIdxLapCompleted",
            "CarIdxLapDistPct",
            "CarIdxLastLapTime",
            "CarIdxOnPitRoad",
            "CarIdxP2P_Count",
            "CarIdxP2P_Status",
            "CarIdxPaceFlags",
            "CarIdxPaceLine",
            "CarIdxPosition",
            "CarIdxQualTireCompound",
            "CarIdxQualTireCompoundLocked",
            "CarIdxRPM",
            "CarIdxSessionFlags",
            "CarIdxSteer",
            "CarIdxTireCompound",
            "CarIdxTrackSurface",
            "CarIdxTrackSurfaceMaterial",
        ],
    )?;

    let options = IbtWriteOptions::from_connection(&connection)?;
    let mut writer = IbtWriter::create(&output_path, projection.target_schema().clone(), options)?;
    let mut target_buffer = vec![0u8; projection.target_schema().frame_size];

    info!("Recording to {:?}", output_path);

    loop {
        match connection.wait_for_update(Duration::from_millis(100))? {
            WaitResult::Signaled => {
                if let Some(frame_data) = connection.get_new_data() {
                    writer.write_projected_frame_with_buffer(
                        frame_data,
                        &projection,
                        &mut target_buffer,
                    )?;
                }
            }
            WaitResult::Timeout => {
                if !connection.is_connected() {
                    info!("iRacing disconnected, stopping recording");
                    break;
                }
            }
        }
    }

    info!("Wrote {} frames, finalizing", writer.frame_count());
    writer.finish()?;
    info!("Recording complete: {:?}", output_path);

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live_to_ibt is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );

    Err(anyhow!("live_to_ibt is only supported on Windows"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_returns_error_on_non_windows() {
        #[cfg(not(windows))]
        {
            let result = run();
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(err_msg.contains("Windows"));
        }
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use anyhow::Result;
    use iracing_sdk::{VariableInfo, VariableSchema, VariableType};
    use std::collections::HashMap;

    fn build_test_schema() -> VariableSchema {
        let mut vars = HashMap::new();

        // Add variables that match the live_to_ibt hardcoded list
        let car_vars = vec![
            "CarIdxBestLapNum",
            "CarIdxBestLapTime",
            "CarIdxClass",
            "CarIdxClassPosition",
            "CarIdxEstTime",
            "CarIdxF2Time",
            "CarIdxFastRepairsUsed",
            "CarIdxGear",
            "CarIdxLap",
            "CarIdxLapCompleted",
            "CarIdxLapDistPct",
            "CarIdxLastLapTime",
            "CarIdxOnPitRoad",
            "CarIdxP2P_Count",
            "CarIdxP2P_Status",
            "CarIdxPaceFlags",
            "CarIdxPaceLine",
            "CarIdxPosition",
            "CarIdxQualTireCompound",
            "CarIdxQualTireCompoundLocked",
            "CarIdxRPM",
            "CarIdxSessionFlags",
            "CarIdxSteer",
            "CarIdxTireCompound",
            "CarIdxTrackSurface",
            "CarIdxTrackSurfaceMaterial",
        ];

        let mut offset = 0;
        for var_name in car_vars {
            vars.insert(
                var_name.to_string(),
                VariableInfo {
                    name: var_name.to_string(),
                    data_type: VariableType::Float32,
                    offset,
                    count: 64, // Typical array size for car index variables
                    count_as_time: false,
                    units: "".to_string(),
                    description: format!("{} description", var_name),
                },
            );
            offset += 64 * 4; // 64 floats * 4 bytes each
        }

        VariableSchema::new(vars, offset).expect("valid schema")
    }

    #[test]
    fn schema_contains_expected_variables() -> Result<()> {
        let schema = build_test_schema();

        // Verify all expected variables are present
        assert!(schema.get_variable("CarIdxBestLapNum").is_some());
        assert!(schema.get_variable("CarIdxBestLapTime").is_some());
        assert!(schema.get_variable("CarIdxLapDistPct").is_some());
        assert!(schema.get_variable("CarIdxPosition").is_some());

        Ok(())
    }

    #[test]
    fn projection_creates_correct_frame_size() -> Result<()> {
        let schema = build_test_schema();

        let projection = FrameProjection::from_variable_names(
            &schema,
            [
                "CarIdxBestLapNum",
                "CarIdxLapDistPct",
                "CarIdxPosition",
            ],
        )?;

        // Each variable is 64 floats, so total is 64 * 4 * 3 = 768 bytes
        assert_eq!(projection.target_schema().frame_size, 64 * 4 * 3);

        Ok(())
    }

    #[test]
    fn projection_validates_all_hardcoded_variables_exist() -> Result<()> {
        let schema = build_test_schema();

        // This should succeed since our test schema includes all these variables
        let projection = FrameProjection::from_variable_names(
            &schema,
            [
                "CarIdxBestLapNum",
                "CarIdxBestLapTime",
                "CarIdxClass",
                "CarIdxClassPosition",
                "CarIdxEstTime",
                "CarIdxF2Time",
                "CarIdxFastRepairsUsed",
                "CarIdxGear",
                "CarIdxLap",
                "CarIdxLapCompleted",
                "CarIdxLapDistPct",
                "CarIdxLastLapTime",
                "CarIdxOnPitRoad",
                "CarIdxP2P_Count",
                "CarIdxP2P_Status",
                "CarIdxPaceFlags",
                "CarIdxPaceLine",
                "CarIdxPosition",
                "CarIdxQualTireCompound",
                "CarIdxQualTireCompoundLocked",
                "CarIdxRPM",
                "CarIdxSessionFlags",
                "CarIdxSteer",
                "CarIdxTireCompound",
                "CarIdxTrackSurface",
                "CarIdxTrackSurfaceMaterial",
            ],
        )?;

        assert_eq!(projection.target_schema().variables.len(), 26);

        Ok(())
    }

    #[test]
    fn projection_fails_on_missing_variable() {
        let schema = build_test_schema();

        // Try to project a variable that doesn't exist
        let result = FrameProjection::from_variable_names(
            &schema,
            ["CarIdxLapDistPct", "NonExistentVariable"],
        );

        assert!(result.is_err());
    }

    #[test]
    fn ibt_write_options_default_has_valid_tick_rate() {
        let options = IbtWriteOptions::default();
        assert!(options.tick_rate > 0);
        assert!(options.lap_count >= 0);
    }
}