//! # position
//!
//! Extracts positional telemetry from an iRacing `.ibt` file and writes it to CSV.
//!
//! ## Overview
//!
//! This CLI utility:
//!
//! 1. Opens an iRacing IBT telemetry file
//! 2. Resolves required telemetry variables from the file schema
//! 3. Iterates through all telemetry frames
//! 4. Extracts positional and pit-state fields
//! 5. Serializes them to a CSV file
//!
//! ## Extracted Variables
//!
//! | Variable Name              | Type  | Description |
//! |----------------------------|-------|-------------|
//! | `LapDist`                  | f32   | Distance around track in meters |
//! | `LapDistPct`               | f32   | Lap distance as percentage |
//! | `Lat`                      | f64   | GPS latitude |
//! | `Lon`                      | f64   | GPS longitude |
//! | `Alt`                      | f32   | Altitude |
//! | `OnPitRoad`                | bool  | Whether the car is on pit road |
//! | `PlayerCarInPitStall`      | bool  | Whether the car is in its pit box |
//!
//! ## Usage
//!
//! ```bash
//! cargo run --bin position -- \
//!   --ibt-path ./session.ibt \
//!   --output-path ./positions.csv
//! ```
//!
//! ## Logging
//!
//! Logging is controlled via `RUST_LOG`. Example:
//!
//! ```bash
//! RUST_LOG=position=info cargo run --bin position -- ...
//! ```

use anyhow::Result;
use clap::Parser;
use csv::Writer;
use iracing_sdk::{IbtReader, types::VarData};
use std::path::PathBuf;
use tracing::info;

/// CLI arguments for the `position` extractor.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct PositionArgs {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the output CSV should be written.
    #[arg(short, long)]
    output_path: PathBuf,
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

/// Entry point for the CLI.
///
/// # Flow
///
/// 1. Initialize logging
/// 2. Parse CLI arguments
/// 3. Open IBT reader
/// 4. Resolve required schema variables
/// 5. Iterate frames
/// 6. Serialize CSV rows
fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization
    // ------------------------------------------------------------
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "position=info".to_string()))
        .init();

    // ------------------------------------------------------------
    // Parse CLI arguments
    // ------------------------------------------------------------
    let PositionArgs {
        ibt_path,
        output_path,
    } = PositionArgs::parse();

    info!(path = %ibt_path.display(), "Opening IBT file");

    // ------------------------------------------------------------
    // Open telemetry reader and CSV writer
    // ------------------------------------------------------------
    let mut reader = IbtReader::open(&ibt_path).expect("Failed to open IBT file");

    let mut writer = Writer::from_path(&output_path).expect("Could not create CSV output");

    info!("Resolving telemetry schema");

    // Clone the schema once to avoid repeated lookups.
    let schema = reader.variables().clone();

    // ------------------------------------------------------------
    // Resolve required variable metadata
    // ------------------------------------------------------------
    let lap_distance_meters_info = schema
        .get_variable("LapDist")
        .expect("No `LapDist` in schema");

    let lap_distance_percentage_info = schema
        .get_variable("LapDistPct")
        .expect("No `LapDistPct` in schema");

    let latitude_info = schema.get_variable("Lat").expect("No `Lat` in schema");

    let longitude_info = schema.get_variable("Lon").expect("No `Lon` in schema");

    let altitude_info = schema.get_variable("Alt").expect("No `Alt` in schema");

    let is_on_pit_road_info = schema
        .get_variable("OnPitRoad")
        .expect("No `OnPitRoad` in schema");

    let is_in_pit_box_info = schema
        .get_variable("PlayerCarInPitStall")
        .expect("No `PlayerCarInPitStall` in schema");

    info!("Beginning frame iteration");

    // ------------------------------------------------------------
    // Frame iteration
    // ------------------------------------------------------------
    //
    // `read_next_frame()` returns:
    //   Result<Option<(data, tick, session_version)>>
    //
    // - Err(_)       => read failure
    // - Ok(None)     => end-of-stream
    // - Ok(Some(...))=> next frame
    //
    while let Some((data, _tick, _session_version)) = reader.read_next_frame()? {
        // Extract strongly-typed values from raw frame bytes.
        let lap_distance_meters = f32::from_bytes(&data, lap_distance_meters_info).unwrap();

        let lap_distance_percentage = f32::from_bytes(&data, lap_distance_percentage_info).unwrap();

        let latitude = f64::from_bytes(&data, latitude_info).unwrap();

        let longitude = f64::from_bytes(&data, longitude_info).unwrap();

        let altitude = f32::from_bytes(&data, altitude_info).unwrap();

        let is_on_pit_road = bool::from_bytes(&data, is_on_pit_road_info).unwrap();

        let is_in_pit_box = bool::from_bytes(&data, is_in_pit_box_info).unwrap();

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

    info!("Finished processing frames");

    writer.flush()?;

    Ok(())
}
