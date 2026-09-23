//! Read one frame through the low-level, random-access IBT API.
//!
//! This example intentionally stops below the provider and connection layers so
//! the relationship between the file layout, variable schema, and frame bytes
//! stays visible.
//!
//! ```text
//! cargo run -p iracing-sdk --example ibt-read-frame -- \
//!   --ibt-path ./session.ibt --frame 0
//! ```

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use iracing_sdk::{VarData, VariableSchema, ibt::IbtReader};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Zero-based frame index to inspect.
    #[arg(short, long, default_value_t = 0)]
    frame: usize,
}

fn main() -> Result<()> {
    let Args { ibt_path, frame } = Args::parse();
    let mut reader = IbtReader::open(&ibt_path)
        .with_context(|| format!("failed to open {}", ibt_path.display()))?;

    let frame_count = reader.layout().frame_count();
    if frame >= frame_count {
        return Err(anyhow!(
            "frame {frame} is out of range; the recording contains {frame_count} frames"
        ));
    }

    let frame_size = reader.layout().frame_size();
    let headers = reader
        .variable_headers_snapshot()?
        .context("recording contains no variable headers")?;
    let schema = VariableSchema::from_snapshot(headers, frame_size)?;
    let data = reader.frame(frame)?;

    let speed = schema
        .get_variable("Speed")
        .context("recording does not contain `Speed`")?;
    let gear = schema
        .get_variable("Gear")
        .context("recording does not contain `Gear`")?;

    println!(
        "frame={frame} speed_mps={} gear={}",
        f32::from_bytes(&data, speed)?,
        i32::from_bytes(&data, gear)?,
    );

    Ok(())
}
