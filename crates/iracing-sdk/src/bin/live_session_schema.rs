//! Live session schema generator (Windows).
//!
//! Connects to iRacing shared memory, reads live session YAML, and generates
//! session JSON Schema (serialized as YAML).
//!
//! # Usage
//! ```text
//! live-session-schema --output-path <SCHEMA.yml> [--allow-stale]
//! ```

use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

/// CLI arguments for the live session schema generator.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the output schema YAML should be written.
    #[arg(short, long, default_value = "live-session-schema.yml")]
    output_path: PathBuf,

    /// Allow schema generation even if iRacing is disconnected (may be stale).
    #[arg(long, action = ArgAction::SetTrue)]
    allow_stale: bool,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

/// Connects to iRacing shared memory, generates the session schema, and writes it to disk.
#[cfg(windows)]
fn run() -> Result<()> {
    use iracing_sdk::{SessionInfoParser, WindowsConnection};
    use std::{fs::File, io::BufWriter};

    let Args {
        output_path,
        allow_stale,
    } = Args::parse();

    tracing::info!("Opening iRacing connection");
    let connection = WindowsConnection::try_connect()?;

    if !connection.is_connected() && !allow_stale {
        return Err(anyhow!(
            "iRacing is not connected (pass --allow-stale to continue)."
        ));
    }

    let raw_session_yaml = connection
        .session_info()
        .ok_or_else(|| anyhow!("No live session YAML is available"))?;

    let parser = SessionInfoParser::new();
    let session = parser.parse(&raw_session_yaml)?;
    let schema = schemars::schema_for_value!(session);

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    tracing::info!(path = %output_path.display(), "Wrote live session schema");

    Ok(())
}

/// Non-Windows stub — always returns an error explaining the platform requirement.
#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live_session_schema is only supported on Windows because it depends on iRacing shared memory APIs."
    );
    Err(anyhow!("live_session_schema is only supported on Windows"))
}
