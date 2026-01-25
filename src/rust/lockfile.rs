//! Lockfile parsing for Rust ecosystem
//!
//! Supports finding package versions from Cargo.lock
//!
//! Note: Cargo.lock can have git dependencies with `source = "git+..."` but
//! this implementation currently only returns version strings. Git dependency
//! detection could be added by parsing the source field.

use crate::cli::VersionInfo;
use crate::lockfile::find_nearest_file;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockfileError {
    #[error("No Cargo.lock found. Specify version explicitly.")]
    NotFound,

    #[error(
        "Version not found for '{package}'. Specify explicitly: dotdeps add rust:{package}@<version>"
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

/// Find the version of a crate by searching Cargo.lock
///
/// Searches upward from the current directory for Cargo.lock
pub fn find_version(package: &str) -> Result<VersionInfo, LockfileError> {
    let lockfile = find_lockfile_path()?;
    parse_version_from_lockfile(&lockfile, package)
}

/// Find the nearest Cargo.lock by walking up from current directory
pub fn find_lockfile_path() -> Result<PathBuf, LockfileError> {
    find_nearest_file(&["Cargo.lock"]).ok_or(LockfileError::NotFound)
}

/// List direct dependencies from Cargo.toml if present, otherwise fall back to Cargo.lock.
pub fn list_direct_dependencies(path: &Path) -> Result<Vec<String>, LockfileError> {
    if let Some(parent) = path.parent() {
        let cargo_toml = parent.join("Cargo.toml");
        if cargo_toml.exists() {
            let deps = parse_cargo_toml_dependencies(&cargo_toml)?;
            let mut unique = deps
                .into_iter()
                .map(|d| normalize_crate_name(&d))
                .collect::<Vec<_>>();
            unique.sort();
            unique.dedup();
            return Ok(unique);
        }
    }

    let deps = list_packages_from_cargo_lock(path)?;
    let mut unique = deps
        .into_iter()
        .map(|d| normalize_crate_name(&d))
        .collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    Ok(unique)
}

/// Structure for Cargo.lock files
///
/// Cargo.lock is TOML with a `[[package]]` array containing name and version
#[derive(Deserialize)]
struct CargoLockfile {
    package: Option<Vec<CargoPackage>>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
}

/// Parse version from Cargo.lock
fn parse_version_from_lockfile(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let lockfile: CargoLockfile = toml::from_str(&content).map_err(|e| LockfileError::Parse {
        path: path.to_path_buf(),
        details: e.to_string(),
    })?;

    let packages = lockfile.package.unwrap_or_default();
    let normalized_package = normalize_crate_name(package);

    // Crate names are case-insensitive and use - or _ interchangeably
    for pkg in packages {
        if normalize_crate_name(&pkg.name) == normalized_package {
            return Ok(VersionInfo::Version(pkg.version));
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

fn parse_cargo_toml_dependencies(path: &Path) -> Result<Vec<String>, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let doc: toml::Value = toml::from_str(&content).map_err(|e| LockfileError::Parse {
        path: path.to_path_buf(),
        details: e.to_string(),
    })?;

    let mut deps = Vec::new();

    if let Some(table) = doc.get("dependencies").and_then(|v| v.as_table()) {
        collect_dependency_table(table, &mut deps);
    }
    if let Some(table) = doc.get("dev-dependencies").and_then(|v| v.as_table()) {
        collect_dependency_table(table, &mut deps);
    }
    if let Some(table) = doc.get("build-dependencies").and_then(|v| v.as_table()) {
        collect_dependency_table(table, &mut deps);
    }

    if let Some(workspace) = doc.get("workspace").and_then(|v| v.as_table())
        && let Some(table) = workspace.get("dependencies").and_then(|v| v.as_table())
    {
        collect_dependency_table(table, &mut deps);
    }

    if let Some(targets) = doc.get("target").and_then(|v| v.as_table()) {
        for target in targets.values() {
            if let Some(target_table) = target.as_table() {
                if let Some(table) = target_table.get("dependencies").and_then(|v| v.as_table()) {
                    collect_dependency_table(table, &mut deps);
                }
                if let Some(table) = target_table
                    .get("dev-dependencies")
                    .and_then(|v| v.as_table())
                {
                    collect_dependency_table(table, &mut deps);
                }
                if let Some(table) = target_table
                    .get("build-dependencies")
                    .and_then(|v| v.as_table())
                {
                    collect_dependency_table(table, &mut deps);
                }
            }
        }
    }

    Ok(deps)
}

fn collect_dependency_table(table: &toml::value::Table, deps: &mut Vec<String>) {
    for (name, value) in table {
        if is_path_dependency(value) {
            continue;
        }
        deps.push(name.to_string());
    }
}

fn is_path_dependency(value: &toml::Value) -> bool {
    match value {
        toml::Value::Table(table) => table.contains_key("path"),
        _ => false,
    }
}

fn list_packages_from_cargo_lock(path: &Path) -> Result<Vec<String>, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let lockfile: CargoLockfile = toml::from_str(&content).map_err(|e| LockfileError::Parse {
        path: path.to_path_buf(),
        details: e.to_string(),
    })?;

    let mut deps = Vec::new();
    let packages = lockfile.package.unwrap_or_default();
    for pkg in packages {
        deps.push(pkg.name);
    }

    Ok(deps)
}

/// Normalize crate name for comparison
///
/// Crate names are case-insensitive and treat - and _ as equivalent
fn normalize_crate_name(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
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
        let dir = std::env::temp_dir().join(format!("dotdeps_rust_test_{}", nanos));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(filename);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_normalize_crate_name() {
        assert_eq!(normalize_crate_name("Serde"), "serde");
        assert_eq!(normalize_crate_name("serde-json"), "serde_json");
        assert_eq!(normalize_crate_name("serde_json"), "serde_json");
        assert_eq!(normalize_crate_name("SERDE-JSON"), "serde_json");
    }

    #[test]
    fn test_parse_cargo_lockfile_content() {
        let content = r#"
# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = "serde"
version = "1.0.228"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc123"

[[package]]
name = "serde_json"
version = "1.0.149"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "def456"
dependencies = [
  "serde",
]
"#;

        let lockfile: CargoLockfile = toml::from_str(content).unwrap();
        let packages = lockfile.package.unwrap();

        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "serde");
        assert_eq!(packages[0].version, "1.0.228");
        assert_eq!(packages[1].name, "serde_json");
        assert_eq!(packages[1].version, "1.0.149");
    }

    #[test]
    fn test_parse_cargo_lockfile_with_hyphen_underscore() {
        let content = r#"
[[package]]
name = "serde-json"
version = "1.0.149"
"#;

        let lockfile: CargoLockfile = toml::from_str(content).unwrap();
        let packages = lockfile.package.unwrap();

        assert_eq!(packages[0].name, "serde-json");

        // Test normalization finds both variants
        assert_eq!(normalize_crate_name("serde-json"), "serde_json");
        assert_eq!(normalize_crate_name("serde_json"), "serde_json");
    }

    #[test]
    fn test_parse_cargo_toml_dependencies() {
        let content = r#"
[dependencies]
serde = "1.0"
local = { path = "../local" }

[dev-dependencies]
anyhow = "1.0"

[target.'cfg(unix)'.dependencies]
libc = "0.2"
"#;
        let path = write_temp_file("Cargo.toml", content);
        let deps = parse_cargo_toml_dependencies(&path).unwrap();
        assert!(deps.contains(&"serde".to_string()));
        assert!(deps.contains(&"anyhow".to_string()));
        assert!(deps.contains(&"libc".to_string()));
        assert!(!deps.contains(&"local".to_string()));
    }
}
