//! Tap module for managing Homebrew taps.
//!
//! Taps are third-party repositories that extend Homebrew with additional formulae and casks.
//! Official taps include homebrew/core and homebrew/cask.

use std::fs;
use std::path::PathBuf;

use crate::paths;

/// Represents a Homebrew tap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tap {
    /// The tap user/organization (e.g., "homebrew", "user")
    pub user: String,
    /// The tap repository name without "homebrew-" prefix (e.g., "core", "tap")
    pub repo: String,
}

impl Tap {
    /// Create a new Tap from user and repo.
    pub fn new(user: impl Into<String>, repo: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            repo: repo.into(),
        }
    }

    /// Parse a tap name like "user/repo" or "user/homebrew-repo".
    /// Returns None if the format is invalid.
    #[allow(dead_code)]
    pub fn parse(name: &str) -> Option<Self> {
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() != 2 {
            return None;
        }

        let user = parts[0].to_string();
        let mut repo = parts[1].to_string();

        // Strip "homebrew-" prefix if present
        if repo.starts_with("homebrew-") {
            repo = repo.trim_start_matches("homebrew-").to_string();
        }

        Some(Self { user, repo })
    }

    /// Get the full tap name (e.g., "homebrew/core").
    pub fn name(&self) -> String {
        format!("{}/{}", self.user, self.repo)
    }

    /// Get the full repository name with "homebrew-" prefix (e.g., "homebrew-core").
    #[allow(dead_code)]
    pub fn full_repo_name(&self) -> String {
        format!("homebrew-{}", self.repo)
    }

    /// Get the path to this tap's directory.
    #[allow(dead_code)]
    pub fn path(&self) -> PathBuf {
        paths::homebrew_taps()
            .join(&self.user)
            .join(self.full_repo_name())
    }

    /// Check if this tap is installed.
    #[allow(dead_code)]
    pub fn is_installed(&self) -> bool {
        self.path().exists()
    }

    /// Check if this is an official Homebrew tap (homebrew/core or homebrew/cask).
    #[allow(dead_code)]
    pub fn is_official(&self) -> bool {
        self.user == "homebrew" && (self.repo == "core" || self.repo == "cask")
    }
}

/// List all installed taps.
pub fn list_installed() -> Vec<Tap> {
    let taps_dir = paths::homebrew_taps();

    if !taps_dir.exists() {
        return Vec::new();
    }

    let mut taps = Vec::new();

    // Iterate over user directories
    if let Ok(user_entries) = fs::read_dir(&taps_dir) {
        for user_entry in user_entries.flatten() {
            let user_path = user_entry.path();
            if !user_path.is_dir() {
                continue;
            }

            let user_name = user_entry.file_name();
            let user_str = user_name.to_string_lossy();

            // Skip hidden directories
            if user_str.starts_with('.') {
                continue;
            }

            // Iterate over tap directories within this user
            if let Ok(tap_entries) = fs::read_dir(&user_path) {
                for tap_entry in tap_entries.flatten() {
                    let tap_path = tap_entry.path();
                    if !tap_path.is_dir() {
                        continue;
                    }

                    let tap_name = tap_entry.file_name();
                    let tap_str = tap_name.to_string_lossy();

                    // Skip hidden directories
                    if tap_str.starts_with('.') {
                        continue;
                    }

                    // Extract repo name (strip "homebrew-" prefix)
                    let repo = if tap_str.starts_with("homebrew-") {
                        tap_str.trim_start_matches("homebrew-").to_string()
                    } else {
                        tap_str.to_string()
                    };

                    taps.push(Tap::new(user_str.to_string(), repo));
                }
            }
        }
    }

    // Sort taps by name for consistent output
    taps.sort_by_key(|a| a.name());

    taps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tap_parse() {
        let tap = Tap::parse("homebrew/core").unwrap();
        assert_eq!(tap.user, "homebrew");
        assert_eq!(tap.repo, "core");
        assert_eq!(tap.name(), "homebrew/core");
        assert_eq!(tap.full_repo_name(), "homebrew-core");
    }

    #[test]
    fn test_tap_parse_with_prefix() {
        let tap = Tap::parse("user/homebrew-tap").unwrap();
        assert_eq!(tap.user, "user");
        assert_eq!(tap.repo, "tap");
        assert_eq!(tap.name(), "user/tap");
        assert_eq!(tap.full_repo_name(), "homebrew-tap");
    }

    #[test]
    fn test_tap_is_official() {
        assert!(Tap::new("homebrew", "core").is_official());
        assert!(Tap::new("homebrew", "cask").is_official());
        assert!(!Tap::new("homebrew", "bundle").is_official());
        assert!(!Tap::new("user", "core").is_official());
    }
}
