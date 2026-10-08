use anyhow::{Context, Result, ensure};
use iracing_sdk::LayoutProvider;

use crate::{
    utils::LiveTelemetry,
    writer::{OutputTarget, RecordStreamFormat, RecordStreamWriter},
};

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
    pub(crate) async fn run(&self) -> Result<()> {
        let mut telemetry = LiveTelemetry::try_connect()?;
        let variables = telemetry.fields_owned();
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
                frame = telemetry.next_frame_async() => {
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
