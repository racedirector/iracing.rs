use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

use crate::{
    types::{DocumentFormat, OutputTarget},
    utils::{get_disk_reader, get_disk_session_info, write_to_output},
};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the session string.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: DocumentFormat,
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
        encoding: DocumentFormat,
    },
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    match command {
        #[cfg(windows)]
        Command::Live { output, encoding } => {
            use crate::utils::{get_connection, get_live_session_info};

            let connection = get_connection()?;
            let session_info = get_live_session_info(&connection)?;
            let unknown_fields = session_info.collect_unknown_fields();
            write_to_output(&unknown_fields, &output, encoding)?;
            tracing::info!(output=%output, encoding=%encoding, "Wrote live session unknown fields snapshot.");
        }
        Command::Ibt {
            path,
            output,
            encoding,
        } => {
            let mut reader = get_disk_reader(&path)?;
            let session_info = get_disk_session_info(&mut reader)?;
            let unknown_fields = session_info.collect_unknown_fields();
            write_to_output(&unknown_fields, &output, encoding)?;
            tracing::info!(ibt_path=%path.display(), output=%output, encoding=%encoding, "Wrote disk session unknown fields snapshot.");
        }
    }

    Ok(())
}
