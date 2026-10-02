use anyhow::{Context, Result, bail};
use serde_yaml_ng::Value;
use std::{collections::BTreeSet, fs, path::Path};

pub fn check(root: &Path) -> Result<()> {
    let manifest: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
    let msrv = manifest["workspace"]["package"]["rust-version"]
        .as_str()
        .context("missing workspace MSRV")?;
    for entry in fs::read_dir(root.join("crates"))? {
        let path = entry?.path().join("Cargo.toml");
        if !path.exists() {
            continue;
        }
        let package: toml::Value = toml::from_str(&fs::read_to_string(&path)?)?;
        if package["package"]["rust-version"]["workspace"].as_bool() != Some(true) {
            bail!("{} must inherit workspace MSRV", path.display());
        }
    }
    let workflow: Value = serde_yaml_ng::from_str(&fs::read_to_string(
        root.join(".github/workflows/support.yml"),
    )?)?;
    let compiler = workflow["jobs"]["msrv"]["steps"]
        .as_sequence()
        .context("missing MSRV steps")?
        .iter()
        .find_map(|step| step["with"]["toolchain"].as_str())
        .context("missing MSRV compiler")?;
    if compiler != format!("{msrv}.0") {
        bail!("MSRV CI compiler {compiler} differs from declared {msrv}");
    }
    let targets = manifest["workspace"]["metadata"]["dist"]["targets"]
        .as_array()
        .context("missing release targets")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .context("invalid release target")
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let cells = workflow["jobs"]["release-targets"]["strategy"]["matrix"]["include"]
        .as_sequence()
        .context("missing release verification policy")?;
    let checked = cells
        .iter()
        .map(|v| {
            v["os"].as_str().context("release cell requires runner")?;
            Ok(v["target"]
                .as_str()
                .context("release cell requires target")?
                .to_owned())
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if targets != checked {
        bail!("release targets {targets:?} differ from CI verification {checked:?}");
    }
    let steps = workflow["jobs"]["release-targets"]["steps"]
        .as_sequence()
        .context("missing release check")?;
    if !steps.iter().any(|s| {
        s["run"].as_str().is_some_and(|s| {
            s.contains(
                "cargo check --locked --workspace --all-features --target ${{ matrix.target }}",
            )
        })
    }) {
        bail!("release matrix must actually compile-check each target");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_contract() {
        super::check(
            super::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        )
        .unwrap();
    }
}
