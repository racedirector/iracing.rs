use crate::{
    commands::dependencies::LiveSessions,
    utils::{DiskTelemetry, SourceKind},
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
    pub(crate) fn run(
        self,
        #[cfg_attr(not(windows), allow(unused_variables))] dependencies: &mut impl LiveSessions,
    ) -> Result<()> {
        let schema = match self.source {
            SourceKind::Ibt { extra } => DiskTelemetry::open(&extra.path)?.session_info()?,
            #[cfg(windows)]
            SourceKind::Live { .. } => dependencies.live_session()?,
        }
        .map(|info| schema_for_value!(info))
        .context("No session info found")?;

        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;
        writer.write(&schema)?;
        writer.finalize()
    }
}
