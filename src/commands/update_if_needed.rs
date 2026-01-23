use crate::paths;
use std::process::Command;
use std::time::SystemTime;

const DEFAULT_AUTO_UPDATE_SECS: u64 = 86400; // 24 hours

pub fn run(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Early returns matching auto-update() function from brew.sh

    // Check if NO_AUTO_UPDATE is set
    if std::env::var("HOMEBREW_NO_AUTO_UPDATE").is_ok() {
        return Ok(());
    }

    // Check if already auto-updating
    if std::env::var("HOMEBREW_AUTO_UPDATING").is_ok() {
        return Ok(());
    }

    // Check if already checked this session
    if std::env::var("HOMEBREW_AUTO_UPDATE_CHECKED").is_ok() {
        return Ok(());
    }

    // Get auto-update interval
    let auto_update_secs = std::env::var("HOMEBREW_AUTO_UPDATE_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_AUTO_UPDATE_SECS);

    // Check FETCH_HEAD files to determine if update is needed
    let repository = paths::homebrew_repository();
    let fetch_head = repository.join(".git/FETCH_HEAD");

    let needs_update = if !fetch_head.exists() {
        // No FETCH_HEAD means we've never fetched
        true
    } else {
        // Check if FETCH_HEAD is older than auto_update_secs
        let metadata = match std::fs::metadata(&fetch_head) {
            Ok(m) => m,
            Err(_) => return Ok(()), // If we can't read it, skip update
        };

        let modified = match metadata.modified() {
            Ok(m) => m,
            Err(_) => return Ok(()), // If we can't get mtime, skip update
        };

        let now = SystemTime::now();
        let age = now
            .duration_since(modified)
            .map(|d| d.as_secs())
            .unwrap_or(u64::MAX);

        age >= auto_update_secs
    };

    if !needs_update {
        // No update needed
        return Ok(());
    }

    // Run brew update with --auto-update flag
    let brew_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return Ok(()), // If we can't find brew, skip update
    };

    let status = match Command::new(&brew_path)
        .arg("update")
        .arg("--auto-update")
        .env("HOMEBREW_AUTO_UPDATING", "1")
        .status()
    {
        Ok(s) => s,
        Err(_) => return Ok(()), // If update fails to start, just return
    };

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
