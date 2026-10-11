mod discover;
mod schema;
mod snapshot;

use anyhow::Result;

/// Parse the file's retained snapshot through the SDK's decoding/sanitization path.
fn parse_ibt_session(
    file: &iracing_sdk::IbtFile,
) -> Result<Option<iracing_sdk::schema::SessionInfo>> {
    file.session_info_bytes()
        .cloned()
        .map(iracing_sdk::schema::SessionInfo::try_from)
        .transpose()
        .map_err(Into::into)
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Captures JSON schema of a session string.
    #[cfg_attr(
        doc,
        doc = "Development contracts: [IBT](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-sdk-cli/docs/commands/session-schema-ibt.md) and [Windows live](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-sdk-cli/docs/commands/session-schema-live.md)."
    )]
    Schema(schema::Args),
    /// Discovers schema additions of a session string.
    #[cfg_attr(
        doc,
        doc = "Development contracts: [IBT](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-sdk-cli/docs/commands/session-discover-ibt.md) and [Windows live](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-sdk-cli/docs/commands/session-discover-live.md)."
    )]
    Discover(discover::Args),
    /// Captures a snapshot of the latest session string.
    #[cfg_attr(
        doc,
        doc = "Development contracts: [IBT](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-sdk-cli/docs/commands/session-snapshot-ibt.md) and [Windows live](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-sdk-cli/docs/commands/session-snapshot-live.md)."
    )]
    Snapshot(snapshot::Args),
}

impl Command {
    /// Execute the selected session snapshot, discovery, or schema command and propagate its errors.
    pub(crate) fn run(
        self,
        dependencies: &mut impl crate::commands::dependencies::LiveSessions,
    ) -> Result<()> {
        match self {
            Command::Snapshot(args) => args.run(dependencies),
            Command::Discover(args) => args.run(dependencies),
            Command::Schema(args) => args.run(dependencies),
        }
    }
}
