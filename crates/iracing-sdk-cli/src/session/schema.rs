use crate::{
    utils::get_disk_reader,
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
};
use anyhow::{Result, bail};
use clap::Subcommand;
use iracing_sdk::provider::{SessionInformationBytesProvider, SessionInformationProvider};
use schemars::{schema_for, schema_for_value};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
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

impl Command {
    /// Write a schema inferred from IBT or live session data, or the static session type schema.
    ///
    /// Uses the selected output format and creates or truncates file destinations.
    ///
    /// # Errors
    ///
    /// Returns an error when a source has no session information. Propagates source
    /// access, session parsing, serialization, and output errors.
    pub(crate) fn run(self) -> Result<()> {
        match self {
            Command::Ibt {
                path,
                output,
                format,
            } => {
                let reader = get_disk_reader(&path)?;
                write_session_info_schema(&reader, output.clone(), format)?;

                tracing::info!(output=%output, format=%format, ibt_path=%path.display(),"Wrote IBT session schema");
            }
            #[cfg(windows)]
            Command::Live { output, format } => {
                use crate::utils::get_connection;

                let connection = get_connection()?;
                write_session_info_schema(&connection, output.clone(), format)?;

                tracing::info!(output=%output,format=%format,"Wrote live session schema");
            }
            Command::Type { output, format } => {
                let schema = schema_for!(iracing_sdk::schema::SessionInfo);

                let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
                writer.write(&schema)?;
                writer.finalize()?;

                tracing::info!(output=%output,format=%format,"Wrote static session schema");
            }
        }

        Ok(())
    }
}

fn write_session_info_schema(
    provider: &impl SessionInformationBytesProvider,
    output: OutputTarget,
    format: DocumentFormat,
) -> Result<()> {
    let Some(session_info) = provider.session_info()? else {
        bail!("Could not retrieve session info")
    };

    let schema = schema_for_value!(session_info);

    let mut writer = DocumentWriter::from_parts(output.clone(), format)?;
    writer.write(&schema)?;
    writer.finalize()
}
