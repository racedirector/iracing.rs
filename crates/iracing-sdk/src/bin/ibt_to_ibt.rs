use anyhow::Result;
use clap::Parser;
use iracing_sdk::{FrameProjection, IbtReader, IbtWriteOptions, IbtWriter};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path to the output `.ibt` telemetry file.
    #[arg(short, long)]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments.
    // ------------------------------------------------------------
    let Args {
        ibt_path,
        output_path,
    } = Args::parse();

    // ------------------------------------------------------------
    // Open telemetry reader.
    // ------------------------------------------------------------
    info!(path = %ibt_path.display(), "Opening IBT file");
    let mut reader = IbtReader::open(&ibt_path)?;

    // ------------------------------------------------------------
    // Create a projection of the variables you want from the
    // source IBT
    // ------------------------------------------------------------
    let projection = FrameProjection::from_variable_names(
        reader.variables(),
        ["SessionTime", "Speed", "RPM", "OnPitRoad"],
    )?;

    let options = IbtWriteOptions::from_reader(&reader)?;
    let mut writer = IbtWriter::create(&output_path, projection.target_schema().clone(), options)?;

    while let Some((frame, _, _)) = reader.read_next_frame()? {
        writer.write_projected_frame(&frame, &projection)?;
    }

    writer.finish()?;
    Ok(())
}
