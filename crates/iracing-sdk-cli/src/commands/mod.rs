#[cfg_attr(not(windows), allow(dead_code))]
pub mod dependencies;
mod headers;
mod session;
mod telemetry;
mod variables;

use anyhow::Result;

#[derive(clap::Subcommand, Debug)]
pub enum Command {
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
            + iracing_broadcast_cli::ReplaySessions
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
