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
    let features = workflow["jobs"]["features"]["steps"]
        .as_sequence()
        .context("missing feature verification")?;
    if !features.iter().any(|s| {
        s["uses"]
            .as_str()
            .is_some_and(|s| s.starts_with("actions/checkout@"))
            && s["with"]["lfs"].as_bool() == Some(true)
    }) {
        bail!("feature tests require hydrated recording fixtures (checkout lfs: true)");
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
    fn fixture() -> tempfile::TempDir {
        let temporary = tempfile::tempdir().unwrap();
        let root = super::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        super::fs::create_dir_all(temporary.path().join(".github/workflows")).unwrap();
        super::fs::copy(root.join("Cargo.toml"), temporary.path().join("Cargo.toml")).unwrap();
        super::fs::copy(
            root.join(".github/workflows/support.yml"),
            temporary.path().join(".github/workflows/support.yml"),
        )
        .unwrap();
        for entry in super::fs::read_dir(root.join("crates")).unwrap() {
            let path = entry.unwrap().path();
            let destination = temporary
                .path()
                .join("crates")
                .join(path.file_name().unwrap());
            super::fs::create_dir_all(&destination).unwrap();
            super::fs::copy(path.join("Cargo.toml"), destination.join("Cargo.toml")).unwrap();
        }
        temporary
    }

    #[test]
    fn detects_unverified_release_target_and_msrv_drift() {
        let temp = fixture();
        let manifest = temp.path().join("Cargo.toml");
        let original = super::fs::read_to_string(&manifest).unwrap();
        super::fs::write(
            &manifest,
            original.replace("targets = [", "targets = [\n  \"i686-unknown-linux-gnu\","),
        )
        .unwrap();
        assert!(
            super::check(temp.path())
                .unwrap_err()
                .to_string()
                .contains("release targets")
        );
        super::fs::write(
            &manifest,
            original.replace("rust-version = \"1.88\"", "rust-version = \"1.89\""),
        )
        .unwrap();
        assert!(
            super::check(temp.path())
                .unwrap_err()
                .to_string()
                .contains("MSRV CI compiler")
        );
    }

    #[test]
    fn detects_missing_fixture_hydration_and_compile_command() {
        let temp = fixture();
        let path = temp.path().join(".github/workflows/support.yml");
        let original = super::fs::read_to_string(&path).unwrap();
        super::fs::write(&path, original.replace("lfs: true", "lfs: false")).unwrap();
        assert!(
            super::check(temp.path())
                .unwrap_err()
                .to_string()
                .contains("hydrated")
        );
        super::fs::write(
            &path,
            original.replace(
                "cargo check --locked --workspace --all-features --target",
                "echo compile --target",
            ),
        )
        .unwrap();
        assert!(
            super::check(temp.path())
                .unwrap_err()
                .to_string()
                .contains("actually compile")
        );
    }

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
