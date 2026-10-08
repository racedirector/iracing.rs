use anyhow::Result;
use iracing_sdk::provider::VariableHeadersProvider;

use crate::{
    utils::SourceKind,
    writer::{DocumentFormat, DocumentWriter, OutputTarget},
};

#[derive(clap::Args, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    source: SourceKind,

    /// Output destination. Use `-` for stdout.
    #[arg(short, long, default_value = "-", global = true)]
    output: OutputTarget,

    /// The encoding for the session string.
    #[arg(long, default_value = "yaml", global = true, value_enum)]
    format: DocumentFormat,
}

impl Args {
    pub(crate) fn run(self) -> Result<()> {
        let headers = self.source.open()?.variable_headers()?;

        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;
        writer.write(&headers)?;
        writer.finalize()
    }
}
