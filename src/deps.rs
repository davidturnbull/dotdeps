//! .deps/ directory management
//!
//! Creates symlinks (or copies on Windows) from:
//! `.deps/<ecosystem>/<package>` -> `~/.cache/dotdeps/<ecosystem>/<package>/<version>/`

use crate::cache;
use crate::cli::Ecosystem;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DepsError {
    #[error("Failed to create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to create symlink from {link} to {target}: {source}")]
    Symlink {
        link: PathBuf,
        target: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to remove {path}: {source}")]
    Remove {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to read .deps directory: {source}")]
    ReadDir { source: std::io::Error },

    #[error("Cache error: {0}")]
    Cache(#[from] cache::CacheError),

    #[error("Failed to copy directory from {from} to {to}: {source}")]
    Copy {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },
}

/// Information about a dependency in .deps/
#[derive(Debug, Clone)]
pub struct DepEntry {
    pub ecosystem: Ecosystem,
    pub package: String,
    pub version: String,
    /// Path to the symlink in .deps/
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Target path the symlink points to (cache location)
    #[allow(dead_code)]
    pub target: PathBuf,
    pub is_broken: bool,
}

/// Returns the .deps directory path in the current working directory
pub fn deps_dir() -> PathBuf {
    PathBuf::from(".deps")
}

/// Returns the path for a specific package in .deps:
/// `.deps/<ecosystem>/<package>`
pub fn package_path(ecosystem: Ecosystem, package: &str) -> PathBuf {
    deps_dir().join(ecosystem.to_string()).join(package)
}

/// Returns the path where a symlink would be created for a package.
/// This is an alias for `package_path` used for dry-run mode.
pub fn link_path(ecosystem: Ecosystem, package: &str) -> PathBuf {
    package_path(ecosystem, package)
}

/// Check if running on Windows
fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Create a symlink (or copy on Windows) from .deps to cache
///
/// Creates: `.deps/<ecosystem>/<package>` -> `~/.cache/dotdeps/<ecosystem>/<package>/<version>/`
pub fn link(ecosystem: Ecosystem, package: &str, version: &str) -> Result<PathBuf, DepsError> {
    let cache_path = cache::package_dir(ecosystem, package, version)?;
    let link_path = package_path(ecosystem, package);

    // Ensure parent directories exist
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent).map_err(|source| DepsError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    // Remove existing link/directory if present
    if link_path.exists() || link_path.symlink_metadata().is_ok() {
        remove_link(&link_path)?;
    }

    if is_windows() {
        // On Windows, copy the directory instead of symlinking
        copy_dir_recursive(&cache_path, &link_path)?;
    } else {
        // On Unix, create a symlink with absolute path
        let absolute_cache = cache_path.canonicalize().unwrap_or(cache_path.clone());

        #[cfg(unix)]
        std::os::unix::fs::symlink(&absolute_cache, &link_path).map_err(|source| {
            DepsError::Symlink {
                link: link_path.clone(),
                target: absolute_cache,
                source,
            }
        })?;

        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&absolute_cache, &link_path).map_err(|source| {
            DepsError::Symlink {
                link: link_path.clone(),
                target: absolute_cache,
                source,
            }
        })?;
    }

    Ok(link_path)
}

/// Remove a symlink or directory
fn remove_link(path: &Path) -> Result<(), DepsError> {
    let metadata = path
        .symlink_metadata()
        .map_err(|source| DepsError::Remove {
            path: path.to_path_buf(),
            source,
        })?;

    if metadata.is_symlink() {
        fs::remove_file(path)
    } else if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|source| DepsError::Remove {
        path: path.to_path_buf(),
        source,
    })
}

/// Recursively copy a directory (used on Windows as symlink fallback)
fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), DepsError> {
    fs::create_dir_all(to).map_err(|source| DepsError::Copy {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })?;

    for entry in fs::read_dir(from).map_err(|source| DepsError::Copy {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| DepsError::Copy {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            source,
        })?;

        let src = entry.path();
        let dst = to.join(entry.file_name());

        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            fs::copy(&src, &dst).map_err(|source| DepsError::Copy {
                from: src,
                to: dst,
                source,
            })?;
        }
    }

    Ok(())
}

