//! Live header dump CLI (Windows)
//!
//! Connects to a running iRacing instance via the Windows shared memory interface
//! and prints the parsed live header structure.
//!
//! This binary is intended for:
//! - Verifying live telemetry connectivity
//! - Inspecting the live shared-memory header layout
//! - Capturing reproducible header diagnostics
//!
//! # Behavior
//! - Attempts to connect via `WindowsConnection::try_connect()`
//! - Reads the live header bytes from shared memory
//! - Parses the header using `IRSDKHeader::parse_from_memory()`
//! - Prints the structure using pretty `Debug` formatting
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
//! live-headers
//! ```
//!
//! # Example
//!
//! ```bash
//! cargo run -p iracing-sdk --bin live_headers
//! ```

use anyhow::{anyhow, Result};
#[cfg(windows)]
use iracing_sdk::schema::header::IRSDKHeader;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
use tracing::info;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

#[cfg(windows)]
fn run() -> Result<()> {
    info!("Opening iRacing connection...");
    let connection = WindowsConnection::try_connect().expect("Failed to connect to iRacing");

    info!("Reading live header bytes");
    let header_ptr = connection.header() as *const iracing_sdk::windows::IRSDKHeader as *const u8;
    let header_bytes = unsafe {
        std::slice::from_raw_parts(header_ptr, std::mem::size_of::<IRSDKHeader>())
    };

    info!("Parsing live header");
    let header = IRSDKHeader::parse_from_memory(header_bytes)?;

    println!("IRSDKHeader:\n{:#?}", header);

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live-headers is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("live-headers is only supported on Windows"))
}
