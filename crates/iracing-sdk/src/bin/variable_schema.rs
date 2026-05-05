//! Static variable schema generator.
//!
//! Generates the baseline JSON Schema for `iracing_sdk::VariableInfo` using the
//! compile-time Rust type definition (not live/discovered runtime data).
//!
//! This is useful as a stable reference schema that can be compared against
//! runtime-derived variable schemas from:
//! - `disk-variable-schema`
//! - `live-variable-schema`
//!
//! # Output format
//! The schema is serialized as YAML.
//!
//! # Usage
//! ```text
//! variable-schema --output-path <PATH>
//! ```

use std::{fs::File, io::BufWriter, path::PathBuf};

use anyhow::Result;
use clap::Parser;

/// CLI arguments for the disk session parser.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the generated schema YAML should be written.
    #[arg(short, long, default_value = "variable-schema.yml")]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments.
    // ------------------------------------------------------------
    let Args { output_path } = Args::parse();

    let schema = schemars::schema_for!(iracing_sdk::VariableInfo);

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    tracing::info!(path=%output_path.display(),"Wrote static variable schema");

    Ok(())
}
