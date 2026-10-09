use anyhow::Result;
use iracing_sdk::provider::SessionInformationProvider;

use crate::{
    commands::dependencies::LiveSessions,
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
    /// Write and flush unknown session fields from the IBT or injected live source.
    ///
    /// An absent session produces an empty list. File output is created or
    /// truncated only after session acquisition and parsing succeed.
    ///
    /// # Errors
    ///
    /// Propagates source initialization, acquisition, session parsing,
    /// serialization, and output errors; these do not produce an empty list.
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
