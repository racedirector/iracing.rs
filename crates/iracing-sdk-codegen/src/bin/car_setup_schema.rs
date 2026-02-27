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
use iracing_sdk::{IbtReader, SessionInfo};
#[cfg(windows)]
use iracing_sdk::{SessionInfoParser, WindowsConnection};
use schemars::schema_for_value;
use tracing::info;
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
    info!(path = %ibt_path.display(), "Opening IBT file");
    let reader = IbtReader::open(&ibt_path)?;

    let session_yaml = reader
        .session_yaml()?
        .ok_or_else(|| anyhow!("No session YAML found in IBT file"))?;

    Ok(SessionInfo::parse(&session_yaml)?)
}

/// Connects to live iRacing shared memory and parses the current session info.
#[cfg(windows)]
fn parse_live_session() -> Result<SessionInfo> {
    info!("Opening iRacing connection");
    let connection = WindowsConnection::try_connect()?;

    if !connection.is_connected() {
        return Err(anyhow!(
            "iRacing is not connected (pass --allow-stale to continue)."
        ));
    }

    let raw_session_yaml = connection
        .session_info()
        .ok_or_else(|| anyhow!("No live session YAML is available"))?;

    let parser = SessionInfoParser::new();

    Ok(parser.parse(raw_session_yaml)?)
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
fn resolve_output_path(args: &Args, session_info: &SessionInfo) -> Result<PathBuf> {
    if let Some(path) = args.output_path.as_ref() {
        // If they passed a relative path, interpret it under output_dir.
        // If absolute, use as-is.
        return Ok(if path.is_absolute() {
            path.clone()
        } else {
            args.output_dir.join(path)
        });
    }

    let file_name = output_file_name(session_info)?; // String
    Ok(args.output_dir.join(file_name))
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    let session_info = if let Some(path) = args.ibt_path.clone() {
        parse_disk_session(path)?
    } else {
        parse_live_session()?
    };

    let schema = schema_for_value!(session_info.car_setup);

    let output_path = resolve_output_path(&args, &session_info)?;

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    Ok(())
}
