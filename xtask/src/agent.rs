use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde::Serialize;

/// Opt-in agent corpus workflows; model responses are collected separately.
#[derive(Debug, Subcommand, Serialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum AgentTask {
    /// Verify corpus and frozen package integrity.
    Check,
    /// Freeze the current guidance as a content-addressed package.
    Freeze,
    /// Prepare isolated case inputs and a manual adjudication template.
    Prepare {
        #[arg(long)]
        run: PathBuf,
        #[arg(long)]
        model: String,
        #[arg(long)]
        tools: String,
        #[arg(long)]
        condition: String,
    },
    /// Score manually adjudicated responses.
    Score {
        #[arg(long)]
        run: PathBuf,
    },
    /// Compare two adjudicated runs with the same model and tools.
    Compare {
        #[arg(long)]
        left: PathBuf,
        #[arg(long)]
        right: PathBuf,
    },
}

pub fn run(task: AgentTask) -> Result<()> {
    // clap owns the public CLI. Python receives only a structured internal request.
    let request = serde_json::to_vec(&task)?;
    let interpreter = std::env::var_os("PYTHON").unwrap_or_else(|| "python3".into());
    let mut child = Command::new(interpreter)
        .arg(super::root().join("evals/agent/run.py"))
        .stdin(Stdio::piped())
        .spawn()
        .context("start agent corpus backend (requires Python 3.10+; override with PYTHON)")?;
    let write_result = child
        .stdin
        .take()
        .context("open backend input")?
        .write_all(&request);
    let status = child.wait().context("wait for agent corpus backend")?;
    write_result.context("send agent corpus request")?;
    if !status.success() {
        bail!("agent corpus backend failed: {status}");
    }
    Ok(())
}
