//! Self-update functionality for dotdeps
//!
//! Provides:
//! - `check_for_update()` - Query GitHub releases API for latest version
//! - `run_update()` - Download and replace current binary
//! - `maybe_notify_update()` - Periodic update notification (every 7 days)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const REPO_OWNER: &str = "davidturnbull";
const REPO_NAME: &str = "dotdeps";
const CHECK_INTERVAL_DAYS: u64 = 7;

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

/// Check GitHub for the latest release version
pub fn check_for_update() -> Result<UpdateCheckResult, Box<dyn std::error::Error>> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    // Query GitHub releases API
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        REPO_OWNER, REPO_NAME
    );

    let response = ureq::get(&url)
        .header("User-Agent", "dotdeps")
        .header("Accept", "application/vnd.github.v3+json")
        .call()?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let latest_version = json["tag_name"]
        .as_str()
        .ok_or("No tag_name in release")?
        .strip_prefix('v')
        .unwrap_or(json["tag_name"].as_str().unwrap())
        .to_string();

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

/// Perform the actual update using self_update
pub fn run_update(show_progress: bool) -> Result<self_update::Status, Box<dyn std::error::Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("dotdeps")
        .show_download_progress(show_progress)
        .show_output(show_progress)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()?
        .update()?;

    // Update state after successful update
    let state = UpdateState {
        last_check: now_timestamp(),
        latest_version: None, // Clear since we just updated
    };
    let _ = save_state(&state);

    Ok(status)
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
