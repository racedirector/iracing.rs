use anyhow::{Result, bail};
use clap::Subcommand;
use std::path::PathBuf;

use crate::{
    types::{DocumentFormat, OutputTarget},
    utils::{get_connection, get_disk_reader},
};

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
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommand,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum SnapshotCommand {
    /// Captures the next frame output from the live telemetry
    #[cfg(windows)]
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
        } => bail!(format!(
            "convert not implemented: path = {}, output = {}, format = {}",
            path.display(),
            output,
            format
        )),
        Command::Record => bail!("record not implemented"),
        Command::Snapshot { command } => handle_snapshot_command(command),
    }
}

fn handle_snapshot_command(command: SnapshotCommand) -> Result<()> {
    match command {
        SnapshotCommand::Live {
            output: _,
            format: _,
        } => {
            let connection = get_connection()?;

            let Some(_) = connection.variable_headers_buffer() else {
                bail!("Could not get variable headers buffer");
            };

            Ok(())
        }
        SnapshotCommand::Ibt {
            path,
            output: _,
            format: _,
        } => {
            let mut reader = get_disk_reader(&path)?;

            let Some(_) = reader.variable_headers_snapshot()? else {
                bail!(format!(
                    "Could not get variable headers from {}",
                    path.display()
                ));
            };

            Ok(())
        }
    }
}
