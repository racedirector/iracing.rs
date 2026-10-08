mod headers;
mod session;
mod telemetry;
pub(crate) mod utils;
mod variables;
mod writer;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "iracing-sdk", version, about = "iRacing SDK tools", long_about = None, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Tools for interacting with disk and live headers, as well as the type schema and layout.
    Headers(headers::Args),
    /// Tools for interacting with disk and live session strings, as well as type schemas.
    Session {
        #[command(subcommand)]
        command: session::Command,
    },
    /// Tools for interacting with disk and live variables.
    Variables(variables::Args),
    /// Tools for capturing telemetry from a source.
    Telemetry {
        #[command(subcommand)]
        command: telemetry::Command,
    },
    /// Tools for sending broadcast commands to the simulator.
    #[cfg(windows)]
    Broadcast {
        #[command(subcommand)]
        command: iracing_broadcast_cli::Command,
    },
}

impl Command {
    /// Execute the selected tool command and propagate its errors.
    pub async fn run(self) -> Result<()> {
        match self {
            Command::Session { command } => command.run(),
            #[cfg(windows)]
            Command::Broadcast { command } => command.run(),
            Command::Headers(args) => args.run(),
            Command::Variables(args) => args.run(),
            Command::Telemetry { command } => command.run().await,
        }
    }
}

/// Run the selected SDK tool, returning any command execution errors.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    Cli::parse().command.run().await?;

    Ok(())
}
