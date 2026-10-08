use anyhow::Result;
use iracing_sdk::LayoutProvider;

use crate::{
    utils::{DiskTelemetry, SourceKind},
    writer::{DocumentFormat, DocumentWriter, OutputTarget, TelemetrySnapshot},
};

#[derive(clap::Args, Debug)]
struct IbtExtraArgs {
    #[arg(long = "index", short = 'i', default_value = "0", global = true)]
    pub frame_index: usize,
}

#[derive(clap::Args, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    source: SourceKind<IbtExtraArgs>,

    /// Output destination. Use `-` for stdout.
    #[arg(short, long, default_value = "-", global = true)]
    output: OutputTarget,

    /// The encoding for the output.
    #[arg(long, default_value = "yaml", global = true, value_enum)]
    format: DocumentFormat,
}

impl Args {
    /// Write and flush one IBT frame or the next live frame in the selected format.
    ///
    /// The IBT index is zero-based. Live capture blocks without an overall timeout
    /// and is available only on Windows. File output is created or truncated
    /// before opening the source, even if capture later fails.
    ///
    /// # Errors
    ///
    /// Propagates source setup, layout validation, frame acquisition, counter
    /// conversion, telemetry decoding, serialization, and output errors,
    /// including an out-of-range IBT index or disconnected live source at setup.
    pub(crate) fn run(&self) -> Result<()> {
        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;

        let packet = match &self.source {
            SourceKind::Ibt { extra } => {
                let telemetry = DiskTelemetry::open(&extra.path)?;
                telemetry.frame_at(extra.extra.frame_index)
            }
            #[cfg(windows)]
            SourceKind::Live { .. } => {
                use crate::utils::LiveTelemetry;
                let mut telemetry = LiveTelemetry::try_connect()?;
                telemetry.next_frame()
            }
        }?;

        let fields_owned = packet.fields_owned();
        let snapshot = TelemetrySnapshot::from_provider(&packet, &fields_owned)?;
        writer.write(&snapshot)?;

        writer.finalize()
    }
}
