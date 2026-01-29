//! Lockfile parsing for Swift ecosystem
//!
//! Supports finding package versions and repository URLs from Package.resolved.
//!
//! Package.resolved has three versions:
//!
//! v1 format (older):
//! ```json
//! {
//!   "object": {
//!     "pins": [{
//!       "package": "PackageName",
//!       "repositoryURL": "https://github.com/...",
//!       "state": { "revision": "...", "version": "1.0.0" }
//!     }]
//!   },
//!   "version": 1
//! }
//! ```
//!
//! v2 format (Swift 5.6+):
//! ```json
//! {
//!   "pins": [{
//!     "identity": "package-name",
//!     "kind": "remoteSourceControl",
//!     "location": "https://github.com/...",
//!     "state": { "revision": "...", "version": "1.0.0" }
//!   }],
//!   "version": 2
//! }
//! ```
//!
//! v3 format (Xcode 15.3+ / Swift 5.10+):
//! ```json
//! {
//!   "originHash": "abc123...",
//!   "pins": [{
//!     "identity": "package-name",
//!     "kind": "remoteSourceControl",
//!     "location": "https://github.com/...",
//!     "state": { "revision": "...", "version": "1.0.0" }
//!   }],
//!   "version": 3
//! }
//! ```
//!
//! Note: v3 is identical to v2 except for the added `originHash` field at the
//! top level. The `pins` array structure is unchanged, so v2 parsing works for v3.

use crate::cli::VersionInfo;
use crate::lockfile::find_nearest_file;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockfileError {
    #[error("No Package.resolved found. Specify version explicitly.")]
    NotFound,

    #[error(
        "Version not found for '{package}'. Specify explicitly: dotdeps add swift:{package}@<version>"
    )]
    VersionNotFound { package: String },

    #[error(
        "Repository URL not found for '{package}'. Add override to ~/.config/dotdeps/config.json"
    )]
    RepoNotFound { package: String },

    #[error("Failed to read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse {path}: {source}")]
    ParseFile {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("Unsupported Package.resolved version: {version}")]
    UnsupportedVersion { version: u32 },
}

/// Find the version of a Swift package from Package.resolved
///
/// Searches upward from the current directory for Package.resolved
pub fn find_version(package: &str) -> Result<VersionInfo, LockfileError> {
    let lockfile = find_lockfile_path()?;
    parse_version_from_lockfile(&lockfile, package)
}

/// Detect the repository URL for a Swift package from Package.resolved
///
/// Swift Package.resolved contains the repository URL directly in the lockfile,
/// unlike other ecosystems that require registry API calls.
pub fn detect_repo_url(package: &str) -> Result<String, LockfileError> {
    let lockfile = find_lockfile_path()?;
    parse_repo_url_from_lockfile(&lockfile, package)
}

/// Find the nearest Package.resolved by walking up from current directory
///
/// Checks both direct Package.resolved and Xcode project paths:
/// - Package.resolved (Swift Package)
/// - *.xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved (Xcode)
/// - *.xcworkspace/xcshareddata/swiftpm/Package.resolved (Xcode workspace)
pub fn find_lockfile_path() -> Result<PathBuf, LockfileError> {
    if let Some(path) = find_nearest_file(&["Package.resolved"]) {
        return Ok(path);
    }

    let cwd = std::env::current_dir().map_err(|_| LockfileError::NotFound)?;
    let mut dir = cwd.as_path();
    loop {
        if let Some(xcode_path) = find_xcode_package_resolved(dir) {
            return Ok(xcode_path);
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    Err(LockfileError::NotFound)
}

/// List direct dependencies from Package.resolved
pub fn list_direct_dependencies(path: &Path) -> Result<Vec<String>, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let version_check: VersionCheck =
        serde_json::from_str(&content).map_err(|source| LockfileError::ParseFile {
            path: path.to_path_buf(),
            source,
        })?;

    let mut deps = Vec::new();

    match version_check.version {
        1 => {
            let resolved: PackageResolvedV1 =
                serde_json::from_str(&content).map_err(|source| LockfileError::ParseFile {
                    path: path.to_path_buf(),
                    source,
                })?;

            for pin in resolved.object.pins {
                deps.push(pin.package);
            }
        }
        2 | 3 => {
            let resolved: PackageResolvedV2 =
                serde_json::from_str(&content).map_err(|source| LockfileError::ParseFile {
                    path: path.to_path_buf(),
                    source,
                })?;

            for pin in resolved.pins {
                if pin.kind != "remoteSourceControl" {
                    continue;
                }
                deps.push(pin.identity);
            }
        }
        v => return Err(LockfileError::UnsupportedVersion { version: v }),
    }

    let mut unique = deps
        .into_iter()
        .map(|d| normalize_package_name(&d))
        .collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    Ok(unique)
}

/// Find Package.resolved inside Xcode project or workspace
fn find_xcode_package_resolved(dir: &Path) -> Option<PathBuf> {
    // Look for .xcodeproj directories
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Check .xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved
                if name.ends_with(".xcodeproj") {
                    let resolved = path
                        .join("project.xcworkspace")
                        .join("xcshareddata")
                        .join("swiftpm")
                        .join("Package.resolved");
                    if resolved.exists() {
                        return Some(resolved);
                    }
                }

                // Check .xcworkspace/xcshareddata/swiftpm/Package.resolved
                if name.ends_with(".xcworkspace") {
                    let resolved = path
                        .join("xcshareddata")
                        .join("swiftpm")
                        .join("Package.resolved");
                    if resolved.exists() {
                        return Some(resolved);
                    }
                }
            }
        }
    }

    None
}

