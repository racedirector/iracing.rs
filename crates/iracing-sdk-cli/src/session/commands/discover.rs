use anyhow::Result;
use clap::Subcommand;
use iracing_sdk::provider::{SessionInformationBytesProvider, SessionInformationProvider};
use std::path::PathBuf;

use crate::{
    utils::get_disk_reader,
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
            use crate::utils::get_connection;

            let connection = get_connection()?;
            write_unknown_fields(&connection, output.clone(), format)?;
            tracing::info!(output=%output, format=%format, "Wrote live session unknown fields snapshot.");
        }
        Command::Ibt {
            path,
            output,
            format,
        } => {
            let reader = get_disk_reader(&path)?;
            write_unknown_fields(&reader, output.clone(), format)?;
            tracing::info!(ibt_path=%path.display(), output=%output, format=%format, "Wrote disk session unknown fields snapshot.");
        }
    }

    Ok(())
}

fn write_unknown_fields(
    provider: &impl SessionInformationBytesProvider,
    output: OutputTarget,
    format: DocumentFormat,
) -> Result<()> {
    let unknown_fields = provider
        .session_info()?
        .map(|info| info.collect_unknown_fields())
        .unwrap_or(vec![]);

    let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
    writer.write(&unknown_fields)?;

    writer.finalize()
}
