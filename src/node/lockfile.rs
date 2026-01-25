//! Lockfile parsing for Node.js ecosystems
//!
//! Supports finding package versions from:
//! - pnpm-lock.yaml (YAML)
//! - yarn.lock (custom format, not YAML)
//! - package-lock.json (JSON)
//! - bun.lock (JSONC format)
//!
//! Lockfile priority order: pnpm-lock.yaml > yarn.lock > package-lock.json > bun.lock
//!
//! Also detects special dependency types:
//! - Git dependencies: URLs starting with `git+`, `git://`, or containing `#commit`
//! - Local path dependencies: `link:`, `file:` URLs

use crate::cli::VersionInfo;
use crate::lockfile::find_nearest_file;
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
    let lockfile = find_lockfile_path()?;
    parse_version_from_lockfile(&lockfile, package)
}

/// Find the nearest lockfile by walking up from current directory
pub fn find_lockfile_path() -> Result<PathBuf, LockfileError> {
    find_nearest_file(&LOCKFILE_PRIORITY).ok_or(LockfileError::NotFound)
}

/// List direct dependencies from a lockfile or manifest.
///
/// - pnpm-lock.yaml: uses importers (root "." if present)
/// - package-lock.json: uses root packages[""] or dependencies (v1)
/// - yarn.lock/bun.lock: uses sibling package.json, falls back to lockfile entries
pub fn list_direct_dependencies(path: &Path) -> Result<Vec<String>, LockfileError> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let mut deps = Vec::new();

    match filename {
        "pnpm-lock.yaml" => {
            deps = list_pnpm_direct_dependencies(path)?;
        }
        "package-lock.json" => {
            deps = list_package_lock_direct_dependencies(path)?;
        }
        "yarn.lock" => {
            if let Some(parent) = path.parent() {
                let package_json = parent.join("package.json");
                if package_json.exists() {
                    deps = list_package_json_dependencies(&package_json)?;
                } else {
                    deps = list_all_packages_from_yarn_lock(path)?;
                }
            }
        }
        "bun.lock" => {
            if let Some(parent) = path.parent() {
                let package_json = parent.join("package.json");
                if package_json.exists() {
                    deps = list_package_json_dependencies(&package_json)?;
                } else {
                    deps = list_all_packages_from_bun_lock(path)?;
                }
            }
        }
        _ => {
            return Err(LockfileError::Parse {
                path: path.to_path_buf(),
                details: format!("Unknown lockfile type: {}", filename),
            });
        }
    }

    let mut unique: Vec<String> = deps.into_iter().map(|d| normalize_node_name(&d)).collect();
    unique.sort();
    unique.dedup();
    Ok(unique)
}

/// Lockfile priority order
const LOCKFILE_PRIORITY: [&str; 4] = [
    "pnpm-lock.yaml",
    "yarn.lock",
    "package-lock.json",
    "bun.lock",
];

/// Parse version from a lockfile
fn parse_version_from_lockfile(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    match filename {
        "pnpm-lock.yaml" => parse_pnpm_lock(path, package),
        "yarn.lock" => parse_yarn_lock(path, package),
        "package-lock.json" => parse_package_lock(path, package),
        "bun.lock" => parse_bun_lock(path, package),
        _ => Err(LockfileError::Parse {
            path: path.to_path_buf(),
            details: format!("Unknown lockfile type: {}", filename),
        }),
    }
}

// === Direct dependency listing ===

fn list_pnpm_direct_dependencies(path: &Path) -> Result<Vec<String>, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let lockfile: PnpmLockfile =
        serde_yml::from_str(&content).map_err(|e| LockfileError::Parse {
            path: path.to_path_buf(),
            details: e.to_string(),
        })?;

    let mut deps = Vec::new();
    let importers = lockfile.importers.unwrap_or_default();
    if let Some(root) = importers.get(".") {
        collect_importer_deps(root, &mut deps);
    } else {
        for importer in importers.values() {
            collect_importer_deps(importer, &mut deps);
        }
    }

    Ok(deps)
}

fn collect_importer_deps(importer: &PnpmImporter, deps: &mut Vec<String>) {
    if let Some(map) = &importer.dependencies {
        collect_dep_keys(map, deps);
    }
    if let Some(map) = &importer.dev_dependencies {
        collect_dep_keys(map, deps);
    }
    if let Some(map) = &importer.optional_dependencies {
        collect_dep_keys(map, deps);
    }
    if let Some(map) = &importer.peer_dependencies {
        collect_dep_keys(map, deps);
    }
}

