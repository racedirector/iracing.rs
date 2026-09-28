use crate::{
    types::{OutputEncoding, OutputTarget},
    utils::{get_disk_reader, get_disk_session_info, write_to_output},
};
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Captures the latest session string from a live iRacing connection and outputs it to the destination in the requested format.
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
    /// Captures the session string from the IBT file and outputs it to the destination in the requested format.
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
        Command::Ibt {
            path,
            output,
            encoding,
        } => {
            let mut reader = get_disk_reader(&path)?;
            let session_info = get_disk_session_info(&mut reader)?;
            // let session_info = capture_disk_session_info(&path)?;
            write_to_output(&session_info, &output, encoding)?;

            tracing::info!(ibt_path=%path.display(), output=%output, encoding=%encoding, "Wrote disk session snapshot.");
        }
        #[cfg(windows)]
        Command::Live { output, encoding } => {
            use crate::utils::{get_connection, get_live_session_info};

            let connection = get_connection()?;
            let session_info = get_live_session_info(&connection)?;
            write_to_output(&session_info, &output, encoding)?;

            tracing::info!(output=%output, encoding=%encoding, "Wrote live session snapshot.");
        }
    }

    Ok(())
}
