//! Test utilities for consistent path resolution and test data access
//!
//! This module provides utilities for finding test data files and other testing helpers
//! that are used across the pitwall workspace.

use std::path::{Path, PathBuf};

/// Guidance shown when telemetry fixtures are missing from the repository checkout.
pub const FIXTURE_INSTALL_GUIDANCE: &str = "Telemetry fixtures are stored under test-data/. Install Git LFS and run `git lfs pull` to download them.";

/// Error returned when a required telemetry fixture cannot be located.
#[derive(Debug, Clone)]
pub struct FixtureError {
    message: String,
}

impl FixtureError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FixtureError {}

/// Require that a specific telemetry fixture exists on disk.
///
/// Returns the resolved [`PathBuf`] when the fixture exists, or a [`FixtureError`]
/// that includes guidance for installing Git LFS when it does not.
pub fn require_fixture<P: AsRef<Path>>(path: P) -> Result<PathBuf, FixtureError> {
    let path_ref = path.as_ref();
    if path_ref.exists() {
        Ok(path_ref.to_path_buf())
    } else {
        Err(FixtureError::new(format!(
            "Missing telemetry fixture: {}. {}",
            path_ref.display(),
            FIXTURE_INSTALL_GUIDANCE
        )))
    }
}

/// Find the git repository root by walking up the directory tree
///
/// This utility ensures consistent test data path resolution across all crates
/// in the workspace, regardless of the current working directory when tests are run.
///
/// # Returns
///
/// Returns the path to the git repository root directory (the directory containing `.git`).
///
/// # Errors
///
/// Returns an error if:
/// - No `.git` directory is found in the current directory or any parent directory
/// - There are filesystem access issues
pub fn find_git_repository_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut current_dir = std::env::current_dir()?;

    loop {
        let git_dir = current_dir.join(".git");
        if git_dir.exists() {
            return Ok(current_dir);
        }

        // Move to parent directory
        if let Some(parent) = current_dir.parent() {
            current_dir = parent.to_path_buf();
        } else {
            return Err("Git repository root not found".into());
        }
    }
}

/// Get the test-data directory path relative to the git repository root
///
/// This is a convenience function that combines `find_git_repository_root()` with
/// the standard test-data directory structure.
pub fn get_test_data_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let repo_root = find_git_repository_root()?;
    Ok(repo_root.join("test-data"))
}

/// Get a list of IBT test files from the test-data/ibt directory
///
/// This function scans the standard IBT test directory and returns all `.ibt` files
/// sorted by filename for consistent test ordering.
///
/// # Returns
///
/// Returns a vector of paths to all `.ibt` files in the test-data/ibt directory.
/// Returns an empty vector if the directory doesn't exist or contains no IBT files.
pub fn get_ibt_test_files() -> Vec<PathBuf> {
    let test_data_dir = match get_test_data_dir() {
        Ok(dir) => dir,
        Err(_) => return vec![],
    };

    let ibt_dir = test_data_dir.join("ibt");
    if !ibt_dir.exists() {
        return vec![];
    }

    let mut ibt_files = vec![];
    if let Ok(entries) = std::fs::read_dir(&ibt_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("ibt") {
                ibt_files.push(path);
            }
        }
    }

    ibt_files.sort();
    ibt_files
}

/// Get the smallest IBT test file for performance-sensitive tests
///
/// This function returns the smallest `.ibt` file from the test data directory,
/// which is useful for tests that need to run quickly or repeatedly.
///
/// # Returns
///
/// Returns `Some(path)` to the smallest IBT file, or `None` if no IBT files are found.
pub fn get_smallest_ibt_test_file() -> Option<PathBuf> {
    let test_files = get_ibt_test_files();
    if test_files.is_empty() {
        return None;
    }

    // Find the smallest file for faster tests
    let mut smallest_file = None;
    let mut smallest_size = u64::MAX;

    for file in test_files {
        if let Ok(metadata) = std::fs::metadata(&file) {
            if metadata.len() < smallest_size {
                smallest_size = metadata.len();
                smallest_file = Some(file);
            }
        }
    }

    smallest_file
}

