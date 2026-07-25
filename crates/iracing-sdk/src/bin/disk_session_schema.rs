//! Disk session schema generator.
//!
//! Opens an iRacing `.ibt` file, reads session YAML, and generates session
//! JSON Schema (serialized as YAML).
//!
//! # Usage
//! ```text
//! disk-session-schema --ibt-path <FILE.ibt> --output-path <SCHEMA.yml>
//! ```

use clap::Parser;
use iracing_sdk::{provider::Provider, providers::ibt::IbtProvider, schema::SessionInfo};
use std::{fs::File, io::BufWriter, path::PathBuf};
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
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let Args {
        ibt_path,
        output_path,
    } = Args::parse();

    tracing::info!(path = %ibt_path.display(), "Opening IBT file");
    let mut provider = IbtProvider::open(&ibt_path)?;

    let session_yaml = provider
        .session_yaml(0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No session YAML found in IBT file"))?;

    let session = SessionInfo::parse(&session_yaml)?;
    let schema = schemars::schema_for_value!(session);

    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    serde_yaml_ng::to_writer(writer, &schema)?;

    tracing::info!(path = %output_path.display(), "Wrote disk session schema");

    Ok(())
}
