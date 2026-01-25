//! Lockfile parsing for Ruby ecosystem
//!
//! Supports finding gem versions from Gemfile.lock.
//!
//! Gemfile.lock uses a custom format (not YAML or TOML):
//! ```
//! GEM
//!   remote: https://rubygems.org/
//!   specs:
//!     rails (7.1.0)
//!       actionpack (= 7.1.0)
//!     actionpack (7.1.0)
//!       rack
//! ```
//!
//! Note: Git gems appear under GIT sections, not GEM sections.
//! This implementation currently only parses GEM sections.

use crate::cli::VersionInfo;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockfileError {
    #[error("No Gemfile.lock found. Specify version explicitly.")]
    NotFound,

    #[error(
        "Version not found for '{package}'. Specify explicitly: dotdeps add ruby:{package}@<version>"
    )]
    VersionNotFound { package: String },

    #[error("Failed to read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Find the version of a gem by searching Gemfile.lock
///
/// Searches upward from the current directory for Gemfile.lock
pub fn find_version(package: &str) -> Result<VersionInfo, LockfileError> {
    let lockfile = find_lockfile()?;
    parse_version_from_lockfile(&lockfile, package)
}

/// Find the nearest Gemfile.lock by walking up from current directory
fn find_lockfile() -> Result<PathBuf, LockfileError> {
    let cwd = std::env::current_dir().map_err(|_| LockfileError::NotFound)?;

    let mut dir = cwd.as_path();
    loop {
        let path = dir.join("Gemfile.lock");
        if path.exists() {
            return Ok(path);
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    Err(LockfileError::NotFound)
}

/// Parse version from Gemfile.lock
///
/// Gemfile.lock format has GEM sections with specs listing gem names and versions.
/// Example:
/// ```
/// GEM
///   remote: https://rubygems.org/
///   specs:
///     rails (7.1.0)
/// ```
fn parse_version_from_lockfile(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let normalized_package = normalize_gem_name(package);

    // State machine to track if we're inside a GEM section's specs
    let mut in_specs = false;

    for line in content.lines() {
        // Detect start of specs section
        if line.trim() == "specs:" {
            in_specs = true;
            continue;
        }

        // Reset state when we hit a new top-level section
        if !line.starts_with(' ') && !line.is_empty() {
            in_specs = false;
            continue;
        }

        if !in_specs {
            continue;
        }

        // Parse gem line: "    gem_name (version)"
        // Gems are indented with 4 spaces, their dependencies with 6 spaces
        if let Some(gem_info) = parse_gem_line(line)
            && normalize_gem_name(&gem_info.name) == normalized_package
        {
            return Ok(VersionInfo::Version(gem_info.version));
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

struct GemInfo {
    name: String,
    version: String,
}

/// Parse a gem line from Gemfile.lock specs section
///
/// Format: "    gem_name (version)" where version may include platform suffix
/// Examples:
///   - "    rails (7.1.0)"
///   - "    nokogiri (1.16.0-x86_64-linux)"
///   - "    bigdecimal (3.1.9-java)"
fn parse_gem_line(line: &str) -> Option<GemInfo> {
    // Gems in specs are indented with exactly 4 spaces
    // Dependencies are indented with 6 spaces, skip those
    if !line.starts_with("    ") || line.starts_with("      ") {
        return None;
    }

    let trimmed = line.trim();

    // Find the version in parentheses
    let open_paren = trimmed.find('(')?;
    let close_paren = trimmed.find(')')?;

    if open_paren >= close_paren {
        return None;
    }

    let name = trimmed[..open_paren].trim().to_string();
    let version_str = &trimmed[open_paren + 1..close_paren];

    // Handle platform-specific versions like "1.16.0-x86_64-linux"
    // We want the version without platform suffix
    let version = extract_version_without_platform(version_str);

    Some(GemInfo { name, version })
}

/// Extract version without platform suffix
///
/// Platform suffixes include: -x86_64-linux, -arm64-darwin, -java, etc.
/// Version format: major.minor.patch[.pre][-platform]
fn extract_version_without_platform(version_str: &str) -> String {
    // Common platform suffixes to strip
    let platform_patterns = [
        "-x86_64-linux",
        "-x86_64-darwin",
        "-arm64-darwin",
        "-aarch64-linux",
        "-java",
        "-mswin",
        "-mingw",
    ];

    let mut version = version_str.to_string();
    for pattern in &platform_patterns {
        if let Some(idx) = version.find(pattern) {
            version.truncate(idx);
            break;
        }
    }

    version
}

/// Normalize gem name for comparison
///
/// Gem names are case-insensitive
fn normalize_gem_name(name: &str) -> String {
    name.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_gem_name() {
        assert_eq!(normalize_gem_name("Rails"), "rails");
        assert_eq!(normalize_gem_name("ActiveRecord"), "activerecord");
        assert_eq!(normalize_gem_name("rack-test"), "rack-test");
    }

    #[test]
    fn test_parse_gem_line() {
        let gem = parse_gem_line("    rails (7.1.0)").unwrap();
        assert_eq!(gem.name, "rails");
        assert_eq!(gem.version, "7.1.0");
    }

    #[test]
    fn test_parse_gem_line_with_platform() {
        let gem = parse_gem_line("    nokogiri (1.16.0-x86_64-linux)").unwrap();
        assert_eq!(gem.name, "nokogiri");
        assert_eq!(gem.version, "1.16.0");
    }

    #[test]
    fn test_parse_gem_line_with_java_platform() {
        let gem = parse_gem_line("    bigdecimal (3.1.9-java)").unwrap();
        assert_eq!(gem.name, "bigdecimal");
        assert_eq!(gem.version, "3.1.9");
    }

    #[test]
    fn test_parse_gem_line_skips_dependencies() {
        // Dependencies are indented with 6 spaces
        assert!(parse_gem_line("      actionpack (= 7.1.0)").is_none());
    }

    #[test]
    fn test_parse_gem_line_skips_empty() {
        assert!(parse_gem_line("").is_none());
        assert!(parse_gem_line("GEM").is_none());
        assert!(parse_gem_line("  remote: https://rubygems.org/").is_none());
    }

    #[test]
    fn test_extract_version_without_platform() {
        assert_eq!(extract_version_without_platform("1.16.0"), "1.16.0");
        assert_eq!(
            extract_version_without_platform("1.16.0-x86_64-linux"),
            "1.16.0"
        );
        assert_eq!(
            extract_version_without_platform("1.16.0-arm64-darwin"),
            "1.16.0"
        );
        assert_eq!(extract_version_without_platform("3.1.9-java"), "3.1.9");
        assert_eq!(extract_version_without_platform("1.0.0.pre1"), "1.0.0.pre1");
    }

    #[test]
    fn test_parse_gemfile_lock_content() {
        let content = r#"GEM
  remote: https://rubygems.org/
  specs:
    actionmailer (7.1.0)
      actionpack (= 7.1.0)
      mail (~> 2.5)
    actionpack (7.1.0)
      rack (>= 2.2.4)
    rails (7.1.0)
      actionmailer (= 7.1.0)
      actionpack (= 7.1.0)
    rack (2.2.8)

PLATFORMS
  ruby

DEPENDENCIES
  rails (= 7.1.0)

BUNDLED WITH
   2.4.0
"#;

        // Manually test parsing logic
        let mut in_specs = false;
        let mut found_gems: Vec<(String, String)> = Vec::new();

        for line in content.lines() {
            if line.trim() == "specs:" {
                in_specs = true;
                continue;
            }

            if !line.starts_with(' ') && !line.is_empty() {
                in_specs = false;
                continue;
            }

            if in_specs && let Some(gem_info) = parse_gem_line(line) {
                found_gems.push((gem_info.name, gem_info.version));
            }
        }

        assert_eq!(found_gems.len(), 4);
        assert!(found_gems.contains(&("actionmailer".to_string(), "7.1.0".to_string())));
        assert!(found_gems.contains(&("actionpack".to_string(), "7.1.0".to_string())));
        assert!(found_gems.contains(&("rails".to_string(), "7.1.0".to_string())));
        assert!(found_gems.contains(&("rack".to_string(), "2.2.8".to_string())));
    }

    #[test]
    fn test_parse_gemfile_lock_multiple_gem_sections() {
        // Some Gemfile.lock files have multiple GEM sections (multiple remotes)
        let content = r#"GEM
  remote: https://gems.example.com/
  specs:
    private-gem (1.0.0)

GEM
  remote: https://rubygems.org/
  specs:
    rails (7.1.0)
    rack (2.2.8)

PLATFORMS
  ruby
"#;

        let mut in_specs = false;
        let mut found_gems: Vec<(String, String)> = Vec::new();

        for line in content.lines() {
            if line.trim() == "specs:" {
                in_specs = true;
                continue;
            }

            if !line.starts_with(' ') && !line.is_empty() {
                in_specs = false;
                continue;
            }

            if in_specs && let Some(gem_info) = parse_gem_line(line) {
                found_gems.push((gem_info.name, gem_info.version));
            }
        }

        assert_eq!(found_gems.len(), 3);
        assert!(found_gems.contains(&("private-gem".to_string(), "1.0.0".to_string())));
        assert!(found_gems.contains(&("rails".to_string(), "7.1.0".to_string())));
        assert!(found_gems.contains(&("rack".to_string(), "2.2.8".to_string())));
    }
}
