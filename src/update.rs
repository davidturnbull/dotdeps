//! Self-update functionality for dotdeps
//!
//! Provides:
//! - `check_for_update()` - Discover latest version via GitHub release redirect
//! - `run_update()` - Download and replace current binary (no GitHub API)
//! - `maybe_notify_update()` - Periodic update notification (every 7 days)

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ureq::ResponseExt;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const REPO_OWNER: &str = "davidturnbull";
const REPO_NAME: &str = "dotdeps";
const CHECK_INTERVAL_DAYS: u64 = 7;
const GITHUB_BASE_URL: &str = "https://github.com";

/// State file for tracking update checks
#[derive(Debug, Serialize, Deserialize, Default)]
struct UpdateState {
    /// Unix timestamp of last update check
    last_check: u64,
    /// Latest version found during last check (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_version: Option<String>,
}

/// Result of checking for updates
#[derive(Debug)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
}

#[derive(Debug)]
pub enum UpdateStatus {
    UpToDate(String),
    Updated(String),
}

/// Get the path to the update state file
fn state_file_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("dotdeps").join("update-check.json"))
}

/// Load update state from disk
fn load_state() -> UpdateState {
    let Some(path) = state_file_path() else {
        return UpdateState::default();
    };

    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save update state to disk
fn save_state(state: &UpdateState) -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = state_file_path() else {
        return Ok(());
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(state)?;
    fs::write(&path, json)?;
    Ok(())
}

/// Get current Unix timestamp
fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Resolve the latest GitHub release tag by following the /releases/latest redirect.
fn latest_release_tag() -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "{}/{}/{}/releases/latest",
        GITHUB_BASE_URL, REPO_OWNER, REPO_NAME
    );
    let response = ureq::get(&url).header("User-Agent", "dotdeps").call()?;
    let final_url = response.get_uri().to_string();

    let marker = "/releases/tag/";
    let Some(tag_part) = final_url.split(marker).nth(1) else {
        return Err(format!("Failed to parse release tag from {}", final_url).into());
    };

    let tag = tag_part.split(['?', '#']).next().unwrap_or(tag_part).trim();

    if tag.is_empty() {
        return Err("Failed to parse release tag from redirect".into());
    }

    Ok(tag.to_string())
}

fn latest_release_version() -> Result<String, Box<dyn std::error::Error>> {
    let tag = latest_release_tag()?;
    Ok(tag.strip_prefix('v').unwrap_or(&tag).to_string())
}

/// Check GitHub for the latest release version
pub fn check_for_update() -> Result<UpdateCheckResult, Box<dyn std::error::Error>> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let latest_version = latest_release_version()?;
    let update_available = is_newer_version(&latest_version, &current_version);

    // Update state
    let state = UpdateState {
        last_check: now_timestamp(),
        latest_version: Some(latest_version.clone()),
    };
    let _ = save_state(&state);

    Ok(UpdateCheckResult {
        current_version,
        latest_version,
        update_available,
    })
}

/// Compare versions to see if `latest` is newer than `current`
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_version =
        |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse::<u32>().ok()).collect() };

    let latest_parts = parse_version(latest);
    let current_parts = parse_version(current);

    for (l, c) in latest_parts.iter().zip(current_parts.iter()) {
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }

    // If all compared parts are equal, newer if latest has more parts
    latest_parts.len() > current_parts.len()
}

fn current_target_triple() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("aarch64-apple-darwin")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("x86_64-apple-darwin")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some("aarch64-unknown-linux-gnu")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("x86_64-unknown-linux-gnu")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("x86_64-pc-windows-msvc")
    } else {
        None
    }
}

fn archive_extension(target: &str) -> &'static str {
    if target.ends_with("windows-msvc") {
        "zip"
    } else {
        "tar.xz"
    }
}

fn download_url(version: &str, target: &str) -> String {
    let ext = archive_extension(target);
    format!(
        "{}/{}/{}/releases/download/v{}/dotdeps-{}.{}",
        GITHUB_BASE_URL, REPO_OWNER, REPO_NAME, version, target, ext
    )
}

fn download_to_path(
    url: &str,
    dest: &Path,
    show_progress: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if show_progress {
        eprintln!("Downloading {}", url);
    }

    let response = ureq::get(url).header("User-Agent", "dotdeps").call()?;
    let mut reader = response.into_body().into_reader();
    let mut out = fs::File::create(dest)?;
    io::copy(&mut reader, &mut out)?;
    Ok(())
}

fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::open(archive_path)?;
    if archive_path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        #[cfg(windows)]
        {
            let mut archive = zip::ZipArchive::new(file)?;
            archive.extract(dest_dir)?;
            return Ok(());
        }
        #[cfg(not(windows))]
        {
            return Err("zip archives are only supported on Windows builds".into());
        }
    }

    let decompressor = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decompressor);
    archive.unpack(dest_dir)?;
    Ok(())
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "dotdeps.exe"
    } else {
        "dotdeps"
    }
}

fn extracted_binary_path(base_dir: &Path, target: &str) -> PathBuf {
    base_dir
        .join(format!("dotdeps-{}", target))
        .join(binary_name())
}

#[cfg(windows)]
fn spawn_windows_replacer(staged: &Path, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let script_path = std::env::temp_dir().join(format!("dotdeps-update-{}.bat", now_timestamp()));
    let script = format!(
        "@echo off\r\n\
        :loop\r\n\
        timeout /t 1 /nobreak >nul\r\n\
        move /y \"{}\" \"{}\" >nul 2>&1\r\n\
        if errorlevel 1 goto loop\r\n\
        del \"%~f0\"\r\n",
        staged.display(),
        target.display()
    );
    fs::write(&script_path, script)?;

    let cmd = format!("start \"\" /B \"{}\"", script_path.display());
    Command::new("cmd").args(["/C", &cmd]).spawn()?;
    Ok(())
}

fn replace_current_binary(new_binary: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let current = std::env::current_exe()?;
    let install_dir = current
        .parent()
        .ok_or("Failed to determine install directory")?;

    let staged_name = format!(
        "{}.new",
        current
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("dotdeps")
    );
    let staged_path = install_dir.join(staged_name);
    fs::copy(new_binary, &staged_path)?;

    #[cfg(unix)]
    {
        fs::set_permissions(&staged_path, fs::Permissions::from_mode(0o755))?;
        fs::rename(&staged_path, &current)?;
    }

    #[cfg(windows)]
    {
        spawn_windows_replacer(&staged_path, &current)?;
    }

    Ok(())
}

/// Perform the actual update using GitHub release assets (no API)
pub fn run_update(show_progress: bool) -> Result<UpdateStatus, Box<dyn std::error::Error>> {
    let result = check_for_update()?;
    if !result.update_available {
        return Ok(UpdateStatus::UpToDate(result.current_version));
    }

    let target = current_target_triple().ok_or("Unsupported platform for updates")?;
    let url = download_url(&result.latest_version, target);
    let temp_dir = std::env::temp_dir().join(format!("dotdeps-update-{}", now_timestamp()));
    fs::create_dir_all(&temp_dir)?;

    let archive_name = format!("dotdeps-{}.{}", target, archive_extension(target));
    let archive_path = temp_dir.join(archive_name);
    download_to_path(&url, &archive_path, show_progress)?;
    extract_archive(&archive_path, &temp_dir)?;

    let extracted = extracted_binary_path(&temp_dir, target);
    if !extracted.exists() {
        return Err(format!(
            "Update archive did not contain expected binary at {}",
            extracted.display()
        )
        .into());
    }

    if show_progress {
        eprintln!("Installing update...");
    }
    replace_current_binary(&extracted)?;

    // Update state after successful update
    let state = UpdateState {
        last_check: now_timestamp(),
        latest_version: None, // Clear since we just updated
    };
    let _ = save_state(&state);

    let _ = fs::remove_dir_all(&temp_dir);

    Ok(UpdateStatus::Updated(result.latest_version))
}

/// Check for updates if it's been more than 7 days since last check.
/// Returns Some(message) if an update is available and should be shown.
pub fn maybe_notify_update() -> Option<String> {
    let state = load_state();
    let now = now_timestamp();
    let interval_secs = CHECK_INTERVAL_DAYS * 24 * 60 * 60;

    // Check if we're within the check interval
    if state.last_check > 0 && now - state.last_check < interval_secs {
        // Within interval - use cached result if available
        if let Some(ref latest) = state.latest_version {
            let current = env!("CARGO_PKG_VERSION");
            if is_newer_version(latest, current) {
                return Some(format!(
                    "A new version of dotdeps is available: {} (current: {}). Run 'dotdeps update' to install.",
                    latest, current
                ));
            }
        }
        return None;
    }

    // Need to check - do it in a way that won't block on failure
    match check_for_update() {
        Ok(result) if result.update_available => Some(format!(
            "A new version of dotdeps is available: {} (current: {}). Run 'dotdeps update' to install.",
            result.latest_version, result.current_version
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.2.0", "0.1.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.2.0"));
        assert!(is_newer_version("0.1.0.1", "0.1.0"));
    }
}
