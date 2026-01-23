use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

use crate::paths;

pub fn execute(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let rbenv_root = std::env::var("HOMEBREW_RBENV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .expect("Cannot determine home directory")
                .join(".rbenv")
        });

    // Don't run multiple times at once
    let rbenv_sync_running = rbenv_root.join(".rbenv_sync_running");
    if rbenv_sync_running.exists() {
        return Ok(());
    }

    // Create lock file
    let rbenv_versions = rbenv_root.join("versions");
    fs::create_dir_all(&rbenv_versions)?;
    fs::write(&rbenv_sync_running, "")?;

    // Ensure we clean up the lock file
    let cleanup = defer::defer(|| {
        let _ = fs::remove_file(&rbenv_sync_running);
    });

    // Find all ruby installations (ruby and ruby@*)
    let cellar = paths::homebrew_cellar();
    let ruby_dirs = fs::read_dir(&cellar)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            if let Some(name) = entry.file_name().to_str() {
                name == "ruby" || name.starts_with("ruby@")
            } else {
                false
            }
        })
        .collect::<Vec<_>>();

    // For each ruby installation, find all versions
    for ruby_dir in ruby_dirs {
        let ruby_path = ruby_dir.path();
        if let Ok(versions) = fs::read_dir(&ruby_path) {
            for version_entry in versions.filter_map(|e| e.ok()) {
                let version_path = version_entry.path();
                if version_path.is_dir() {
                    link_rbenv_versions(&version_path, &rbenv_versions)?;
                }
            }
        }
    }

    // Remove broken symlinks
    if let Ok(entries) = fs::read_dir(&rbenv_versions) {
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

fn link_rbenv_versions(
    path: &Path,
    rbenv_versions: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(rbenv_versions)?;

    // Extract version from path (last component)
    let version_str = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid path")?;

    // Parse version (e.g., "3.2.1")
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
        // e.g. 3.2.0 => 3.2.0
        vec![patch]
    } else {
        // Create folder symlinks for all patch versions to the latest patch version
        // e.g. 3.2.0 => 3.2.1
        (0..=patch).collect()
    };

    for pat in &patch_range {
        let link_path = rbenv_versions.join(format!("{}.{}.{}", major, minor, pat));

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
    }

    Ok(())
}
