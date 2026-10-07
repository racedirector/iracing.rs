use crate::{
    utils::get_disk_reader,
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
};
use anyhow::{Context, Result};
use clap::Subcommand;
use iracing_sdk::provider::{SessionInformationBytesProvider, SessionInformationProvider};
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

impl Command {
    /// Write parsed IBT or live session information in the selected output format.
    ///
    /// File output creates or truncates the destination.
    ///
    /// # Errors
    ///
    /// Returns an error when session information is absent. Propagates source access,
    /// session parsing, serialization, and output errors.
    pub(crate) fn run(self) -> Result<()> {
        match self {
            Command::Ibt {
                path,
                output,
                format,
            } => {
                let reader = get_disk_reader(&path)?;
                write_session_info(&reader, output.clone(), format)?;
                tracing::info!(ibt_path=%path.display(), output=%output, encoding=%format, "Wrote disk session snapshot.");
                Ok(())
            }
            #[cfg(windows)]
            Command::Live { output, format } => {
                use crate::utils::get_connection;

                let connection = get_connection()?;
                write_session_info(&connection, output.clone(), format)?;
                tracing::info!(output=%output, encoding=%format, "Wrote live session snapshot.");
                Ok(())
            }
        }
    }
}

fn write_session_info(
    provider: &impl SessionInformationBytesProvider,
    output: OutputTarget,
    format: DocumentFormat,
) -> Result<()> {
    let session_info = provider
        .session_info()?
        .context("Session information is unavailable")?;
    let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
    writer.write(&session_info)?;
    writer.finalize()
}
