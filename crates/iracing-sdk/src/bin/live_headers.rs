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
//! - Parses the header using the shared `Header` wire type
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

use anyhow::Result;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
#[cfg(windows)]
use iracing_sdk::types::irsdk::{Header, WireType};

fn main() -> Result<()> {
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
            "live-headers is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
        );
        Err(anyhow::anyhow!("live-headers is only supported on Windows"))
    }

    #[cfg(windows)]
    {
        let connection = WindowsConnection::try_connect().expect("Failed to connect to iRacing");

        tracing::info!("Reading live header bytes");
        let header_ptr = connection.header() as *const Header as *const u8;
        let header_bytes =
            unsafe { std::slice::from_raw_parts(header_ptr, std::mem::size_of::<Header>()) };

        tracing::info!("Parsing live header");
        let header = Header::read_from_bytes(header_bytes)?;
        header.validate_live()?;

        println!("Header:\n{:#?}", header);

        Ok(())
    }
}
