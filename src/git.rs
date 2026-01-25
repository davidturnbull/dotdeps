//! Git operations for cloning repositories
//!
//! Handles shallow cloning with tag resolution:
//! 1. Try `v{version}` tag first
//! 2. Fall back to `{version}` tag
//! 3. Try `{package}-{version}` for monorepo crates
//! 4. Try `{package}-v{version}` for monorepo crates
//! 5. Fall back to default branch (with warning)

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
/// 3. `{package}-{version}` (e.g., `tokio-1.0.0`) - for monorepo crates
/// 4. `{package}-v{version}` (e.g., `tokio-v1.0.0`) - for monorepo crates
/// 5. Default branch (warns user)
///
/// On failure, cleans up any partial clone.
pub fn clone(
    repo_url: &str,
    version: &str,
    package: &str,
    dest: &Path,
) -> Result<CloneResult, GitError> {
    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| GitError::CommandFailed {
            message: format!("Failed to create directory {}: {}", parent.display(), e),
        })?;
    }

    // Build list of tags to try
    let tags_to_try = build_tag_candidates(version, package);

    // Try each tag in order
    for tag in &tags_to_try {
        if let Ok(result) = try_clone_at_ref(repo_url, tag, dest) {
            return Ok(result);
        }
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

/// Build the list of tag candidates to try for a given version and package
///
/// Returns tags in priority order:
/// 1. `v{version}` - most common format
/// 2. `{version}` - used by some projects
/// 3. `{package}-{version}` - monorepo format (e.g., tokio-1.0.0)
/// 4. `{package}-v{version}` - monorepo format with v prefix
fn build_tag_candidates(version: &str, package: &str) -> Vec<String> {
    // Extract the base package name (last component of path-like names)
    // e.g., "github.com/org/repo" -> "repo"
    // e.g., "@scope/pkg" -> "pkg"
    // e.g., "simple-name" -> "simple-name"
    let base_name = extract_base_package_name(package);

    vec![
        format!("v{}", version),
        version.to_string(),
        format!("{}-{}", base_name, version),
        format!("{}-v{}", base_name, version),
    ]
}

/// Extract the base package name for monorepo tag patterns
///
/// For path-like packages, returns the last component:
/// - "github.com/org/repo" -> "repo"
/// - "@scope/pkg" -> "pkg"
/// - "simple-name" -> "simple-name"
fn extract_base_package_name(package: &str) -> &str {
    // Try splitting by "/" and take the last non-empty part
    package
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(package)
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

    #[test]
    fn test_build_tag_candidates_simple_name() {
        let tags = build_tag_candidates("1.0.0", "serde");
        assert_eq!(tags, vec!["v1.0.0", "1.0.0", "serde-1.0.0", "serde-v1.0.0"]);
    }

    #[test]
    fn test_build_tag_candidates_scoped_npm() {
        let tags = build_tag_candidates("4.17.21", "@types/node");
        assert_eq!(
            tags,
            vec!["v4.17.21", "4.17.21", "node-4.17.21", "node-v4.17.21"]
        );
    }

    #[test]
    fn test_build_tag_candidates_go_module() {
        let tags = build_tag_candidates("1.9.1", "github.com/gin-gonic/gin");
        assert_eq!(tags, vec!["v1.9.1", "1.9.1", "gin-1.9.1", "gin-v1.9.1"]);
    }

    #[test]
    fn test_extract_base_package_name_simple() {
        assert_eq!(extract_base_package_name("tokio"), "tokio");
        assert_eq!(extract_base_package_name("serde-json"), "serde-json");
    }

    #[test]
    fn test_extract_base_package_name_scoped() {
        assert_eq!(extract_base_package_name("@types/node"), "node");
        assert_eq!(extract_base_package_name("@org/pkg"), "pkg");
    }

    #[test]
    fn test_extract_base_package_name_path() {
        assert_eq!(extract_base_package_name("github.com/gin-gonic/gin"), "gin");
        assert_eq!(extract_base_package_name("golang.org/x/sync"), "sync");
    }
}
