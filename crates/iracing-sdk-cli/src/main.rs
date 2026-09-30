#[cfg(windows)]
mod broadcast;
mod headers;
mod session;
mod telemetry;
pub(crate) mod utils;
mod variables;
mod writer;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
use broadcast::{Command as BroadcastCommand, handle_command as handle_broadcast_command};
use headers::{Command as HeadersCommand, handle_command as handle_headers_command};
use session::{Command as SessionCommand, handle_command as handle_session_command};
use telemetry::{Command as TelemetryCommand, handle_command as handle_telemetry_command};
use variables::{Command as VariablesCommand, handle_command as handle_variables_command};

#[derive(Parser)]
#[command(name = "iracing-sdk", version, about = "iRacing SDK tools", long_about = None, arg_required_else_help = true)]
struct Args {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Tools for interacting with disk and live headers, as well as the type schema and layout.
    Headers {
        #[command(subcommand)]
        command: HeadersCommand,
    },
    /// Tools for interacting with disk and live session strings, as well as type schemas.
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// Tools for interacting with disk and live variables.
    Variables {
        #[command(subcommand)]
        command: VariablesCommand,
    },
    /// Tools for capturing telemetry from a source.
    Telemetry {
        #[command(subcommand)]
        command: TelemetryCommand,
    },
    /// Tools for sending broadcast commands to the simulator.
    #[cfg(windows)]
    Broadcast {
        #[command(subcommand)]
        command: BroadcastCommand,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    match args.commands {
        Commands::Session { command } => handle_session_command(command)?,
        #[cfg(windows)]
        Commands::Broadcast { command } => handle_broadcast_command(command)?,
        Commands::Headers { command } => handle_headers_command(command)?,
        Commands::Variables { command } => handle_variables_command(command)?,
        Commands::Telemetry { command } => handle_telemetry_command(command).await?,
    }

    Ok(())
}
