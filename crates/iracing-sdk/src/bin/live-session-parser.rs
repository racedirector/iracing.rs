use anyhow::{Result, anyhow};
use clap::Parser;
use iracing_sdk::WindowsConnection;
use std::{fs, path::PathBuf};
use tracing::info;
use tracing_subscriber::EnvFilter;

/// CLI arguments for the live session parser.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct LiveSessionParserArgs {
    /// Path where the session YAML should be written.
    #[arg(short, long)]
    output_path: PathBuf,

    #[arg(long, default_value_t = true)]
    live_only: bool,

    #[arg(long, overrides_with = "live_only")]
    no_live_only: bool,
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments.
    // ------------------------------------------------------------
    let LiveSessionParserArgs {
        output_path,
        live_only,
        no_live_only,
    } = LiveSessionParserArgs::parse();

    let effective_live_only = if no_live_only { false } else { live_only };

    info!("Opening iRacing connection...");
    // ------------------------------------------------------------
    // Open telemetry connection
    // ------------------------------------------------------------
    let connection = WindowsConnection::try_connect().expect("Failed to connect to iRacing");
    if effective_live_only && !connection.is_connected() {
        return Err(anyhow!("Live only is enabled."));
    }

    // ------------------------------------------------------------
    // Write session string to output path.
    // ------------------------------------------------------------
    info!("Parsing session information");
    if let Some(session) = connection.session_info() {
        fs::write(output_path, session)?;
    }

    info!("Finished parsing session information.");

    Ok(())
}
