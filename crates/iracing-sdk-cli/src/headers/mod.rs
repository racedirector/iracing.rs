use anyhow::Result;
use clap::Subcommand;
use iracing_sdk::irsdk::{DiskSubHeader, Header, VariableBuffer, VariableHeader};
use std::{fs::File, io::Read, path::PathBuf};
use type_layout::TypeLayout;

use crate::{
    types::{DocumentFormat, OutputTarget},
    utils::{write_to_output, write_to_writer},
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
    /// Prints the type information for the header data structures.
    Type,
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    match command {
        Command::Ibt {
            path,
            output,
            format,
        } => {
            let file = File::open(path)?;
            let mut handle =
                file.take(size_of::<Header>() as u64 + size_of::<DiskSubHeader>() as u64);

            let header = Header::try_from_reader(&mut handle)?;
            let sub_header = DiskSubHeader::try_from_reader(&mut handle)?;

            write_to_output(&header, &output, format)?;
            write_to_output(&sub_header, &output, format)?;

            Ok(())
        }
        #[cfg(windows)]
        Command::Live { output, format } => {
            use crate::utils::get_connection;

            let connection = get_connection()?;
            let header = connection.header();

            write_to_output(&header, &output, format)?;

            Ok(())
        }
        Command::Type => {
            let stdout = std::io::stdout();

            write_to_writer(
                &format!("{}", VariableBuffer::type_layout()).to_string(),
                stdout.lock(),
                DocumentFormat::None,
            )?;

            write_to_writer(
                &format!("{}", Header::type_layout()).to_string(),
                stdout.lock(),
                DocumentFormat::None,
            )?;

            write_to_writer(
                &format!("{}", DiskSubHeader::type_layout()).to_string(),
                stdout.lock(),
                DocumentFormat::None,
            )?;

            write_to_writer(
                &format!("{}", VariableHeader::type_layout()).to_string(),
                stdout.lock(),
                DocumentFormat::None,
            )?;

            Ok(())
        }
    }
}
