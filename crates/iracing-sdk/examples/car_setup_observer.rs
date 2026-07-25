#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
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
        use futures::StreamExt;
        use iracing_sdk::LiveConnection;

        let connection = LiveConnection::builder().build()?;
        let mut stream = Box::pin(connection.session_updates());
        let mut previous_setup_update: i32 = -1;

        // Observe the stream until it closes
        while let Some(session) = stream.next().await {
            if let Some(setup) = &session.car_setup
                && previous_setup_update != setup.update_count
            {
                let serialized_setup = serde_yaml_ng::to_string(&setup)?;

                tracing::info!("\n{}", serialized_setup);

                previous_setup_update = setup.update_count;
            }
        }

        Ok(())
    }
}
