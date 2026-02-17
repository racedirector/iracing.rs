use anyhow::{Result, anyhow};
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    csv_output_path: PathBuf,
}

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

#[cfg(windows)]
fn run() -> Result<()> {
    let Args { csv_output_path: _ } = Args::parse();
    tracing::warn!("live-position example is not implemented yet.");
    Err(anyhow!("live-position example is not implemented yet"))
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    tracing::warn!(
        "live-position example is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
    );
    Err(anyhow!(
        "live-position example is only supported on Windows"
    ))
}
