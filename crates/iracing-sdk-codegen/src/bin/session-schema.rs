use std::{fs::File, io::BufWriter, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use schemars::schema_for;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// CLI arguments for the disk session parser.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the session YAML should be written.
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
    let Args { output_path } = Args::parse();

    let schema = schema_for!(iracing_sdk::SessionInfo);

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    info!(path=%output_path.display(),"Wrote schema");

    Ok(())
}
