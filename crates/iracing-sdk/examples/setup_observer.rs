use anyhow::Result;
use anyhow::anyhow;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
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
    tracing::info!("Opening iRacing connection...");
    let connection = WindowsConnection::try_connect().expect("Failed to connect to iRacing");
    if !connection.is_connected() {
        return Err(anyhow!("iRacing telemetry is not connected"));
    }

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live-session-parser is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("live-session-parser is only supported on Windows"))
}
