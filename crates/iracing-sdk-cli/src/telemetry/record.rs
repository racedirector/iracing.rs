use crate::dependencies::LiveFrames;
use anyhow::{Context, Result, ensure};

use crate::writer::{OutputTarget, RecordStreamFormat, RecordStreamWriter};

/// Records a live connection to the given format at output.
#[derive(clap::Args, Debug)]
pub(crate) struct Args {
    /// Output destination. Use `-` for stdout.
    #[arg(short, long, default_value = "-", global = true)]
    output: OutputTarget,

    /// The encoding for the output.
    #[arg(long, default_value = "csv", global = true, value_enum)]
    format: RecordStreamFormat,
}

impl Args {
    pub(crate) async fn run(&self, dependencies: &mut impl LiveFrames) -> Result<()> {
        let variables = dependencies.live_fields()?;
        ensure!(
            !variables.is_empty(),
            "No telemetry variables were available from the live connection"
        );

        let mut writer =
            RecordStreamWriter::from_variables(self.output.clone(), self.format, variables)?
                .prepare()?;

        tracing::info!("Recording live telemetry; press Ctrl+C to stop");
        let shutdown = tokio::signal::ctrl_c();
        tokio::pin!(shutdown);
        let mut frame_count = 0usize;
        loop {
            tokio::select! {
                result = &mut shutdown => {
                    result.context("Failed to listen for Ctrl+C")?;
                    break;
                }
                frame = dependencies.next_live_frame_async() => {
                    let Some(packet) = frame? else { break };
                    writer.write(&packet)?;
                    frame_count += 1;
                    if frame_count.is_multiple_of(10_000) {
                        tracing::debug!(frames_exported = frame_count, "Live export progress");
                    }
                }
            }
        }
        writer.finalize()?;
        tracing::info!(frames_exported = frame_count, "Finished live export");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    struct EmptyFrames;
    impl LiveFrames for EmptyFrames {
        fn live_fields(&mut self) -> Result<Vec<iracing_sdk::FieldLayout>> {
            Ok(vec![])
        }
        fn next_live_frame(&mut self) -> Result<iracing_sdk::FramePacket> {
            panic!("record uses async capture")
        }
        async fn next_live_frame_async(&mut self) -> Result<Option<iracing_sdk::FramePacket>> {
            panic!("empty layout must fail before capture")
        }
    }
    #[tokio::test]
    async fn injected_empty_layout_fails_before_creating_output() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("record.csv");
        let crate::Command::Telemetry {
            command: crate::telemetry::Command::Record(args),
        } = crate::Cli::try_parse_from([
            "iracing-sdk",
            "telemetry",
            "record",
            "--output",
            output.to_str().unwrap(),
        ])?
        .command
        else {
            panic!("expected record command")
        };
        let error = args.run(&mut EmptyFrames).await.unwrap_err();
        assert!(error.to_string().contains("No telemetry variables"));
        assert!(!output.exists());
        Ok(())
    }
}
