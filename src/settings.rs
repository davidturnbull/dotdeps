use crate::paths::homebrew_repository;
use std::process::Command;

/// Read a Homebrew setting from the git config.
pub fn read(setting: &str) -> Option<String> {
    let repo = homebrew_repository();
    let config_path = repo.join(".git/config");

    if !config_path.exists() {
        return None;
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--get")
        .arg(format!("homebrew.{}", setting))
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();

    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Write a Homebrew setting to the git config.
pub fn write(setting: &str, value: &str) -> std::io::Result<()> {
    let repo = homebrew_repository();
    let config_path = repo.join(".git/config");

    if !config_path.exists() {
        return Ok(());
    }

    // Check if the value is already set
    if let Some(current) = read(setting)
        && current == value
    {
        return Ok(());
    }

    let status = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--replace-all")
        .arg(format!("homebrew.{}", setting))
        .arg(value)
        .status()?;

    if !status.success() {
        return Err(std::io::Error::other("Failed to write git config"));
    }

    Ok(())
}

/// Delete a Homebrew setting from the git config.
pub fn delete(setting: &str) -> std::io::Result<()> {
    let repo = homebrew_repository();
    let config_path = repo.join(".git/config");

    if !config_path.exists() {
        return Ok(());
    }

    // Check if the setting exists
    if read(setting).is_none() {
        return Ok(());
    }

    let status = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("config")
        .arg("--unset-all")
        .arg(format!("homebrew.{}", setting))
        .status()?;

    if !status.success() {
        return Err(std::io::Error::other("Failed to delete git config"));
    }

    Ok(())
}

/// Check if analytics is disabled.
pub fn analytics_disabled() -> bool {
    // Check environment variable first
    if std::env::var("HOMEBREW_NO_ANALYTICS").is_ok() {
        return true;
    }

    // Check git config
    read("analyticsdisabled")
        .map(|v| v == "true")
        .unwrap_or(false)
}
