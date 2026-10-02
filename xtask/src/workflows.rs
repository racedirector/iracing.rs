use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSetBuilder};
use serde_yaml_ng::Value;
use std::{fs, path::Path};

// Cover real production inputs, including changes below newly introduced modules.
const POLICIES: &[(&str, &[&str])] = &[
    (
        "benchmark-ibt.yml",
        &[
            "crates/iracing-irsdk",
            "crates/iracing-sdk/src",
            "crates/iracing-sdk/benches/ibt_reader_performance.rs",
            "crates/iracing-sdk/benches/support/ibt.rs",
        ],
    ),
    (
        "benchmark-delivery.yml",
        &[
            "crates/iracing-sdk/src/telemetry",
            "crates/iracing-sdk/src/adapters",
            "crates/iracing-sdk/src/types",
        ],
    ),
    (
        "benchmarks-codspeed.yml",
        &[
            "crates/iracing-irsdk",
            "crates/iracing-sdk/src/adapters",
            "crates/iracing-sdk/src/types",
            "crates/iracing-sdk/src/schema",
        ],
    ),
];

fn files(root: &Path, path: &Path, output: &mut Vec<String>) -> Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            files(root, &entry?.path(), output)?;
        }
    } else {
        output.push(
            path.strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/"),
        );
    }
    Ok(())
}

fn validate(root: &Path, patterns: &[String], required: &[&str]) -> Result<()> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        // Negated filters need ordered inclusion semantics; reject rather than silently mischeck.
        if pattern.starts_with('!') {
            bail!("negative benchmark filter unsupported: {pattern}");
        }
        let prefix = pattern
            .split(['*', '?', '[', '{'])
            .next()
            .unwrap()
            .trim_end_matches('/');
        if !prefix.is_empty() && !root.join(prefix).exists() {
            bail!("stale workflow path: {pattern}");
        }
        builder.add(Glob::new(pattern)?);
    }
    let matcher = builder.build()?;
    for path in required {
        let absolute = root.join(path);
        if !absolute.exists() {
            bail!("required benchmark input disappeared: {path}; review policy against callers");
        }
        let mut inputs = Vec::new();
        files(root, &absolute, &mut inputs)?;
        for input in inputs {
            if !matcher.is_match(&input) {
                bail!("benchmark filter does not cover {input}");
            }
        }
    }
    Ok(())
}

pub fn check(root: &Path) -> Result<()> {
    for (workflow, required) in POLICIES {
        let path = root.join(".github/workflows").join(workflow);
        let yaml: Value = serde_yaml_ng::from_str(&fs::read_to_string(&path)?)?;
        if yaml["on"]["pull_request"].is_null() {
            bail!("{workflow}: missing pull_request benchmark trigger");
        }
        for event in ["pull_request", "push"] {
            if yaml["on"][event].is_null() {
                continue;
            }
            let patterns: Vec<String> = yaml["on"][event]["paths"]
                .as_sequence()
                .context("benchmark event must declare paths")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .context("path must be a string")
                })
                .collect::<Result<_>>()?;
            validate(root, &patterns, required).with_context(|| format!("{workflow}: {event}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_removed_pr_trigger() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join(".github/workflows");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("benchmark-ibt.yml"), "on: {}\n").unwrap();
        assert!(
            check(temporary.path())
                .unwrap_err()
                .to_string()
                .contains("missing pull_request")
        );
    }

    #[test]
    fn current_policy() {
        check(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    }
    #[test]
    fn detects_missing_coverage_and_stale_prefix() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(validate(root, &["src/**".into()], &["Cargo.toml"]).is_err());
        assert!(validate(root, &["removed/**".into()], &[]).is_err());
        validate(
            root,
            &["src/**".into(), "Cargo.toml".into()],
            &["src", "Cargo.toml"],
        )
        .unwrap();
    }
}
