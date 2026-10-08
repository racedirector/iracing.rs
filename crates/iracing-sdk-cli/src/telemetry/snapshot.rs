use anyhow::{Result, ensure};
use iracing_sdk::LayoutProvider;

use crate::{
    utils::{DiskTelemetry, SourceKind},
    writer::{DocumentFormat, DocumentWriter, OutputTarget, TelemetrySnapshot},
};

#[derive(clap::Args, Debug)]
struct IbtExtraArgs {
    /// Zero-based frame index in the IBT recording.
    #[arg(long = "index", short = 'i', default_value = "0")]
    pub frame_index: usize,
}

/// Capture one telemetry frame; live capture is available only on Windows.
///
/// ```text
/// iracing-sdk telemetry snapshot live --output telemetry.json --format json
/// iracing-sdk telemetry snapshot live --output telemetry.yaml
/// iracing-sdk telemetry snapshot ibt --path ./test-data/ibt/profile_small.ibt --output telemetry.yaml
/// iracing-sdk telemetry snapshot ibt --path ./test-data/ibt/profile_small.ibt --index 11 --format json
/// ```
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
    /// only after the frame has been acquired and decoded.
    ///
    /// # Errors
    ///
    /// Propagates source setup, layout validation, frame acquisition, counter
    /// conversion, telemetry decoding, serialization, and output errors,
    /// including an out-of-range IBT index or a live source disconnecting before
    /// a frame arrives.
    pub(crate) fn run(&self) -> Result<()> {
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
        ensure!(
            !fields_owned.is_empty(),
            "No telemetry variables were available from the source"
        );
        let snapshot = TelemetrySnapshot::from_provider(&packet, &fields_owned)?;
        let mut writer = DocumentWriter::from_parts(self.output.clone(), self.format)?;
        writer.write(&snapshot)?;

        writer.finalize()
    }
}
