fn main() -> anyhow::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    #[cfg(not(windows))]
    {
        tracing::warn!(
            "broadcast_reload_textures example is only supported on Windows because iRacing broadcast messaging uses Win32 APIs."
        );
        Err(anyhow::anyhow!(
            "broadcast_reload_textures example is only supported on Windows"
        ))
    }

    #[cfg(windows)]
    {
        use iracing_sdk::windows::{Broadcast, BroadcastCommand};

        let client = Broadcast::new().expect("Could not create iRacing broadcast client");
        client.send_message(BroadcastCommand::ReloadAllTextures)?;

        tracing::info!("Sent broadcast message: reload all textures");

        Ok(())
    }
}
