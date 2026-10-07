use anyhow::Result;

use crate::{
    utils::{SourceKind, TelemetrySource},
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
        let source = self.source.open()?;

        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;
        match source {
            TelemetrySource::Disk(reader) => {
                writer.write(&reader.header())?;
                writer.write(&reader.disk_header())?;
            }
            #[cfg(windows)]
            TelemetrySource::Live(connection) => {
                let header = connection.header_snapshot()?;
                writer.write(&header)?;
            }
        }

        writer.finalize()
    }
}
