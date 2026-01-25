//! Lockfile parsing for Node.js ecosystems
//!
//! Supports finding package versions from:
//! - pnpm-lock.yaml (YAML)
//! - yarn.lock (custom format, not YAML)
//! - package-lock.json (JSON)
//!
//! Lockfile priority order: pnpm-lock.yaml > yarn.lock > package-lock.json
//!
//! Also detects special dependency types:
//! - Git dependencies: URLs starting with `git+`, `git://`, or containing `#commit`
//! - Local path dependencies: `link:`, `file:` URLs

use crate::cli::VersionInfo;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockfileError {
    #[error("No lockfile found. Specify version explicitly.")]
    NotFound,

    #[error(
        "Version not found for '{package}'. Specify explicitly: dotdeps add node:{package}@<version>"
    )]
    VersionNotFound { package: String },

    #[error("Failed to read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse {path}: {details}")]
    Parse { path: PathBuf, details: String },
}

/// Find the version of a package by searching lockfiles
///
/// Searches upward from the current directory for lockfiles in priority order:
/// pnpm-lock.yaml > yarn.lock > package-lock.json
///
/// Returns `VersionInfo` which can be:
/// - `Version(string)` for regular registry packages
/// - `Git { url, commit }` for git dependencies
/// - `LocalPath { path }` for local link/file dependencies
pub fn find_version(package: &str) -> Result<VersionInfo, LockfileError> {
    let lockfile = find_lockfile()?;
    parse_version_from_lockfile(&lockfile, package)
}

/// Lockfile types in priority order
#[derive(Debug, Clone, Copy)]
enum LockfileType {
    Pnpm,
    Yarn,
    Npm,
}

impl LockfileType {
    fn filename(&self) -> &'static str {
        match self {
            LockfileType::Pnpm => "pnpm-lock.yaml",
            LockfileType::Yarn => "yarn.lock",
            LockfileType::Npm => "package-lock.json",
        }
    }

    fn priority_order() -> &'static [LockfileType] {
        &[LockfileType::Pnpm, LockfileType::Yarn, LockfileType::Npm]
    }
}

