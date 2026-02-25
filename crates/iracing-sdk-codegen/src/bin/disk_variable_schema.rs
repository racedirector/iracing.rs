//! Disk telemetry schema generator.
//!
//! Opens an iRacing `.ibt` file and emits telemetry variable JSON Schema
//! (serialized as YAML) based on the embedded variable headers.
//!
//! # Behavior
//! - Opens `--ibt-path` via `iracing_sdk::IbtReader`
//! - Reads the telemetry variable schema from the file
//! - Converts it to JSON Schema and writes YAML to `--output-path`
//!
//! # Usage
//! ```text
//! disk-variable-schema --ibt-path <FILE.ibt> --output-path <SCHEMA.yml>
//! ```

use anyhow::Result;
use clap::Parser;
use iracing_sdk::IbtReader;
use iracing_sdk_codegen::primitive_annotations::annotate_variable_schema;
use schemars::schema_for_value;
use std::{fs::File, io::BufWriter, path::PathBuf};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// CLI arguments for the disk telemetry schema generator.
///
/// Uses `clap` derive API for parsing.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the output schema YAML should be written.
    #[arg(short, long, default_value = "disk-variable-schema.yml")]
    output_path: PathBuf,

    /// Annotate `irsdk_*` units with primitive enum/bitflag refs and inject used defs.
    #[arg(long)]
    annotate: bool,
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
        annotate,
    } = Args::parse();

    info!(path = %ibt_path.display(), "Opening IBT file");

    // ------------------------------------------------------------
    // Open telemetry reader
    // ------------------------------------------------------------
    let reader = IbtReader::open(&ibt_path).expect("Failed to open IBT file");

    let variable_schema = reader.variables().clone();
    let mut schema = schema_for_value!(variable_schema);

    if annotate {
        let report = annotate_variable_schema(&mut schema)?;
        info!(
            annotated_variables = report.annotated_variables,
            injected_defs = report.injected_defs,
            "Annotated telemetry schema with primitive references"
        );

        for unit in report.unknown_units {
            warn!(unit = %unit, "Unknown irsdk_* units token; skipping annotation");
        }
    }

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);

    serde_yaml_ng::to_writer(writer, &schema)?;

    info!(path=%output_path.display(),"Wrote disk telemetry schema");

    Ok(())
}
