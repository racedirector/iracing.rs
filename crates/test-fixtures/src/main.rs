//! Command-line interface for deterministic workspace fixture maintenance.
//!
//! With no subcommand, the binary performs the full check workflow: generate,
//! verify, then check the generated directories for Git drift.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Top-level command-line arguments shared by every workflow.
#[derive(Debug, Parser)]
#[command(version, about = "Generate and verify deterministic IBT test fixtures")]
struct Args {
    /// Focused workflow; omission selects [`FixtureCommand::Check`].
    #[command(subcommand)]
    command: Option<FixtureCommand>,

    /// Skip the scoped Git drift check performed by the check workflow.
    #[arg(long, global = true)]
    no_drift_check: bool,

    /// Override the repository root (primarily useful for isolated testing).
    #[arg(long, global = true)]
    repo_root: Option<PathBuf>,
}

/// Fixture maintenance workflows exposed by the binary.
#[derive(Debug, Subcommand)]
enum FixtureCommand {
    /// Generate fixture files and their manifest.
    Generate,
    /// Verify existing fixture files against their manifest.
    Verify,
    /// Generate, verify, and check for Git drift.
    Check,
}

/// Parses CLI arguments, runs the requested workflow, and prints a short report.
///
/// # Errors
///
/// Returns workflow errors to the runtime so failures produce a nonzero exit
/// status and retain their contextual diagnostic chain.
fn main() -> Result<()> {
    let args = Args::parse();
    let root = args.repo_root.unwrap_or_else(test_fixtures::workspace_root);
    match args.command.unwrap_or(FixtureCommand::Check) {
        FixtureCommand::Generate => {
            let report = test_fixtures::generate(&root)?;
            println!(
                "generated {} fixtures across {} files",
                report.fixture_count, report.file_count
            );
        }
        FixtureCommand::Verify => {
            let report = test_fixtures::verify(&root)?;
            println!(
                "verified {} fixtures and {} frames",
                report.fixture_count, report.frame_count
            );
        }
        FixtureCommand::Check => {
            let report = test_fixtures::check(&root, !args.no_drift_check)?;
            println!(
                "generated and verified {} fixtures{}",
                report.verification.fixture_count,
                if report.drift_checked {
                    " with no Git drift"
                } else {
                    ""
                }
            );
        }
    }
    Ok(())
}
