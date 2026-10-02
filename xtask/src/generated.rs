use anyhow::{Context, Result, bail};
use schemars::schema_for;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    artifact: Vec<Artifact>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    file: String,
    kind: String,
    source: String,
    command: String,
    context: String,
}

fn outputs() -> Result<Vec<(&'static str, String)>> {
    let values = [(
        "session-schema.yml",
        serde_json::to_value(schema_for!(iracing_sdk::schema::SessionInfo))?,
    )];
    values
        .into_iter()
        .map(|(name, value)| Ok((name, serde_yaml_ng::to_string(&value)?)))
        .collect()
}

fn compare(name: &str, generated: &str, committed: &str) -> Result<()> {
    if generated != committed {
        bail!("reference drift: {name}; regenerate with cargo xtask generate-reference");
    }
    Ok(())
}

pub fn generate(root: &Path) -> Result<()> {
    for (file, bytes) in outputs()? {
        fs::write(root.join("docs/reference").join(file), bytes)?;
    }
    Ok(())
}

pub fn check(root: &Path) -> Result<()> {
    let directory = root.join("docs/reference");
    let manifest: Manifest =
        toml::from_str(&fs::read_to_string(directory.join("generation.toml"))?)?;
    let generated = outputs()?;
    let temporary = tempfile::tempdir()?;
    let mut registered = BTreeSet::new();
    for artifact in manifest.artifact {
        if artifact.file.contains('/')
            || artifact.file.contains('\\')
            || !artifact.file.ends_with(".yml")
        {
            bail!("invalid reference filename {}", artifact.file);
        }
        if !registered.insert(artifact.file.clone()) {
            bail!("duplicate reference {}", artifact.file);
        }
        for field in [&artifact.source, &artifact.command, &artifact.context] {
            if field.trim().is_empty() {
                bail!("missing provenance for {}", artifact.file);
            }
        }
        let committed = fs::read_to_string(directory.join(&artifact.file))?;
        let schema: Value = serde_yaml_ng::from_str(&committed)
            .with_context(|| format!("invalid reference {}", artifact.file))?;
        if !schema.is_object() || schema["$schema"].as_str().is_none() || schema["type"] != "object"
        {
            bail!("invalid schema envelope {}", artifact.file);
        }
        match artifact.kind.as_str() {
            "generated" => {
                let (_, bytes) = generated
                    .iter()
                    .find(|(name, _)| *name == artifact.file)
                    .context("no generator registered")?;
                let path = temporary.path().join(&artifact.file);
                fs::write(&path, bytes)?;
                compare(&artifact.file, &fs::read_to_string(path)?, &committed)?;
            }
            "captured" => {
                if schema["examples"].as_array().is_none_or(|v| v.is_empty()) {
                    bail!("captured schema requires examples: {}", artifact.file);
                }
            }
            _ => bail!("unknown artifact kind {}", artifact.kind),
        }
    }
    let actual = fs::read_dir(&directory)?
        .map(|entry| {
            let name = entry?.file_name().to_string_lossy().into_owned();
            Ok(name)
        })
        .collect::<Result<Vec<String>>>()?
        .into_iter()
        .filter(|s| s.ends_with(".yml"))
        .collect::<BTreeSet<_>>();
    if registered != actual {
        bail!("reference manifest does not match committed YAML inventory");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unregistered_and_malformed_reference_artifacts_without_rewriting() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("docs/reference");
        fs::create_dir_all(&destination).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("docs/reference");
        for entry in fs::read_dir(source).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                fs::copy(&path, destination.join(path.file_name().unwrap())).unwrap();
            }
        }
        check(temp.path()).unwrap();
        let artifact = destination.join("session-schema.yml");
        let original = fs::read_to_string(&artifact).unwrap();
        let drift = format!("{original}\n");
        fs::write(&artifact, &drift).unwrap();
        assert!(
            check(temp.path())
                .unwrap_err()
                .to_string()
                .contains("reference drift")
        );
        assert_eq!(fs::read_to_string(&artifact).unwrap(), drift);
        fs::write(&artifact, original).unwrap();
        fs::write(destination.join("unregistered.yml"), "{}").unwrap();
        assert!(check(temp.path()).is_err());
        fs::remove_file(destination.join("unregistered.yml")).unwrap();
        fs::write(destination.join("live-variable-schema.yml"), "[malformed").unwrap();
        assert!(
            check(temp.path())
                .unwrap_err()
                .to_string()
                .contains("invalid reference")
        );
    }

    #[test]
    fn deterministic_output_and_drift() {
        let first = outputs().unwrap();
        assert_eq!(first, outputs().unwrap());
        compare("schema", &first[0].1, &first[0].1).unwrap();
        assert!(compare("schema", &first[0].1, "changed").is_err());
    }
}
