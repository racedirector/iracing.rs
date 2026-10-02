mod generated;
mod support;
mod workflows;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::{path::Path, process::Command};

/// Repository maintenance workflows.
#[derive(Debug, Parser)]
#[command(version, about, name = "cargo xtask")]
struct Args {
    #[command(subcommand)]
    command: Task,
}

#[derive(Debug, Subcommand)]
enum Task {
    /// Check repository configuration for drift.
    CheckRepo,
    /// Check deterministic generated references without writing them.
    CheckGenerated,
    /// Regenerate deterministic reference artifacts.
    GenerateReference,
    /// Run the shared sequential local quality gate.
    PrePush,
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn cargo(args: &[&str]) -> Result<()> {
    let status = Command::new("cargo")
        .args(args)
        .current_dir(root())
        .status()
        .with_context(|| format!("run cargo {}", args.join(" ")))?;
    if !status.success() {
        bail!("cargo {} failed: {status}", args.join(" "));
    }
    Ok(())
}

fn check_repo() -> Result<()> {
    workflows::check(root())?;
    support::check(root())?;
    generated::check(root())?;
    println!("Repository consistency checks passed");
    Ok(())
}

fn main() -> Result<()> {
    match Args::parse().command {
        Task::CheckRepo => check_repo(),
        Task::CheckGenerated => generated::check(root()),
        Task::GenerateReference => generated::generate(root()),
        Task::PrePush => {
            check_repo()?;
            for args in [
                vec!["test-fixtures"],
                vec!["fmt", "--all", "--", "--check"],
                vec![
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--keep-going",
                    "--",
                    "-D",
                    "warnings",
                ],
                vec!["test", "--workspace", "--all-targets"],
                vec![
                    "bench",
                    "-p",
                    "iracing-sdk",
                    "--features",
                    "benchmark",
                    "--no-run",
                ],
            ] {
                cargo(&args)?;
            }
            Ok(())
        }
    }
}
