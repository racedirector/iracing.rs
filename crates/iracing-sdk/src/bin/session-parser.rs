use anyhow::Result;
use clap::Parser;
use iracing_sdk::IbtReader;
use std::{fs, path::PathBuf};
use tracing::info;

/// CLI arguments for the `session` parser.
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
    // ------------------------------------------------------------
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "session-parser=info".to_string()),
        )
        .init();

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
