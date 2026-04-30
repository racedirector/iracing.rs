#[cfg(windows)]
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
#[cfg(windows)]
use iracing_sdk::SessionInfo;
#[cfg(windows)]
use iracing_sdk::WaitResult;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
use serde_yaml_ng::to_string;
use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run().await
}

#[cfg(windows)]
async fn run() -> Result<()> {
    tracing::info!("Opening iRacing connection...");

    let connection = WindowsConnection::try_connect().expect("Failed to connect to iRacing");
    if !connection.is_connected() {
        return Err(anyhow!("iRacing telemetry is not connected"));
    }

    let mut previous_session_info_update: i32 = -1;
    let mut previous_setup_update: i32 = -1;
    loop {
        if !connection.is_connected() {
            return Err(anyhow!("iRacing disconnected"));
        }

        match connection.wait_for_update(Duration::from_millis(500)) {
            Ok(WaitResult::Signaled) => {
                let current_update = connection.session_info_update();

                if current_update != previous_session_info_update
                    && let Some(session_info_yaml) = connection.session_info()
                    && let Some(session_info) = SessionInfo::parse(&session_info_yaml).ok()
                {
                    if let Some(setup) = session_info.car_setup
                        && previous_setup_update != setup.update_count
                    {
                        let serialized_setup = to_string(&setup)?;

                        tracing::info!("\n{}", serialized_setup);

                        previous_setup_update = setup.update_count;
                    }

                    previous_session_info_update = current_update;
                }
                continue;
            }
            Ok(WaitResult::Timeout) => continue,
            Err(err) => return Err(anyhow!("{}", err.to_string())),
        }
    }
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "setup-observer is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!("setup-observer is only supported on Windows"))
}
