fn main() -> anyhow::Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    #[cfg(not(windows))]
    {
        tracing::warn!(
            "setup-observer is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
        );
        Err(anyhow::anyhow!(
            "setup-observer is only supported on Windows"
        ))
    }

    #[cfg(windows)]
    {
        use iracing_sdk::SessionInfo;
        use iracing_sdk::WaitResult;
        use iracing_sdk::WindowsConnection;
        use serde_yaml_ng::to_string;
        use std::time::Duration;

        tracing::info!("Opening iRacing connection...");

        let connection = WindowsConnection::try_connect().expect("Failed to connect to iRacing");
        if !connection.is_connected() {
            return Err(anyhow::anyhow!("iRacing telemetry is not connected"));
        }

        let mut previous_session_info_update: i32 = -1;
        let mut previous_setup_update: i32 = -1;
        loop {
            if !connection.is_connected() {
                return Err(anyhow::anyhow!("iRacing disconnected"));
            }

            match connection.wait_for_update(Duration::from_millis(500)) {
                Ok(WaitResult::Signaled) => {
                    let current_update = connection.session_info_update();

                    if current_update != previous_session_info_update
                        && let Some(session_info_yaml) = connection.session_info()
                        && let Some(session_info) = SessionInfo::parse(session_info_yaml).ok()
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
                Err(err) => return Err(anyhow::anyhow!("{}", err)),
            }
        }
    }
}
