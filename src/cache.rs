//! Cache management for dotdeps
//!
//! The cache stores cloned repositories at:
//! `~/.cache/dotdeps/<ecosystem>/<package>/<version>/`
//!
//! Package paths are preserved as nested directories:
//! - `~/.cache/dotdeps/node/@org/pkg/4.17.21/`
//! - `~/.cache/dotdeps/go/github.com/org/repo/v2/1.0.0/`
//!
//! Cache eviction uses LRU (least recently used) strategy based on filesystem
//! access time (atime). When cache exceeds the configured limit, oldest entries
//! are removed first.

use crate::cli::Ecosystem;
use std::path::PathBuf;
use std::time::SystemTime;
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

    #[error("Failed to read directory {path}: {source}")]
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to delete directory {path}: {source}")]
    DeleteDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(
        "Cache limit ({limit_bytes} bytes) is smaller than the entry ({entry_bytes} bytes). Increase cache_limit_gb or set to 0 for unlimited."
    )]
    CacheTooSmall { limit_bytes: u64, entry_bytes: u64 },
}

/// Information about a cached package for eviction purposes
#[derive(Debug)]
pub struct CacheEntry {
    /// Full path to the cached package version directory
    pub path: PathBuf,
    /// Size in bytes of all files in the directory
    pub size: u64,
    /// Last access time (atime) of the directory
    pub accessed: SystemTime,
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

/// List all cached packages with their size and access time
pub fn list_entries() -> Result<Vec<CacheEntry>, CacheError> {
    let base = base_dir()?;
    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    collect_cache_entries(&base, &mut entries)?;
    Ok(entries)
}

/// Calculate total cache size in bytes
#[allow(dead_code)]
pub fn total_size() -> Result<u64, CacheError> {
    let entries = list_entries()?;
    Ok(entries.iter().map(|e| e.size).sum())
}

/// Get the size of a single cache entry
pub fn entry_size(path: &PathBuf) -> u64 {
    get_dir_stats(path).0
}

/// Evict least recently accessed cache entries until under the limit
///
/// If `exclude` is provided, that path will not be evicted (used to protect
/// a newly added entry from being immediately removed).
///
/// Returns the paths of evicted directories.
pub fn evict_to_limit(
    limit_bytes: u64,
    exclude: Option<&PathBuf>,
) -> Result<Vec<PathBuf>, CacheError> {
    let mut entries = list_entries()?;
    let mut current_size: u64 = entries.iter().map(|e| e.size).sum();

    if current_size <= limit_bytes {
        return Ok(Vec::new());
    }

    // Sort by access time, oldest first (LRU)
    entries.sort_by(|a, b| a.accessed.cmp(&b.accessed));

    let mut evicted = Vec::new();

    for entry in entries {
        if current_size <= limit_bytes {
            break;
        }

        // Skip the excluded entry (newly added dependency)
        if let Some(excl) = exclude
            && &entry.path == excl
        {
            continue;
        }

        // Delete the directory
        std::fs::remove_dir_all(&entry.path).map_err(|source| CacheError::DeleteDir {
            path: entry.path.clone(),
            source,
        })?;

        current_size = current_size.saturating_sub(entry.size);
        evicted.push(entry.path);

        // Clean up empty parent directories
        cleanup_empty_parents(evicted.last().unwrap())?;
    }

    Ok(evicted)
}

/// Recursively collect cache entries (version directories containing .git)
fn collect_cache_entries(dir: &PathBuf, entries: &mut Vec<CacheEntry>) -> Result<(), CacheError> {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(CacheError::ReadDir {
                path: dir.clone(),
                source,
            });
        }
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // Skip entries we can't read
        };

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Check if this is a version directory (has .git)
        if path.join(".git").is_dir() {
            let (size, accessed) = get_dir_stats(&path);
            entries.push(CacheEntry {
                path,
                size,
                accessed,
            });
        } else {
            // Recurse into subdirectories
            collect_cache_entries(&path, entries)?;
        }
    }

    Ok(())
}

/// Get directory size and last access time
fn get_dir_stats(dir: &PathBuf) -> (u64, SystemTime) {
    let mut size = 0u64;
    let mut latest_access = SystemTime::UNIX_EPOCH;

    // Use the .git directory's access time as a proxy for cache entry access
    // This is more reliable than trying to track access to every file
    if let Ok(metadata) = std::fs::metadata(dir.join(".git"))
        && let Ok(accessed) = metadata.accessed()
    {
        latest_access = accessed;
    }

    // Calculate size recursively
    if let Ok(entries) = walkdir(dir) {
        for entry in entries {
            if let Ok(metadata) = entry.metadata()
                && metadata.is_file()
            {
                size += metadata.len();
            }
        }
    }

    (size, latest_access)
}

/// Simple recursive directory walker
fn walkdir(dir: &PathBuf) -> Result<Vec<std::fs::DirEntry>, std::io::Error> {
    let mut results = Vec::new();
    walkdir_recursive(dir, &mut results)?;
    Ok(results)
}

fn walkdir_recursive(
    dir: &PathBuf,
    results: &mut Vec<std::fs::DirEntry>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        results.push(entry);
        let path = results.last().unwrap().path();
        if path.is_dir() {
            walkdir_recursive(&path, results)?;
        }
    }
    Ok(())
}

/// Clean up empty parent directories up to the cache base
fn cleanup_empty_parents(path: &std::path::Path) -> Result<(), CacheError> {
    let base = base_dir()?;
    let mut current = path.parent();

    while let Some(parent) = current {
        // Stop at or before the base directory
        if parent == base || !parent.starts_with(&base) {
            break;
        }

        // Check if directory is empty
        let is_empty = match std::fs::read_dir(parent) {
            Ok(mut entries) => entries.next().is_none(),
            Err(_) => break,
        };

        if is_empty {
            let _ = std::fs::remove_dir(parent);
            current = parent.parent();
        } else {
            break;
        }
    }

    Ok(())
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
