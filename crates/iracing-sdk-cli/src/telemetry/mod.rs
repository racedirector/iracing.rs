#[cfg(windows)]
use crate::utils::get_connection;
mod snapshot;

#[cfg(windows)]
use anyhow::bail;
use anyhow::{Context, Result};
use clap::Subcommand;
use futures::StreamExt;
use iracing_sdk::{DynamicFrame, IbtConnection, LayoutProvider};
use std::path::PathBuf;

use snapshot::{Command as SnapshotCommand, handle_command as handle_snapshot_command};

use crate::writer::{OutputTarget, RecordStreamFormat, RecordStreamWriter};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Converts an IBT to CSV or JSONL
    Convert {
        /// The path of the IBT
        #[arg(short, long)]
        path: PathBuf,

        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the output.
        #[arg(long, default_value = "csv", value_enum)]
        format: RecordStreamFormat,
    },
    /// Records a live connection telemetry stream to the output as CSV or JSONL
    #[cfg(windows)]
    Record {
        /// Output destination. Use `-` for stdout.
        #[arg(short, long, default_value = "-")]
        output: OutputTarget,

        /// The encoding for the output.
        #[arg(long, default_value = "csv", value_enum)]
        format: RecordStreamFormat,
    },
    /// Snapshots a telemetry source
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommand,
    },
}

pub(crate) async fn handle_command(command: Command) -> Result<()> {
    match command {
        Command::Convert {
            path,
            output,
            format,
        } => {
            tracing::info!(path = %path.display(), "Opening IBT file");
            let connection = IbtConnection::builder()
                .with_path(&path)
                .build()
                .await
                .context("Failed to open IBT telemetry file")?;
            let mut variables = connection.fields_owned();
            variables.sort_unstable_by(|left, right| {
                left.region()
                    .offset()
                    .cmp(&right.region().offset())
                    .then_with(|| left.name().cmp(right.name()))
            });
            let mut frames = Box::pin(connection.subscribe::<DynamicFrame>()?);
            let mut writer =
                RecordStreamWriter::from_variables(output, format, variables)?.prepare()?;
            connection.start()?;

            let mut frame_count = 0usize;
            while let Some(frame) = frames.next().await {
                writer.write(&frame)?;
                frame_count += 1;
                if frame_count.is_multiple_of(10_000) {
                    tracing::debug!(frames_exported = frame_count, "IBT export progress");
                }
            }
            writer.finalize()?;
            tracing::info!(frames_exported = frame_count, "Finished IBT export");
            Ok(())
        }
        #[cfg(windows)]
        Command::Record { output, format } => {
            use iracing_sdk::{LiveConnection, UpdateRate, providers::live::LiveProvider};

            let provider = LiveProvider::builder()
                .with_connection(get_connection()?)
                .build()?;
            let connection = LiveConnection::builder().with_provider(provider).build()?;
            let mut variables = connection.fields_owned();
            if variables.is_empty() {
                bail!("No telemetry variables were available from the live connection");
            }
            variables.sort_unstable_by(|left, right| {
                left.region()
                    .offset()
                    .cmp(&right.region().offset())
                    .then_with(|| left.name().cmp(right.name()))
            });
            let mut frames = Box::pin(connection.subscribe::<DynamicFrame>(UpdateRate::Native)?);
            let mut writer =
                RecordStreamWriter::from_variables(output, format, variables)?.prepare()?;

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
                    frame = frames.next() => {
                        let Some(frame) = frame else { break };
                        writer.write(&frame)?;
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
        Command::Snapshot { command } => handle_snapshot_command(command).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_sdk::test_utils::require_named_ibt_fixture;
    use std::time::Duration;

    #[tokio::test]
    async fn convert_exports_every_frame_in_order() -> Result<()> {
        let path = require_named_ibt_fixture("profile_small.ibt")?;
        let directory = tempfile::tempdir()?;
        for format in [RecordStreamFormat::Csv, RecordStreamFormat::Jsonl] {
            let output = directory.path().join("export");
            tokio::time::timeout(
                Duration::from_secs(10),
                handle_command(Command::Convert {
                    path: path.clone(),
                    output: OutputTarget::File(output.clone()),
                    format,
                }),
            )
            .await??;
            let text = std::fs::read_to_string(output)?;
            assert!(text.ends_with('\n'));
            match format {
                RecordStreamFormat::Csv => {
                    let mut reader = csv::Reader::from_reader(text.as_bytes());
                    assert_eq!(
                        reader.headers()?.iter().collect::<Vec<_>>(),
                        [
                            "SessionTime",
                            "Speed",
                            "LapDist",
                            "LapCompleted",
                            "Brake",
                            "Throttle",
                            "RPM",
                            "Gear"
                        ]
                    );
                    let rows = reader
                        .records()
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    assert_eq!(rows.len(), 12);
                    for (index, row) in rows.iter().enumerate() {
                        assert_eq!(row[0].parse::<f64>()?, index as f64 / 60.0);
                        assert_eq!(row[1].parse::<f64>()?, 35.0 + index as f64 * 0.25);
                    }
                }
                RecordStreamFormat::Jsonl => {
                    let rows = text
                        .lines()
                        .map(serde_json::from_str::<serde_json::Value>)
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    assert_eq!(rows.len(), 12);
                    for (index, row) in rows.iter().enumerate() {
                        assert_eq!(row.as_object().unwrap().len(), 8);
                        assert!(
                            (row["SessionTime"].as_f64().unwrap() - index as f64 / 60.0).abs()
                                < 1e-12
                        );
                        assert_eq!(row["Speed"], 35.0 + index as f64 * 0.25);
                    }
                    assert!(text.starts_with("{\"SessionTime\":"));
                }
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn convert_propagates_input_and_output_errors() -> Result<()> {
        let path = require_named_ibt_fixture("profile_small.ibt")?;
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("existing");
        std::fs::write(&output, "keep this")?;
        let error = handle_command(Command::Convert {
            path: directory.path().join("missing.ibt"),
            output: OutputTarget::File(output.clone()),
            format: RecordStreamFormat::Jsonl,
        })
        .await
        .unwrap_err();
        assert!(error.to_string().contains("Failed to open IBT"));
        assert_eq!(std::fs::read_to_string(output)?, "keep this");
        let error = handle_command(Command::Convert {
            path,
            output: OutputTarget::File(directory.path().join("missing").join("output")),
            format: RecordStreamFormat::Csv,
        })
        .await
        .unwrap_err();
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        Ok(())
    }
}
