use anyhow::Result;
use clap::Subcommand;
use iracing_sdk::provider::VariableHeadersProvider;
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

impl Command {
    /// Write the IBT or live variable headers to the selected destination and format.
    ///
    /// File output creates or truncates the destination.
    ///
    /// # Errors
    ///
    /// Propagates source access, variable-header retrieval, serialization,
    /// and output creation, write, or flush errors.
    pub fn run(self) -> Result<()> {
        match self {
            Command::Ibt {
                path,
                output,
                format,
            } => {
                let reader = get_disk_reader(&path)?;

                write_headers(reader, output, format)
            }
            #[cfg(windows)]
            Command::Live { output, format } => {
                use crate::utils::get_connection;

                let connection = get_connection()?;

                write_headers(connection, output, format)
            }
        }
    }
}

fn write_headers(
    provider: impl VariableHeadersProvider,
    output: OutputTarget,
    format: DocumentFormat,
) -> Result<()> {
    let headers = provider.variable_headers()?;

    let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
    writer.write(&headers)?;
    writer.finalize()
}
