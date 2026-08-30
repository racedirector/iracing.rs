//! Variable schema generator.
//!
//! Generates YAML-serialized JSON Schema from the Rust type, an IBT recording,
//! or live iRacing shared memory on Windows.
//!
//! # Usage
//! ```text
//! variable-schema type --output-path <SCHEMA.yml>
//! variable-schema ibt --path <FILE.ibt> --output-path <SCHEMA.yml>
//! variable-schema live --output-path <SCHEMA.yml>
//! ```

mod schema_writer;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use iracing_sdk::{VariableSchema, reader::ibt::IbtReader};
use schemars::Schema;

#[derive(Parser)]
#[command(
    name = "variable-schema",
    version,
    about = "iRacing variable schema utilities",
    long_about = None,
    arg_required_else_help = true,
)]
struct Args {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generates a schema from a live iRacing connection.
    #[cfg(windows)]
    Live {
        /// Path where the generated schema YAML should be written.
        #[arg(short, long, default_value = "live-variable-schema.yml")]
        output_path: PathBuf,
    },
    /// Generates a schema from variable headers embedded in an IBT file.
    Ibt {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,

        /// Path where the generated schema YAML should be written.
        #[arg(short, long, default_value = "disk-variable-schema.yml")]
        output_path: PathBuf,
    },
    /// Generates the baseline schema from the Rust variable type.
    Type {
        /// Path where the generated schema YAML should be written.
        #[arg(short, long, default_value = "variable-schema-type.yml")]
        output_path: PathBuf,
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
        Commands::Ibt { path, output_path } => {
            capture_disk_schema(&path, &output_path)?;
            tracing::info!(output_path=%output_path.display(), path=%path.display(),"Wrote disk variable schema");
        }
        #[cfg(windows)]
        Commands::Live { output_path } => {
            capture_live_schema(&output_path)?;
            tracing::info!(output_path=%output_path.display(),"Wrote live variable schema");
        }
        Commands::Type { output_path } => {
            let schema = schemars::schema_for!(iracing_sdk::VariableInfo);
            write_schema_to_output(schema, &output_path)?;
            tracing::info!(output_path=%output_path.display(), "Wrote variable type schema.");
        }
    }

    Ok(())
}

#[cfg(windows)]
fn capture_live_schema(output_path: &PathBuf) -> Result<()> {
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

    let schema =
        VariableSchema::from_connection(&connection).map(|v| schemars::schema_for_value!(v))?;

    write_schema_to_output(schema, output_path)?;

    Ok(())
}

fn capture_disk_schema(ibt_path: &PathBuf, output_path: &PathBuf) -> Result<()> {
    let reader = IbtReader::open(ibt_path)?;

    let schema = VariableSchema::from_reader(&reader).map(|v| schemars::schema_for_value!(v))?;

    write_schema_to_output(schema, output_path)?;

    Ok(())
}

fn write_schema_to_output(schema: Schema, output_path: &PathBuf) -> Result<()> {
    use schema_writer::{SchemaOutputEncoding, write_to_output};
    write_to_output(&schema, output_path, SchemaOutputEncoding::Yaml)?;
    Ok(())
}
