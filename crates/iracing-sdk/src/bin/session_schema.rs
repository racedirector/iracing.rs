//! Session schema generator.
//!
//! Generates YAML-serialized JSON Schema from the Rust type, an IBT recording,
//! or live iRacing shared memory on Windows.
//!
//! # Usage
//! ```text
//! session-schema type --output-path <SCHEMA.yml>
//! session-schema ibt --path <FILE.ibt> --output-path <SCHEMA.yml>
//! session-schema live --output-path <SCHEMA.yml>
//! ```

use anyhow::Result;
use clap::{Parser, Subcommand};
use iracing_sdk::{SessionInfo, reader::ibt::IbtReader, yaml_utils};
use schemars::Schema;
use std::{fs::File, io::BufWriter, path::PathBuf};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "session-schema",
    version,
    about = "iRacing session schema utilities",
    long_about = None,
    arg_required_else_help = true,
)]
struct Args {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generates a schema from a live iRacing session.
    #[cfg(windows)]
    Live {
        /// Path where the generated schema YAML should be written.
        #[arg(short, long, default_value = "live-session-schema.yml")]
        output_path: PathBuf,
    },
    /// Generates a schema from session data embedded in an IBT file.
    Ibt {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,

        /// Path where the generated schema YAML should be written.
        #[arg(short, long, default_value = "disk-session-schema.yml")]
        output_path: PathBuf,
    },
    /// Generates the baseline schema from the Rust session type.
    Type {
        /// Path where the generated schema YAML should be written.
        #[arg(short, long, default_value = "session-schema-type.yml")]
        output_path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    match args.commands {
        Commands::Ibt { path, output_path } => {
            capture_disk_session_schema(&path, &output_path)?;
            tracing::info!(output_path=%output_path.display(),"Wrote disk session schema.");
        }
        #[cfg(windows)]
        Commands::Live { output_path } => {
            capture_live_session_schema(&output_path)?;
            tracing::info!(output_path=%output_path.display(), "Wrote live session schema.");
        }
        Commands::Type { output_path } => {
            let schema = schemars::schema_for!(SessionInfo);
            write_schema_to_output(schema, &output_path)?;
            tracing::info!(output_path=%output_path.display(), "Wrote session type schema.");
        }
    }

    Ok(())
}

fn capture_disk_session_schema(ibt_path: &PathBuf, output_path: &PathBuf) -> Result<()> {
    let reader = IbtReader::open(ibt_path)?;

    let session_schema = reader
        .session_info_buffer()?
        .map(TryInto::<String>::try_into)
        .ok_or(anyhow::anyhow!(
            "Could not convert `SessionInfoBuffer` to `String`"
        ))?
        .map(|session_yaml| yaml_utils::preprocess_iracing_yaml(&session_yaml))?
        .map(|session_yaml| SessionInfo::parse(&session_yaml))?
        .map(|i| schemars::schema_for_value!(i))?;

    write_schema_to_output(session_schema, output_path)?;

    Ok(())
}

#[cfg(windows)]
fn capture_live_session_schema(output_path: &PathBuf) -> Result<()> {
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

    let session_schema = connection
        .session_info_buffer()
        .map(TryInto::<String>::try_into)
        .ok_or(anyhow::anyhow!(
            "Could not convert `SessionInfoBuffer` to `String`"
        ))?
        .map(|session_yaml| yaml_utils::preprocess_iracing_yaml(&session_yaml))?
        .map(|session_yaml| SessionInfo::parse(&session_yaml))?
        .map(|i| schemars::schema_for_value!(i))?;

    write_schema_to_output(session_schema, output_path)?;

    Ok(())
}

fn write_schema_to_output(schema: Schema, output_path: &PathBuf) -> Result<()> {
    let output_file = File::create(output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    Ok(())
}
