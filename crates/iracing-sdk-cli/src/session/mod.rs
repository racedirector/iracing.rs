mod discover;
mod schema;
mod snapshot;

use anyhow::Result;
use clap::Subcommand;

use discover::Command as DiscoverCommand;
use schema::Command as SchemaCommand;
use snapshot::Command as SnapshotCommand;

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Captures JSON schema of a session string.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
    /// Discovers schema additions of a session string.
    Discover {
        #[command(subcommand)]
        command: DiscoverCommand,
    },
    /// Captures a snapshot of the latest session string.
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommand,
    },
}

impl Command {
    /// Execute the selected session snapshot, discovery, or schema command and propagate its errors.
    pub(crate) fn run(self) -> Result<()> {
        match self {
            Command::Snapshot { command } => command.run(),
            Command::Discover { command } => command.run(),
            Command::Schema { command } => command.run(),
        }
    }
}
