use anyhow::Result;
#[cfg(windows)]
use iracing_sdk::windows::{Broadcast, BroadcastCommand};
#[cfg(windows)]
use tracing::info;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

#[cfg(windows)]
fn run() -> Result<()> {
    let client = Broadcast::new().expect("Could not create iRacing broadcast client");
    client.send_message(BroadcastCommand::ReloadAllTextures)?;

    info!("Sent broadcast message: reload all textures");

    Ok(())
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "broadcast_reload_textures example is only supported on Windows because iRacing broadcast messaging uses Win32 APIs."
    );
    Err(anyhow!(
        "broadcast_reload_textures example is only supported on Windows"
    ))
}
