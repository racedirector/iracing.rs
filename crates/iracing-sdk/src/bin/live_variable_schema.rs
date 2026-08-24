//! Live telemetry schema generator (Windows).
//!
//! Connects to iRacing shared memory and emits telemetry variable JSON Schema
//! (serialized as YAML) using the currently available variable definitions.
//!
//! # Behavior
//! - Opens live iRacing connection
//! - Waits until iRacing reports a connected telemetry session
//! - Builds telemetry schema from variable headers + frame size
//! - Writes schema YAML to `--output-path`
//!
//! # Usage
//! ```text
//! live-variable-schema --output-path <SCHEMA.yml>
//! ```

#[cfg(windows)]
use clap::Parser;
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
}

pub fn main() -> anyhow::Result<()> {
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
        Err(anyhow::anyhow!(
            "live-telemetry-schema is only supported on Windows"
        ))
    }

    // Connects to iRacing shared memory, generates the telemetry variable schema, and writes it to disk.
    #[cfg(windows)]
    {
        use iracing_sdk::{VariableSchema, WindowsConnection};
        use std::{fs::File, io::BufWriter, thread, time::Duration};

        // ------------------------------------------------------------
        // Parse CLI arguments
        // ------------------------------------------------------------
        let Args { output_path } = Args::parse();

        // ------------------------------------------------------------
        // Open iRacing connection
        // ------------------------------------------------------------
        tracing::info!("Opening iRacing connection...");
        let connection = loop {
            match WindowsConnection::try_connect() {
                Ok(connection) if connection.is_connected() => break connection,
                Ok(_) => {
                    tracing::debug!("Shared memory opened but telemetry is not connected yet");
                }
                Err(error) => {
                    tracing::debug!(%error, "Waiting for iRacing shared memory");
                }
            }

            thread::sleep(Duration::from_secs(1));
        };

        let variable_schema = VariableSchema::from_connection(&connection)?;
        let schema = schemars::schema_for_value!(variable_schema);

        let output_file = File::create(&output_path)?;
        let writer = BufWriter::new(output_file);

        serde_yaml_ng::to_writer(writer, &schema)?;

        tracing::info!(path=%output_path.display(),"Wrote live telemetry schema");

        Ok(())
    }
}
