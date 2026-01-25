//! Lockfile parsing for Python ecosystems
//!
//! Supports finding package versions from:
//! - poetry.lock (TOML, `[[package]]` array)
//! - uv.lock (TOML, `[[package]]` array)
//! - requirements.txt (line-based, `package==version`)
//! - pyproject.toml (TOML, `[tool.poetry.dependencies]` or `[project.dependencies]`)
//!
//! Lockfile priority order: poetry.lock > uv.lock > requirements.txt > pyproject.toml
//!
//! Also detects special dependency types:
//! - Git dependencies: `[package.source] type = "git"`
//! - Local path dependencies: `[package.source] type = "directory"`

use crate::cli::VersionInfo;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockfileError {
    #[error("No lockfile found. Specify version explicitly.")]
    NotFound,

    #[error(
        "Version not found for '{package}'. Specify explicitly: dotdeps add python:{package}@<version>"
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
/// poetry.lock > uv.lock > requirements.txt > pyproject.toml
///
/// Returns `VersionInfo` which can be:
/// - `Version(string)` for regular registry packages
/// - `Git { url, commit }` for git dependencies
/// - `LocalPath { path }` for local directory dependencies
pub fn find_version(package: &str) -> Result<VersionInfo, LockfileError> {
    let lockfile = find_lockfile()?;
    parse_version_from_lockfile(&lockfile, package)
}

/// Lockfile types in priority order
#[derive(Debug, Clone, Copy)]
enum LockfileType {
    PoetryLock,
    UvLock,
    RequirementsTxt,
    PyprojectToml,
}

impl LockfileType {
    fn filename(&self) -> &'static str {
        match self {
            LockfileType::PoetryLock => "poetry.lock",
            LockfileType::UvLock => "uv.lock",
            LockfileType::RequirementsTxt => "requirements.txt",
            LockfileType::PyprojectToml => "pyproject.toml",
        }
    }

    fn priority_order() -> &'static [LockfileType] {
        &[
            LockfileType::PoetryLock,
            LockfileType::UvLock,
            LockfileType::RequirementsTxt,
            LockfileType::PyprojectToml,
        ]
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
        "poetry.lock" | "uv.lock" => parse_toml_lockfile(path, package),
        "requirements.txt" => parse_requirements_txt(path, package),
        "pyproject.toml" => parse_pyproject_toml(path, package),
        _ => Err(LockfileError::Parse {
            path: path.to_path_buf(),
            details: format!("Unknown lockfile type: {}", filename),
        }),
    }
}

// === TOML Lockfile Parsing (poetry.lock, uv.lock) ===

/// Structure for poetry.lock and uv.lock files
#[derive(Deserialize)]
struct TomlLockfile {
    package: Option<Vec<TomlPackage>>,
}

#[derive(Deserialize)]
struct TomlPackage {
    name: String,
    version: String,
    /// Source information for non-registry packages (git, directory, url)
    source: Option<TomlPackageSource>,
}

/// Source information for a package
///
/// Poetry/uv lockfiles use this to specify non-registry sources:
/// - `type = "git"` with `url` and `resolved_reference` (commit hash)
/// - `type = "directory"` with `url` (local path)
/// - `type = "url"` with `url` (direct URL to archive)
#[derive(Deserialize)]
struct TomlPackageSource {
    /// Source type: "git", "directory", "url"
    #[serde(rename = "type")]
    source_type: Option<String>,
    /// URL for git repos or local paths
    url: Option<String>,
    /// Resolved commit hash for git dependencies
    resolved_reference: Option<String>,
}

