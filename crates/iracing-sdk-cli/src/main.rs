// Production simulator commands are Windows-only; retain portable adapters/contracts.
#[cfg_attr(not(windows), allow(dead_code))]
mod application;
#[cfg_attr(not(windows), allow(dead_code))]
mod dependencies;
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
    pub async fn run<D>(self, dependencies: &mut D) -> Result<()>
    where
        D: iracing_broadcast_cli::BroadcastCommands
            + dependencies::LiveHeaders
            + dependencies::LiveSessions
            + dependencies::LiveVariables
            + dependencies::LiveFrames,
    {
        match self {
            Command::Session { command } => command.run(dependencies),
            #[cfg(windows)]
            Command::Broadcast { command } => command.run(dependencies),
            Command::Headers(args) => args.run(dependencies),
            Command::Variables(args) => args.run(dependencies),
            Command::Telemetry { command } => command.run(dependencies).await,
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

    let mut application = application::Application::new();
    Cli::parse().command.run(&mut application).await?;

    Ok(())
}
