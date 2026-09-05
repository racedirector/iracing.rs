//! End-to-end coverage for the fixture maintenance binary.

use std::process::Command;

/// Exercises each focused workflow and the default check behavior in isolation.
#[test]
fn focused_and_default_commands_succeed() {
    let directory = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_test-fixtures");

    let generate = Command::new(binary)
        .args([
            "--repo-root",
            directory.path().to_str().unwrap(),
            "generate",
        ])
        .output()
        .unwrap();
    assert!(
        generate.status.success(),
        "{}",
        String::from_utf8_lossy(&generate.stderr)
    );

    let verify = Command::new(binary)
        .args(["--repo-root", directory.path().to_str().unwrap(), "verify"])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let default_check = Command::new(binary)
        .args([
            "--repo-root",
            directory.path().to_str().unwrap(),
            "--no-drift-check",
        ])
        .output()
        .unwrap();
    assert!(
        default_check.status.success(),
        "{}",
        String::from_utf8_lossy(&default_check.stderr)
    );

    let explicit_check = Command::new(binary)
        .args([
            "--repo-root",
            directory.path().to_str().unwrap(),
            "check",
            "--no-drift-check",
        ])
        .output()
        .unwrap();
    assert!(
        explicit_check.status.success(),
        "{}",
        String::from_utf8_lossy(&explicit_check.stderr)
    );
}
