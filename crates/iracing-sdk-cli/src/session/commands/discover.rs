use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

use crate::types::{OutputEncoding, OutputTarget};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    match command {
        #[cfg(windows)]
        Command::Live { output, encoding } => {
            // let session_info = capture_live_session_info()?;
            // let unknown_fields = session_info.collect_unknown_fields();
            // write_to_output(&unknown_fields, &output, encoding)?;
        }
        Command::Ibt {
            path,
            output,
            encoding,
        } => {
            // let session_info = capture_disk_session_info(&path)?;
            // let unknown_fields = session_info.collect_unknown_fields();
            // write_to_output(&unknown_fields, &output, encoding)?;
        }
    }

    Ok(())
}
