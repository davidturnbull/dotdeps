//! Configuration file support for dotdeps
//!
//! Reads configuration from `~/.config/dotdeps/config.json`:
//!
//! ```json
//! {
//!   "cache_limit_gb": 5,
//!   "overrides": {
//!     "python": {
//!       "some-obscure-lib": {
//!         "repo": "https://github.com/someone/some-obscure-lib"
//!       }
//!     }
//!   }
//! }
//! ```

use crate::cli::Ecosystem;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Default cache limit in GB
const DEFAULT_CACHE_LIMIT_GB: f64 = 5.0;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Cannot determine config directory. HOME environment variable not set.")]
    NoConfigDir,

    #[error("Failed to read config file {path}: {source}")]
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse config file {path}: {source}")]
    ParseError {
        path: PathBuf,
        source: serde_json::Error,
    },
}

/// Package-specific override configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PackageOverride {
    /// Custom repository URL for this package
    pub repo: Option<String>,
}

/// Top-level configuration structure
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// Maximum cache size in GB (default: 5)
    #[serde(default = "default_cache_limit")]
    pub cache_limit_gb: f64,

    /// Per-ecosystem, per-package overrides
    /// Structure: { "ecosystem": { "package": { "repo": "url" } } }
    #[serde(default)]
    pub overrides: HashMap<String, HashMap<String, PackageOverride>>,
}

fn default_cache_limit() -> f64 {
    DEFAULT_CACHE_LIMIT_GB
}

impl Config {
    /// Load configuration from the default path or return defaults if not found
    pub fn load() -> Result<Self, ConfigError> {
        let path = config_path()?;

        if !path.exists() {
            return Ok(Config::default());
        }

        let content = std::fs::read_to_string(&path).map_err(|source| ConfigError::ReadError {
            path: path.clone(),
            source,
        })?;

        serde_json::from_str(&content).map_err(|source| ConfigError::ParseError { path, source })
    }

    /// Get the cache limit in bytes
    pub fn cache_limit_bytes(&self) -> u64 {
        (self.cache_limit_gb * 1024.0 * 1024.0 * 1024.0) as u64
    }

    /// Look up a custom repository URL override for an ecosystem/package pair
    pub fn repo_override(&self, ecosystem: Ecosystem, package: &str) -> Option<&str> {
        let ecosystem_key = ecosystem.to_string();
        // Normalize package name for lookup (lowercase)
        let package_lower = package.to_lowercase();

        self.overrides
            .get(&ecosystem_key)
            .and_then(|packages| packages.get(&package_lower))
            .and_then(|override_cfg| override_cfg.repo.as_deref())
    }
}

/// Returns the config file path: `~/.config/dotdeps/config.json`
pub fn config_path() -> Result<PathBuf, ConfigError> {
    // Use XDG_CONFIG_HOME if set, otherwise fall back to ~/.config
    let config_base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join(".config"))
                .unwrap_or_default()
        });

    if config_base.as_os_str().is_empty() {
        return Err(ConfigError::NoConfigDir);
    }

    Ok(config_base.join("dotdeps").join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.cache_limit_gb, 0.0); // serde default doesn't apply to Default trait
        assert!(config.overrides.is_empty());
    }

    #[test]
    fn test_parse_minimal_config() {
        let json = r#"{}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.cache_limit_gb, DEFAULT_CACHE_LIMIT_GB);
        assert!(config.overrides.is_empty());
    }

    #[test]
    fn test_parse_full_config() {
        let json = r#"{
            "cache_limit_gb": 10,
            "overrides": {
                "python": {
                    "obscure-lib": {
                        "repo": "https://github.com/someone/obscure-lib"
                    }
                },
                "node": {
                    "@private/pkg": {
                        "repo": "https://github.com/org/private-pkg"
                    }
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.cache_limit_gb, 10.0);

        let python_overrides = config.overrides.get("python").unwrap();
        assert_eq!(
            python_overrides.get("obscure-lib").unwrap().repo,
            Some("https://github.com/someone/obscure-lib".to_string())
        );

        let node_overrides = config.overrides.get("node").unwrap();
        assert_eq!(
            node_overrides.get("@private/pkg").unwrap().repo,
            Some("https://github.com/org/private-pkg".to_string())
        );
    }

    #[test]
    fn test_cache_limit_bytes() {
        let config = Config {
            cache_limit_gb: 5.0,
            overrides: HashMap::new(),
        };
        // 5 GB = 5 * 1024 * 1024 * 1024 = 5368709120 bytes
        assert_eq!(config.cache_limit_bytes(), 5368709120);
    }

    #[test]
    fn test_repo_override_lookup() {
        let json = r#"{
            "overrides": {
                "python": {
                    "obscure-lib": {
                        "repo": "https://github.com/someone/obscure-lib"
                    }
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();

        // Exact match
        assert_eq!(
            config.repo_override(Ecosystem::Python, "obscure-lib"),
            Some("https://github.com/someone/obscure-lib")
        );

        // Case insensitive lookup
        assert_eq!(
            config.repo_override(Ecosystem::Python, "Obscure-Lib"),
            Some("https://github.com/someone/obscure-lib")
        );

        // Not found
        assert_eq!(config.repo_override(Ecosystem::Python, "other-lib"), None);
        assert_eq!(config.repo_override(Ecosystem::Node, "obscure-lib"), None);
    }

    #[test]
    fn test_config_path() {
        let path = config_path().unwrap();
        assert!(path.to_string_lossy().contains("dotdeps/config.json"));
    }
}
