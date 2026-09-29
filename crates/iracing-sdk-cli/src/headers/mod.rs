use anyhow::Result;
use clap::Subcommand;
use iracing_sdk::irsdk::{DiskSubHeader, Header, VariableBuffer, VariableHeader};
use std::{fs::File, io::Read, path::PathBuf};
use type_layout::TypeLayout;

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Gets headers from a provided IBT
    Ibt {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,
    },
    /// Gets headers from a live iRacing connection.
    #[cfg(windows)]
    Live,
    /// Prints the type information for the header data structures.
    Type,
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    match command {
        Command::Ibt { path } => {
            let file = File::open(path)?;
            let mut handle =
                file.take(size_of::<Header>() as u64 + size_of::<DiskSubHeader>() as u64);

            let header = Header::try_from_reader(&mut handle)?;
            let sub_header = DiskSubHeader::try_from_reader(&mut handle)?;

            tracing::info!(
                "Parsed IBT header and sub-header:\n{:#?}\n{:#?}",
                header,
                sub_header
            );

            Ok(())
        }
        #[cfg(windows)]
        Command::Live => {
            use crate::utils::get_connection;

            let connection = get_connection()?;
            let header = connection.header();

            tracing::info!("Parsed live header:\n{:#?}", header,);

            Ok(())
        }
        Command::Type => {
            println!(
                "VariableBuffer type layout:\n{}",
                VariableBuffer::type_layout()
            );

            println!("Header type layout:\n{}", Header::type_layout());

            println!(
                "DiskSubHeader type layout:\n{}",
                DiskSubHeader::type_layout()
            );

            println!(
                "VariableHeader type layout:\n{}",
                VariableHeader::type_layout()
            );

            Ok(())
        }
    }
}
