use anyhow::{Result, anyhow};
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use csv::Writer;
#[cfg(windows)]
use iracing_sdk::{VariableSchema, WaitResult, WindowsConnection, types::VarData};
#[cfg(windows)]
use std::{path::PathBuf, sync::Arc, thread, time::Duration};
#[cfg(windows)]
use tracing::{debug, info, trace};
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    csv_output_path: PathBuf,
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
    // ------------------------------------------------------------
    // Parse CLI arguments
    // ------------------------------------------------------------
    let Args { csv_output_path } = Args::parse();

    // ------------------------------------------------------------
    // Open iRacing connection and CSV writer
    // ------------------------------------------------------------
    let mut connection = WindowsConnection::try_connect().expect("Failed to connect to iRacing");
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
    let schema = Arc::new(VariableSchema::new(variable_map, frame_size)?);

    // ------------------------------------------------------------
    // Resolve required variable metadata
    // ------------------------------------------------------------
    let lap_distance_meters_info = schema
        .get_variable("LapDist")
        .expect("No `LapDist` in schema");

    let lap_distance_percentage_info = schema
        .get_variable("LapDistPct")
        .expect("No `LapDistPct` in schema");

    let is_on_pit_road_info = schema
        .get_variable("OnPitRoad")
        .expect("No `OnPitRoad` in schema");

    let is_in_pit_box_info = schema
        .get_variable("PlayerCarInPitStall")
        .expect("No `PlayerCarInPitStall` in schema");

    let mut writer = Writer::from_path(&csv_output_path).expect("Could not create CSV output");

    let mut was_connected = false;
    let mut wait_ticks = 0u32;
    loop {
        let is_connected = connection.is_connected();

        if was_connected && !is_connected {
            info!("iRacing disconnected; stopping telemetry capture.");
            break;
        }

        if !is_connected {
            wait_ticks += 1;

            if wait_ticks == 1 {
                info!("Waiting for iRacing to start a session...");
            } else if wait_ticks % 20 == 0 {
                debug!(
                    "Still waiting for iRacing session ({}s elapsed)",
                    wait_ticks / 2
                );
            }

            thread::sleep(Duration::from_millis(500));
            continue;
        }

        // Reset counter when we get a connection
        if wait_ticks > 0 {
            info!("iRacing session detected, resuming telemetry");
            wait_ticks = 0;
        }

        if !was_connected {
            was_connected = true;
        }

        if let Some(raw_data) = connection.get_new_data() {
            let data = raw_data.to_vec();

            // Extract strongly-typed values from raw frame bytes.
            let lap_distance_meters = f32::from_bytes(&data, lap_distance_meters_info).unwrap();

            let lap_distance_percentage =
                f32::from_bytes(&data, lap_distance_percentage_info).unwrap();

            let is_on_pit_road = bool::from_bytes(&data, is_on_pit_road_info).unwrap();

            let is_in_pit_box = bool::from_bytes(&data, is_in_pit_box_info).unwrap();

            // Serialize row to CSV.
            writer.serialize(Row {
                lap_distance_meters,
                lap_distance_percentage,
                is_in_pit_box,
                is_on_pit_road,
            })?;
        }

        match connection.wait_for_update(Duration::from_millis(500))? {
            WaitResult::Signaled => {
                trace!("Event signaled, checking for new data");
                continue;
            }
            WaitResult::Timeout => {
                trace!("Wait timeout, continuing to poll");
                continue;
            }
        }
    }

    writer.flush()?;
    info!("Finished processing frames");

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live-position example is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("live-position example is only supported on Windows"))
}
