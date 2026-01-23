//! API module for accessing Homebrew's cached formula and cask data.
//!
//! Homebrew caches formula and cask data from the API in JWS (JSON Web Signature) format.
//! This module parses that cache to get formula information.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::paths;

/// Bottle file information for a specific platform/arch.
#[derive(Debug, Deserialize, Serialize)]
pub struct BottleFile {
    pub cellar: String,
    pub url: String,
    pub sha256: String,
}

/// Bottle specification for a formula.
#[derive(Debug, Deserialize, Serialize)]
pub struct BottleSpec {
    pub rebuild: i32,
    pub root_url: String,
    pub files: HashMap<String, BottleFile>,
}

/// Bottle information (stable builds).
#[derive(Debug, Deserialize, Serialize)]
pub struct Bottle {
    pub stable: Option<BottleSpec>,
}

/// URL specification for source downloads.
#[derive(Debug, Deserialize, Serialize)]
pub struct UrlSpec {
    pub url: String,
    pub checksum: Option<String>,
    pub tag: Option<String>,
    pub revision: Option<String>,
}

/// URLs for a formula (stable, head, etc.).
#[derive(Debug, Deserialize, Serialize)]
pub struct Urls {
    pub stable: Option<UrlSpec>,
    pub head: Option<UrlSpec>,
}

/// Version information for a formula.
#[derive(Debug, Deserialize, Serialize)]
pub struct Versions {
    pub stable: Option<String>,
    pub head: Option<String>,
}

/// Formula data from the API.
#[derive(Debug, Deserialize, Serialize)]
pub struct FormulaInfo {
    pub name: String,
    pub full_name: String,
    pub tap: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub versioned_formulae: Vec<String>,
    pub desc: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub versions: Versions,
    pub urls: Option<Urls>,
    pub revision: i32,
    pub version_scheme: i32,
    pub bottle: Option<Bottle>,
    #[serde(default)]
    pub keg_only: bool,
    pub keg_only_reason: Option<KegOnlyReason>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub build_dependencies: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub test_dependencies: Vec<String>,
    #[serde(default)]
    pub recommended_dependencies: Vec<String>,
    #[serde(default)]
    pub optional_dependencies: Vec<String>,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    pub caveats: Option<String>,
    #[serde(default)]
    pub installed: Vec<InstalledVersion>,
    pub linked_keg: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub outdated: bool,
    #[serde(default)]
    pub deprecated: bool,
    pub deprecation_date: Option<String>,
    pub deprecation_reason: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    pub disable_date: Option<String>,
    pub disable_reason: Option<String>,
}

/// Keg-only reason information.
#[derive(Debug, Deserialize, Serialize)]
pub struct KegOnlyReason {
    pub reason: String,
    pub explanation: Option<String>,
}

/// Requirement specification.
#[derive(Debug, Deserialize, Serialize)]
pub struct Requirement {
    pub name: String,
    #[serde(default)]
    pub cask: Option<String>,
    #[serde(default)]
    pub download: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub contexts: Vec<String>,
}

/// Installed version information.
#[derive(Debug, Deserialize, Serialize)]
pub struct InstalledVersion {
    pub version: String,
    pub used_options: Vec<String>,
    pub built_as_bottle: bool,
    pub poured_from_bottle: bool,
    pub time: Option<i64>,
    pub runtime_dependencies: Vec<RuntimeDependency>,
    pub installed_as_dependency: bool,
    pub installed_on_request: bool,
}

/// Runtime dependency information.
#[derive(Debug, Deserialize, Serialize)]
pub struct RuntimeDependency {
    pub full_name: String,
    pub version: String,
    pub revision: i32,
    pub pkg_version: String,
    pub declared_directly: bool,
}

/// Cask URL variations for different platforms.
#[derive(Debug, Deserialize, Serialize)]
pub struct CaskVariation {
    pub url: Option<String>,
    pub sha256: Option<String>,
}

/// Cask data from the API.
#[derive(Debug, Deserialize, Serialize)]
pub struct CaskInfo {
    pub token: String,
    pub full_token: String,
    #[serde(default)]
    pub name: Vec<String>,
    pub desc: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    pub url: Option<String>,
    /// Per-platform URL variations
    pub variations: Option<HashMap<String, CaskVariation>>,
}

/// Get the path to the formula API cache.
fn formula_cache_path() -> PathBuf {
    paths::homebrew_cache().join("api/formula.jws.json")
}

/// Get the path to the cask API cache.
fn cask_cache_path() -> PathBuf {
    paths::homebrew_cache().join("api/cask.jws.json")
}

