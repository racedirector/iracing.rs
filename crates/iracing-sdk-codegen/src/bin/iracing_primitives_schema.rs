//! iRacing primitive schema generator.
//!
//! Emits a JSON Schema (YAML-serialized) describing the exported `irsdk_*` primitive wrappers from
//! `iracing_sdk::types` (enum and bitflag families).

use std::{fs::File, io::BufWriter, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use iracing_sdk_codegen::primitive_annotations::build_primitive_schema;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the generated schema YAML should be written.
    #[arg(short, long, default_value = "iracing-primitives-schema.yml")]
    output_path: PathBuf,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args { output_path } = Args::parse();
    let schema = build_primitive_schema()?;

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    info!(
        path = %output_path.display(),
        "Wrote iRacing primitive schema"
    );

    Ok(())
}
