use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

use crate::{
    types::{DocumentFormat, OutputTarget},
    utils::{get_disk_reader, write_to_output},
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
        encoding: DocumentFormat,
    },
    /// Gets headers from a live iRacing connection.
    #[cfg(windows)]
    Live,
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    match command {
        Command::Ibt {
            path,
            output,
            encoding,
        } => {
            let mut reader = get_disk_reader(&path)?;

            let Some(snapshot) = reader.variable_headers_snapshot()? else {
                return anyhow::bail!(format!(
                    "Could not retrieve variable headers from ibt: {}",
                    path.display()
                ));
            };

            write_to_output(&snapshot, &output, encoding)?;

            Ok(())
        }
        #[cfg(windows)]
        Command::Live => {
            // use crate::utils::get_connection;

            // let connection = get_connection()?;

            anyhow::bail!("Not implemented")
            // Ok(())
        }
    }
}
