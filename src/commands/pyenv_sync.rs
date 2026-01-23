use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

use crate::paths;

pub fn execute(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let pyenv_root = std::env::var("HOMEBREW_PYENV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .expect("Cannot determine home directory")
                .join(".pyenv")
        });

    // Don't run multiple times at once
    let pyenv_sync_running = pyenv_root.join(".pyenv_sync_running");
    if pyenv_sync_running.exists() {
        return Ok(());
    }

    // Create lock file
    let pyenv_versions = pyenv_root.join("versions");
    fs::create_dir_all(&pyenv_versions)?;
    fs::write(&pyenv_sync_running, "")?;

    // Ensure we clean up the lock file
    let cleanup = defer::defer(|| {
        let _ = fs::remove_file(&pyenv_sync_running);
    });

    // Find all python installations (python and python@*)
    let cellar = paths::homebrew_cellar();
    let python_dirs = fs::read_dir(&cellar)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            if let Some(name) = entry.file_name().to_str() {
                name == "python" || name.starts_with("python@")
            } else {
                false
            }
        })
        .collect::<Vec<_>>();

    // For each python installation, find all versions
    for python_dir in python_dirs {
        let python_path = python_dir.path();
        if let Ok(versions) = fs::read_dir(&python_path) {
            for version_entry in versions.filter_map(|e| e.ok()) {
                let version_path = version_entry.path();
                if version_path.is_dir() {
                    link_pyenv_versions(&version_path, &pyenv_versions)?;
                }
            }
        }
    }

    // Remove broken symlinks
    if let Ok(entries) = fs::read_dir(&pyenv_versions) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_symlink() && !path.exists() {
                let _ = fs::remove_file(&path);
            }
        }
    }

    drop(cleanup);
    Ok(())
}

fn link_pyenv_versions(
    path: &Path,
    pyenv_versions: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(pyenv_versions)?;

    // Extract version from path (last component)
    let version_str = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid path")?;

    // Parse version (e.g., "3.11.0")
    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.len() < 3 {
        return Ok(());
    }

    let major: u32 = parts[0].parse()?;
    let minor: u32 = parts[1].parse()?;
    let patch: u32 = parts[2].parse()?;

    // Check if strict mode is enabled
    let strict_mode = std::env::var("HOMEBREW_ENV_SYNC_STRICT")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    let patch_range: Vec<u32> = if strict_mode {
        // Only create symlinks for the exact installed patch version
        // e.g. 3.11.0 => 3.11.0
        vec![patch]
    } else {
        // Create folder symlinks for all patch versions to the latest patch version
        // e.g. 3.11.0 => 3.11.3
        (0..=patch).collect()
    };

    for pat in &patch_range {
        let link_path = pyenv_versions.join(format!("{}.{}.{}", major, minor, pat));

        // Don't clobber existing user installations (non-symlinks)
        if link_path.exists() && !link_path.is_symlink() {
            continue;
        }

        // Remove existing symlink if present
        if link_path.exists() {
            fs::remove_file(&link_path)?;
        }

        // Create symlink
        unix_fs::symlink(path, &link_path)?;

        // Create unversioned symlinks for python3, pip3, etc.
        // This is what pyenv expects to find in ~/.pyenv/versions/___/bin
        let executables = ["python3", "pip3", "wheel3", "idle3", "pydoc3"];
        for executable in &executables {
            let major_link_path = link_path.join("bin").join(executable);

            // Don't clobber existing user installations
            if major_link_path.exists() && !major_link_path.is_symlink() {
                continue;
            }

            let executable_link_path = link_path
                .join("bin")
                .join(format!("{}.{}", executable, minor));

            // Remove existing symlink if present
            if major_link_path.exists() {
                let _ = fs::remove_file(&major_link_path);
            }

            // Create symlink (ignore errors if source doesn't exist)
            let _ = unix_fs::symlink(&executable_link_path, &major_link_path);
        }
    }

    Ok(())
}
