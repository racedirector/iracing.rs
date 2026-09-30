use crate::{
    utils::{get_disk_reader, get_disk_session_info},
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
};
use anyhow::Result;
use clap::Subcommand;
use schemars::{schema_for, schema_for_value};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Captures the most-recent session string and outputs JSON schema to `output` in
    /// the requested format.
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The format for the JSON schema.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
    /// Captures the session string from the IBT file and outputs JSON schema to `output`
    /// in the requested format.
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The format for the JSON schema.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
    /// Outputs a JSON schema of the underlying library type to the output in the requested format.
    Type {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The format for the JSON schema.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
}

pub(crate) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Ibt {
            path,
            output,
            format,
        } => {
            let mut reader = get_disk_reader(&path)?;
            let session_info = get_disk_session_info(&mut reader)?;
            let schema = schema_for_value!(session_info);

            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&schema)?;
            tracing::info!(output=%output, format=%format, ibt_path=%path.display(),"Wrote IBT session schema");
            writer.finalize()
        }
        #[cfg(windows)]
        Commands::Live { output, format } => {
            use crate::utils::{get_connection, get_live_session_info};

            let connection = get_connection()?;
            let session_info = get_live_session_info(&connection)?;
            let schema = schema_for_value!(session_info);

            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&schema)?;
            tracing::info!(output=%output,format=%format,"Wrote live session schema");
            writer.finalize()
        }
        Commands::Type { output, format } => {
            let schema = schema_for!(iracing_sdk::schema::SessionInfo);

            let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
            writer.write(&schema)?;
            tracing::info!(output=%output,format=%format,"Wrote static session schema");
            writer.finalize()
        }
    }
}
