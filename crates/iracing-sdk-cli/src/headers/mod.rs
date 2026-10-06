use anyhow::Result;
use clap::Subcommand;
use iracing_irsdk::{DiskSubHeader, Header, IbtHeader, VariableBuffer, VariableHeader};
use std::{fs::File, io::Read, path::PathBuf};
use type_layout::TypeLayout;

use crate::writer::{DocumentFormat, DocumentWriter, OutputTarget};

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
    /// Prints the type information for the header data structures.
    Type,
}

impl Command {
    /// Write IBT or live headers in the selected format, or print type layouts to stdout.
    ///
    /// File output creates or truncates the destination.
    ///
    /// # Errors
    ///
    /// Propagates file or live-connection access, header decoding, serialization,
    /// and output creation, write, or flush errors.
    pub fn run(self) -> Result<()> {
        match self {
            Command::Ibt {
                path,
                output,
                format,
            } => {
                // Open the file
                let file = File::open(path)?;
                let mut handle = file.take(size_of::<IbtHeader>() as u64);

                // Read the header
                let header = IbtHeader::try_from_reader(&mut handle)?;

                // Write output
                let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
                writer.write(&header)?;
                writer.finalize()
            }
            #[cfg(windows)]
            Command::Live { output, format } => {
                use crate::utils::get_connection;

                // Open the connection
                let connection = get_connection()?;
                // Read the header
                let header = connection.header_snapshot()?;

                // Write output
                let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
                writer.write(&header)?;
                writer.finalize()
            }
            Command::Type => {
                let mut writer =
                    DocumentWriter::from_parts(OutputTarget::Stdout, DocumentFormat::None)?;

                for layout in [
                    VariableBuffer::type_layout(),
                    Header::type_layout(),
                    DiskSubHeader::type_layout(),
                    VariableHeader::type_layout(),
                ] {
                    writer.write(&layout.to_string())?;
                }

                writer.finalize()
            }
        }
    }
}
