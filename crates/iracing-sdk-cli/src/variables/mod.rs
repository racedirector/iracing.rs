use anyhow::{Result, bail};
use clap::Subcommand;
use std::path::PathBuf;

use crate::{
    utils::get_disk_reader,
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Gets headers from a provided IBT
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
    /// Gets headers from a live iRacing connection.
    #[cfg(windows)]
    Live {
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
        Command::Ibt {
            path,
            output,
            format,
        } => {
            let mut reader = get_disk_reader(&path)?;

            let Some(snapshot) = reader.variable_headers_snapshot()? else {
                bail!(format!(
                    "No variable headers found in IBT file: {}",
                    path.display()
                ))
            };

            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&snapshot)?;
            writer.finalize()
        }
        #[cfg(windows)]
        Command::Live { output, format } => {
            use crate::utils::get_connection;

            let connection = get_connection()?;

            let Some(snapshot) = connection.variable_headers_buffer() else {
                bail!("Could not retrieve variable headers from live connection")
            };

            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&snapshot)?;
            writer.finalize()
        }
    }
}
