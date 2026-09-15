#![warn(missing_docs)]
//! Deterministic IBT fixture generation and verification for this workspace.
//!
//! This unpublished crate owns the canonical generated files under
//! `test-data/ibt` and `test-data/session-yaml`. It uses `iracing-sdk` as the
//! source of truth for SDK wire structures rather than duplicating their
//! layouts.
//!
//! # Layout invariants
//!
//! An IBT fixture begins with a 112-byte [`iracing_sdk::irsdk::Header`], followed
//! by a 32-byte [`iracing_sdk::irsdk::DiskSubHeader`]. The complete IBT preamble
//! is therefore 144 bytes, which is also the first variable-header offset. Each
//! [`iracing_sdk::irsdk::VariableHeader`] is 144 bytes. Session YAML follows the
//! variable-header array, and fixed-size telemetry frames follow the YAML.
//!
//! The distinction between the 112-byte main header and 144-byte IBT preamble is
//! essential. The schema-version-1 manifest retains the legacy field name
//! `live_header_prefix_size` for the 112-byte value even though this crate only
//! generates recorded IBT data.
//!
//! # Workflows
//!
//! [`generate`] writes the canonical artifacts, [`verify`] reads and validates
//! them, and [`check`] generates, verifies, then optionally invokes a scoped Git
//! drift check. Generation builds every output in memory before it begins
//! writing files. Verification combines independent manifest, hash, wire-layout,
//! YAML, schema, and full-frame-reader checks.
//!
//! ```no_run
//! use test_fixtures::{verify, workspace_root};
//!
//! let report = verify(&workspace_root())?;
//! println!("verified {} frames", report.frame_count);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! See the crate README for the command interface and maintenance workflow.

mod generate;
mod model;
mod verify;

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

/// Summary returned after generating the canonical artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationReport {
    /// Number of IBT fixtures generated.
    pub fixture_count: usize,
    /// Total number of files written, including YAML and the manifest.
    pub file_count: usize,
}

/// Summary returned after verifying canonical artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationReport {
    /// Number of IBT fixtures verified.
    pub fixture_count: usize,
    /// Total telemetry frames decoded.
    pub frame_count: usize,
}

/// Summary returned by the complete generation and verification workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckReport {
    /// Generation summary.
    pub generation: GenerationReport,
    /// Verification summary.
    pub verification: VerificationReport,
    /// Whether Git drift was checked.
    pub drift_checked: bool,
}

/// Returns the compile-time workspace root containing this internal tool crate.
///
/// The crate is expected to remain at `<workspace>/crates/test-fixtures`. Use an
/// explicit path with [`generate`], [`verify`], or [`check`] when operating on an
/// isolated repository-shaped directory.
///
/// # Panics
///
/// Panics if the crate's compile-time manifest directory does not have the
/// expected two parent directories.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("test-fixtures must be located under <workspace>/crates")
        .to_path_buf()
}

/// Generates all deterministic IBT, YAML, and manifest artifacts below `repo_root`.
///
/// Profiles are deterministic: non-random values use fixed formulas, while
/// steering values use a profile-seeded `ChaCha8Rng`. The function constructs
/// the complete artifact set in memory before creating directories and writing
/// files.
///
/// # Errors
///
/// Returns an error when a profile violates its frame geometry, a value cannot
/// fit the SDK wire representation, manifest serialization fails, or an output
/// directory/file cannot be created or written.
pub fn generate(repo_root: &Path) -> Result<GenerationReport> {
    generate::generate(repo_root)
}

/// Verifies all manifest-backed fixture artifacts using `iracing-sdk`.
///
/// This operation is read-only. In addition to checking manifest values and
/// SHA-256 hashes, it decodes SDK wire headers, compares embedded YAML, builds an
/// [`iracing_sdk::ibt::IbtReader`], checks its schema, and consumes every frame.
///
/// # Errors
///
/// Returns an error when the manifest or an artifact is missing, malformed,
/// unsafe to resolve below `repo_root`, inconsistent with the layout contract,
/// hash-mismatched, or unreadable through `IbtReader`.
pub fn verify(repo_root: &Path) -> Result<VerificationReport> {
    verify::verify(repo_root)
}

/// Generates, verifies, and optionally checks generated directories for Git drift.
///
/// When `drift_check` is `true`, this runs `git diff --exit-code` scoped to
/// `test-data/ibt` and `test-data/session-yaml` after successful generation and
/// verification. Because generation runs first, this function rewrites canonical
/// fixture paths even when the final drift check fails.
///
/// # Errors
///
/// Returns any error from [`generate`] or [`verify`]. It also returns an error if
/// Git cannot be launched or reports differences in the generated directories.
pub fn check(repo_root: &Path, drift_check: bool) -> Result<CheckReport> {
    let generation = generate(repo_root)?;
    let verification = verify(repo_root)?;
    if drift_check {
        let status = Command::new("git")
            .args([
                "diff",
                "--exit-code",
                "--",
                "test-data/ibt",
                "test-data/session-yaml",
            ])
            .current_dir(repo_root)
            .status()
            .context("running git diff for generated fixture drift")?;
        if !status.success() {
            bail!(
                "generated fixture drift detected; review the diff and commit intentional changes"
            );
        }
    }
    Ok(CheckReport {
        generation,
        verification,
        drift_checked: drift_check,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_and_verification_work_in_an_isolated_root() {
        let directory = tempfile::tempdir().unwrap();
        let generated = generate(directory.path()).unwrap();
        assert_eq!(generated.fixture_count, 3);
        assert_eq!(generated.file_count, 7);
        let verified = verify(directory.path()).unwrap();
        assert_eq!(verified.fixture_count, 3);
        assert_eq!(verified.frame_count, 84);
    }

    #[test]
    fn verifier_reports_corruption() {
        let directory = tempfile::tempdir().unwrap();
        generate(directory.path()).unwrap();
        let path = directory.path().join("test-data/ibt/profile_small.ibt");
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        let error = verify(directory.path()).unwrap_err().to_string();
        assert!(error.contains("SHA-256 mismatch"), "{error}");
    }
}
