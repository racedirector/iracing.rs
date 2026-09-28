mod commands;
use anyhow::Result;
use clap::Subcommand;

use commands::{DiscoverCommand, SchemaCommand, SnapshotCommand};

use crate::session::commands::{
    handle_discover_command, handle_schema_command, handle_snapshot_command,
};

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Captures JSON schema of a session string.
    Schema {
        #[command(subcommand)]
        commands: SchemaCommand,
    },
    /// Discovers schema additions of a session string.
    Discover {
        #[command(subcommand)]
        commands: DiscoverCommand,
    },
    /// Captures a snapshot of the latest session string.
    Snapshot {
        #[command(subcommand)]
        commands: SnapshotCommand,
    },
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    match command {
        Command::Snapshot { commands } => handle_snapshot_command(commands),
        Command::Discover { commands } => handle_discover_command(commands),
        Command::Schema { commands } => handle_schema_command(commands),
    }
}
