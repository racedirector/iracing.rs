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
//! This tool relies on `iracing_sdk::WindowsConnection`, so it is typically only
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
//! cargo run -p iracing-sdk --bin live-session-parser -- \
//!   --output-path "C:\path\to\live-session.yaml"
//! ```
//!
//! Best-effort mode: do not error if iRacing isn’t live (useful for quick checks):
//!
//! ```bash
//! cargo run -p iracing-sdk --bin live-session-parser -- \
//!   --output-path "C:\path\to\live-session.yaml" \
//!   --no-live-only
//! ```
//!
//! Reduce logging verbosity:
//!
//! ```bash
//! RUST_LOG=info cargo run -p iracing-sdk --bin live-session-parser -- \
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
//! `Provider::session_yaml()` (e.g., return an error or write a placeholder file).

use anyhow::Result;
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

/// CLI arguments for the live session parser.
///
/// Uses `clap` derive API for parsing.
#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the session YAML should be written.
    #[arg(short, long)]
    output_path: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    #[cfg(not(windows))]
    {
        use anyhow::anyhow;

        tracing::warn!(
            "live-session-parser is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
        );
        Err(anyhow!("live-session-parser is only supported on Windows"))
    }

    #[cfg(windows)]
    {
        use iracing_sdk::{WindowsConnection, provider::Provider, providers::live::LiveProvider};
        use std::{fs, thread, time::Duration};

        let Args { output_path } = Args::parse();

        tracing::info!("Opening iRacing connection...");
        let windows_connection = loop {
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

        let mut provider = LiveProvider::builder()
            .with_connection(windows_connection)
            .build()?;

        // ------------------------------------------------------------
        // Write session string to output path.
        // ------------------------------------------------------------
        tracing::info!("Parsing session information");

        if let Some(yaml) = provider.session_yaml(0).await? {
            let session_info_string = yaml.into_string();

            // If we have an output path, write the result to the file, otherwise log
            if let Some(output_path) = output_path {
                fs::write(output_path, session_info_string)?;
            } else {
                tracing::info!("\n{}", session_info_string);
            }
        };

        tracing::info!("Finished parsing session information.");

        Ok(())
    }
}
