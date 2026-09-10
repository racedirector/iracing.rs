mod output_writer;

use anyhow::Result;
use clap::{Parser, Subcommand};
use iracing_sdk::{ibt::reader::IbtReader, schema::SessionInfo};
use schemars::{schema_for, schema_for_value};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use output_writer::{OutputEncoding, OutputTarget, write_to_output};

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
    /// Captures JSON schema of a session string.
    Schema {
        #[command(subcommand)]
        commands: SchemaOutputCommands,
    },
    /// Discovers schema additions of a session string.
    Discover {
        #[command(subcommand)]
        commands: DiscoveryCommands,
    },
    /// Captures a snapshot of the latest session string.
    Snapshot {
        #[command(subcommand)]
        commands: SnapshotOutputCommands,
    },
}

#[derive(Subcommand)]
enum SchemaOutputCommands {
    /// Captures the most-recent session string and outputs JSON schema to `output` in
    /// the requested format.
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the JSON schema.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
    /// Captures the session string from the IBT file and outputs JSON schema to `output`
    /// in the requested format.
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the JSON schema.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
    /// Outputs a JSON schema of the underlying library type to the output in the requested format.
    Type {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the JSON schema.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
}

#[derive(Subcommand)]
enum DiscoveryCommands {
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
}

#[derive(Subcommand)]
enum SnapshotOutputCommands {
    /// Captures the latest session string from a live iRacing connection and outputs it to the destination in the requested format.
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
    /// Captures the session string from the IBT file and outputs it to the destination in the requested format.
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
}

fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    match args.commands {
        Commands::Snapshot { commands } => {
            handle_snapshot_command(commands)?;
        }
        Commands::Schema { commands } => {
            handle_schema_command(commands)?;
        }
        Commands::Discover { commands } => {
            handle_discovery_command(commands)?;
        }
    }

    Ok(())
}

fn handle_discovery_command(command: DiscoveryCommands) -> Result<()> {
    match command {
        DiscoveryCommands::Live { output, encoding } => {
            let session_info = capture_live_session_info()?;
            let unknown_fields = session_info.collect_unknown_fields();
            write_to_output(&unknown_fields, &output, encoding)?;
        }
        DiscoveryCommands::Ibt {
            path,
            output,
            encoding,
        } => {
            let session_info = capture_disk_session_info(&path)?;
            let unknown_fields = session_info.collect_unknown_fields();
            write_to_output(&unknown_fields, &output, encoding)?;
        }
    }

    Ok(())
}

fn handle_snapshot_command(command: SnapshotOutputCommands) -> Result<()> {
    match command {
        SnapshotOutputCommands::Ibt {
            path,
            output,
            encoding,
        } => {
            let session_info = capture_disk_session_info(&path)?;
            write_to_output(&session_info, &output, encoding)?;
            tracing::info!(output=%output,"Wrote disk session snapshot.");
        }
        #[cfg(windows)]
        SnapshotOutputCommands::Live { output, encoding } => {
            let session_info = capture_live_session_info()?;
            write_to_output(&session_info, &output, encoding)?;
            tracing::info!(output=%output, "Wrote live session snapshot.");
        }
    }

    Ok(())
}

fn handle_schema_command(command: SchemaOutputCommands) -> Result<()> {
    match command {
        SchemaOutputCommands::Ibt {
            path,
            output,
            encoding,
        } => {
            let session_info = capture_disk_session_info(&path)?;
            let schema = schema_for_value!(session_info);
            write_to_output(&schema, &output, encoding)?;
            tracing::info!(output=%output, path=%path.display(),"Wrote IBT session schema");
        }
        #[cfg(windows)]
        SchemaOutputCommands::Live { output, encoding } => {
            let session_info = capture_live_session_info()?;
            let schema = schema_for_value!(session_info);
            write_to_output(&schema, &output, encoding)?;
            tracing::info!(output=%output,"Wrote live session schema");
        }
        SchemaOutputCommands::Type { output, encoding } => {
            let schema = schema_for!(iracing_sdk::schema::SessionInfo);
            write_to_output(&schema, &output, encoding)?;
            tracing::info!(output=%output,"Wrote static session schema");
        }
    }

    Ok(())
}

fn capture_disk_session_info(ibt_path: &PathBuf) -> Result<SessionInfo> {
    let reader = IbtReader::open(ibt_path)?;

    let buffer = reader
        .session_info_buffer()
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
