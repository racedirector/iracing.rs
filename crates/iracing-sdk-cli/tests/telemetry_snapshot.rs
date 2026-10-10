use anyhow::Result;
use iracing_sdk::test_utils::require_named_ibt_fixture;
use std::{path::Path, process::Command};

fn snapshot(path: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_iracing-sdk"));
    command.args(["telemetry", "snapshot", "ibt", "--path"]);
    command.arg(path);
    command
}

#[test]
fn selected_frames_write_plain_values_in_all_formats() -> Result<()> {
    let path = require_named_ibt_fixture("profile_small.ibt")?;
    let directory = tempfile::tempdir()?;
    for index in [0, 11] {
        for format in ["json", "json-pretty", "yaml", "none"] {
            let output = directory.path().join("snapshot");
            let result = snapshot(&path)
                .args([
                    "--index",
                    &index.to_string(),
                    "--format",
                    format,
                    "--output",
                ])
                .arg(&output)
                .output()?;
            assert!(result.status.success(), "{:?}", result);
            assert!(result.stdout.is_empty());
            let text = std::fs::read_to_string(&output)?;
            let value: serde_json::Value = if format == "yaml" {
                serde_yaml_ng::from_str(&text)?
            } else {
                serde_json::from_str(&text)?
            };
            assert_eq!(value.as_object().unwrap().len(), 8);
            assert_eq!(value["Speed"], 35.0 + index as f64 * 0.25);
            assert_eq!(value["SessionTime"], index as f64 / 60.0);
            assert_eq!(value["LapDist"], index as f64 * 18.5);
            assert!(text.ends_with('\n'));
            if format == "json" {
                assert!(text.starts_with("{\"SessionTime\":"));
            }
        }
    }
    Ok(())
}

#[test]
fn defaults_write_first_frame_as_yaml_to_stdout() -> Result<()> {
    let path = require_named_ibt_fixture("profile_small.ibt")?;
    let result = snapshot(&path).output()?;
    assert!(result.status.success(), "{:?}", result);
    let text = String::from_utf8(result.stdout)?;
    assert!(text.starts_with("SessionTime:"));
    let value: serde_json::Value = serde_yaml_ng::from_str(&text)?;
    assert_eq!(value["Speed"], 35.0);
    Ok(())
}

#[test]
fn snapshot_preserves_published_header_order() -> Result<()> {
    let path = require_named_ibt_fixture("profile_small.ibt")?;
    let header = *iracing_sdk::IbtFile::open(&path)?.header();
    let start = usize::try_from(header.variable_header_offset)?;
    let width = std::mem::size_of::<iracing_irsdk::VariableHeader>();
    let end = start + usize::try_from(header.variable_count)? * width;
    let mut bytes = std::fs::read(&path)?;
    let reordered: Vec<u8> = bytes[start..end]
        .chunks_exact(width)
        .rev()
        .flatten()
        .copied()
        .collect();
    bytes[start..end].copy_from_slice(&reordered);
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("reordered.ibt");
    std::fs::write(&input, bytes)?;
    let result = snapshot(&input).args(["--format", "json"]).output()?;
    assert!(result.status.success(), "{:?}", result);
    let text = String::from_utf8(result.stdout)?;
    assert!(text.starts_with("{\"Gear\":"));
    let value: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(value.as_object().unwrap().len(), 8);
    assert_eq!(value["Speed"], 35.0);
    Ok(())
}

#[test]
fn missing_variables_preserve_existing_output() -> Result<()> {
    let path = require_named_ibt_fixture("profile_small.ibt")?;
    let mut bytes = std::fs::read(&path)?;
    let offset = std::mem::offset_of!(iracing_irsdk::Header, variable_count);
    bytes[offset..offset + 4].copy_from_slice(&0_i32.to_le_bytes());
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("no-variables.ibt");
    let output = directory.path().join("snapshot");
    std::fs::write(&input, bytes)?;
    std::fs::write(&output, "keep this")?;
    let result = snapshot(&input).arg("--output").arg(&output).output()?;
    assert!(!result.status.success());
    assert!(String::from_utf8(result.stderr)?.contains("No telemetry variables"));
    assert_eq!(std::fs::read_to_string(output)?, "keep this");
    Ok(())
}

#[test]
fn output_options_work_before_and_after_source_subcommand() -> Result<()> {
    let path = require_named_ibt_fixture("profile_small.ibt")?;
    for before_source in [true, false] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_iracing-sdk"));
        command.args(["telemetry", "snapshot"]);
        if before_source {
            command.args(["--format", "json", "--output", "-"]);
        }
        command.args(["ibt", "--path"]).arg(&path);
        if !before_source {
            command.args(["--format", "json", "--output", "-"]);
        }
        let result = command.output()?;
        assert!(result.status.success(), "{:?}", result);
        let value: serde_json::Value = serde_json::from_slice(&result.stdout)?;
        assert_eq!(value["Speed"], 35.0);
    }
    Ok(())
}

#[test]
fn source_errors_preserve_existing_output_and_do_not_create_files() -> Result<()> {
    let path = require_named_ibt_fixture("profile_small.ibt")?;
    let directory = tempfile::tempdir()?;
    let malformed = directory.path().join("truncated.ibt");
    std::fs::write(&malformed, &std::fs::read(&path)?[..16])?;
    for (input, index) in [
        (path, "12"),
        (directory.path().join("missing.ibt"), "0"),
        (malformed, "0"),
    ] {
        for existing in [true, false] {
            let output = directory
                .path()
                .join(if existing { "existing" } else { "new" });
            if existing {
                std::fs::write(&output, "keep this")?;
            }
            let result = snapshot(&input)
                .args(["--index", index, "--output"])
                .arg(&output)
                .output()?;
            assert!(!result.status.success(), "{:?}", result);
            assert!(!result.stderr.is_empty());
            assert!(result.stdout.is_empty());
            if existing {
                assert_eq!(std::fs::read_to_string(&output)?, "keep this");
            } else {
                assert!(!output.exists());
            }
        }
    }
    Ok(())
}

#[test]
fn output_errors_fail_the_command() -> Result<()> {
    let path = require_named_ibt_fixture("profile_small.ibt")?;
    let directory = tempfile::tempdir()?;
    let result = snapshot(&path)
        .arg("--output")
        .arg(directory.path().join("missing").join("snapshot"))
        .output()?;
    assert!(!result.status.success());
    assert!(!result.stderr.is_empty());
    Ok(())
}

#[cfg(not(windows))]
#[test]
fn live_source_is_unavailable_on_non_windows_targets() -> Result<()> {
    let result = Command::new(env!("CARGO_BIN_EXE_iracing-sdk"))
        .args(["telemetry", "snapshot", "live"])
        .output()?;
    assert!(!result.status.success());
    assert!(String::from_utf8(result.stderr)?.contains("unrecognized subcommand 'live'"));
    Ok(())
}
