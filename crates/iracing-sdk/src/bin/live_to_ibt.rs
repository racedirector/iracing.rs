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

/// Program entry point: initialize logging and run the application.
///
/// Initializes a tracing subscriber using the `RUST_LOG` environment variable as the filter;
/// if `RUST_LOG` is not set or invalid, the filter defaults to `"trace"`. After configuring
/// logging, delegates execution to `run()`.
///
/// # Returns
///
/// `Ok(())` on success, or the error returned by `run()`.
///
/// # Examples
///
/// ```
/// // Call the program entry point and ensure it runs without error in tests/examples.
/// let result = crate::main();
/// assert!(result.is_ok());
/// ```
fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

/// Runs the live-to-ibt recording process: connects to iRacing, projects a fixed set of telemetry
/// variables, and streams projected frames into the IBT file specified by the CLI `--output-path`
/// until iRacing disconnects.
///
/// On success, finalizes the IBT file and returns without error.
///
/// # Examples
///
/// ```no_run
/// // On Windows, invoke the CLI entrypoint to start recording to the path provided via
/// // `--output-path`. This example demonstrates the simplest programmatic invocation.
/// #[cfg(windows)]
/// fn main() {
///     live_to_ibt::run().unwrap();
/// }
/// ```
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

/// Runs the CLI on non-Windows platforms and immediately returns an error indicating Windows-only support.
///
/// This build of `run` always fails on non-Windows targets because the tool depends on iRacing's Windows shared memory APIs.
///
/// # Returns
///
/// An `Err` describing that live_to_ibt is only supported on Windows.
///
/// # Examples
///
/// ```
/// # #[cfg(not(windows))] {
/// assert!(crate::run().is_err());
/// # }
/// ```
#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live_to_ibt is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );

    Err(anyhow!("live_to_ibt is only supported on Windows"))
}
