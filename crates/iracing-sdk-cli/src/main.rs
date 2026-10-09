// Production simulator commands are Windows-only; retain portable adapters/contracts.
#[cfg_attr(not(windows), allow(dead_code))]
mod application;
mod commands;
pub(crate) mod utils;
mod writer;

use anyhow::Result;
use clap::Parser;
use commands::Command;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "iracing-sdk", version, about = "iRacing SDK tools", long_about = None, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Run the selected SDK tool, returning any command execution errors.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let mut application = application::Application::new();
    Cli::parse().command.run(&mut application).await?;

    Ok(())
}
