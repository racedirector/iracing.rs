mod agent;
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
    /// Prepare and adjudicate the opt-in agent review corpus.
    Agent {
        #[command(subcommand)]
        command: agent::AgentTask,
    },
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
        Task::Agent { command } => agent::run(command),
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

#[cfg(test)]
mod cli_tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn clap_rejects_missing_or_unknown_commands_and_arguments() {
        for args in [
            vec!["xtask"],
            vec!["xtask", "unknown"],
            vec!["xtask", "check-repo", "extra"],
            vec!["xtask", "agent"],
            vec!["xtask", "agent", "prepare", "--run", "output"],
            vec!["xtask", "agent", "score"],
            vec!["xtask", "agent", "compare", "--left", "baseline"],
        ] {
            assert!(Args::try_parse_from(args).is_err());
        }
        assert_eq!(
            Args::try_parse_from(["xtask", "agent", "--help"])
                .unwrap_err()
                .kind(),
            ErrorKind::DisplayHelp
        );
    }

    #[test]
    fn clap_passes_agent_paths_and_values_as_structured_data() {
        let args = Args::try_parse_from([
            "xtask",
            "agent",
            "prepare",
            "--run",
            "runs/path with spaces",
            "--model",
            "model",
            "--tools",
            "tools; literal",
            "--condition",
            "baseline",
        ])
        .unwrap();
        let Task::Agent { command } = args.command else {
            panic!("expected agent task")
        };
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            serde_json::json!({
                "command": "prepare", "run": "runs/path with spaces", "model": "model",
                "tools": "tools; literal", "condition": "baseline",
            })
        );
    }
}
