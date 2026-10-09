mod convert;
#[cfg(windows)]
mod record;
mod snapshot;

use anyhow::Result;
use clap::Subcommand;

/// Export recorded telemetry, record live telemetry, or capture a single frame.
#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Converts an IBT to CSV or JSONL
    Convert(convert::Args),
    /// Records a live connection telemetry stream to the output as CSV or JSONL
    #[cfg(windows)]
    Record(record::Args),
    /// Snapshots a telemetry source
    Snapshot(snapshot::Args),
}

impl Command {
    /// Execute the selected telemetry command and propagate source and output errors.
    pub async fn run(self, dependencies: &mut impl crate::dependencies::LiveFrames) -> Result<()> {
        match self {
            Self::Convert(args) => args.run(),
            #[cfg(windows)]
            Self::Record(args) => args.run(dependencies).await,
            Self::Snapshot(args) => args.run(dependencies),
        }
    }
}
