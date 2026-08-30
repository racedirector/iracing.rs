mod schema_writer;

use anyhow::Result;
use clap::{Parser, Subcommand};
use iracing_sdk::{SessionInfo, reader::ibt::IbtReader};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use schema_writer::{SchemaOutputEncoding, write_to_output};

#[derive(Parser)]
#[command(
    name = "session",
    version,
    about = "iRacing session info utilities",
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
}

#[derive(Subcommand)]
enum SchemaOutputCommands {
    #[cfg(windows)]
    Live {
        /// Path where the session YAML should be written.
        #[arg(short, long, default_value = "live-session-schema.yml")]
        output_path: PathBuf,

        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,
    },
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        /// Path where the session YAML should be written.
        #[arg(short, long, default_value = "disk-session-schema.yml")]
        output_path: PathBuf,

        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,
    },
}

#[derive(Subcommand)]
enum SnapshotOutputCommands {
    #[cfg(windows)]
    Live {
        /// Path where the session YAML should be written.
        #[arg(short, long, default_value = "live-session-snapshot.yml")]
        output_path: PathBuf,

        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,
    },
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        /// Path where the session YAML should be written.
        #[arg(short, long, default_value = "disk-session-snapshot.yml")]
        output_path: PathBuf,

        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,
    },
}

fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    match args.commands {
        Commands::Snapshot { commands } => {
            handle_snapshot_command(commands)?;
        }
        #[cfg(windows)]
        Commands::Schema { commands } => {
            handle_schema_command(commands)?;
        }
    }

    Ok(())
}

fn handle_snapshot_command(command: SnapshotOutputCommands) -> Result<()> {
    match command {
        SnapshotOutputCommands::Ibt {
            path,
            output_path,
            encoding,
        } => {
            let session_info = capture_disk_session_info(&path)?;
            write_to_output(&session_info, &output_path, encoding)?;
            tracing::info!(output_path=%output_path.display(),"Wrote disk session snapshot.");
        }
        #[cfg(windows)]
        SnapshotOutputCommands::Live {
            output_path,
            encoding,
        } => {
            let session_info = capture_live_session_info()?;
            write_to_output(&session_info, &output_path, encoding)?;
            tracing::info!(output_path=%output_path.display(), "Wrote live session snapshot.");
        }
    }

    Ok(())
}

fn handle_schema_command(command: SchemaOutputCommands) -> Result<()> {
    match command {
        SchemaOutputCommands::Ibt {
            path,
            output_path,
            encoding,
        } => {
            let session_info = capture_disk_session_info(&path)?;
            let schema = schemars::schema_for_value!(session_info);
            write_to_output(&schema, &output_path, encoding)?;
        }
        SchemaOutputCommands::Live {
            output_path,
            encoding,
        } => {
            let session_info = capture_live_session_info()?;
            let schema = schemars::schema_for_value!(session_info);
            write_to_output(&schema, &output_path, encoding)?;
        }
    }

    Ok(())
}

fn capture_disk_session_info(ibt_path: &PathBuf) -> Result<SessionInfo> {
    let reader = IbtReader::open(ibt_path)?;

    let buffer = reader
        .session_info_buffer()?
        .ok_or_else(|| anyhow::anyhow!("IBT contains no session information"))?;

    Ok(SessionInfo::try_from(buffer)?)
}

#[cfg(windows)]
fn capture_live_session_info() -> Result<SessionInfo> {
    use iracing_sdk::WindowsConnection;

    let connection = match WindowsConnection::try_connect() {
        Ok(c) if c.is_connected() => c,
        Ok(_) => {
            return Err(anyhow::anyhow!(
                "Shared memory opened but telemetry is not connected yet"
            ));
        }
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    let buffer = connection
        .session_info_buffer()
        .ok_or_else(|| anyhow::anyhow!("Live connection contains no session information"))?;

    Ok(SessionInfo::try_from(buffer)?)
}
