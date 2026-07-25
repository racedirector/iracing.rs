//! Disk session parser CLI
//!
//! Parses **session information** from an iRacing `.ibt` telemetry file and writes the
//! raw **session YAML** to disk.
//!
//! This binary is intended for:
//! - Quick inspection of session metadata embedded in `.ibt` files
//! - Creating reproducible fixtures for tests / debugging
//! - Converting `.ibt` session info into a standalone YAML file for sharing
//!
//! # Behavior
//! - Opens the `.ibt` file using [`iracing_sdk::ibt::IbtReader`]
//! - Extracts session YAML via `reader.session_yaml()?`
//! - If session YAML is present, writes it to `--output-path`
//! - If session YAML is **absent**, the tool exits successfully without writing a file
//!
//! # Logging
//! Uses `tracing` + `tracing_subscriber`.
//!
//! - Defaults to `trace` level
//! - Override with `RUST_LOG`, e.g. `RUST_LOG=info`
//!
//! # Usage
//!
//! ```text
//! ibt-session-parser --ibt-path <PATH_TO_FILE.ibt> --output-path <OUTPUT_FILE.yaml>
//! ```
//!
//! Short flags are also supported:
//!
//! ```text
//! ibt-session-parser -i <PATH_TO_FILE.ibt> -o <OUTPUT_FILE.yaml>
//! ```
//!
//! # Examples
//!
//! Parse an `.ibt` file and write session YAML:
//!
//! ```bash
//! cargo run -p iracing-sdk --bin ibt-session-parser -- \
//!   --ibt-path "C:\path\to\telemetry.ibt" \
//!   --output-path "C:\path\to\session.yaml"
//! ```
//!
//! Reduce log noise:
//!
//! ```bash
//! RUST_LOG=info cargo run -p iracing-sdk --bin ibt-session-parser -- \
//!   -i "./telemetry.ibt" \
//!   -o "./session.yaml"
//! ```
//!
//! # Exit codes / errors
//! - Returns an error if the `.ibt` cannot be opened or read
//! - Returns an error if the output file cannot be written
//!
//! # Notes
//! If you want to ensure a file is always written, you can add an `else` branch to
//! write a placeholder or return an error when session YAML is missing.

use anyhow::Result;
use clap::Parser;
use iracing_sdk::ibt::IbtReader;
use std::{fs, path::PathBuf};
use tracing_subscriber::EnvFilter;

/// CLI arguments for the disk session parser.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the session YAML should be written.
    #[arg(short, long)]
    output_path: Option<PathBuf>,
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
    let Args {
        ibt_path,
        output_path,
    } = Args::parse();

    // ------------------------------------------------------------
    // Open telemetry reader.
    // ------------------------------------------------------------
    let reader = IbtReader::open(&ibt_path).expect("Failed to open IBT file");

    // ------------------------------------------------------------
    // Write session string to output path.
    // ------------------------------------------------------------
    tracing::info!("Parsing session information");

    let session_info_string = match reader.session_yaml() {
        Ok(Some(session)) => session,
        _ => return Err(anyhow::anyhow!("Could not parse session yaml.")),
    };

    // If we have an output path, write the result to the file, otherwise log
    if let Some(output_path) = output_path {
        fs::write(output_path, session_info_string)?;
    } else {
        tracing::info!("\n{}", session_info_string);
    }

    tracing::info!("Finished parsing session information.");

    Ok(())
}
