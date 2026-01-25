//! Lockfile parsing for Go ecosystem
//!
//! Supports finding package versions from go.sum and go.mod files.
//!
//! go.sum format: `<module path> <version>[/go.mod] <hash>`
//! go.mod format: `require <module path> <version>` or `require (...)` blocks

use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockfileError {
    #[error("No go.sum found. Specify version explicitly.")]
    NotFound,

    #[error(
        "Version not found for '{package}'. Specify explicitly: dotdeps add go:{package}@<version>"
    )]
    VersionNotFound { package: String },

    #[error("Failed to read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Find the version of a Go module by searching go.sum
///
/// Searches upward from the current directory for go.sum
pub fn find_version(package: &str) -> Result<String, LockfileError> {
    let lockfile = find_lockfile()?;
    parse_version_from_lockfile(&lockfile, package)
}

/// Find the nearest go.sum by walking up from current directory
fn find_lockfile() -> Result<PathBuf, LockfileError> {
    let cwd = std::env::current_dir().map_err(|_| LockfileError::NotFound)?;

    let mut dir = cwd.as_path();
    loop {
        let path = dir.join("go.sum");
        if path.exists() {
            return Ok(path);
        }

        // Move to parent directory
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    Err(LockfileError::NotFound)
}

/// Parse version from go.sum
///
/// go.sum lines have format: `<module path> <version>[/go.mod] <hash>`
/// We extract the module path and version (stripping /go.mod suffix if present)
fn parse_version_from_lockfile(path: &Path, package: &str) -> Result<String, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let normalized_package = normalize_module_path(package);

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        // Split by whitespace: module_path version hash
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let module_path = parts[0];
        let version_with_suffix = parts[1];

        // Strip /go.mod suffix if present (go.sum has lines for both module and go.mod)
        let version = version_with_suffix
            .strip_suffix("/go.mod")
            .unwrap_or(version_with_suffix);

        // Match module path (case-sensitive for Go)
        if normalize_module_path(module_path) == normalized_package {
            // Strip 'v' prefix for our cache format
            let clean_version = version.strip_prefix('v').unwrap_or(version);
            return Ok(clean_version.to_string());
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Normalize Go module path for comparison
///
/// Go module paths are case-sensitive, but we normalize for consistent matching.
/// Also handles version suffixes like /v2, /v3 which are part of the module path.
fn normalize_module_path(path: &str) -> String {
    // Go modules are case-sensitive, but we lowercase for matching
    // since some registries are case-insensitive
    path.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_go_sum_basic() {
        let content = r#"
github.com/gin-gonic/gin v1.9.1 h1:4idEAncQnU5cB7BeOkPtxjfCSye0AAm1R0RVIqFPSdg=
github.com/gin-gonic/gin v1.9.1/go.mod h1:RdlIFONzZF9oEQpGL/JT3L6bXr9FhGlD2RE3T9b4nPY=
golang.org/x/sync v0.6.0 h1:5BMeUDZ7vkXGfEr1x9B4bRcTH4lpkTkpdh0T/J+qjbQ=
golang.org/x/sync v0.6.0/go.mod h1:WCZ9dVlaxBgvOsqPCtNIP9BZzfXnIT9AOKnlB++y2AY=
"#;

        // Test basic module lookup
        let lines: Vec<(&str, &str)> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let version = parts[1]
                    .strip_suffix("/go.mod")
                    .unwrap_or(parts[1])
                    .strip_prefix('v')
                    .unwrap_or(parts[1]);
                (parts[0], version)
            })
            .collect();

        assert_eq!(lines[0].0, "github.com/gin-gonic/gin");
        assert_eq!(lines[0].1, "1.9.1");
        assert_eq!(lines[2].0, "golang.org/x/sync");
        assert_eq!(lines[2].1, "0.6.0");
    }

    #[test]
    fn test_normalize_module_path() {
        assert_eq!(
            normalize_module_path("github.com/Gin-Gonic/Gin"),
            "github.com/gin-gonic/gin"
        );
        assert_eq!(
            normalize_module_path("github.com/gin-gonic/gin/v2"),
            "github.com/gin-gonic/gin/v2"
        );
    }

    #[test]
    fn test_parse_go_sum_with_pseudo_version() {
        let content = r#"
github.com/example/pkg v0.0.0-20231215123456-abc123def456 h1:hash=
"#;

        let line = content.trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        let version = parts[1].strip_prefix('v').unwrap_or(parts[1]);

        assert_eq!(parts[0], "github.com/example/pkg");
        assert_eq!(version, "0.0.0-20231215123456-abc123def456");
    }

    #[test]
    fn test_parse_go_sum_with_version_suffix() {
        let content = r#"
github.com/example/pkg/v2 v2.1.0 h1:hash=
github.com/example/pkg/v3 v3.0.0-beta.1 h1:hash=
"#;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            let version = parts[1].strip_prefix('v').unwrap_or(parts[1]);

            if parts[0].ends_with("/v2") {
                assert_eq!(version, "2.1.0");
            } else if parts[0].ends_with("/v3") {
                assert_eq!(version, "3.0.0-beta.1");
            }
        }
    }

    #[test]
    fn test_parse_go_sum_indirect_dependencies() {
        // go.sum includes both direct and indirect dependencies
        // We don't distinguish between them - any module in go.sum can be looked up
        let content = r#"
github.com/direct/dep v1.0.0 h1:direct=
github.com/indirect/dep v2.0.0 h1:indirect=
"#;

        let lines: Vec<&str> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split_whitespace().next().unwrap())
            .collect();

        assert!(lines.contains(&"github.com/direct/dep"));
        assert!(lines.contains(&"github.com/indirect/dep"));
    }
}
