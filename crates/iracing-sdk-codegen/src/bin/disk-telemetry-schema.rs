use anyhow::Result;
use clap::Parser;
use iracing_sdk::IbtReader;
use schemars::schema::RootSchema;
use std::{fs::File, io::BufWriter, path::PathBuf};
use tracing::info;
use tracing_subscriber::EnvFilter;

/// CLI arguments for the `disk-position` extractor.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the output YML should be written.
    #[arg(short, long)]
    output_path: PathBuf,
}

pub fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments
    // ------------------------------------------------------------
    let Args {
        ibt_path,
        output_path,
    } = Args::parse();

    info!(path = %ibt_path.display(), "Opening IBT file");

    // ------------------------------------------------------------
    // Open telemetry reader
    // ------------------------------------------------------------
    let reader = IbtReader::open(&ibt_path).expect("Failed to open IBT file");

    let variable_schema = reader.variables().clone();
    let schema: RootSchema = variable_schema.into();

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);

    serde_yaml_ng::to_writer(writer, &schema)?;

    info!(path=%output_path.display(),"Wrote disk telemetry schema");

    Ok(())
}