/// Find the nearest lockfile by walking up from current directory
fn find_lockfile() -> Result<PathBuf, LockfileError> {
    let cwd = std::env::current_dir().map_err(|_| LockfileError::NotFound)?;

    let mut dir = cwd.as_path();
    loop {
        // Check for each lockfile type in priority order
        for lockfile_type in LockfileType::priority_order() {
            let path = dir.join(lockfile_type.filename());
            if path.exists() {
                return Ok(path);
            }
        }

        // Move to parent directory
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    Err(LockfileError::NotFound)
}

/// Parse version from a lockfile
fn parse_version_from_lockfile(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    match filename {
        "pnpm-lock.yaml" => parse_pnpm_lock(path, package),
        "yarn.lock" => parse_yarn_lock(path, package),
        "package-lock.json" => parse_package_lock(path, package),
        _ => Err(LockfileError::Parse {
            path: path.to_path_buf(),
            details: format!("Unknown lockfile type: {}", filename),
        }),
    }
}

// === pnpm-lock.yaml Parsing ===

/// Structure for pnpm-lock.yaml (lockfileVersion 9.0)
///
/// pnpm uses a YAML format with packages defined under `packages:` key.
/// Each package key is in the format `name@version` with resolution info.
/// We use serde_json::Value for package values since we only need the keys.
#[derive(Deserialize)]
struct PnpmLockfile {
    packages: Option<HashMap<String, serde_json::Value>>,
}

/// Parse version from pnpm-lock.yaml
fn parse_pnpm_lock(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let lockfile: PnpmLockfile =
        serde_yml::from_str(&content).map_err(|e| LockfileError::Parse {
            path: path.to_path_buf(),
            details: e.to_string(),
        })?;

    let packages = lockfile.packages.unwrap_or_default();
    let normalized_package = normalize_node_name(package);

    // pnpm packages keys are in format: `name@version` or `@scope/name@version`
    for key in packages.keys() {
        if let Some((name, version)) = parse_pnpm_package_key(key)
            && normalize_node_name(&name) == normalized_package
        {
            return Ok(parse_node_version_string(&version));
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Parse pnpm package key into (name, version)
///
/// Keys are in format: `name@version` or `@scope/name@version`
fn parse_pnpm_package_key(key: &str) -> Option<(String, String)> {
    // Handle scoped packages (@scope/name@version)
    if key.starts_with('@') {
        // Find the second @ which separates name from version
        let after_scope = key.find('/')? + 1;
        let version_sep = key[after_scope..].find('@')? + after_scope;
        let name = key[..version_sep].to_string();
        let version = key[version_sep + 1..].to_string();
        Some((name, version))
    } else {
        // Regular package (name@version)
        let at_idx = key.find('@')?;
        let name = key[..at_idx].to_string();
        let version = key[at_idx + 1..].to_string();
        Some((name, version))
    }
}

// === yarn.lock Parsing ===

/// Parse version from yarn.lock
///
/// yarn.lock uses a custom format (not standard YAML). Each entry looks like:
/// ```
/// packagename@^version-range, packagename@~other-range:
///   version "resolved-version"
///   resolved "https://..."
///   integrity sha512-...
/// ```
fn parse_yarn_lock(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let normalized_package = normalize_node_name(package);

    // Parse the custom yarn.lock format line by line
    let mut current_packages: Vec<String> = Vec::new();
    let mut in_entry = false;
    let mut current_resolved: Option<String> = None;

    for line in content.lines() {
        let line = line.trim_end();

        // Entry header line: ends with colon, contains package names
        if !line.starts_with(' ') && !line.starts_with('#') && line.ends_with(':') {
            // Parse the package names from the header
            current_packages = parse_yarn_lock_header(line);
            in_entry = true;
            current_resolved = None;
            continue;
        }

        // Inside an entry, capture resolved URL for git detection
        if in_entry && line.trim().starts_with("resolved ") {
            current_resolved = extract_yarn_resolved(line);
        }

        // Inside an entry, look for version line
        if in_entry && line.trim().starts_with("version ") {
            let version = extract_yarn_version(line)?;

            // Check if any of the current packages match
            for pkg_spec in &current_packages {
                if let Some(name) = extract_package_name_from_yarn_spec(pkg_spec)
                    && normalize_node_name(&name) == normalized_package
                {
                    // Check if this is a git dependency by looking at the resolved URL
                    if let Some(resolved) = &current_resolved
                        && let Some(git_info) = parse_git_url(resolved)
                    {
                        return Ok(git_info);
                    }
                    return Ok(VersionInfo::Version(version));
                }
            }

            // Reset for next entry
            in_entry = false;
            current_packages.clear();
            current_resolved = None;
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Parse yarn.lock header line into package specifications
///
/// Header format: `pkg@^1.0.0, pkg@~2.0.0:` or `"@scope/pkg@^1.0.0":`
fn parse_yarn_lock_header(line: &str) -> Vec<String> {
    // Remove trailing colon
    let line = line.strip_suffix(':').unwrap_or(line);

    // Split by comma and clean up each part
    line.split(',')
        .map(|s| {
            let s = s.trim();
            // Remove surrounding quotes if present
            s.trim_matches('"').to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract package name from yarn spec like `lodash@^4.17.0` or `@types/node@^18.0.0`
fn extract_package_name_from_yarn_spec(spec: &str) -> Option<String> {
    // Handle scoped packages (@scope/name@version)
    if spec.starts_with('@') {
        let after_scope = spec.find('/')? + 1;
        let version_sep = spec[after_scope..].find('@')? + after_scope;
        Some(spec[..version_sep].to_string())
    } else {
        // Regular package (name@version)
        let at_idx = spec.find('@')?;
        Some(spec[..at_idx].to_string())
    }
}

/// Extract version from yarn.lock version line
///
/// Format: `  version "1.2.3"` or `  version: "1.2.3"` (yarn berry)
fn extract_yarn_version(line: &str) -> Result<String, LockfileError> {
    let line = line.trim();

    // Handle both `version "x.y.z"` and `version: "x.y.z"` formats
    let version_part = line
        .strip_prefix("version ")
        .or_else(|| line.strip_prefix("version: "))
        .ok_or_else(|| LockfileError::Parse {
            path: PathBuf::from("yarn.lock"),
            details: format!("Invalid version line: {}", line),
        })?;

    // Remove surrounding quotes
    let version = version_part.trim().trim_matches('"').to_string();
    Ok(version)
}

// === package-lock.json Parsing ===

/// Structure for package-lock.json (lockfileVersion 2 and 3)
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageLockfile {
    packages: Option<HashMap<String, PackageLockEntry>>,
    // Fallback for lockfileVersion 1
    dependencies: Option<HashMap<String, PackageLockDep>>,
}

#[derive(Deserialize)]
struct PackageLockEntry {
    version: Option<String>,
    /// Resolved URL - can be registry URL or git URL
    resolved: Option<String>,
}

#[derive(Deserialize)]
struct PackageLockDep {
    version: String,
}

/// Parse version from package-lock.json
fn parse_package_lock(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let lockfile: PackageLockfile =
        serde_json::from_str(&content).map_err(|e| LockfileError::Parse {
            path: path.to_path_buf(),
            details: e.to_string(),
        })?;

    let normalized_package = normalize_node_name(package);

    // Try lockfileVersion 2/3 format (packages object with node_modules/ keys)
    if let Some(packages) = &lockfile.packages {
        for (key, entry) in packages {
            // Keys are like "node_modules/lodash" or "node_modules/@types/node"
            let pkg_name = key.strip_prefix("node_modules/").unwrap_or(key);
            if normalize_node_name(pkg_name) == normalized_package {
                // Check for git dependency in resolved field first
                if let Some(resolved) = &entry.resolved
                    && let Some(git_info) = parse_git_url(resolved)
                {
                    return Ok(git_info);
                }
                // Check for link/file dependencies
                if let Some(version) = &entry.version {
                    if version.starts_with("link:") || version.starts_with("file:") {
                        return Ok(VersionInfo::LocalPath {
                            path: version.clone(),
                        });
                    }
                    // Check if version itself is a git URL
                    if let Some(git_info) = parse_git_url(version) {
                        return Ok(git_info);
                    }
                    return Ok(VersionInfo::Version(version.clone()));
                }
            }
        }
    }

    // Try lockfileVersion 1 format (dependencies object)
    if let Some(deps) = &lockfile.dependencies {
        for (name, dep) in deps {
            if normalize_node_name(name) == normalized_package {
                // Check if version is a git URL
                if let Some(git_info) = parse_git_url(&dep.version) {
                    return Ok(git_info);
                }
                return Ok(VersionInfo::Version(dep.version.clone()));
            }
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Normalize Node.js package name for comparison
///
/// Node.js package names are case-sensitive on npm, but we normalize to lowercase
/// for consistent cache keys (as specified in the PRD)
fn normalize_node_name(name: &str) -> String {
    name.to_lowercase()
}

/// Parse a version string that might be a git URL or regular version
fn parse_node_version_string(version: &str) -> VersionInfo {
    // Check for link/file prefixes (local deps)
    if version.starts_with("link:") || version.starts_with("file:") {
        return VersionInfo::LocalPath {
            path: version.to_string(),
        };
    }

    // Check for git URL patterns
    if let Some(git_info) = parse_git_url(version) {
        return git_info;
    }

    // Regular version
    VersionInfo::Version(version.to_string())
}

/// Parse a git URL into VersionInfo::Git
///
/// Handles various git URL formats:
/// - `git+https://github.com/org/repo.git#commit`
/// - `git+ssh://git@github.com/org/repo.git#commit`
/// - `git://github.com/org/repo#commit`
/// - `https://github.com/org/repo.git#commit` (if contains #commit)
fn parse_git_url(url: &str) -> Option<VersionInfo> {
    // Check for git URL patterns
    let is_git_url = url.starts_with("git+")
        || url.starts_with("git://")
        || url.starts_with("git@")
        || (url.contains(".git") && url.contains('#'));

    if !is_git_url {
        return None;
    }

    // Extract the commit hash (after #)
    let (url_part, commit) = if let Some(idx) = url.rfind('#') {
        let commit = url[idx + 1..].to_string();
        let url = url[..idx].to_string();
        (url, commit)
    } else {
        // Git URL without commit - use HEAD
        (url.to_string(), "HEAD".to_string())
    };

    // Clean up the URL
    let clean_url = url_part
        .strip_prefix("git+")
        .unwrap_or(&url_part)
        .to_string();

    // Convert ssh URLs to https
    let clean_url = if clean_url.starts_with("ssh://git@") {
        // ssh://git@github.com/org/repo.git -> https://github.com/org/repo.git
        clean_url.replace("ssh://git@", "https://")
    } else if clean_url.starts_with("git@") {
        // git@github.com:org/repo.git -> https://github.com/org/repo.git
        clean_url
            .replace("git@", "https://")
            .replace(".com:", ".com/")
            .replace(".org:", ".org/")
    } else if clean_url.starts_with("git://") {
        // git://github.com/org/repo -> https://github.com/org/repo
        clean_url.replace("git://", "https://")
    } else {
        clean_url
    };

    Some(VersionInfo::Git {
        url: clean_url,
        commit,
    })
}

/// Extract resolved URL from yarn.lock resolved line
fn extract_yarn_resolved(line: &str) -> Option<String> {
    let line = line.trim();
    let resolved_part = line
        .strip_prefix("resolved ")
        .or_else(|| line.strip_prefix("resolved: "))?;
    Some(resolved_part.trim().trim_matches('"').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_node_name() {
        assert_eq!(normalize_node_name("Lodash"), "lodash");
        assert_eq!(normalize_node_name("@types/node"), "@types/node");
        assert_eq!(normalize_node_name("@Types/Node"), "@types/node");
    }

    #[test]
    fn test_parse_pnpm_package_key_regular() {
        assert_eq!(
            parse_pnpm_package_key("lodash@4.17.21"),
            Some(("lodash".to_string(), "4.17.21".to_string()))
        );
    }

    #[test]
    fn test_parse_pnpm_package_key_scoped() {
        assert_eq!(
            parse_pnpm_package_key("@types/node@18.0.0"),
            Some(("@types/node".to_string(), "18.0.0".to_string()))
        );
    }

    #[test]
    fn test_extract_package_name_from_yarn_spec_regular() {
        assert_eq!(
            extract_package_name_from_yarn_spec("lodash@^4.17.0"),
            Some("lodash".to_string())
        );
    }

    #[test]
    fn test_extract_package_name_from_yarn_spec_scoped() {
        assert_eq!(
            extract_package_name_from_yarn_spec("@types/node@^18.0.0"),
            Some("@types/node".to_string())
        );
    }

    #[test]
    fn test_parse_yarn_lock_header() {
        let header = r#"lodash@^4.17.0, lodash@~4.17.0:"#;
        let packages = parse_yarn_lock_header(header);
        assert_eq!(packages, vec!["lodash@^4.17.0", "lodash@~4.17.0"]);
    }

    #[test]
    fn test_parse_yarn_lock_header_scoped() {
        let header = r#""@types/node@^18.0.0":"#;
        let packages = parse_yarn_lock_header(header);
        assert_eq!(packages, vec!["@types/node@^18.0.0"]);
    }

    #[test]
    fn test_extract_yarn_version() {
        assert_eq!(
            extract_yarn_version(r#"  version "4.17.21""#).unwrap(),
            "4.17.21"
        );
        assert_eq!(
            extract_yarn_version(r#"  version: "4.17.21""#).unwrap(),
            "4.17.21"
        );
    }

    #[test]
    fn test_parse_yarn_lock_content() {
        let content = r#"# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.
# yarn lockfile v1

lodash@^4.17.0:
  version "4.17.21"
  resolved "https://registry.yarnpkg.com/lodash/-/lodash-4.17.21.tgz"
  integrity sha512-v2kDE...

"@types/node@^18.0.0":
  version "18.19.0"
  resolved "https://registry.yarnpkg.com/@types/node/-/node-18.19.0.tgz"
  integrity sha512-abc...
"#;

        // Test parsing logic directly
        let mut found_lodash = false;
        let mut found_types_node = false;
        let mut current_packages: Vec<String> = Vec::new();
        let mut in_entry = false;

        for line in content.lines() {
            let line = line.trim_end();

            if !line.starts_with(' ') && !line.starts_with('#') && line.ends_with(':') {
                current_packages = parse_yarn_lock_header(line);
                in_entry = true;
                continue;
            }

            if in_entry && line.trim().starts_with("version ") {
                let version = extract_yarn_version(line).unwrap();

                for pkg_spec in &current_packages {
                    if let Some(name) = extract_package_name_from_yarn_spec(pkg_spec) {
                        if name == "lodash" && version == "4.17.21" {
                            found_lodash = true;
                        }
                        if name == "@types/node" && version == "18.19.0" {
                            found_types_node = true;
                        }
                    }
                }
                in_entry = false;
                current_packages.clear();
            }
        }

        assert!(found_lodash, "Should find lodash@4.17.21");
        assert!(found_types_node, "Should find @types/node@18.19.0");
    }

    #[test]
    fn test_parse_package_lock_v3() {
        let content = r#"{
  "name": "test",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "test",
      "dependencies": {
        "lodash": "^4.17.21"
      }
    },
    "node_modules/lodash": {
      "version": "4.17.21"
    },
    "node_modules/@types/node": {
      "version": "18.19.0"
    }
  }
}"#;

        let lockfile: PackageLockfile = serde_json::from_str(content).unwrap();
        let packages = lockfile.packages.unwrap();

        // Check lodash
        let lodash = packages.get("node_modules/lodash").unwrap();
        assert_eq!(lodash.version.as_deref(), Some("4.17.21"));

        // Check scoped package
        let types_node = packages.get("node_modules/@types/node").unwrap();
        assert_eq!(types_node.version.as_deref(), Some("18.19.0"));
    }

    #[test]
    fn test_parse_pnpm_lock_content() {
        let content = r#"lockfileVersion: '9.0'

packages:
  '@types/node@25.0.10':
    resolution: {integrity: sha512-abc...}

  lodash@4.17.21:
    resolution: {integrity: sha512-xyz...}
"#;

        let lockfile: PnpmLockfile = serde_yml::from_str(content).unwrap();
        let packages = lockfile.packages.unwrap();

        // Check keys exist
        assert!(packages.contains_key("lodash@4.17.21"));
        assert!(packages.contains_key("@types/node@25.0.10"));

        // Parse keys
        assert_eq!(
            parse_pnpm_package_key("lodash@4.17.21"),
            Some(("lodash".to_string(), "4.17.21".to_string()))
        );
        assert_eq!(
            parse_pnpm_package_key("@types/node@25.0.10"),
            Some(("@types/node".to_string(), "25.0.10".to_string()))
        );
    }

    #[test]
    fn test_parse_git_url_https() {
        let url = "git+https://github.com/org/repo.git#abc123";
        let result = parse_git_url(url).unwrap();
        assert_eq!(
            result,
            VersionInfo::Git {
                url: "https://github.com/org/repo.git".to_string(),
                commit: "abc123".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_git_url_ssh() {
        // ssh:// URLs are converted to https://
        let url = "git+ssh://git@github.com/org/repo.git#abc123";
        let result = parse_git_url(url).unwrap();
        assert_eq!(
            result,
            VersionInfo::Git {
                url: "https://github.com/org/repo.git".to_string(),
                commit: "abc123".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_git_url_git_protocol() {
        let url = "git://github.com/org/repo#abc123";
        let result = parse_git_url(url).unwrap();
        assert_eq!(
            result,
            VersionInfo::Git {
                url: "https://github.com/org/repo".to_string(),
                commit: "abc123".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_git_url_not_git() {
        let url = "https://registry.npmjs.org/lodash/-/lodash-4.17.21.tgz";
        assert!(parse_git_url(url).is_none());
    }

    #[test]
    fn test_parse_node_version_string_regular() {
        let result = parse_node_version_string("4.17.21");
        assert_eq!(result, VersionInfo::Version("4.17.21".to_string()));
    }

    #[test]
    fn test_parse_node_version_string_link() {
        let result = parse_node_version_string("link:../local-pkg");
        assert_eq!(
            result,
            VersionInfo::LocalPath {
                path: "link:../local-pkg".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_node_version_string_file() {
        let result = parse_node_version_string("file:../local-pkg");
        assert_eq!(
            result,
            VersionInfo::LocalPath {
                path: "file:../local-pkg".to_string(),
            }
        );
    }
}
