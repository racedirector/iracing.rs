use std::{
    ffi::OsString,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use iracing_sdk::{LiveConnection, SessionInfo, reader::ibt::IbtReader};
use tracing_subscriber::EnvFilter;

use crate::SchemaOutputEncoding::{Json, JsonPretty, Yaml};

/// CLI arguments for the car setup schema generator.
///
/// Pass `--ibt-path` to source the setup from a replay file.
/// Omit it on Windows to read from the live iRacing connection instead.
#[derive(Parser)]
#[command(
    name = "car-setup",
    version,
    about = "iRacing car setup utilities",
    long_about = None,
    arg_required_else_help = true,
)]
struct Args {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Captures the JSON schema for a setup.
    Schema {
        #[command(subcommand)]
        commands: SchemaOutputCommands,
    },
    /// Captures a snapshot of the setup.
    Snapshot {
        #[command(subcommand)]
        commands: SnapshotOutputCommands,
    },
    /// Captures a snapshot of the setup on each update. Only available for live connections.
    #[cfg(windows)]
    Stream {
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,

        /// Output directory for updates. Defaults the the car's setup directory in a track-specific folder.
        #[arg(short, long = "output-dir")]
        output_dir: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum SchemaOutputCommands {
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,

        /// Output path
        #[arg(short, long = "output-path")]
        output_path: PathBuf,
    },
    #[cfg(windows)]
    Live {
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,

        /// Output path
        #[arg(short, long = "output-path")]
        output_path: PathBuf,
    },
}

#[derive(Subcommand)]
enum SnapshotOutputCommands {
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,

