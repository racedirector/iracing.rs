use anyhow::Result;

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

    /// The encoding for the session string.
    #[arg(long, default_value = "yaml", global = true, value_enum)]
    format: DocumentFormat,
}

impl Args {
    pub(crate) fn run(self) -> Result<()> {
        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;

        match self.source {
            SourceKind::Ibt { extra } => {
                let telemetry = DiskTelemetry::open(&extra.path)?;
                writer.write(&telemetry.reader.header())?;
                writer.write(&telemetry.reader.disk_header())?;
            }
            #[cfg(windows)]
            SourceKind::Live { .. } => {
                use crate::utils::LiveTelemetry;

                let telemetry = LiveTelemetry::try_connect()?;
                let header = telemetry.connection.header_snapshot()?;
                writer.write(&header)?;
            }
        }

        writer.finalize()
    }
}
