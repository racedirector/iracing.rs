use anyhow::{Result, ensure};
use iracing_sdk::{IRacingSDKError, IbtFile, LayoutProvider};

use crate::{
    commands::dependencies::LiveFrames,
    utils::SourceKind,
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
    /// a frame arrives. Returns an error if the frame has no telemetry fields,
    /// before opening output.
    pub(crate) fn run(
        &self,
        #[cfg_attr(not(windows), allow(unused_variables))] dependencies: &mut impl LiveFrames,
    ) -> Result<()> {
        let packet = match &self.source {
            SourceKind::Ibt { extra } => {
                let telemetry = IbtFile::open(&extra.path).map_err(snapshot_input_error)?;
                telemetry
                    .frame(extra.extra.frame_index)?
                    .into_packet()
                    .map_err(anyhow::Error::from)
            }
            #[cfg(windows)]
            SourceKind::Live { .. } => dependencies.next_live_frame(),
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

/// Preserve snapshot's existing missing-variable message for this one canonical
/// construction failure; unrelated parsing and I/O failures retain their cause.
fn snapshot_input_error(error: IRacingSDKError) -> anyhow::Error {
    if matches!(
        &error,
        IRacingSDKError::Parse { context, details }
            if context == "IbtFile::from_source"
                && details == "Telemetry frames require variable-header metadata"
    ) {
        anyhow::Error::new(error).context("No telemetry variables were available from the source")
    } else {
        error.into()
    }
}

#[cfg(test)]
mod input_error_tests {
    use super::*;

    #[test]
    fn only_missing_variable_metadata_gets_snapshot_context() {
        let missing = IRacingSDKError::parse_error(
            "IbtFile::from_source",
            "Telemetry frames require variable-header metadata",
        );
        assert!(
            snapshot_input_error(missing)
                .to_string()
                .contains("No telemetry variables")
        );
        for (context, details) in [
            (
                "IbtFile::from_source",
                "Source is shorter than the IBT preamble",
            ),
            (
                "another parser",
                "Telemetry frames require variable-header metadata",
            ),
        ] {
            let error = IRacingSDKError::parse_error(context, details);
            let message = error.to_string();
            assert_eq!(snapshot_input_error(error).to_string(), message);
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use clap::Parser;
    struct FakeFrames;
    impl LiveFrames for FakeFrames {
        fn live_fields(&mut self) -> Result<Vec<iracing_sdk::FieldLayout>> {
            panic!("snapshot only requests a frame")
        }
        fn next_live_frame(&mut self) -> Result<iracing_sdk::FramePacket> {
            anyhow::bail!("fake frame acquisition")
        }
        async fn next_live_frame_async(&mut self) -> Result<Option<iracing_sdk::FramePacket>> {
            panic!("snapshot uses synchronous acquisition")
        }
    }
    #[test]
    fn live_command_uses_injected_frame_capability() {
        let crate::Command::Telemetry {
            command: crate::commands::telemetry::Command::Snapshot(args),
        } = crate::Cli::try_parse_from(["iracing-sdk", "telemetry", "snapshot", "live"])
            .unwrap()
            .command
        else {
            panic!("expected snapshot command")
        };
        assert_eq!(
            args.run(&mut FakeFrames).unwrap_err().to_string(),
            "fake frame acquisition"
        );
    }
}
