use anyhow::Result;
use iracing_sdk::provider::SessionInformationProvider;

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

    /// The format for the output.
    #[arg(long, default_value = "yaml", global = true, value_enum)]
    format: DocumentFormat,
}

impl Args {
    pub(crate) fn run(self) -> Result<()> {
        let unknown_fields = self
            .source
            .open()?
            .session_info()?
            .map(|info| info.collect_unknown_fields())
            .unwrap_or(vec![]);

        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;
        writer.write(&unknown_fields)?;

        writer.finalize()
    }
}
