//! Lockfile parsing for Python ecosystems
//!
//! Supports finding package versions from:
//! - poetry.lock (TOML, `[[package]]` array)
//! - uv.lock (TOML, `[[package]]` array)
//! - requirements.txt (line-based, `package==version`)
//! - pyproject.toml (TOML, `[tool.poetry.dependencies]` or `[project.dependencies]`)
//!
//! Lockfile priority order: poetry.lock > uv.lock > requirements.txt > pyproject.toml

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
pub fn find_version(package: &str) -> Result<String, LockfileError> {
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
fn parse_version_from_lockfile(path: &Path, package: &str) -> Result<String, LockfileError> {
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
}

/// Parse version from poetry.lock or uv.lock
fn parse_toml_lockfile(path: &Path, package: &str) -> Result<String, LockfileError> {
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
            return Ok(pkg.version);
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

// === requirements.txt Parsing ===

/// Parse version from requirements.txt
///
/// Handles formats like:
/// - requests==2.31.0
/// - requests>=2.31.0
/// - requests~=2.31.0
/// - requests[security]==2.31.0
fn parse_requirements_txt(path: &Path, package: &str) -> Result<String, LockfileError> {
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
            return Ok(version);
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
fn parse_pyproject_toml(path: &Path, package: &str) -> Result<String, LockfileError> {
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
    if let Some(version) = extract_poetry_dependency(&doc, &normalized_package) {
        return Ok(version);
    }

    // Try project.dependencies (PEP 621)
    if let Some(version) = extract_pep621_dependency(&doc, &normalized_package) {
        return Ok(version);
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Extract version from [tool.poetry.dependencies]
fn extract_poetry_dependency(doc: &toml::Value, normalized_package: &str) -> Option<String> {
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
fn extract_version_from_poetry_dep(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(s) => Some(strip_version_constraint(s)),
        toml::Value::Table(t) => t.get("version")?.as_str().map(strip_version_constraint),
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
}
