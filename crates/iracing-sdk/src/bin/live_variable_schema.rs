//! Live telemetry schema generator (Windows).
//!
//! Connects to iRacing shared memory and emits telemetry variable JSON Schema
//! (serialized as YAML) using the currently available variable definitions.
//!
//! # Behavior
//! - Opens live iRacing connection
//! - Optionally allows stale/not-connected state with `--allow-stale`
//! - Builds telemetry schema from variable headers + frame size
//! - Writes schema YAML to `--output-path`
//!
//! # Usage
//! ```text
//! live-variable-schema --output-path <SCHEMA.yml> [--allow-stale]
//! ```

use anyhow::{Result, anyhow};
#[cfg(windows)]
use clap::{ArgAction, Parser};
#[cfg(windows)]
use std::path::PathBuf;

/// CLI arguments for the live telemetry schema generator.
#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the output schema YAML should be written.
    #[arg(short, long, default_value = "live-variable-schema.yml")]
    output_path: PathBuf,

    /// Allow schema generation even if iRacing is disconnected (may be stale).
    #[arg(long, action = ArgAction::SetTrue)]
    allow_stale: bool,
}

pub fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    #[cfg(not(windows))]
    {
        tracing::warn!(
            "live-telemetry-schema is only supported on Windows because it depends on iRacing shared memory APIs."
        );
        Err(anyhow!(
            "live-telemetry-schema is only supported on Windows"
        ))
    }

    /// Connects to iRacing shared memory, generates the telemetry variable schema, and writes it to disk.
    #[cfg(windows)]
    {
        use iracing_sdk::{VariableSchema, WindowsConnection};
        use std::{fs::File, io::BufWriter};

        // ------------------------------------------------------------
        // Parse CLI arguments
        // ------------------------------------------------------------
        let Args {
            output_path,
            allow_stale,
        } = Args::parse();

        // ------------------------------------------------------------
        // Open iRacing connection
        // ------------------------------------------------------------
        let connection = WindowsConnection::try_connect().expect("Failed to connect to iRacing");
        if !connection.is_connected() && !allow_stale {
            return Err(anyhow!(
                "iRacing is not connected (pass --allow-stale to continue)."
            ));
        }

        // Build schema from variables
        let variables: Vec<_> = connection.get_variables();
        let mut variable_map = std::collections::HashMap::new();

        for var_info in variables {
            variable_map.insert(var_info.name.clone(), var_info);
        }

        let frame_size = connection.header().buf_len as usize;
        let variable_schema = VariableSchema::new(variable_map, frame_size)?;
        let schema = schemars::schema_for_value!(variable_schema);

        let output_file = File::create(&output_path)?;
        let writer = BufWriter::new(output_file);

        serde_yaml_ng::to_writer(writer, &schema)?;

        tracing::info!(path=%output_path.display(),"Wrote live telemetry schema");

        Ok(())
    }
}