/// Require that at least one IBT telemetry fixture is available and return them.
///
/// This is typically used by integration tests that rely on Git LFS-hosted
/// `.ibt` recordings. Tests should use this helper instead of silently skipping
/// when fixtures are missing so that CI surfaces actionable failures.
pub fn require_ibt_fixtures() -> Result<Vec<PathBuf>, FixtureError> {
    let fixtures = get_ibt_test_files();
    if fixtures.is_empty() {
        Err(FixtureError::new(format!(
            "No IBT telemetry fixtures found in test-data/ibt. {}",
            FIXTURE_INSTALL_GUIDANCE
        )))
    } else {
        Ok(fixtures)
    }
}

/// Require a named IBT telemetry fixture within `test-data/ibt` and return its path.
pub fn require_named_ibt_fixture(file_name: &str) -> Result<PathBuf, FixtureError> {
    let fixtures = require_ibt_fixtures()?;
    fixtures
        .into_iter()
        .find(|path| path.file_name().and_then(|n| n.to_str()) == Some(file_name))
        .ok_or_else(|| {
            FixtureError::new(format!(
                "Expected telemetry fixture '{}' in test-data/ibt. {}",
                file_name, FIXTURE_INSTALL_GUIDANCE
            ))
        })
}

/// Require the smallest IBT telemetry fixture, useful for performance-sensitive tests.
pub fn require_smallest_ibt_fixture() -> Result<PathBuf, FixtureError> {
    get_smallest_ibt_test_file().ok_or_else(|| {
        FixtureError::new(format!(
            "No IBT telemetry fixtures found in test-data/ibt. {}",
            FIXTURE_INSTALL_GUIDANCE
        ))
    })
}

/// Require a file inside `test-data/` by name.
#[cfg(windows)]
pub fn require_test_data_file(file_name: &str) -> Result<PathBuf, FixtureError> {
    let test_data_dir = get_test_data_dir().map_err(|err| {
        FixtureError::new(format!(
            "Failed to resolve test-data directory: {}. {}",
            err, FIXTURE_INSTALL_GUIDANCE
        ))
    })?;

    require_fixture(test_data_dir.join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_find_git_repository_root_works() {
        // Test that we can find the git repository root
        let repo_root = find_git_repository_root().expect("Should find git repository root");

        // Verify it contains a .git directory
        assert!(
            repo_root.join(".git").exists(),
            "Repository root should contain .git directory"
        );

        // Verify it's actually the expected project root by checking for key files
        assert!(
            repo_root.join("Cargo.toml").exists(),
            "Repository root should contain workspace Cargo.toml"
        );
        assert!(
            repo_root.join("README.md").exists(),
            "Repository root should contain README.md"
        );

        println!("Found git repository root: {}", repo_root.display());
    }

    #[test]
    fn test_get_test_data_dir() {
        let test_data_dir = get_test_data_dir().expect("Should find test-data directory");

        // The directory should exist (it contains the IBT test files you added)
        assert!(test_data_dir.exists(), "test-data directory should exist");
        assert!(test_data_dir.is_dir(), "test-data should be a directory");

        println!("Found test-data directory: {}", test_data_dir.display());
    }

    #[test]
    fn test_get_ibt_test_files() {
        let ibt_files = get_ibt_test_files();

        // Should find the IBT files you added
        if !ibt_files.is_empty() {
            println!("Found {} IBT test files:", ibt_files.len());
            for file in &ibt_files {
                println!("  {}", file.display());
            }

            // Verify they all have .ibt extension
            for file in &ibt_files {
                assert_eq!(file.extension().and_then(|s| s.to_str()), Some("ibt"));
            }
        } else {
            println!("No IBT test files found (this is expected in CI or clean checkouts)");
        }
    }

    #[test]
    fn test_get_smallest_ibt_test_file() {
        let smallest_file = get_smallest_ibt_test_file();

        if let Some(file) = smallest_file {
            println!("Smallest IBT test file: {}", file.display());

            if let Ok(metadata) = std::fs::metadata(&file) {
                println!("Size: {} bytes", metadata.len());
            }

            // Should have .ibt extension
            assert_eq!(file.extension().and_then(|s| s.to_str()), Some("ibt"));
        } else {
            println!("No IBT test files found for smallest file selection");
        }
    }

    #[test]
    fn test_require_fixture_errors_when_missing() {
        let result = require_fixture(Path::new("test-data/__missing_fixture"));
        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("Missing telemetry fixture"));
        assert!(message.contains("git lfs pull"));
    }
}
