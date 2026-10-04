use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use iracing_sdk::provider::VariableHeadersProvider;

use crate::{
    utils::get_disk_reader,
    writer::{DocumentFormat, DocumentWriter, OutputTarget, TelemetrySnapshot},
};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Captures the next frame output from the live telemetry
    #[cfg(windows)]
    Live {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the output.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,
    },
    /// Captures a frame from the IBT
    Ibt {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the telemetry frame.
        #[arg(long, default_value = "yaml", value_enum)]
        format: DocumentFormat,

        #[arg(long = "index", short = 'i', default_value = "0")]
        frame_index: usize,
    },
}

pub(super) async fn handle_command(command: Command) -> Result<()> {
    match command {
        #[cfg(windows)]
        Command::Live { output, format } => {
            use anyhow::anyhow;
            use futures::StreamExt;
            use iracing_sdk::{DynamicFrame, LayoutProvider, LiveConnection, UpdateRate};

            let connection = LiveConnection::builder().build()?;

            let mut variables = connection.fields_owned();
            if variables.is_empty() {
                return Err(anyhow!(
                    "No telemetry variables were available from the live connection"
                ));
            }
            variables.sort_unstable_by(|left, right| {
                left.region()
                    .offset()
                    .cmp(&right.region().offset())
                    .then_with(|| left.name().cmp(right.name()))
            });

            let mut frames = Box::pin(connection.subscribe::<DynamicFrame>(UpdateRate::Native)?);

            let frame = frames
                .next()
                .await
                .ok_or_else(|| anyhow!("Live telemetry ended before a frame was received"))?;

            let snapshot = TelemetrySnapshot::from_provider(&frame, &variables)?;
            let mut writer = DocumentWriter::from_parts(output, format)?;

            writer.write(&snapshot)?;
            writer.finalize()
        }
        Command::Ibt {
            path,
            output,
            format,
            frame_index,
        } => {
            let mut reader = get_disk_reader(&path)?;

            use iracing_sdk::{FramePacket, LayoutProvider, TelemetryLayout};
            use std::sync::Arc;

            let frame_count = reader.layout().frame_count();
            if frame_count == 0 {
                bail!("IBT file contains no telemetry frames");
            }
            if frame_index >= frame_count {
                bail!(
                    "Frame index {frame_index} is out of range; valid range: 0..={}",
                    frame_count - 1
                );
            }
            let data = reader
                .frame(frame_index)
                .with_context(|| format!("Could not read IBT frame {frame_index}"))?;

            let headers = reader.variable_headers()?;
            if headers.is_empty() {
                bail!("IBT contains no telemetry variable headers");
            }
            let layout = TelemetryLayout::try_from_headers(&headers, reader.layout().frame_size())?;

            let frame = FramePacket::new(
                data,
                u32::try_from(frame_index)
                    .context("Frame index exceeds the supported tick range")?,
                reader.header().session_info_update as u32,
                Arc::new(layout),
            )?;
            let mut variables = frame.fields_owned();
            variables.sort_unstable_by(|left, right| {
                left.region()
                    .offset()
                    .cmp(&right.region().offset())
                    .then_with(|| left.name().cmp(right.name()))
            });
            let snapshot = TelemetrySnapshot::from_provider(&frame, &variables)?;
            let mut writer = DocumentWriter::from_parts(output, format)?;
            writer.write(&snapshot)?;
            writer.finalize()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_sdk::test_utils::require_named_ibt_fixture;

    #[tokio::test]
    async fn ibt_writes_selected_frame_in_all_formats() -> Result<()> {
        let path = require_named_ibt_fixture("profile_small.ibt")?;
        let directory = tempfile::tempdir()?;
        for frame_index in [0, 11] {
            for format in [
                DocumentFormat::Json,
                DocumentFormat::JsonPretty,
                DocumentFormat::Yaml,
                DocumentFormat::None,
            ] {
                let output = directory.path().join("snapshot");
                handle_command(Command::Ibt {
                    path: path.clone(),
                    output: OutputTarget::File(output.clone()),
                    format,
                    frame_index,
                })
                .await?;
                let text = std::fs::read_to_string(output)?;
                let value: serde_json::Value = match format {
                    DocumentFormat::Yaml => serde_yaml_ng::from_str(&text)?,
                    _ => serde_json::from_str(&text)?,
                };
                // Deterministic profile_small values from the fixture generator.
                assert_eq!(value.as_object().unwrap().len(), 8);
                assert_eq!(value["Speed"], 35.0 + frame_index as f64 * 0.25);
                assert_eq!(value["SessionTime"], frame_index as f64 / 60.0);
                assert_eq!(value["LapDist"], frame_index as f64 * 18.5);
                if matches!(format, DocumentFormat::Json) {
                    assert!(text.starts_with("{\"SessionTime\":"));
                    assert!(text.ends_with('\n'));
                }
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn invalid_ibt_index_preserves_existing_output() -> Result<()> {
        let path = require_named_ibt_fixture("profile_small.ibt")?;
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("snapshot");
        std::fs::write(&output, "existing output")?;
        let error = handle_command(Command::Ibt {
            path,
            output: OutputTarget::File(output.clone()),
            format: DocumentFormat::Json,
            frame_index: 12,
        })
        .await
        .unwrap_err();
        assert!(error.to_string().contains("valid range: 0..=11"));
        assert_eq!(std::fs::read_to_string(output)?, "existing output");
        Ok(())
    }

    #[tokio::test]
    async fn ibt_snapshot_propagates_output_errors() -> Result<()> {
        let path = require_named_ibt_fixture("profile_small.ibt")?;
        let directory = tempfile::tempdir()?;
        let error = handle_command(Command::Ibt {
            path,
            output: OutputTarget::File(directory.path().join("missing").join("snapshot")),
            format: DocumentFormat::Json,
            frame_index: 0,
        })
        .await
        .unwrap_err();
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        Ok(())
    }
}
