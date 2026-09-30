use crate::{
    utils::{get_disk_reader, get_disk_session_info},
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
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

        /// The format for the output.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
    /// Captures the session string from the IBT file and outputs it to the destination in the requested format.
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The format for the output.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    match command {
        Command::Ibt {
            path,
            output,
            format,
        } => {
            let mut reader = get_disk_reader(&path)?;
            let session_info = get_disk_session_info(&mut reader)?;

            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&session_info)?;
            tracing::info!(ibt_path=%path.display(), output=%output, encoding=%format, "Wrote disk session snapshot.");
            writer.finalize()
        }
        #[cfg(windows)]
        Command::Live { output, format } => {
            use crate::utils::{get_connection, get_live_session_info};

            let connection = get_connection()?;
            let session_info = get_live_session_info(&connection)?;

            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&session_info)?;
            tracing::info!(output=%output, encoding=%format, "Wrote live session snapshot.");
            writer.finalize()
        }
    }
}
