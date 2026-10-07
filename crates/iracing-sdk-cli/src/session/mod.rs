mod discover;
mod schema;
mod snapshot;

use anyhow::Result;

use discover::Args as DiscoverArgs;
use schema::Args as SchemaArgs;
use snapshot::Args as SnapshotArgs;

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Captures JSON schema of a session string.
    Schema(SchemaArgs),
    /// Discovers schema additions of a session string.
    Discover(DiscoverArgs),
    /// Captures a snapshot of the latest session string.
    Snapshot(SnapshotArgs),
}

impl Command {
    /// Execute the selected session snapshot, discovery, or schema command and propagate its errors.
    pub(crate) fn run(self) -> Result<()> {
        match self {
            Command::Snapshot(args) => args.run(),
            Command::Discover(args) => args.run(),
            Command::Schema(args) => args.run(),
        }
    }
}