        /// Output path
        #[arg(short, long = "output-path")]
        output_path: PathBuf,
    },
    #[cfg(windows)]
    Live {
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,

        /// Output path
        #[arg(short, long = "output-path")]
        output_path: PathBuf,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum SchemaOutputEncoding {
    Json,
    JsonPretty,
    Yaml,
}

/// Opens an `.ibt` file and parses session info from the embedded YAML.
fn parse_disk_session(ibt_path: PathBuf) -> Result<SessionInfo> {
    tracing::info!(path = %ibt_path.display(), "Opening IBT file");

    let reader = IbtReader::open(&ibt_path)?;

    let session_buffer = reader
        .session_info_buffer()?
        .ok_or_else(|| anyhow!("No session YAML found in IBT file"))?;

    Ok(SessionInfo::try_from(session_buffer)?)
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

    Ok(SessionInfo::try_from(session_buffer)?)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    match args.commands {
        Commands::Schema { commands } => {
            handle_schema_command(commands)?;
        }
        Commands::Snapshot { commands } => {
            handle_snapshot_command(commands)?;
        }
        #[cfg(windows)]
        Commands::Stream {
            encoding,
            output_dir,
        } => handle_setup_stream(encoding, output_dir).await?,
    }

    Ok(())
}

fn handle_schema_command(source: SchemaOutputCommands) -> Result<()> {
    match source {
        SchemaOutputCommands::Ibt {
            path,
            output_path,
            encoding,
        } => {
            let session_info = parse_disk_session(path)?;
            let schema = schemars::schema_for_value!(session_info);
            write_to_output(&schema, &output_path, encoding)?;
        }
        #[cfg(windows)]
        SchemaOutputCommands::Live {
            output_path,
            encoding,
        } => {
            let session_info = parse_live_session()?;
            let schema = schemars::schema_for_value!(session_info);
            write_to_output(&schema, &output_path, encoding)?;
        }
    }

    Ok(())
}

fn handle_snapshot_command(source: SnapshotOutputCommands) -> Result<()> {
    match source {
        SnapshotOutputCommands::Ibt {
            path,
            encoding,
            output_path,
        } => {
            if let session_info = parse_disk_session(path)?
                && let Some(setup) = session_info.car_setup
            {
                write_to_output(&setup, &output_path, encoding)?;
            } else {
                return Err(anyhow::anyhow!("Could not find car setup on session info"));
            }
        }
        SnapshotOutputCommands::Live {
            encoding,
            output_path,
        } => {
            if let session_info = parse_live_session()?
                && let Some(setup) = session_info.car_setup
            {
                write_to_output(&setup, &output_path, encoding)?;
            } else {
                return Err(anyhow::anyhow!("Could not find car setup on session info"));
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
async fn handle_setup_stream(
    encoding: SchemaOutputEncoding,
    output_dir: Option<PathBuf>,
) -> Result<()> {
    use futures::StreamExt;

    let is_custom_output_dir = output_dir.is_some();

    let mut output_dir =
        output_dir.unwrap_or_else(|| dirs::home_dir().unwrap().join("Documents/iRacing/setups"));

    let connection = LiveConnection::builder().build()?;

    let mut stream = Box::pin(connection.session_updates());
    let mut previous_setup_update: i32 = -1;
    let mut is_output_dir_resolved = is_custom_output_dir;

    // Create the custom output if it doesn't already exist
    if is_custom_output_dir {
        std::fs::create_dir_all(&output_dir)?;
    }

    while let Some(session) = stream.next().await {
        if !is_output_dir_resolved && resolve_output_dir(&session, &mut output_dir).is_ok() {
            std::fs::create_dir_all(&output_dir)?;

            is_output_dir_resolved = true;
            tracing::info!(output_dir = %output_dir.display(), "Set output directory");
        }

        if let Some(setup) = &session.car_setup
            && previous_setup_update != setup.update_count
        {
            let output_path = resolve_output_path(&session, &output_dir, setup.update_count)?;
            write_to_output(setup, &output_path, encoding)?;
            previous_setup_update = setup.update_count
        }
    }

    Ok(())
}

fn resolve_output_dir(session: &SessionInfo, output_dir: &mut PathBuf) -> Result<()> {
    let driver_info = session
        .driver_info
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Could not find driver info on session info"))?;

    let driver_idx = driver_info
        .driver_car_idx
        .and_then(|idx| usize::try_from(idx).ok())
        .ok_or_else(|| anyhow::anyhow!("Could not find current driver index"))?;

    let driver = driver_info
        .drivers
        .as_ref()
        .and_then(|drivers| drivers.get(driver_idx))
        .ok_or_else(|| anyhow::anyhow!("Could not find current driver"))?;

    let car_path = driver
        .car_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Could not find car path on current driver"))?;

    output_dir.push(car_path);
    output_dir.push(&session.weekend_info.track_name);

    Ok(())
}

fn resolve_output_path(
    session: &SessionInfo,
    output_dir: &Path,
    update_count: i32,
) -> Result<PathBuf> {
    let driver_info = session
        .driver_info
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Could not find driver info on session info"))?;

    let setup_name = driver_info
        .driver_setup_name
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Could not find setup name in driver info"))?;

    let setup_path = Path::new(setup_name);

    let normalized_path = with_suffix(setup_path, update_count);

    Ok(output_dir.join(normalized_path))
}

fn with_suffix(path: &Path, index: i32) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default();
    let extension = path.extension();

    let timestamp = Local::now().format("%Y-%m-%d_%H%M%S");

    let mut name = OsString::from(stem);
    name.push(format!("_{timestamp}_{index}"));

    if let Some(extension) = extension {
        name.push(".");
        name.push(extension);
    }

    path.with_file_name(name)
}

fn write_to_output<T>(value: &T, output_path: &PathBuf, format: SchemaOutputEncoding) -> Result<()>
where
    T: ?Sized + serde::Serialize,
{
    let output_file = File::create(output_path)?;
    let writer = BufWriter::new(output_file);

    match format {
        Yaml => {
            serde_yaml_ng::to_writer(writer, &value)?;
        }
        Json => {
            serde_json::to_writer(writer, &value)?;
        }
        JsonPretty => {
            serde_json::to_writer_pretty(writer, &value)?;
        }
    }

    Ok(())
}
