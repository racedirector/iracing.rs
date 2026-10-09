use crate::dependencies::LiveSessions;
use anyhow::Result;
use iracing_sdk::provider::SessionInformationProvider;

use crate::{
    utils::{DiskTelemetry, SourceKind},
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
    pub(crate) fn run(
        self,
        #[cfg_attr(not(windows), allow(unused_variables))] dependencies: &mut impl LiveSessions,
    ) -> Result<()> {
        let unknown_fields = match self.source {
            SourceKind::Ibt { extra } => DiskTelemetry::open(&extra.path)?.session_info()?,
            #[cfg(windows)]
            SourceKind::Live { .. } => dependencies.live_session()?,
        }
        .map(|info| info.collect_unknown_fields())
        .unwrap_or(vec![]);

        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;
        writer.write(&unknown_fields)?;

        writer.finalize()
    }
}