/// Decode a JWS payload.
/// The payload may be base64url encoded or raw JSON depending on the cache format.
fn decode_jws_payload(jws_content: &str) -> Result<String, String> {
    let jws: serde_json::Value =
        serde_json::from_str(jws_content).map_err(|e| format!("Failed to parse JWS JSON: {e}"))?;

    let payload = jws["payload"].as_str().ok_or("No payload field in JWS")?;

    // Check if the payload is raw JSON (starts with [ or {) or base64 encoded
    let trimmed = payload.trim();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        // Raw JSON - return as-is
        return Ok(payload.to_string());
    }

    // Base64url decode - handle padding
    let mut payload_str = payload.to_string();

    // Add padding if needed
    let padding = (4 - payload_str.len() % 4) % 4;
    for _ in 0..padding {
        payload_str.push('=');
    }

    // Replace base64url chars with base64 chars
    let base64_payload = payload_str.replace('-', "+").replace('_', "/");

    // Decode
    let decoded = base64_decode(&base64_payload)?;

    String::from_utf8(decoded).map_err(|e| format!("Invalid UTF-8 in payload: {e}"))
}

/// Simple base64 decoder.
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;

    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }

        let value = ALPHABET
            .iter()
            .position(|&c| c == byte)
            .ok_or_else(|| format!("Invalid base64 character: {}", byte as char))?
            as u32;

        buffer = (buffer << 6) | value;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    Ok(result)
}

/// Load formula data from the API cache.
pub fn load_formula_cache() -> Result<HashMap<String, FormulaInfo>, String> {
    let cache_path = formula_cache_path();

    if !cache_path.exists() {
        return Err("Formula API cache not found. Run `brew update` first.".into());
    }

    let jws_content = fs::read_to_string(&cache_path)
        .map_err(|e| format!("Failed to read formula cache: {e}"))?;

    let payload = decode_jws_payload(&jws_content)?;

    // The payload is a JSON array, not an object
    let formulas_list: Vec<FormulaInfo> =
        serde_json::from_str(&payload).map_err(|e| format!("Failed to parse formula data: {e}"))?;

    // Convert to HashMap keyed by name
    let mut formulas = HashMap::new();
    for formula in formulas_list {
        formulas.insert(formula.name.clone(), formula);
    }

    Ok(formulas)
}

/// Load cask data from the API cache.
pub fn load_cask_cache() -> Result<HashMap<String, CaskInfo>, String> {
    let cache_path = cask_cache_path();

    if !cache_path.exists() {
        return Err("Cask API cache not found. Run `brew update` first.".into());
    }

    let jws_content =
        fs::read_to_string(&cache_path).map_err(|e| format!("Failed to read cask cache: {e}"))?;

    let payload = decode_jws_payload(&jws_content)?;

    // The payload is a JSON array, not an object
    let casks_list: Vec<CaskInfo> =
        serde_json::from_str(&payload).map_err(|e| format!("Failed to parse cask data: {e}"))?;

    // Convert to HashMap keyed by token
    let mut casks = HashMap::new();
    for cask in casks_list {
        casks.insert(cask.token.clone(), cask);
    }

    Ok(casks)
}

/// Get information for a specific formula.
pub fn get_formula(name: &str) -> Result<FormulaInfo, String> {
    let formulas = load_formula_cache()?;

    // Try exact match first
    if let Some(formula) = formulas.get(name) {
        // We need to clone since we're returning ownership
        return Ok(clone_formula_info(formula));
    }

    // Try normalized name (strip tap prefix)
    let normalized = crate::formula::normalize_name(name);
    if let Some(formula) = formulas.get(normalized) {
        return Ok(clone_formula_info(formula));
    }

    Err(format!("No available formula with the name \"{name}\"."))
}

/// Get information for a specific cask.
pub fn get_cask(name: &str) -> Result<CaskInfo, String> {
    let casks = load_cask_cache()?;

    // Try exact match first
    if let Some(cask) = casks.get(name) {
        return Ok(clone_cask_info(cask));
    }

    // Try without tap prefix
    let normalized = if name.contains('/') {
        name.rsplit('/').next().unwrap_or(name)
    } else {
        name
    };

    if let Some(cask) = casks.get(normalized) {
        return Ok(clone_cask_info(cask));
    }

    Err(format!("No available cask with the name \"{name}\"."))
}

