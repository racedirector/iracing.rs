//! Live session schema generator (Windows).
//!
//! Connects to iRacing shared memory, reads live session YAML, and generates
//! session JSON Schema (serialized as YAML).
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
//! live_session_schema --output-path <SCHEMA.yml> [--allow-stale] [--discover]
//! live_session_schema --output-path <SCHEMA.yml> --diff <BASELINE.yml> [--diff-output-path <DIFF.yml>]
//! ```

use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser};
#[cfg(windows)]
use iracing_sdk::{SessionInfoParser, WindowsConnection};
use iracing_sdk_codegen::schema_diff::{diff_schemas, summarize_diff};
use std::path::PathBuf;
#[cfg(windows)]
use std::{fs::File, io::BufWriter};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path where the output schema YAML should be written.
    #[arg(short, long, default_value = "live-session-schema.yml")]
    output_path: PathBuf,

    /// Allow schema generation even if iRacing is disconnected (may be stale).
    #[arg(long, action = ArgAction::SetTrue)]
    allow_stale: bool,

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

    run()
}

#[cfg(windows)]
fn run() -> Result<()> {
    use schemars::schema_for_value;

    let Args {
        output_path,
        allow_stale,
        discover: _,
        diff,
        diff_output_path,
    } = Args::parse();

    if diff.is_none() && diff_output_path.is_some() {
        return Err(anyhow!("--diff-output-path requires --diff"));
    }

    info!("Opening iRacing connection");
    let connection = WindowsConnection::try_connect()?;

    if !connection.is_connected() && !allow_stale {
        return Err(anyhow!(
            "iRacing is not connected (pass --allow-stale to continue)."
        ));
    }

    let raw_session_yaml = connection
        .session_info()
        .ok_or_else(|| anyhow!("No live session YAML is available"))?;

    let parser = SessionInfoParser::new();
    let session = parser.parse(raw_session_yaml)?;
    let schema = schema_for_value!(session);

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    info!(path = %output_path.display(), "Wrote live session schema");

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

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live_session_schema is only supported on Windows because it depends on iRacing shared memory APIs."
    );
    Err(anyhow!("live_session_schema is only supported on Windows"))
}
