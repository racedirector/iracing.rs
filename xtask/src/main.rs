mod workflows;

use anyhow::{Context, Result, bail};
use std::{path::Path, process::Command};

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
    println!("Repository consistency checks passed");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command] if command == "check-repo" => check_repo(),
        [command] if command == "pre-push" => {
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
        _ => bail!("usage: cargo xtask <check-repo|pre-push>"),
    }
}
