//! Live session parser CLI (Windows)
//!
//! Connects to a **running iRacing instance** via the Windows shared memory interface
//! and writes the **live session info YAML** to disk.
//!
//! This binary is intended for:
//! - Verifying Windows live telemetry connectivity (`WindowsConnection`)
//! - Capturing live session YAML for debugging / comparison against `.ibt` session YAML
//! - Quickly exporting session metadata while the sim is running
//!
//! # Platform
//! This tool relies on [`iracing_sdk::WindowsConnection`], so it is typically only
//! usable on **Windows**, and requires iRacing to be installed and (optionally) running.
//!
//! # Behavior
//! - Attempts to connect via `WindowsConnection::try_connect()`
//! - Optionally enforces that iRacing is currently “live/connected” (see `--live-only`)
//! - If session info is available, writes it to `--output-path`
//! - If session info is **absent**, the tool exits successfully without writing a file
//!
//! # Logging
//! Uses `tracing` + `tracing_subscriber`.
//!
//! - Defaults to `trace` level
//! - Override with `RUST_LOG`, e.g. `RUST_LOG=info`
//!
//! # Flags
//! - `--output-path <PATH>`: where to write the session YAML
//! - `--live-only` (default): fail if iRacing is not connected
//! - `--no-live-only`: disable the live-only requirement (best-effort capture)
//!
//! The `--no-live-only` flag overrides `--live-only`.
//!
//! # Usage
//!
//! ```text
//! live-session-parser --output-path <OUTPUT_FILE.yaml> [--live-only|--no-live-only]
//! ```
//!
//! # Examples
//!
//! Strict mode (default): requires iRacing to be connected:
//!
//! ```bash
//! cargo run -p <YOUR_BIN_CRATE_NAME> -- \
//!   --output-path "C:\path\to\live-session.yaml"
//! ```
//!
//! Best-effort mode: do not error if iRacing isn’t live (useful for quick checks):
//!
//! ```bash
//! cargo run -p <YOUR_BIN_CRATE_NAME> -- \
//!   --output-path "C:\path\to\live-session.yaml" \
//!   --no-live-only
//! ```
//!
//! Reduce logging verbosity:
//!
//! ```bash
//! RUST_LOG=info cargo run -p <YOUR_BIN_CRATE_NAME> -- \
//!   --output-path ".\live-session.yaml"
//! ```
//!
//! # Exit codes / errors
//! - Returns an error if the Windows connection cannot be created
//! - Returns an error if `--live-only` is effective and `connection.is_connected()` is false
//! - Returns an error if the output file cannot be written
//!
//! # Notes
//! If you want guaranteed output, add handling for the `None` case from
//! `connection.session_info()` (e.g., return an error or write a placeholder file).

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
