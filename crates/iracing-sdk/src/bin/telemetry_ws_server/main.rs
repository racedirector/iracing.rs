mod app;
mod config;
mod server;

use anyhow::Result;
use clap::Parser;
use config::{Cli, ServerConfig};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = ServerConfig::from_cli(Cli::parse())?;
    server::serve(config).await
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,iracing_sdk=debug,tower_http=info,axum=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}
