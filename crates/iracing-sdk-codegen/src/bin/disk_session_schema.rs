//! Disk session schema generator.
//!
//! Opens an iRacing `.ibt` file, reads session YAML, and generates session
//! JSON Schema (serialized as YAML).
//!
//! # Discovery mode
//! `--discover` overlays unknown fields discovered in runtime session YAML onto
//! the emitted schema using `iracing_sdk::session_root_schema_with_discovery`.
//!
//! # Diff mode
//! `--diff <PATH>` compares generated schema vs baseline schema and logs a
//! path/type summary. Use `--diff-output-path <PATH>` to also write full diff YAML.
//!
//! # Usage
//! ```text
//! disk_session_schema --ibt-path <FILE.ibt> --output-path <SCHEMA.yml> [--discover]
//! disk_session_schema --ibt-path <FILE.ibt> --output-path <SCHEMA.yml> --diff <BASELINE.yml> [--diff-output-path <DIFF.yml>]
//! ```

use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser};
use iracing_sdk::{IbtReader, SessionInfo};
use iracing_sdk_codegen::schema_diff::{diff_schemas, summarize_diff};
use schemars::schema_for_value;
use std::{fs::File, io::BufWriter, path::PathBuf};
use tracing::info;
use tracing_subscriber::EnvFilter;

/// CLI arguments for the disk session schema generator.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the input `.ibt` telemetry file.
    #[arg(short, long)]
    ibt_path: PathBuf,

    /// Path where the output schema YAML should be written.
    #[arg(short, long, default_value = "disk-session-schema.yml")]
    output_path: PathBuf,

    /// Merge discovered session fields into the emitted schema.
    #[arg(long, action = ArgAction::SetTrue)]
    discover: bool,

    /// Compare generated schema against this baseline schema YAML file.
    #[arg(long)]
    diff: Option<PathBuf>,

    /// Optional path to write a detailed schema diff report YAML.
    #[arg(long)]
    diff_output_path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args {
        ibt_path,
        output_path,
        discover: _,
        diff,
        diff_output_path,
    } = Args::parse();

    if diff.is_none() && diff_output_path.is_some() {
        return Err(anyhow!("--diff-output-path requires --diff"));
    }

    info!(path = %ibt_path.display(), "Opening IBT file");
    let reader = IbtReader::open(&ibt_path)?;

    let session_yaml = reader
        .session_yaml()?
        .ok_or_else(|| anyhow!("No session YAML found in IBT file"))?;

    let session = SessionInfo::parse(&session_yaml)?;
    let schema = schema_for_value!(session);

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    info!(path = %output_path.display(), "Wrote disk session schema");

    if let Some(diff_path) = diff {
        let file = File::open(diff_path)?;
        let baseline = serde_yaml_ng::from_reader(file)?;
        let report = diff_schemas(&schema, &baseline);
        info!("{}", summarize_diff(&report));

        if let Some(report_path) = diff_output_path {
            let file = File::create(&report_path)?;
            let writer = BufWriter::new(file);
            serde_yaml_ng::to_writer(writer, &report)?;
            info!(path = %report_path.display(), "Wrote session schema diff report");
        }
    }

    Ok(())
}
