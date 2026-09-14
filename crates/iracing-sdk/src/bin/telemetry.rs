use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use iracing_sdk::irsdk::{DiskSubHeader, Header, VariableBuffer, VariableHeader};
use std::{fs::File, io::BufReader, path::PathBuf};
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
use std::time::Duration;

#[derive(Parser)]
#[command(
  name="telemetry",
  version,
  about="iRacing telemetry utilities",
  long_about = None,
  arg_required_else_help = true
)]
struct Args {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Convert,
    Snapshot,
}

#[derive(Subcommand)]
enum ConvertCommands {
    Ibt {
        /// Path to the input `.ibt` telemetry file.
        #[arg(short, long)]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum SnapshotCommands {}

fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    match args.commands {
        Commands::Convert => {
            tracing::info!("Convert command!");
        }
        Commands::Snapshot => {
            tracing::info!("Snapshot command!")
        }
    }

    Ok(())
}
