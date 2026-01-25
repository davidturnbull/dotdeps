//! Git operations for cloning repositories
//!
//! Handles shallow cloning with tag resolution:
//! 1. Try `v{version}` tag first
//! 2. Fall back to `{version}` tag
//! 3. Fall back to default branch (with warning)

use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Git command failed: {message}")]
    CommandFailed { message: String },

    #[error("Failed to execute git: {source}")]
    Exec { source: std::io::Error },
}

/// Result of a clone operation
pub struct CloneResult {
    /// Whether we fell back to the default branch
    pub used_default_branch: bool,
    /// The ref that was actually cloned (tag name or "default branch")
    pub cloned_ref: String,
}

/// Clone a repository to the specified directory with shallow clone
///
/// Tries tags in order:
/// 1. `v{version}` (e.g., `v2.31.0`)
/// 2. `{version}` (e.g., `2.31.0`)
/// 3. Default branch (warns user)
///
/// On failure, cleans up any partial clone.
pub fn clone(repo_url: &str, version: &str, dest: &Path) -> Result<CloneResult, GitError> {
    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| GitError::CommandFailed {
            message: format!("Failed to create directory {}: {}", parent.display(), e),
        })?;
    }

    // Try v{version} tag first
    let v_tag = format!("v{}", version);
    if let Ok(result) = try_clone_at_ref(repo_url, &v_tag, dest) {
        return Ok(result);
    }

    // Try {version} tag
    if let Ok(result) = try_clone_at_ref(repo_url, version, dest) {
        return Ok(result);
    }

    // Fall back to default branch
    match try_clone_default_branch(repo_url, dest) {
        Ok(mut result) => {
            result.used_default_branch = true;
            result.cloned_ref = "default branch".to_string();
            Ok(result)
        }
        Err(e) => {
            // Clean up partial clone on failure
            cleanup_partial_clone(dest);
            Err(e)
        }
    }
}

/// Try to clone at a specific git ref (tag or branch)
fn try_clone_at_ref(repo_url: &str, git_ref: &str, dest: &Path) -> Result<CloneResult, GitError> {
    // Clean up any existing partial clone first
    cleanup_partial_clone(dest);

    let output = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            git_ref,
            "--single-branch",
            repo_url,
        ])
        .arg(dest)
        .output()
        .map_err(|source| GitError::Exec { source })?;

    if output.status.success() {
        Ok(CloneResult {
            used_default_branch: false,
            cloned_ref: git_ref.to_string(),
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(GitError::CommandFailed {
            message: stderr.to_string(),
        })
    }
}

/// Clone the default branch
fn try_clone_default_branch(repo_url: &str, dest: &Path) -> Result<CloneResult, GitError> {
    // Clean up any existing partial clone first
    cleanup_partial_clone(dest);

    let output = Command::new("git")
        .args(["clone", "--depth", "1", repo_url])
        .arg(dest)
        .output()
        .map_err(|source| GitError::Exec { source })?;

    if output.status.success() {
        Ok(CloneResult {
            used_default_branch: true,
            cloned_ref: "default branch".to_string(),
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(GitError::CommandFailed {
            message: stderr.trim().to_string(),
        })
    }
}

/// Remove a partial clone directory if it exists
fn cleanup_partial_clone(dest: &Path) {
    if dest.exists() {
        let _ = std::fs::remove_dir_all(dest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_nonexistent_dir() {
        // Should not panic when directory doesn't exist
        cleanup_partial_clone(Path::new("/nonexistent/path/that/does/not/exist"));
    }
}
