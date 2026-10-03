//! # ibt-read-frame
//!
//! Extracts positional telemetry from an iRacing `.ibt` file and writes it to CSV.
//!
//! ## Overview
//!
//! This CLI utility:
//!
//! 1. Opens an iRacing IBT telemetry file
//! 2. Resolves required telemetry variables from the file layout
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
//! cargo run --example ibt-read-frame -- \
//!   --ibt-path ./session.ibt \
//!   --output-path ./positions.csv
//! ```
//!
//! ## Logging
//!
//! Logging is controlled via `RUST_LOG`. Example:
//!
//! ```bash
//! RUST_LOG=ibt-read-frame=info cargo run --example ibt-read-frame -- ...
//! ```

use anyhow::Result;
use clap::Parser;
use csv::Writer;
use futures::executor::block_on;
use iracing_sdk::provider::Provider;
use iracing_sdk::{LayoutProvider, providers::ibt::IbtProvider, types::VarData};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

/// CLI arguments for the `ibt-read-frame` extractor.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
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

/// Entry point for the CLI.
///
/// # Flow
///
/// 1. Initialize logging
/// 2. Parse CLI arguments
/// 3. Open IBT reader
/// 4. Resolve required layout variables
/// 5. Iterate frames
/// 6. Serialize CSV rows
fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
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
    let mut reader = IbtProvider::open(&ibt_path).expect("Failed to open IBT file");
    let mut writer = Writer::from_path(&csv_output_path).expect("Could not create CSV output");

    tracing::info!("Resolving telemetry layout");

    // Clone the layout once to avoid repeated lookups.
    let layout = reader.layout().clone();

    // ------------------------------------------------------------
    // Resolve required variable metadata
    // ------------------------------------------------------------
    let lap_distance_meters_info = layout
        .field_by_name("LapDist")
        .map(|(_, field)| field)
        .expect("No `LapDist` in layout");

    let lap_distance_percentage_info = layout
        .field_by_name("LapDistPct")
        .map(|(_, field)| field)
        .expect("No `LapDistPct` in layout");

    let latitude_info = layout
        .field_by_name("Lat")
        .map(|(_, field)| field)
        .expect("No `Lat` in layout");

    let longitude_info = layout
        .field_by_name("Lon")
        .map(|(_, field)| field)
        .expect("No `Lon` in layout");

    let altitude_info = layout
        .field_by_name("Alt")
        .map(|(_, field)| field)
        .expect("No `Alt` in layout");

    let is_on_pit_road_info = layout
        .field_by_name("OnPitRoad")
        .map(|(_, field)| field)
        .expect("No `OnPitRoad` in layout");

    let is_in_pit_box_info = layout
        .field_by_name("PlayerCarInPitStall")
        .map(|(_, field)| field)
        .expect("No `PlayerCarInPitStall` in layout");

    tracing::info!("Beginning frame iteration");

    // ------------------------------------------------------------
    // Frame iteration
    // ------------------------------------------------------------
    //
    // `next_frame()` returns:
    //   Result<Option<FramePacket>>
    //
    // - Err(_)       => read failure
    // - Ok(None)     => end-of-stream
    // - Ok(Some(...))=> next frame
    //
    while let Some(packet) = block_on(reader.next_frame())? {
        let data = packet.data();
        // Extract strongly-typed values from raw frame bytes.
        let lap_distance_meters = f32::decode_field(data, lap_distance_meters_info).unwrap();

        let lap_distance_percentage =
            f32::decode_field(data, lap_distance_percentage_info).unwrap();

        let latitude = f64::decode_field(data, latitude_info).unwrap();

        let longitude = f64::decode_field(data, longitude_info).unwrap();

        let altitude = f32::decode_field(data, altitude_info).unwrap();

        let is_on_pit_road = bool::decode_field(data, is_on_pit_road_info).unwrap();

        let is_in_pit_box = bool::decode_field(data, is_in_pit_box_info).unwrap();

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
    tracing::info!("Finished processing frames");

    Ok(())
}