// Helper to clone FormulaInfo (since Deserialize doesn't give us Clone)
fn clone_formula_info(info: &FormulaInfo) -> FormulaInfo {
    FormulaInfo {
        name: info.name.clone(),
        full_name: info.full_name.clone(),
        tap: info.tap.clone(),
        aliases: info.aliases.clone(),
        versioned_formulae: info.versioned_formulae.clone(),
        desc: info.desc.clone(),
        license: info.license.clone(),
        homepage: info.homepage.clone(),
        versions: Versions {
            stable: info.versions.stable.clone(),
            head: info.versions.head.clone(),
        },
        urls: info.urls.as_ref().map(|u| Urls {
            stable: u.stable.as_ref().map(|s| UrlSpec {
                url: s.url.clone(),
                checksum: s.checksum.clone(),
                tag: s.tag.clone(),
                revision: s.revision.clone(),
            }),
            head: u.head.as_ref().map(|h| UrlSpec {
                url: h.url.clone(),
                checksum: h.checksum.clone(),
                tag: h.tag.clone(),
                revision: h.revision.clone(),
            }),
        }),
        revision: info.revision,
        version_scheme: info.version_scheme,
        bottle: info.bottle.as_ref().map(|b| Bottle {
            stable: b.stable.as_ref().map(|s| BottleSpec {
                rebuild: s.rebuild,
                root_url: s.root_url.clone(),
                files: s
                    .files
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            BottleFile {
                                cellar: v.cellar.clone(),
                                url: v.url.clone(),
                                sha256: v.sha256.clone(),
                            },
                        )
                    })
                    .collect(),
            }),
        }),
        keg_only: info.keg_only,
        keg_only_reason: info.keg_only_reason.as_ref().map(|r| KegOnlyReason {
            reason: r.reason.clone(),
            explanation: r.explanation.clone(),
        }),
        options: info.options.clone(),
        build_dependencies: info.build_dependencies.clone(),
        dependencies: info.dependencies.clone(),
        test_dependencies: info.test_dependencies.clone(),
        recommended_dependencies: info.recommended_dependencies.clone(),
        optional_dependencies: info.optional_dependencies.clone(),
        requirements: info
            .requirements
            .iter()
            .map(|r| Requirement {
                name: r.name.clone(),
                cask: r.cask.clone(),
                download: r.download.clone(),
                version: r.version.clone(),
                contexts: r.contexts.clone(),
            })
            .collect(),
        caveats: info.caveats.clone(),
        installed: info
            .installed
            .iter()
            .map(|i| InstalledVersion {
                version: i.version.clone(),
                used_options: i.used_options.clone(),
                built_as_bottle: i.built_as_bottle,
                poured_from_bottle: i.poured_from_bottle,
                time: i.time,
                runtime_dependencies: i
                    .runtime_dependencies
                    .iter()
                    .map(|d| RuntimeDependency {
                        full_name: d.full_name.clone(),
                        version: d.version.clone(),
                        revision: d.revision,
                        pkg_version: d.pkg_version.clone(),
                        declared_directly: d.declared_directly,
                    })
                    .collect(),
                installed_as_dependency: i.installed_as_dependency,
                installed_on_request: i.installed_on_request,
            })
            .collect(),
        linked_keg: info.linked_keg.clone(),
        pinned: info.pinned,
        outdated: info.outdated,
        deprecated: info.deprecated,
        deprecation_date: info.deprecation_date.clone(),
        deprecation_reason: info.deprecation_reason.clone(),
        disabled: info.disabled,
        disable_date: info.disable_date.clone(),
        disable_reason: info.disable_reason.clone(),
    }
}

fn clone_cask_info(info: &CaskInfo) -> CaskInfo {
    CaskInfo {
        token: info.token.clone(),
        full_token: info.full_token.clone(),
        name: info.name.clone(),
        desc: info.desc.clone(),
        homepage: info.homepage.clone(),
        version: info.version.clone(),
        url: info.url.clone(),
        variations: info.variations.as_ref().map(|v| {
            v.iter()
                .map(|(k, val)| {
                    (
                        k.clone(),
                        CaskVariation {
                            url: val.url.clone(),
                            sha256: val.sha256.clone(),
                        },
                    )
                })
                .collect()
        }),
    }
}

/// Load all formulae from the API cache.
pub fn load_all_formulae() -> Result<Vec<FormulaInfo>, String> {
    let formulas = load_formula_cache()?;
    Ok(formulas.into_values().collect())
}

/// Load all casks from the API cache.
pub fn load_all_casks() -> Result<Vec<CaskInfo>, String> {
    let casks = load_cask_cache()?;
    Ok(casks.into_values().collect())
}
