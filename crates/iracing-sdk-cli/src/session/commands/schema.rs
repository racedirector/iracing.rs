use crate::types::{OutputEncoding, OutputTarget};
use anyhow::Result;
use clap::Subcommand;
// use schemars::{schema_for, schema_for_value};
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

        /// The encoding for the JSON schema.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
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

        /// The encoding for the JSON schema.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
    /// Outputs a JSON schema of the underlying library type to the output in the requested format.
    Type {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the JSON schema.
        #[arg(long, default_value = "yaml", value_enum)]
        encoding: OutputEncoding,
    },
}

pub(crate) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Ibt {
            path,
            output,
            encoding,
        } => {
            // let session_info = capture_disk_session_info(&path)?;
            // let schema = schema_for_value!(session_info);
            // write_to_output(&schema, &output, encoding)?;
            tracing::info!(output=%output, encoding=%encoding, ibt_path=%path.display(),"Wrote IBT session schema");
        }
        #[cfg(windows)]
        Commands::Live { output, encoding } => {
            // let session_info = capture_live_session_info()?;
            // let schema = schema_for_value!(session_info);
            // write_to_output(&schema, &output, encoding)?;
            tracing::info!(output=%output,encoding=%encoding,"Wrote live session schema");
        }
        Commands::Type { output, encoding } => {
            // let schema = schema_for!(iracing_sdk::schema::SessionInfo);
            // write_to_output(&schema, &output, encoding)?;
            tracing::info!(output=%output,encoding=%encoding,"Wrote static session schema");
        }
    }

    Ok(())
}
