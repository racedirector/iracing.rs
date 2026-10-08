use crate::{
    utils::SourceKind,
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
};
use anyhow::{Context, Result};
use iracing_sdk::provider::SessionInformationProvider;
use schemars::schema_for_value;

#[derive(clap::Args, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    source: SourceKind,

    /// Output destination. Use `-` for stdout.
    #[arg(short, long, default_value = "-", global = true)]
    output: OutputTarget,

    /// The format for the output.
    #[arg(long, default_value = "yaml", global = true, value_enum)]
    format: DocumentFormat,
}

impl Args {
    pub(crate) fn run(self) -> Result<()> {
        let schema = self
            .source
            .open()?
            .session_info()?
            .map(|info| schema_for_value!(info))
            .context("No session info found")?;

        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;
        writer.write(&schema)?;
        writer.finalize()
    }
}