fn list_package_lock_direct_dependencies(path: &Path) -> Result<Vec<String>, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let lockfile: PackageLockfile =
        serde_json::from_str(&content).map_err(|e| LockfileError::Parse {
            path: path.to_path_buf(),
            details: e.to_string(),
        })?;

    let mut deps = Vec::new();

    if let Some(packages) = &lockfile.packages
        && let Some(root) = packages.get("")
    {
        if let Some(map) = &root.dependencies {
            collect_dep_keys(map, &mut deps);
        }
        if let Some(map) = &root.dev_dependencies {
            collect_dep_keys(map, &mut deps);
        }
        if let Some(map) = &root.optional_dependencies {
            collect_dep_keys(map, &mut deps);
        }
        if let Some(map) = &root.peer_dependencies {
            collect_dep_keys(map, &mut deps);
        }
        return Ok(deps);
    }

    if let Some(deps_map) = &lockfile.dependencies {
        for (name, dep) in deps_map {
            if is_local_version_string(&dep.version) {
                continue;
            }
            deps.push(name.to_string());
        }
    }

    Ok(deps)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageJson {
    dependencies: Option<HashMap<String, serde_json::Value>>,
    dev_dependencies: Option<HashMap<String, serde_json::Value>>,
    optional_dependencies: Option<HashMap<String, serde_json::Value>>,
    peer_dependencies: Option<HashMap<String, serde_json::Value>>,
}

fn list_package_json_dependencies(path: &Path) -> Result<Vec<String>, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let manifest: PackageJson =
        serde_json::from_str(&content).map_err(|e| LockfileError::Parse {
            path: path.to_path_buf(),
            details: e.to_string(),
        })?;

    let mut deps = Vec::new();
    if let Some(map) = &manifest.dependencies {
        collect_dep_keys(map, &mut deps);
    }
    if let Some(map) = &manifest.dev_dependencies {
        collect_dep_keys(map, &mut deps);
    }
    if let Some(map) = &manifest.optional_dependencies {
        collect_dep_keys(map, &mut deps);
    }
    if let Some(map) = &manifest.peer_dependencies {
        collect_dep_keys(map, &mut deps);
    }

    Ok(deps)
}

fn list_all_packages_from_yarn_lock(path: &Path) -> Result<Vec<String>, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let mut deps = Vec::new();
    for line in content.lines() {
        let line = line.trim_end();
        if !line.starts_with(' ') && !line.starts_with('#') && line.ends_with(':') {
            let packages = parse_yarn_lock_header(line);
            for pkg_spec in packages {
                if let Some(name) = extract_package_name_from_yarn_spec(&pkg_spec) {
                    deps.push(name);
                }
            }
        }
    }

    Ok(deps)
}

fn list_all_packages_from_bun_lock(path: &Path) -> Result<Vec<String>, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let clean_content = strip_jsonc_trailing_commas(&content);
    let lockfile: BunLockfile =
        serde_json::from_str(&clean_content).map_err(|e| LockfileError::Parse {
            path: path.to_path_buf(),
            details: e.to_string(),
        })?;

    let mut deps = Vec::new();
    let packages = lockfile.packages.unwrap_or_default();
    for key in packages.keys() {
        if key.contains('/') && !key.starts_with('@') {
            continue;
        }
        deps.push(key.to_string());
    }

    Ok(deps)
}

fn collect_dep_keys(map: &HashMap<String, serde_json::Value>, deps: &mut Vec<String>) {
    for (name, value) in map {
        if is_local_node_spec(value) {
            continue;
        }
        deps.push(name.to_string());
    }
}

fn is_local_node_spec(value: &serde_json::Value) -> bool {
    if let Some(spec) = value.as_str() {
        return is_local_version_string(spec);
    }

    if let Some(obj) = value.as_object() {
        if let Some(spec) = obj.get("version").and_then(|v| v.as_str())
            && is_local_version_string(spec)
        {
            return true;
        }
        if let Some(spec) = obj.get("specifier").and_then(|v| v.as_str())
            && is_local_version_string(spec)
        {
            return true;
        }
    }

    false
}

fn is_local_version_string(version: &str) -> bool {
    version.starts_with("link:")
        || version.starts_with("file:")
        || version.starts_with("workspace:")
        || version.starts_with("path:")
}

// === pnpm-lock.yaml Parsing ===

/// Structure for pnpm-lock.yaml (lockfileVersion 9.0)
///
/// pnpm uses a YAML format with packages defined under `packages:` key.
/// Each package key is in the format `name@version` with resolution info.
/// We use serde_json::Value for package values since we only need the keys.
#[derive(Deserialize)]
struct PnpmLockfile {
    importers: Option<HashMap<String, PnpmImporter>>,
    packages: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PnpmImporter {
    dependencies: Option<HashMap<String, serde_json::Value>>,
    dev_dependencies: Option<HashMap<String, serde_json::Value>>,
    optional_dependencies: Option<HashMap<String, serde_json::Value>>,
    peer_dependencies: Option<HashMap<String, serde_json::Value>>,
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
    dependencies: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "optionalDependencies")]
    optional_dependencies: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "peerDependencies")]
    peer_dependencies: Option<HashMap<String, serde_json::Value>>,
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

