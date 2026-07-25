use anyhow::Result;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    #[cfg(not(windows))]
    {
        use anyhow::anyhow;

        tracing::warn!(
            "session-update-observer is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
        );
        Err(anyhow!(
            "session-update-observer is only supported on Windows"
        ))
    }

    #[cfg(windows)]
    {
        use futures::StreamExt;
        use iracing_sdk::{LiveConnection, providers::live::LiveProvider};

        let provider = LiveProvider::builder()
            .without_no_connection_limit()
            .build()?;

        let connection = LiveConnection::builder().with_provider(provider).build()?;
        let mut stream = Box::pin(connection.session_updates());
        let mut previous_session_info = None;

        while let Some(session) = stream.next().await {
            let changed = previous_session_info
                .as_deref()
                .is_none_or(|previous_value| previous_value != session.as_ref());

            if changed {
                tracing::info!("{session:?}");
                previous_session_info = Some(session);
            }
        }

        Ok(())
    }
}