/// Remove a dependency from .deps/
/// Returns true if the dependency existed and was removed, false if it didn't exist
pub fn remove(ecosystem: Ecosystem, package: &str) -> Result<bool, DepsError> {
    let link_path = package_path(ecosystem, package);
    let existed = link_path.exists() || link_path.symlink_metadata().is_ok();

    if existed {
        remove_link(&link_path)?;
    }

    // Clean up empty parent directories, but keep .deps/
    let ecosystem_dir = deps_dir().join(ecosystem.to_string());
    if ecosystem_dir.exists()
        && let Ok(entries) = fs::read_dir(&ecosystem_dir)
        && entries.count() == 0
    {
        let _ = fs::remove_dir(&ecosystem_dir);
    }

    Ok(existed)
}

/// List all dependencies in .deps/
pub fn list() -> Result<Vec<DepEntry>, DepsError> {
    let deps = deps_dir();
    let mut entries = Vec::new();

    if !deps.exists() {
        return Ok(entries);
    }

    for ecosystem_entry in fs::read_dir(&deps).map_err(|source| DepsError::ReadDir { source })? {
        let ecosystem_entry = ecosystem_entry.map_err(|source| DepsError::ReadDir { source })?;
        let ecosystem_path = ecosystem_entry.path();

        if !ecosystem_path.is_dir() {
            continue;
        }

        let Some(ecosystem_name) = ecosystem_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let Ok(ecosystem) = ecosystem_name.parse::<Ecosystem>() else {
            continue;
        };

        // Recursively find all package directories (handles nested paths like @org/pkg)
        collect_packages(&ecosystem_path, &ecosystem_path, ecosystem, &mut entries)?;
    }

    Ok(entries)
}

/// Recursively collect package entries from an ecosystem directory
fn collect_packages(
    base: &Path,
    current: &Path,
    ecosystem: Ecosystem,
    entries: &mut Vec<DepEntry>,
) -> Result<(), DepsError> {
    for entry in fs::read_dir(current).map_err(|source| DepsError::ReadDir { source })? {
        let entry = entry.map_err(|source| DepsError::ReadDir { source })?;
        let path = entry.path();

        let metadata = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.is_symlink() {
            // This is a symlink to a cached package
            let package = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            let (target, is_broken, version) = match fs::read_link(&path) {
                Ok(target) => {
                    let is_broken = !target.exists();
                    let version = extract_version_from_path(&target);
                    (target, is_broken, version)
                }
                Err(_) => (PathBuf::new(), true, "unknown".to_string()),
            };

            entries.push(DepEntry {
                ecosystem,
                package,
                version,
                path: path.clone(),
                target,
                is_broken,
            });
        } else if metadata.is_dir() {
            // Could be a nested directory (e.g., @org in @org/pkg) or a copied directory on Windows
            // Check if it looks like a cache directory (has .git)
            if path.join(".git").is_dir() {
                // This is a copied directory (Windows fallback)
                let package = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                entries.push(DepEntry {
                    ecosystem,
                    package,
                    version: "local-copy".to_string(),
                    path: path.clone(),
                    target: path.clone(),
                    is_broken: false,
                });
            } else {
                // Recurse into nested directories
                collect_packages(base, &path, ecosystem, entries)?;
            }
        }
    }

    Ok(())
}

/// Extract version from cache path
/// e.g., `/home/user/.cache/dotdeps/python/requests/2.31.0` -> `2.31.0`
fn extract_version_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Remove the entire .deps directory
/// Returns true if the directory existed and was removed, false if it didn't exist
pub fn clean() -> Result<bool, DepsError> {
    let deps = deps_dir();
    let existed = deps.exists();
    if existed {
        fs::remove_dir_all(&deps).map_err(|source| DepsError::Remove { path: deps, source })?;
    }
    Ok(existed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_path_simple() {
        let path = package_path(Ecosystem::Python, "requests");
        assert_eq!(path, PathBuf::from(".deps/python/requests"));
    }

    #[test]
    fn test_package_path_scoped() {
        let path = package_path(Ecosystem::Node, "@org/pkg");
        assert_eq!(path, PathBuf::from(".deps/node/@org/pkg"));
    }

    #[test]
    fn test_package_path_go_module() {
        let path = package_path(Ecosystem::Go, "github.com/org/repo/v2");
        assert_eq!(path, PathBuf::from(".deps/go/github.com/org/repo/v2"));
    }

    #[test]
    fn test_extract_version_from_path() {
        let path = PathBuf::from("/home/user/.cache/dotdeps/python/requests/2.31.0");
        assert_eq!(extract_version_from_path(&path), "2.31.0");
    }

    #[test]
    fn test_is_windows() {
        // This test just ensures the function compiles and runs
        let _ = is_windows();
    }
}