// === bun.lock Parsing ===

/// Structure for bun.lock (JSONC format, lockfileVersion 1)
///
/// bun.lock is a JSONC file (JSON with trailing commas). Each package entry
/// in the `packages` object is an array: ["name@version", "registry", {deps}, "integrity"]
/// Keys can be package names ("lodash", "@types/node") or nested paths ("send/ms").
#[derive(Deserialize)]
struct BunLockfile {
    packages: Option<HashMap<String, serde_json::Value>>,
}

/// Parse version from bun.lock
///
/// bun.lock format:
/// ```json
/// {
///   "packages": {
///     "lodash": ["lodash@4.17.21", "", {}, "sha512-..."],
///     "@types/node": ["@types/node@22.0.0", "", {...}, "sha512-..."]
///   }
/// }
/// ```
fn parse_bun_lock(path: &Path, package: &str) -> Result<VersionInfo, LockfileError> {
    let content = fs::read_to_string(path).map_err(|source| LockfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    // bun.lock is JSONC (JSON with trailing commas), but serde_json handles it in lenient mode
    // We need to strip trailing commas for strict JSON parsing
    let clean_content = strip_jsonc_trailing_commas(&content);

    let lockfile: BunLockfile =
        serde_json::from_str(&clean_content).map_err(|e| LockfileError::Parse {
            path: path.to_path_buf(),
            details: e.to_string(),
        })?;

    let packages = lockfile.packages.unwrap_or_default();
    let normalized_package = normalize_node_name(package);

    // Look for direct package match (handles both regular and scoped packages)
    for (key, value) in &packages {
        // Skip nested packages like "send/ms" unless they match exactly
        // A nested package key contains "/" but the first segment is not "@"
        if key.contains('/') && !key.starts_with('@') {
            continue;
        }

        if normalize_node_name(key) == normalized_package {
            return parse_bun_package_entry(value, package, path);
        }
    }

    Err(LockfileError::VersionNotFound {
        package: package.to_string(),
    })
}

/// Parse a bun.lock package entry array into VersionInfo
///
/// Entry format: ["name@version", "registry/tarball", {dependencies}, "integrity"]
fn parse_bun_package_entry(
    value: &serde_json::Value,
    package: &str,
    path: &Path,
) -> Result<VersionInfo, LockfileError> {
    let arr = value.as_array().ok_or_else(|| LockfileError::Parse {
        path: path.to_path_buf(),
        details: format!("Expected array for package {}", package),
    })?;

    // First element is "name@version"
    let name_version =
        arr.first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| LockfileError::Parse {
                path: path.to_path_buf(),
                details: format!("Missing name@version for package {}", package),
            })?;

    // Parse "name@version" to extract version
    if let Some(version) = extract_version_from_bun_entry(name_version) {
        // Check if this is a git URL or local path
        return Ok(parse_node_version_string(&version));
    }

    Err(LockfileError::Parse {
        path: path.to_path_buf(),
        details: format!("Could not parse version from: {}", name_version),
    })
}

/// Extract version from bun.lock entry like "lodash@4.17.21" or "@types/node@22.0.0"
fn extract_version_from_bun_entry(entry: &str) -> Option<String> {
    // Handle scoped packages (@scope/name@version)
    if entry.starts_with('@') {
        let after_scope = entry.find('/')? + 1;
        let version_sep = entry[after_scope..].find('@')? + after_scope;
        Some(entry[version_sep + 1..].to_string())
    } else {
        // Regular package (name@version)
        let at_idx = entry.find('@')?;
        Some(entry[at_idx + 1..].to_string())
    }
}

