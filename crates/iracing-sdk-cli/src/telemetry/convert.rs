use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use iracing_sdk::LayoutProvider;

use crate::{
    utils::DiskTelemetry,
    writer::{OutputTarget, RecordStreamFormat, RecordStreamWriter},
};

/// Converts an IBT to the given format at output.
#[derive(clap::Args, Debug)]
pub(crate) struct Args {
    /// The path of the IBT
    #[arg(short, long)]
    path: PathBuf,

    /// First frame to export (inclusive).
    #[arg(long = "start", default_value = "0", global = true)]
    start_index: usize,

    /// Stop before this frame; defaults to the end of the recording.
    #[arg(long = "end", global = true)]
    end_index: Option<usize>,

    /// Output destination. Use `-` for stdout.
    #[arg(short, long, default_value = "-", global = true)]
    output: OutputTarget,

    /// The encoding for the output.
    #[arg(long, default_value = "csv", global = true, value_enum)]
    format: RecordStreamFormat,
}

impl Args {
    pub(crate) fn run(&self) -> Result<()> {
        tracing::info!(path = %self.path.display(), "Opening IBT file");
        let telemetry =
            DiskTelemetry::open(&self.path).context("Failed to open IBT telemetry file")?;
        let frame_count = telemetry.reader.frame_count();
        let end_index = self.end_index.unwrap_or(frame_count);
        ensure!(
            self.start_index <= end_index && end_index <= frame_count,
            "Invalid frame range {}..{} for an IBT containing {} frames",
            self.start_index,
            end_index,
            frame_count
        );

        let variables = telemetry.fields_owned();
        let mut writer =
            RecordStreamWriter::from_variables(self.output.clone(), self.format, variables)?
                .prepare()?;

        let mut exported = 0usize;
        for index in self.start_index..end_index {
            let packet = telemetry.frame_at(index)?;
            writer.write(&packet)?;
            exported += 1;
            if exported.is_multiple_of(10_000) {
                tracing::debug!(frames_exported = exported, "IBT export progress");
            }
        }

        writer.finalize()?;
        tracing::info!(frames_exported = exported, "Finished IBT export");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_sdk::test_utils::require_named_ibt_fixture;

    #[test]
    fn convert_exports_every_frame_in_order() -> Result<()> {
        let path = require_named_ibt_fixture("profile_small.ibt")?;
        let directory = tempfile::tempdir()?;
        for format in [RecordStreamFormat::Csv, RecordStreamFormat::Jsonl] {
            let output = directory.path().join("export");
            Args {
                path: path.clone(),
                start_index: 0,
                end_index: None,
                output: OutputTarget::File(output.clone()),
                format,
            }
            .run()?;
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

    #[test]
    fn convert_propagates_input_and_output_errors() -> Result<()> {
        let path = require_named_ibt_fixture("profile_small.ibt")?;
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("existing");
        std::fs::write(&output, "keep this")?;
        let error = Args {
            start_index: 0,
            end_index: None,
            path: directory.path().join("missing.ibt"),
            output: OutputTarget::File(output.clone()),
            format: RecordStreamFormat::Jsonl,
        }
        .run()
        .unwrap_err();
        assert!(error.to_string().contains("Failed to open IBT"));
        assert_eq!(std::fs::read_to_string(output)?, "keep this");
        let error = Args {
            start_index: 0,
            end_index: None,
            path,
            output: OutputTarget::File(directory.path().join("missing").join("output")),
            format: RecordStreamFormat::Csv,
        }
        .run()
        .unwrap_err();
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        Ok(())
    }

    #[test]
    fn convert_exports_selected_and_empty_ranges() -> Result<()> {
        let path = require_named_ibt_fixture("profile_small.ibt")?;
        let directory = tempfile::tempdir()?;
        for (start, end, count) in [(3, Some(6), 3), (10, None, 2), (12, None, 0)] {
            for format in [RecordStreamFormat::Csv, RecordStreamFormat::Jsonl] {
                let output = directory.path().join("range");
                Args {
                    path: path.clone(),
                    start_index: start,
                    end_index: end,
                    output: OutputTarget::File(output.clone()),
                    format,
                }
                .run()?;
                let text = std::fs::read_to_string(output)?;
                let speeds: Vec<f64> = match format {
                    RecordStreamFormat::Csv => csv::Reader::from_reader(text.as_bytes())
                        .records()
                        .map(|row| Ok(row?[1].parse()?))
                        .collect::<Result<_>>()?,
                    RecordStreamFormat::Jsonl => text
                        .lines()
                        .map(|line| {
                            let value: serde_json::Value = serde_json::from_str(line)?;
                            Ok(value["Speed"].as_f64().unwrap())
                        })
                        .collect::<Result<_>>()?,
                };
                assert_eq!(speeds.len(), count);
                for (offset, speed) in speeds.iter().enumerate() {
                    assert_eq!(*speed, 35.0 + (start + offset) as f64 * 0.25);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn convert_rejects_invalid_ranges_before_truncating_output() -> Result<()> {
        let path = require_named_ibt_fixture("profile_small.ibt")?;
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("existing");
        std::fs::write(&output, "keep this")?;
        for (start_index, end_index) in [(7, Some(3)), (0, Some(13)), (13, None)] {
            let error = Args {
                path: path.clone(),
                start_index,
                end_index,
                output: OutputTarget::File(output.clone()),
                format: RecordStreamFormat::Csv,
            }
            .run()
            .unwrap_err();
            assert!(error.to_string().contains("Invalid frame range"));
            assert_eq!(std::fs::read_to_string(&output)?, "keep this");
        }
        Ok(())
    }
}
