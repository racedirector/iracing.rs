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
        if *workflow == "benchmarks-codspeed.yml" {
            validate_codspeed(&yaml)?;
        }
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

fn exact_version(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn validate_codspeed(yaml: &Value) -> Result<()> {
    let job = &yaml["jobs"]["codspeed"];
    let steps = job["steps"]
        .as_sequence()
        .context("CodSpeed steps missing")?;
    let toolchain = steps
        .iter()
        .find(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("dtolnay/rust-toolchain@"))
        })
        .context("CodSpeed Rust installation missing")?;
    if !toolchain["with"]["toolchain"]
        .as_str()
        .is_some_and(exact_version)
    {
        bail!("CodSpeed benchmark compiler must be an exact major.minor.patch version");
    }
    let runner = steps
        .iter()
        .find(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("CodSpeedHQ/action@"))
        })
        .context("CodSpeed action missing")?;
    let version = runner["with"]["runner-version"]
        .as_str()
        .context("CodSpeed runner pin missing")?;
    if !exact_version(version) {
        bail!("CodSpeed runner must be an exact version");
    }
    let action = runner["uses"].as_str().unwrap();
    for step in [toolchain, runner] {
        let uses = step["uses"].as_str().unwrap();
        let revision = uses.split_once('@').unwrap().1;
        if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("CodSpeed build actions must use immutable full commit revisions: {uses}");
        }
    }
    if runner["with"]["mode"].as_str() != Some("simulation") {
        bail!("Tier-B CodSpeed must use simulation mode");
    }
    let summary = steps
        .iter()
        .find(|step| step["name"].as_str() == Some("Record benchmark build identity"))
        .context("CodSpeed build identity step missing")?["run"]
        .as_str()
        .context("CodSpeed identity script missing")?;
    // Narrow checks for this workflow's straight-line script, not a shell parser.
    let summary = summary
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for command in ["rustc -Vv", "cargo -V", "sha256sum Cargo.lock"] {
        if !summary.lines().any(|line| line == command) {
            bail!("CodSpeed build identity must execute {command}");
        }
    }
    for required in [
        "rustc -Vv",
        "cargo -V",
        "cargo codspeed --version",
        "git rev-parse HEAD",
        "sha256sum Cargo.lock",
        "GITHUB_STEP_SUMMARY",
        "profile=bench",
        "features=default,benchmark",
        "mode=simulation",
        action.strip_prefix("CodSpeedHQ/action@").unwrap(),
    ] {
        if !summary.contains(required) {
            bail!("CodSpeed build identity missing {required}");
        }
    }
    if !summary.contains(&format!("codspeed-runner={version}")) {
        bail!("CodSpeed recorded runner differs from configured runner");
    }
    let target = job["env"]["CARGO_BUILD_TARGET"]
        .as_str()
        .context("CodSpeed explicit target missing")?;
    if !summary.contains(&format!("target={target}")) {
        bail!("CodSpeed recorded target differs from configured target");
    }
    let build = steps
        .iter()
        .find(|step| step["name"].as_str() == Some("Build CodSpeed benchmarks"))
        .context("CodSpeed build step missing")?["run"]
        .as_str()
        .context("CodSpeed build command missing")?;
    for required in [
        "cargo codspeed build",
        "--locked",
        "--profile bench",
        "--features benchmark",
    ] {
        if !build.contains(required) {
            bail!("CodSpeed build must preserve {required}");
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
    fn codspeed_identity_rejects_drift() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let yaml: Value = serde_yaml_ng::from_str(
            &fs::read_to_string(root.join(".github/workflows/benchmarks-codspeed.yml")).unwrap(),
        )
        .unwrap();
        for (needle, replacement) in [
            ("1.88.0", "stable"),
            ("rustc -Vv", "rustc -V"),
            ("runner-version: \"5.2.1\"", "runner-version: latest"),
            ("codspeed-runner=5.2.1", "codspeed-runner=5.0.0"),
            ("--locked", "--offline"),
            ("rustc -Vv", "# rustc -Vv"),
            ("cargo codspeed build", "cargo build"),
            ("mode: simulation", "mode: walltime"),
            (
                "CodSpeedHQ/action@373d6868929f444bc08d901fd0eb0ad52a8875ea",
                "CodSpeedHQ/action@v5",
            ),
            (
                "dtolnay/rust-toolchain@89b12181fb390509a0842a86cc55eeb8eb928c1d",
                "dtolnay/rust-toolchain@stable",
            ),
        ] {
            let serialized = serde_yaml_ng::to_string(&yaml).unwrap();
            // Mutate the original source so quoting cannot hide a lost contract.
            let original =
                fs::read_to_string(root.join(".github/workflows/benchmarks-codspeed.yml")).unwrap();
            assert!(
                original.contains(needle),
                "missing mutation input {needle}: {serialized}"
            );
            let changed: Value =
                serde_yaml_ng::from_str(&original.replace(needle, replacement)).unwrap();
            assert!(validate_codspeed(&changed).is_err(), "accepted {needle}");
        }
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
