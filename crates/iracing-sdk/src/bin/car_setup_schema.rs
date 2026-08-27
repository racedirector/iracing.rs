//! Car setup schema generator.
//!
//! Reads a car setup from an iRacing `.ibt` telemetry file or live iRacing shared
//! memory (Windows only), then emits a JSON Schema (serialized as YAML) describing
//! the structure of `SessionInfo::car_setup`.
//!
//! The output filename is computed from `CarID` and `SeriesID` embedded in the
//! session unless `--output-path` is given explicitly.
//!
//! # Usage
//! ```text
//! car_setup_schema --ibt-path <FILE.ibt> [--output-dir <DIR>] [--output-path <SCHEMA.yml>]
//! car_setup_schema [--output-dir <DIR>] [--output-path <SCHEMA.yml>]   # live (Windows)
//! ```

use std::{fs::File, io::BufWriter, path::PathBuf};

use anyhow::{Result, anyhow};
use clap::Parser;
use iracing_sdk::{SessionInfo, reader::ibt::IbtReader, yaml_utils};
use tracing_subscriber::EnvFilter;

/// CLI arguments for the car setup schema generator.
///
/// Pass `--ibt-path` to source the setup from a replay file.
/// Omit it on Windows to read from the live iRacing connection instead.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: Option<PathBuf>,

    /// Directory where the output YAML should be written.
    #[arg(short = 'o', long = "output-dir", default_value = ".")]
    output_dir: PathBuf,

    /// Optional explicit output path (overrides --output-dir + computed filename).
    #[arg(long = "output-path")]
    output_path: Option<PathBuf>,
}

/// Opens an `.ibt` file and parses session info from the embedded YAML.
fn parse_disk_session(ibt_path: PathBuf) -> Result<SessionInfo> {
    tracing::info!(path = %ibt_path.display(), "Opening IBT file");

    let reader = IbtReader::open(&ibt_path)?;

    let raw_session_yaml: String = reader
        .session_info_buffer()?
        .ok_or_else(|| anyhow!("No session YAML found in IBT file"))?
        .try_into()?;
    let session_yaml = yaml_utils::preprocess_iracing_yaml(&raw_session_yaml)?;

    Ok(SessionInfo::parse(&session_yaml)?)
}

/// Connects to live iRacing shared memory and parses the current session info.
#[cfg(windows)]
fn parse_live_session() -> Result<SessionInfo> {
    use iracing_sdk::WindowsConnection;

    tracing::info!("Opening iRacing connection");

    let connection = WindowsConnection::try_connect()?;

    if !connection.is_connected() {
        return Err(anyhow!("iRacing is not connected."));
    }

    let session_buffer = connection
        .session_info_buffer()
        .ok_or_else(|| anyhow!("No live session YAML is available"))?;
    let raw_session_yaml: String = session_buffer.try_into()?;

    let session_yaml = yaml_utils::preprocess_iracing_yaml(&raw_session_yaml)?;
    Ok(SessionInfo::parse(&session_yaml)?)
}

/// Non-Windows stub — always returns an error directing the caller to use `--ibt-path`.
#[cfg(not(windows))]
fn parse_live_session() -> Result<SessionInfo> {
    Err(anyhow!(
        "live session parsing is only supported on Windows; pass --ibt-path to parse from disk."
    ))
}

/// Computes a default output filename of the form `<CarID>-<SeriesID>-setup.yml`.
fn output_file_name(session_info: &SessionInfo) -> Result<String> {
    let series_id = session_info
        .weekend_info
        .series_id
        .ok_or_else(|| anyhow!("Could not get `SeriesID`"))?;

    let driver_info = session_info
        .driver_info
        .as_ref()
        .ok_or_else(|| anyhow!("Could not get `DriverInfo`"))?;

    let driver_idx: usize = driver_info
        .driver_car_idx
        .ok_or_else(|| anyhow!("Could not get `DriverCarIdx`"))?
        .try_into()
        .map_err(|_| anyhow!("Invalid `DriverCarIdx`"))?;

    let drivers = driver_info
        .drivers
        .as_ref()
        .ok_or_else(|| anyhow!("Could not find `Drivers`"))?;

    let driver_car_id = drivers[driver_idx]
        .car_id
        .ok_or_else(|| anyhow!("Could not find `CarID`"))?;

    Ok(format!("{}-{}-setup.yml", driver_car_id, series_id))
}

/// Resolves the final output path, preferring an explicit `--output-path` when provided.
fn resolve_output_path(
    session_info: &SessionInfo,
    output_dir: PathBuf,
    output_path: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(path) = output_path {
        // If they passed a relative path, interpret it under output_dir.
        // If absolute, use as-is.
        return Ok(if path.is_absolute() {
            path
        } else {
            output_dir.join(path)
        });
    }

    let file_name = output_file_name(session_info)?;
    Ok(output_dir.join(file_name))
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args {
        ibt_path,
        output_dir,
        output_path,
    } = Args::parse();

    let session_info = if let Some(path) = ibt_path {
        parse_disk_session(path)?
    } else {
        parse_live_session()?
    };

    let schema = schemars::schema_for_value!(session_info.car_setup);

    let output_path = resolve_output_path(&session_info, output_dir, output_path)?;

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    Ok(())
}
