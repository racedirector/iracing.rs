//! Capture one IBT telemetry frame as JSONL.
//!
//! Opens an iRacing `.ibt` recording, selects one zero-based frame number, and
//! writes it as a compact JSON object followed by a newline.
//!
//! # Usage
//!
//! ```text
//! ibt-json-snapshot --ibt-path <INPUT.ibt> --output-path <OUTPUT.jsonl> [--frame-number <INDEX>]
//! ```

mod json_telemetry_writer;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use iracing_sdk::{
    DynamicFrame, FrameAdapter, SchemaProvider, VariableInfo, provider::Provider,
    providers::ibt::IbtProvider,
};
use json_telemetry_writer::{DynamicFrameSnapshot, JsonTelemetryWriter};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the telemetry JSONL snapshot should be written.
    #[arg(short, long)]
    output_path: PathBuf,

    /// Zero-based frame number to capture. Defaults to the first frame.
    #[arg(short = 'f', long)]
    frame_number: Option<usize>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args {
        ibt_path,
        output_path,
        frame_number,
    } = Args::parse();
    let frame_number = frame_number.unwrap_or(0);

    tracing::info!(path = %ibt_path.display(), frame_number, "Opening IBT frame");
    let mut provider = IbtProvider::open(&ibt_path).context("Failed to open IBT telemetry file")?;
    seek_to_frame(&mut provider, frame_number)?;

    let mut variables: Vec<VariableInfo> = provider.variables();
    variables.sort_unstable_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.name.cmp(&right.name))
    });

    let validation = DynamicFrame::validate_schema(provider.schema())?;
    let packet = provider
        .next_frame()
        .await?
        .ok_or_else(|| anyhow!("IBT frame {frame_number} could not be read"))?;
    let frame = DynamicFrame::adapt(&packet, &validation);
    let snapshot = DynamicFrameSnapshot::from_frame(&frame, &variables)?;

    let mut writer = JsonTelemetryWriter::from_path(&output_path)?;
    writer.write_record(&snapshot)?;
    writer.flush()?;

    tracing::info!(
        path = %output_path.display(),
        frame_number,
        tick_count = frame.tick_count(),
        variable_count = variables.len(),
        "Wrote IBT telemetry JSON snapshot"
    );
    Ok(())
}

fn seek_to_frame(provider: &mut IbtProvider, frame_number: usize) -> Result<()> {
    let total_frames = provider.total_frames();
    validate_frame_number(total_frames, frame_number)?;

    provider
        .seek_to_frame(frame_number)
        .with_context(|| format!("Failed to seek to IBT frame {frame_number}"))
}

fn validate_frame_number(total_frames: usize, frame_number: usize) -> Result<()> {
    if total_frames == 0 {
        return Err(anyhow!("IBT file contains no telemetry frames"));
    }
    if frame_number >= total_frames {
        return Err(anyhow!(
            "Frame number {frame_number} is out of range; the IBT file contains {total_frames} frames (valid range: 0..={})",
            total_frames - 1
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_frame_number_defaults_to_the_first_frame() {
        let args = Args::try_parse_from([
            "ibt-json-snapshot",
            "--ibt-path",
            "input.ibt",
            "--output-path",
            "output.jsonl",
        ])
        .expect("arguments should parse");

        assert_eq!(args.frame_number.unwrap_or(0), 0);
    }

    #[test]
    fn frame_number_validation_accepts_valid_indices_and_rejects_invalid_ones() -> Result<()> {
        validate_frame_number(3, 0)?;
        validate_frame_number(3, 2)?;

        let error = validate_frame_number(3, 3).expect_err("the end index should be out of range");
        assert!(error.to_string().contains("out of range"));

        let error = validate_frame_number(0, 0).expect_err("an empty file should be rejected");
        assert!(error.to_string().contains("no telemetry frames"));
        Ok(())
    }
}