/// Parse version from Package.resolved (handles both v1 and v2 formats)
fn parse_version_from_lockfile(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let version_check: VersionCheck =
        serde_json::from_str(&content).map_err(|source| LockfileError::ParseFile {
            path: path.to_path_buf(),
            source,
        })?;

    let normalized_package = normalize_package_name(package);

    match version_check.version {
        1 => {
            let resolved: PackageResolvedV1 =
                serde_json::from_str(&content).map_err(|source| LockfileError::ParseFile {
                    path: path.to_path_buf(),
                    source,
                })?;

            for pin in resolved.object.pins {
                if (normalize_package_name(&pin.package) == normalized_package
                    || matches_repo_identity(&pin.repository_url, &normalized_package))
                    && let Some(version) = pin.state.version
                {
                    return Ok(VersionInfo::Version(strip_v_prefix(&version)));
                }
            }
        }
        2 | 3 => {
            let resolved: PackageResolvedV2 =
                serde_json::from_str(&content).map_err(|source| LockfileError::ParseFile {
                    path: path.to_path_buf(),
                    source,
                })?;

            for pin in resolved.pins {
                if (normalize_package_name(&pin.identity) == normalized_package
                    || matches_repo_identity(&pin.location, &normalized_package))
                    && let Some(version) = pin.state.version
                {
                    return Ok(VersionInfo::Version(strip_v_prefix(&version)));
                }
            }
        }
        v => return Err(LockfileError::UnsupportedVersion { version: v }),
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Parse repository URL from Package.resolved
fn parse_repo_url_from_lockfile(path: &Path, package: &str) -> Result<String, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let version_check: VersionCheck =
        serde_json::from_str(&content).map_err(|source| LockfileError::ParseFile {
            path: path.to_path_buf(),
            source,
        })?;

    let normalized_package = normalize_package_name(package);

    match version_check.version {
        1 => {
            let resolved: PackageResolvedV1 =
                serde_json::from_str(&content).map_err(|source| LockfileError::ParseFile {
                    path: path.to_path_buf(),
                    source,
                })?;

            for pin in resolved.object.pins {
                if normalize_package_name(&pin.package) == normalized_package
                    || matches_repo_identity(&pin.repository_url, &normalized_package)
                {
                    return Ok(normalize_repo_url(&pin.repository_url));
                }
            }
        }
        2 | 3 => {
            let resolved: PackageResolvedV2 =
                serde_json::from_str(&content).map_err(|source| LockfileError::ParseFile {
                    path: path.to_path_buf(),
                    source,
                })?;

            for pin in resolved.pins {
                // Only consider remote source control (not local packages)
                if pin.kind != "remoteSourceControl" {
                    continue;
                }

                if normalize_package_name(&pin.identity) == normalized_package
                    || matches_repo_identity(&pin.location, &normalized_package)
                {
                    return Ok(normalize_repo_url(&pin.location));
                }
            }
        }
        v => return Err(LockfileError::UnsupportedVersion { version: v }),
    }

    Err(LockfileError::RepoNotFound {
        package: package.to_string(),
    })
}

// JSON structure for version detection
#[derive(Deserialize)]
struct VersionCheck {
    version: u32,
}

// v1 format structures
#[derive(Deserialize)]
struct PackageResolvedV1 {
    object: ObjectV1,
}

#[derive(Deserialize)]
struct ObjectV1 {
    pins: Vec<PinV1>,
}

#[derive(Deserialize)]
struct PinV1 {
    package: String,
    #[serde(rename = "repositoryURL")]
    repository_url: String,
    state: StateV1,
}

#[derive(Deserialize)]
struct StateV1 {
    version: Option<String>,
}

// v2 format structures
#[derive(Deserialize)]
struct PackageResolvedV2 {
    pins: Vec<PinV2>,
}

#[derive(Deserialize)]
struct PinV2 {
    identity: String,
    kind: String,
    location: String,
    state: StateV2,
}

#[derive(Deserialize)]
struct StateV2 {
    version: Option<String>,
}

/// Normalize Swift package name for comparison
///
/// Swift package identities are case-insensitive and typically lowercase
fn normalize_package_name(name: &str) -> String {
    name.to_lowercase()
}

/// Check if a repository URL matches a package identity
///
/// For example, "https://github.com/apple/swift-argument-parser" matches "swift-argument-parser"
fn matches_repo_identity(repo_url: &str, package: &str) -> bool {
    // Extract repo name from URL
    let repo_name = repo_url
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    repo_name == package
}

/// Strip 'v' prefix from version if present
fn strip_v_prefix(version: &str) -> String {
    version.strip_prefix('v').unwrap_or(version).to_string()
}

/// Normalize repository URL for git cloning
///
/// Ensures URL ends with .git for consistency
fn normalize_repo_url(url: &str) -> String {
    if url.ends_with(".git") {
        url.to_string()
    } else {
        format!("{}.git", url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_file(filename: &str, content: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("dotdeps_swift_test_{}", nanos));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(filename);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_normalize_package_name() {
        assert_eq!(
            normalize_package_name("SwiftArgumentParser"),
            "swiftargumentparser"
        );
        assert_eq!(
            normalize_package_name("swift-argument-parser"),
            "swift-argument-parser"
        );
    }

    #[test]
    fn test_matches_repo_identity() {
        assert!(matches_repo_identity(
            "https://github.com/apple/swift-argument-parser",
            "swift-argument-parser"
        ));
        assert!(matches_repo_identity(
            "https://github.com/apple/swift-argument-parser.git",
            "swift-argument-parser"
        ));
        assert!(!matches_repo_identity(
            "https://github.com/apple/swift-nio",
            "swift-argument-parser"
        ));
    }

    #[test]
    fn test_strip_v_prefix() {
        assert_eq!(strip_v_prefix("v1.0.0"), "1.0.0");
        assert_eq!(strip_v_prefix("1.0.0"), "1.0.0");
        assert_eq!(strip_v_prefix("v2.3.4-beta"), "2.3.4-beta");
    }

    #[test]
    fn test_normalize_repo_url() {
        assert_eq!(
            normalize_repo_url("https://github.com/apple/swift-argument-parser"),
            "https://github.com/apple/swift-argument-parser.git"
        );
        assert_eq!(
            normalize_repo_url("https://github.com/apple/swift-argument-parser.git"),
            "https://github.com/apple/swift-argument-parser.git"
        );
    }

    #[test]
    fn test_parse_v1_format() {
        let content = r#"{
  "object": {
    "pins": [
      {
        "package": "swift-argument-parser",
        "repositoryURL": "https://github.com/apple/swift-argument-parser",
        "state": {
          "branch": null,
          "revision": "41982a3656a71c768319979febd796c6fd111d5c",
          "version": "1.5.0"
        }
      }
    ]
  },
  "version": 1
}"#;

        let resolved: PackageResolvedV1 = serde_json::from_str(content).unwrap();
        assert_eq!(resolved.object.pins.len(), 1);
        assert_eq!(resolved.object.pins[0].package, "swift-argument-parser");
        assert_eq!(
            resolved.object.pins[0].repository_url,
            "https://github.com/apple/swift-argument-parser"
        );
        assert_eq!(
            resolved.object.pins[0].state.version,
            Some("1.5.0".to_string())
        );
    }

    #[test]
    fn test_parse_v2_format() {
        let content = r#"{
  "pins": [
    {
      "identity": "swift-argument-parser",
      "kind": "remoteSourceControl",
      "location": "https://github.com/apple/swift-argument-parser",
      "state": {
        "revision": "41982a3656a71c768319979febd796c6fd111d5c",
        "version": "1.5.0"
      }
    }
  ],
  "version": 2
}"#;

        let resolved: PackageResolvedV2 = serde_json::from_str(content).unwrap();
        assert_eq!(resolved.pins.len(), 1);
        assert_eq!(resolved.pins[0].identity, "swift-argument-parser");
        assert_eq!(resolved.pins[0].kind, "remoteSourceControl");
        assert_eq!(
            resolved.pins[0].location,
            "https://github.com/apple/swift-argument-parser"
        );
        assert_eq!(resolved.pins[0].state.version, Some("1.5.0".to_string()));
    }

    #[test]
    fn test_parse_v2_with_branch() {
        // Some pins may have branch instead of version
        let content = r#"{
  "pins": [
    {
      "identity": "swift-nio",
      "kind": "remoteSourceControl",
      "location": "https://github.com/apple/swift-nio",
      "state": {
        "branch": "main",
        "revision": "abc123"
      }
    }
  ],
  "version": 2
}"#;

        let resolved: PackageResolvedV2 = serde_json::from_str(content).unwrap();
        assert_eq!(resolved.pins.len(), 1);
        assert_eq!(resolved.pins[0].state.version, None);
    }

    #[test]
    fn test_version_check() {
        let v1_content = r#"{"object": {"pins": []}, "version": 1}"#;
        let v2_content = r#"{"pins": [], "version": 2}"#;

        let v1: VersionCheck = serde_json::from_str(v1_content).unwrap();
        let v2: VersionCheck = serde_json::from_str(v2_content).unwrap();

        assert_eq!(v1.version, 1);
        assert_eq!(v2.version, 2);
    }

    #[test]
    fn test_list_direct_dependencies_v2() {
        let content = r#"{
  "pins": [
    {
      "identity": "swift-argument-parser",
      "kind": "remoteSourceControl",
      "location": "https://github.com/apple/swift-argument-parser",
      "state": {
        "revision": "abc",
        "version": "1.5.0"
      }
    },
    {
      "identity": "local-pkg",
      "kind": "localSourceControl",
      "location": "../local",
      "state": {
        "revision": "def"
      }
    }
  ],
  "version": 2
}"#;
        let path = write_temp_file("Package.resolved", content);
        let deps = list_direct_dependencies(&path).unwrap();
        assert!(deps.contains(&"swift-argument-parser".to_string()));
        assert!(!deps.contains(&"local-pkg".to_string()));
    }

    #[test]
    fn test_parse_v3_format() {
        // v3 format introduced in Xcode 15.3 / Swift 5.10
        // Identical to v2 except for the originHash field
        let content = r#"{
  "originHash": "d1f87b2c9e4a5f3c8b7d6e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b",
  "pins": [
    {
      "identity": "swift-argument-parser",
      "kind": "remoteSourceControl",
      "location": "https://github.com/apple/swift-argument-parser",
      "state": {
        "revision": "41982a3656a71c768319979febd796c6fd111d5c",
        "version": "1.5.0"
      }
    },
    {
      "identity": "swift-nio",
      "kind": "remoteSourceControl",
      "location": "https://github.com/apple/swift-nio.git",
      "state": {
        "revision": "abc123def456",
        "version": "2.58.0"
      }
    }
  ],
  "version": 3
}"#;

        // Test that v3 can be parsed as v2 structure (ignoring originHash)
        let resolved: PackageResolvedV2 = serde_json::from_str(content).unwrap();
        assert_eq!(resolved.pins.len(), 2);
        assert_eq!(resolved.pins[0].identity, "swift-argument-parser");
        assert_eq!(resolved.pins[0].kind, "remoteSourceControl");
        assert_eq!(resolved.pins[0].state.version, Some("1.5.0".to_string()));
        assert_eq!(resolved.pins[1].identity, "swift-nio");
        assert_eq!(resolved.pins[1].state.version, Some("2.58.0".to_string()));

        // Test version check
        let version_check: VersionCheck = serde_json::from_str(content).unwrap();
        assert_eq!(version_check.version, 3);
    }

    #[test]
    fn test_list_direct_dependencies_v3() {
        let content = r#"{
  "originHash": "abc123",
  "pins": [
    {
      "identity": "swift-argument-parser",
      "kind": "remoteSourceControl",
      "location": "https://github.com/apple/swift-argument-parser",
      "state": {
        "revision": "abc",
        "version": "1.5.0"
      }
    },
    {
      "identity": "local-pkg",
      "kind": "localSourceControl",
      "location": "../local",
      "state": {
        "revision": "def"
      }
    }
  ],
  "version": 3
}"#;
        let path = write_temp_file("Package.resolved", content);
        let deps = list_direct_dependencies(&path).unwrap();
        assert!(deps.contains(&"swift-argument-parser".to_string()));
        assert!(!deps.contains(&"local-pkg".to_string()));
    }
}
