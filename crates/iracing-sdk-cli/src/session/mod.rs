mod discover;
mod schema;
mod snapshot;

use anyhow::Result;

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Captures JSON schema of a session string.
    Schema(schema::Args),
    /// Discovers schema additions of a session string.
    Discover(discover::Args),
    /// Captures a snapshot of the latest session string.
    Snapshot(snapshot::Args),
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
