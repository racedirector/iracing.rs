mod schema_writer;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use iracing_sdk::{VariableInfo, reader::ibt::IbtReader};

use schema_writer::{SchemaOutputEncoding, write_to_output};

#[derive(Parser)]
#[command(name = "telemetry")]
struct Args {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Captures a snapshot of a telemetry frame as JSON or YAML
    Snapshot,
    /// Capture a JSON schema of the available telemetry variables as an array.
    Schema {
        #[command(subcommand)]
        commands: SchemaCommands,
    },
    /// Outputs a stream of telemetry frames as CSV or JSONL
    Record,
}

#[derive(Subcommand)]
enum SchemaCommands {
    Ibt {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,

        /// Path where the generated schema YAML should be written.
        #[arg(short, long, default_value = "disk-variable-schema.yml")]
        output_path: PathBuf,

        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,
    },

    #[cfg(windows)]
    Live {
        /// Path where the session YAML should be written.
        #[arg(short, long, default_value = "live-variable-schema.yml")]
        output_path: PathBuf,

        #[arg(long, default_value = "yaml", value_enum)]
        encoding: SchemaOutputEncoding,
    },
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments.
    // ------------------------------------------------------------
    let args = Args::parse();

    match args.commands {
        Commands::Snapshot => {
            handle_snapshot_command()?;
        }
        Commands::Schema { commands } => {
            handle_schema_command(commands)?;
        }
        Commands::Record => {
            handle_stream_command()?;
        }
    }

    Ok(())
}

fn handle_snapshot_command() -> Result<()> {
    tracing::info!("Handling snapshot command...");

    // Snapshot commands should include capturing the next available live frame,
    // or a requested frame from an IBT.

    Ok(())
}

fn handle_schema_command(commands: SchemaCommands) -> Result<()> {
    tracing::info!("Handling schema command...");

    match commands {
        SchemaCommands::Ibt {
            path,
            output_path,
            encoding,
        } => {
            let variables = capture_disk_schema(&path)?;
            let schema = schemars::schema_for_value!(variables);
            write_to_output(&schema, &output_path, encoding)?;
        }
        SchemaCommands::Live {
            output_path,
            encoding,
        } => {
            let variables = capture_live_schema()?;
            let schema = schemars::schema_for_value!(variables);
            write_to_output(&schema, &output_path, encoding)?;
        }
    }

    Ok(())
}

fn handle_stream_command() -> Result<()> {
    tracing::info!("Handling stream command...");
    Ok(())
}

fn capture_disk_schema(ibt_path: &PathBuf) -> Result<Vec<VariableInfo>> {
    let reader = IbtReader::open(ibt_path)?;

    let variable_headers = reader
        .variable_headers_buffer()?
        .ok_or(anyhow::anyhow!("IBT does not contain variable headers"))?;

    Ok(variable_headers.try_into()?)
}

#[cfg(windows)]
fn capture_live_schema() -> Result<Vec<VariableInfo>> {
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

    let variable_headers = connection.variable_headers_buffer().ok_or(anyhow::anyhow!(
        "Live connection does not contain variable headers"
    ))?;

    Ok(variable_headers.try_into()?)
}