/// Parse version from poetry.lock or uv.lock
fn parse_toml_lockfile(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let lockfile: TomlLockfile = toml::from_str(&content).map_err(|e| LockfileError::Parse {
        path: path.to_path_buf(),
        details: e.to_string(),
    })?;

    let packages = lockfile.package.unwrap_or_default();
    let normalized_package = package.to_lowercase();

    // Python package names are case-insensitive and often use - or _ interchangeably
    for pkg in packages {
        if normalize_python_name(&pkg.name) == normalize_python_name(&normalized_package) {
            // Check for special source types (git, directory)
            if let Some(source) = &pkg.source {
                return extract_version_info_from_source(source, &pkg.version);
            }
            // Regular registry package
            return Ok(VersionInfo::Version(pkg.version));
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Extract VersionInfo from a package source
///
/// Handles:
/// - `type = "git"` -> VersionInfo::Git with url and commit hash
/// - `type = "directory"` -> VersionInfo::LocalPath
/// - Other types (url, etc.) -> Fall back to version string
fn extract_version_info_from_source(
    source: &TomlPackageSource,
    version: &str,
) -> Result<VersionInfo, LockfileError> {
    match source.source_type.as_deref() {
        Some("git") => {
            let url = source.url.clone().unwrap_or_default();
            let commit = source.resolved_reference.clone().unwrap_or_default();

            if url.is_empty() || commit.is_empty() {
                // Malformed git source, fall back to version
                Ok(VersionInfo::Version(version.to_string()))
            } else {
                Ok(VersionInfo::Git { url, commit })
            }
        }
        Some("directory") => {
            let path = source.url.clone().unwrap_or_default();
            Ok(VersionInfo::LocalPath { path })
        }
        // For "url" type or unknown types, use the version string
        _ => Ok(VersionInfo::Version(version.to_string())),
    }
}

// === requirements.txt Parsing ===

/// Parse version from requirements.txt
///
/// Handles formats like:
/// - requests==2.31.0
/// - requests>=2.31.0
/// - requests~=2.31.0
/// - requests[security]==2.31.0
///
/// Note: requirements.txt doesn't typically contain git deps in a parseable format,
/// so this always returns VersionInfo::Version
fn parse_requirements_txt(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let normalized_package = normalize_python_name(package);

    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Skip options like -r, -e, --extra-index-url
        if line.starts_with('-') {
            continue;
        }

        // Parse package name and version
        if let Some((name, version)) = parse_requirement_line(line)
            && normalize_python_name(&name) == normalized_package
        {
            return Ok(VersionInfo::Version(version));
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Parse a single requirement line into (package_name, version)
///
/// Returns None if the line doesn't specify an exact version
fn parse_requirement_line(line: &str) -> Option<(String, String)> {
    // Remove inline comments
    let line = line.split('#').next()?.trim();

    // Remove environment markers (e.g., ; python_version >= "3.8")
    let line = line.split(';').next()?.trim();

    // Remove extras (e.g., requests[security] -> requests)
    let line = if let Some(bracket_idx) = line.find('[') {
        if let Some(close_idx) = line.find(']') {
            format!("{}{}", &line[..bracket_idx], &line[close_idx + 1..])
        } else {
            line.to_string()
        }
    } else {
        line.to_string()
    };

    // Find version specifier
    // Priority: == (exact), then try to extract from other specifiers
    let version_patterns = ["==", "~=", ">=", "<=", ">", "<", "!="];

    for pattern in version_patterns {
        if let Some(idx) = line.find(pattern) {
            let name = line[..idx].trim().to_string();
            let version_part = line[idx + pattern.len()..].trim();

            // Extract version (stop at comma for multiple specifiers)
            let version = version_part
                .split(',')
                .next()
                .unwrap_or(version_part)
                .trim()
                .to_string();

            if !name.is_empty() && !version.is_empty() {
                // For == we have an exact version, for others we have at least a minimum
                // Only return exact versions (==) for reliability
                if pattern == "==" {
                    return Some((name, version));
                }
            }
        }
    }

    None
}

// === pyproject.toml Parsing ===

/// Parse version from pyproject.toml
///
/// Looks for dependencies in:
/// - [tool.poetry.dependencies]
/// - [project.dependencies]
///
/// Note: pyproject.toml can have git deps but parsing them reliably is complex.
/// This primarily handles version strings; git deps should use poetry.lock/uv.lock.
fn parse_pyproject_toml(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let doc: toml::Value = toml::from_str(&content).map_err(|e| LockfileError::Parse {
        path: path.to_path_buf(),
        details: e.to_string(),
    })?;

    let normalized_package = normalize_python_name(package);

    // Try tool.poetry.dependencies first
    if let Some(version_info) = extract_poetry_dependency(&doc, &normalized_package) {
        return Ok(version_info);
    }

    // Try project.dependencies (PEP 621)
    if let Some(version) = extract_pep621_dependency(&doc, &normalized_package) {
        return Ok(VersionInfo::Version(version));
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Extract version from [tool.poetry.dependencies]
fn extract_poetry_dependency(doc: &toml::Value, normalized_package: &str) -> Option<VersionInfo> {
    let deps = doc
        .get("tool")?
        .get("poetry")?
        .get("dependencies")?
        .as_table()?;

    for (name, value) in deps {
        if normalize_python_name(name) == normalized_package {
            return extract_version_from_poetry_dep(value);
        }
    }

    None
}

/// Extract version from a Poetry dependency value
///
/// Can be:
/// - String: "^2.31.0" or "2.31.0"
/// - Table: { version = "^2.31.0", optional = true }
/// - Table with git: { git = "url", rev = "commit" }
/// - Table with path: { path = "../local" }
fn extract_version_from_poetry_dep(value: &toml::Value) -> Option<VersionInfo> {
    match value {
        toml::Value::String(s) => Some(VersionInfo::Version(strip_version_constraint(s))),
        toml::Value::Table(t) => {
            // Check for git dependency
            if let Some(git_url) = t.get("git").and_then(|v| v.as_str()) {
                let commit = t
                    .get("rev")
                    .or_else(|| t.get("tag"))
                    .or_else(|| t.get("branch"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("HEAD")
                    .to_string();
                return Some(VersionInfo::Git {
                    url: git_url.to_string(),
                    commit,
                });
            }

            // Check for path dependency
            if let Some(path) = t.get("path").and_then(|v| v.as_str()) {
                return Some(VersionInfo::LocalPath {
                    path: path.to_string(),
                });
            }

            // Regular version dependency
            t.get("version")?
                .as_str()
                .map(|s| VersionInfo::Version(strip_version_constraint(s)))
        }
        _ => None,
    }
}

/// Extract version from [project.dependencies] (PEP 621)
fn extract_pep621_dependency(doc: &toml::Value, normalized_package: &str) -> Option<String> {
    let deps = doc.get("project")?.get("dependencies")?.as_array()?;

    for dep in deps {
        let dep_str = dep.as_str()?;
        if let Some((name, version)) = parse_requirement_line(dep_str)
            && normalize_python_name(&name) == normalized_package
        {
            return Some(version);
        }
    }

    None
}

/// Strip version constraint prefixes (^, ~, >=, etc.) to get base version
fn strip_version_constraint(version: &str) -> String {
    let version = version.trim();

    // Remove common constraint prefixes
    let stripped = version
        .strip_prefix('^')
        .or_else(|| version.strip_prefix('~'))
        .or_else(|| version.strip_prefix(">="))
        .or_else(|| version.strip_prefix("<="))
        .or_else(|| version.strip_prefix("=="))
        .or_else(|| version.strip_prefix('>'))
        .or_else(|| version.strip_prefix('<'))
        .unwrap_or(version);

    // Handle range constraints like ">=2.0,<3.0" - take the first version
    stripped
        .split(',')
        .next()
        .unwrap_or(stripped)
        .trim()
        .to_string()
}

/// Normalize Python package name for comparison
///
/// Python package names are case-insensitive and treat - and _ as equivalent
fn normalize_python_name(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_python_name() {
        assert_eq!(normalize_python_name("Requests"), "requests");
        assert_eq!(
            normalize_python_name("typing-extensions"),
            "typing_extensions"
        );
        assert_eq!(
            normalize_python_name("typing_extensions"),
            "typing_extensions"
        );
        assert_eq!(
            normalize_python_name("TYPING-EXTENSIONS"),
            "typing_extensions"
        );
    }

    #[test]
    fn test_parse_requirement_line_exact() {
        assert_eq!(
            parse_requirement_line("requests==2.31.0"),
            Some(("requests".to_string(), "2.31.0".to_string()))
        );
    }

    #[test]
    fn test_parse_requirement_line_with_extras() {
        assert_eq!(
            parse_requirement_line("requests[security]==2.31.0"),
            Some(("requests".to_string(), "2.31.0".to_string()))
        );
    }

    #[test]
    fn test_parse_requirement_line_with_comment() {
        assert_eq!(
            parse_requirement_line("requests==2.31.0  # HTTP library"),
            Some(("requests".to_string(), "2.31.0".to_string()))
        );
    }

    #[test]
    fn test_parse_requirement_line_with_marker() {
        assert_eq!(
            parse_requirement_line("requests==2.31.0; python_version >= '3.8'"),
            Some(("requests".to_string(), "2.31.0".to_string()))
        );
    }

    #[test]
    fn test_parse_requirement_line_no_version() {
        // Lines without == don't return a version (we only want exact pins)
        assert_eq!(parse_requirement_line("requests>=2.31.0"), None);
        assert_eq!(parse_requirement_line("requests"), None);
    }

    #[test]
    fn test_strip_version_constraint() {
        assert_eq!(strip_version_constraint("^2.31.0"), "2.31.0");
        assert_eq!(strip_version_constraint("~2.31.0"), "2.31.0");
        assert_eq!(strip_version_constraint(">=2.31.0"), "2.31.0");
        assert_eq!(strip_version_constraint("==2.31.0"), "2.31.0");
        assert_eq!(strip_version_constraint("2.31.0"), "2.31.0");
        assert_eq!(strip_version_constraint(">=2.0,<3.0"), "2.0");
    }

    #[test]
    fn test_parse_toml_lockfile_content() {
        let content = r#"
[[package]]
name = "requests"
version = "2.31.0"

[[package]]
name = "urllib3"
version = "2.0.7"
"#;

        let lockfile: TomlLockfile = toml::from_str(content).unwrap();
        let packages = lockfile.package.unwrap();

        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "requests");
        assert_eq!(packages[0].version, "2.31.0");
    }

    #[test]
    fn test_parse_toml_lockfile_git_dependency() {
        let content = r#"
[[package]]
name = "alembic"
version = "1.3.1"

[package.source]
type = "git"
url = "https://github.com/sqlalchemy/alembic.git"
reference = "rel_1_3_1"
resolved_reference = "8d6bb007a4de046c4d338f4b79b40c9fcbf73ab7"

[[package]]
name = "requests"
version = "2.31.0"
"#;

        let lockfile: TomlLockfile = toml::from_str(content).unwrap();
        let packages = lockfile.package.unwrap();

        assert_eq!(packages.len(), 2);

        // Git dependency
        let alembic = &packages[0];
        assert_eq!(alembic.name, "alembic");
        let source = alembic.source.as_ref().unwrap();
        assert_eq!(source.source_type.as_deref(), Some("git"));
        assert_eq!(
            source.url.as_deref(),
            Some("https://github.com/sqlalchemy/alembic.git")
        );
        assert_eq!(
            source.resolved_reference.as_deref(),
            Some("8d6bb007a4de046c4d338f4b79b40c9fcbf73ab7")
        );

        // Regular dependency
        let requests = &packages[1];
        assert_eq!(requests.name, "requests");
        assert!(requests.source.is_none());
    }

    #[test]
    fn test_parse_toml_lockfile_directory_dependency() {
        let content = r#"
[[package]]
name = "local-pkg"
version = "0.1.0"

[package.source]
type = "directory"
url = "src/local-pkg"
"#;

        let lockfile: TomlLockfile = toml::from_str(content).unwrap();
        let packages = lockfile.package.unwrap();

        assert_eq!(packages.len(), 1);
        let pkg = &packages[0];
        let source = pkg.source.as_ref().unwrap();
        assert_eq!(source.source_type.as_deref(), Some("directory"));
        assert_eq!(source.url.as_deref(), Some("src/local-pkg"));
    }

    #[test]
    fn test_extract_version_info_from_source_git() {
        let source = TomlPackageSource {
            source_type: Some("git".to_string()),
            url: Some("https://github.com/org/repo.git".to_string()),
            resolved_reference: Some("abc123".to_string()),
        };

        let result = extract_version_info_from_source(&source, "1.0.0").unwrap();
        assert_eq!(
            result,
            VersionInfo::Git {
                url: "https://github.com/org/repo.git".to_string(),
                commit: "abc123".to_string(),
            }
        );
    }

    #[test]
    fn test_extract_version_info_from_source_directory() {
        let source = TomlPackageSource {
            source_type: Some("directory".to_string()),
            url: Some("../local-pkg".to_string()),
            resolved_reference: None,
        };

        let result = extract_version_info_from_source(&source, "1.0.0").unwrap();
        assert_eq!(
            result,
            VersionInfo::LocalPath {
                path: "../local-pkg".to_string(),
            }
        );
    }
}
