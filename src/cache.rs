//! Cache management for dotdeps
//!
//! The cache stores cloned repositories at:
//! `~/.cache/dotdeps/<ecosystem>/<package>/<version>/`
//!
//! Package paths are preserved as nested directories:
//! - `~/.cache/dotdeps/node/@org/pkg/4.17.21/`
//! - `~/.cache/dotdeps/go/github.com/org/repo/v2/1.0.0/`

use crate::cli::Ecosystem;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Cannot determine cache directory. HOME environment variable not set.")]
    NoCacheDir,

    #[error("Cannot write to {path}. Check permissions.")]
    NotWritable { path: PathBuf },

    #[error("Failed to create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Returns the base cache directory: `~/.cache/dotdeps`
pub fn base_dir() -> Result<PathBuf, CacheError> {
    // Use XDG_CACHE_HOME if set, otherwise fall back to ~/.cache
    let cache_base = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join(".cache"))
                .unwrap_or_default()
        });

    if cache_base.as_os_str().is_empty() {
        return Err(CacheError::NoCacheDir);
    }

    Ok(cache_base.join("dotdeps"))
}

/// Returns the cache path for a specific package version:
/// `~/.cache/dotdeps/<ecosystem>/<package>/<version>/`
///
/// Package paths are preserved as nested directories.
pub fn package_dir(
    ecosystem: Ecosystem,
    package: &str,
    version: &str,
) -> Result<PathBuf, CacheError> {
    let base = base_dir()?;
    // Package may contain path separators (e.g., go modules, scoped npm packages)
    // We preserve them as nested directories
    Ok(base.join(ecosystem.to_string()).join(package).join(version))
}

/// Check if the cache directory exists and contains a valid clone
pub fn exists(ecosystem: Ecosystem, package: &str, version: &str) -> Result<bool, CacheError> {
    let path = package_dir(ecosystem, package, version)?;
    // Check for .git directory as indicator of a valid clone
    Ok(path.join(".git").is_dir())
}

/// Ensure the cache base directory exists and is writable
pub fn ensure_writable() -> Result<PathBuf, CacheError> {
    let base = base_dir()?;

    // Create the directory if it doesn't exist
    if !base.exists() {
        std::fs::create_dir_all(&base).map_err(|source| CacheError::CreateDir {
            path: base.clone(),
            source,
        })?;
    }

    // Verify we can write to it by checking metadata
    let metadata = std::fs::metadata(&base).map_err(|source| CacheError::CreateDir {
        path: base.clone(),
        source,
    })?;

    if metadata.permissions().readonly() {
        return Err(CacheError::NotWritable { path: base });
    }

    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_dir_uses_home() {
        // This test validates base_dir() works when XDG_CACHE_HOME is not set
        // We can't safely modify env vars in Rust 2024, so just check the result
        let base = base_dir().unwrap();
        assert!(base.to_string_lossy().ends_with("dotdeps"));
    }

    #[test]
    fn test_package_dir_simple() {
        let path = package_dir(Ecosystem::Python, "requests", "2.31.0").unwrap();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("dotdeps/python/requests/2.31.0"));
    }

    #[test]
    fn test_package_dir_scoped_npm() {
        let path = package_dir(Ecosystem::Node, "@org/pkg", "4.17.21").unwrap();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("dotdeps/node/@org/pkg/4.17.21"));
    }

    #[test]
    fn test_package_dir_go_module() {
        let path = package_dir(Ecosystem::Go, "github.com/org/repo/v2", "1.0.0").unwrap();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("dotdeps/go/github.com/org/repo/v2/1.0.0"));
    }
}