/// Strip trailing commas from JSONC to make it valid JSON
///
/// bun.lock uses JSONC format which allows trailing commas. This function
/// removes them so serde_json can parse the content.
fn strip_jsonc_trailing_commas(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if in_string {
            result.push(c);
            continue;
        }

        // Outside string: check for trailing comma
        if c == ',' {
            // Look ahead for ] or } (skipping whitespace)
            let mut peek_chars = chars.clone();
            loop {
                match peek_chars.peek() {
                    Some(']') | Some('}') => {
                        // This is a trailing comma, skip it
                        break;
                    }
                    Some(c) if c.is_whitespace() => {
                        peek_chars.next();
                        continue;
                    }
                    _ => {
                        // Not a trailing comma, keep it
                        result.push(',');
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }

    result
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
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_file(filename: &str, content: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("dotdeps_node_test_{}", nanos));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(filename);
        fs::write(&path, content).unwrap();
        path
    }

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

    #[test]
    fn test_list_package_json_dependencies_skips_local() {
        let content = r#"{
  "dependencies": {
    "react": "^18.0.0",
    "local": "file:../local"
  },
  "devDependencies": {
    "@types/node": "^20.0.0"
  }
}"#;
        let path = write_temp_file("package.json", content);
        let deps = list_package_json_dependencies(&path).unwrap();
        assert!(deps.contains(&"react".to_string()));
        assert!(deps.contains(&"@types/node".to_string()));
        assert!(!deps.contains(&"local".to_string()));
    }

    #[test]
    fn test_list_all_packages_from_yarn_lock_headers() {
        let content = r#"
lodash@^4.17.0:
  version "4.17.21"
"#;
        let path = write_temp_file("yarn.lock", content);
        let deps = list_all_packages_from_yarn_lock(&path).unwrap();
        assert!(deps.contains(&"lodash".to_string()));
    }

    // === bun.lock tests ===

    #[test]
    fn test_strip_jsonc_trailing_commas() {
        let input = r#"{"a": 1, "b": 2,}"#;
        let expected = r#"{"a": 1, "b": 2}"#;
        assert_eq!(strip_jsonc_trailing_commas(input), expected);
    }

    #[test]
    fn test_strip_jsonc_trailing_commas_array() {
        let input = r#"[1, 2, 3,]"#;
        let expected = r#"[1, 2, 3]"#;
        assert_eq!(strip_jsonc_trailing_commas(input), expected);
    }

    #[test]
    fn test_strip_jsonc_trailing_commas_nested() {
        let input = r#"{"a": {"b": 1,}, "c": [1, 2,],}"#;
        let expected = r#"{"a": {"b": 1}, "c": [1, 2]}"#;
        assert_eq!(strip_jsonc_trailing_commas(input), expected);
    }

    #[test]
    fn test_strip_jsonc_preserves_commas_in_strings() {
        let input = r#"{"a": "hello, world,",}"#;
        let expected = r#"{"a": "hello, world,"}"#;
        assert_eq!(strip_jsonc_trailing_commas(input), expected);
    }

    #[test]
    fn test_extract_version_from_bun_entry_regular() {
        assert_eq!(
            extract_version_from_bun_entry("lodash@4.17.21"),
            Some("4.17.21".to_string())
        );
    }

    #[test]
    fn test_extract_version_from_bun_entry_scoped() {
        assert_eq!(
            extract_version_from_bun_entry("@types/node@22.19.7"),
            Some("22.19.7".to_string())
        );
    }

    #[test]
    fn test_parse_bun_lock_content() {
        let content = r#"{
  "lockfileVersion": 1,
  "packages": {
    "lodash": ["lodash@4.17.21", "", {}, "sha512-..."],
    "@types/node": ["@types/node@22.19.7", "", {}, "sha512-..."],
  }
}"#;

        let clean_content = strip_jsonc_trailing_commas(content);
        let lockfile: BunLockfile = serde_json::from_str(&clean_content).unwrap();
        let packages = lockfile.packages.unwrap();

        // Check keys exist
        assert!(packages.contains_key("lodash"));
        assert!(packages.contains_key("@types/node"));

        // Check array structure
        let lodash = packages.get("lodash").unwrap().as_array().unwrap();
        assert_eq!(lodash[0].as_str().unwrap(), "lodash@4.17.21");

        let types_node = packages.get("@types/node").unwrap().as_array().unwrap();
        assert_eq!(types_node[0].as_str().unwrap(), "@types/node@22.19.7");
    }

    #[test]
    fn test_parse_bun_lock_skips_nested_packages() {
        // Nested packages like "send/ms" should be skipped when looking for "ms"
        let content = r#"{
  "lockfileVersion": 1,
  "packages": {
    "ms": ["ms@2.1.3", "", {}, "sha512-..."],
    "send/ms": ["ms@2.0.0", "", {}, "sha512-..."],
  }
}"#;

        let clean_content = strip_jsonc_trailing_commas(content);
        let lockfile: BunLockfile = serde_json::from_str(&clean_content).unwrap();
        let packages = lockfile.packages.unwrap();

        // Both should exist in the parsed structure
        assert!(packages.contains_key("ms"));
        assert!(packages.contains_key("send/ms"));

        // But when parsing, we should find the root "ms", not "send/ms"
        let ms = packages.get("ms").unwrap().as_array().unwrap();
        assert_eq!(ms[0].as_str().unwrap(), "ms@2.1.3");
    }
}
