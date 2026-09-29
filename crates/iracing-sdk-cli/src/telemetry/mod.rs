use anyhow::{Result, bail};
use clap::Subcommand;
use std::path::PathBuf;

use crate::types::{DocumentFormat, OutputTarget};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Converts an IBT to CSV or JSONL
    Convert {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the output.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
    /// Records a live connection telemetry stream to the output as CSV or JSONL
    #[cfg(windows)]
    Record,
    /// Snapshots a telemetry source
    Snapshot,
}

#[derive(Subcommand, Debug)]
pub(crate) enum SnapshotCommand {
    /// Captures the next frame output from the live telemetry
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the output.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
    /// Captures a frame from the IBT
    Ibt {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    match command {
        Command::Convert {
            path,
            output,
            format,
        } => bail!("convert not implemented"),
        Command::Record => bail!("record not implemented"),
        Command::Snapshot => bail!("snapshot not implemented"),
    }
}
