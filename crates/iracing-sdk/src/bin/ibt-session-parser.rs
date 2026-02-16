use anyhow::Result;
use clap::Parser;
use iracing_sdk::IbtReader;
use std::{fs, path::PathBuf};
use tracing::info;
use tracing_subscriber::EnvFilter;

/// CLI arguments for the disk session parser.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct SessionParserArgs {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the session YAML should be written.
    #[arg(short, long)]
    output_path: PathBuf,
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
    let SessionParserArgs {
        ibt_path,
        output_path,
    } = SessionParserArgs::parse();

    // ------------------------------------------------------------
    // Open telemetry reader.
    // ------------------------------------------------------------
    let reader = IbtReader::open(&ibt_path).expect("Failed to open IBT file");

    // ------------------------------------------------------------
    // Write session string to output path.
    // ------------------------------------------------------------
    info!("Parsing session information");
    if let Some(session) = reader.session_yaml()? {
        fs::write(output_path, session)?;
    }

    info!("Finished parsing session information.");

    Ok(())
}
