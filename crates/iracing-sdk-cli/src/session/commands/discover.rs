use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

use crate::{
    utils::{get_disk_reader, get_disk_session_info},
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The format for the output.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
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
        #[cfg(windows)]
        Command::Live { output, format } => {
            use crate::utils::{get_connection, get_live_session_info};

            let connection = get_connection()?;
            let session_info = get_live_session_info(&connection)?;
            let unknown_fields = session_info.collect_unknown_fields();

            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&unknown_fields)?;
            tracing::info!(output=%output, format=%format, "Wrote live session unknown fields snapshot.");
            writer.finalize()
        }
        Command::Ibt {
            path,
            output,
            format,
        } => {
            let mut reader = get_disk_reader(&path)?;
            let session_info = get_disk_session_info(&mut reader)?;
            let unknown_fields = session_info.collect_unknown_fields();

            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&unknown_fields)?;
            tracing::info!(ibt_path=%path.display(), output=%output, format=%format, "Wrote disk session unknown fields snapshot.");
            writer.finalize()
        }
    }
}
