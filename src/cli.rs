use clap::{Parser, Subcommand};
use std::fmt;
use std::str::FromStr;

/// CLI tool that fetches dependency source code for LLM context
#[derive(Parser, Debug)]
#[command(name = "dotdeps")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Remove all .deps/ in current directory
    #[arg(long)]
    pub clean: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Add a dependency to .deps/
    Add {
        /// Dependency specification: <ecosystem>:<package>[@<version>]
        spec: DepSpec,
    },
    /// Remove a dependency from .deps/
    Remove {
        /// Dependency specification: <ecosystem>:<package>
        spec: DepSpec,
    },
    /// List all dependencies in .deps/
    List,
}

/// Supported package ecosystems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecosystem {
    Python,
    Node,
    Go,
    Rust,
    Ruby,
}

impl fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ecosystem::Python => write!(f, "python"),
            Ecosystem::Node => write!(f, "node"),
            Ecosystem::Go => write!(f, "go"),
            Ecosystem::Rust => write!(f, "rust"),
            Ecosystem::Ruby => write!(f, "ruby"),
        }
    }
}

impl FromStr for Ecosystem {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "python" => Ok(Ecosystem::Python),
            "node" | "nodejs" | "npm" => Ok(Ecosystem::Node),
            "go" | "golang" => Ok(Ecosystem::Go),
            "rust" | "cargo" => Ok(Ecosystem::Rust),
            "ruby" | "gem" | "rubygems" => Ok(Ecosystem::Ruby),
            _ => Err(format!(
                "Unknown ecosystem '{}'. Supported: python, node, go, rust, ruby",
                s
            )),
        }
    }
}

/// A dependency specification: ecosystem:package@version
#[derive(Debug, Clone)]
pub struct DepSpec {
    pub ecosystem: Ecosystem,
    pub package: String,
    pub version: Option<String>,
}

impl fmt::Display for DepSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version {
            Some(v) => write!(f, "{}:{}@{}", self.ecosystem, self.package, v),
            None => write!(f, "{}:{}", self.ecosystem, self.package),
        }
    }
}

impl FromStr for DepSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Format: <ecosystem>:<package>[@<version>]
        let Some((ecosystem_str, rest)) = s.split_once(':') else {
            return Err(format!(
                "Invalid format '{}'. Expected: <ecosystem>:<package>[@<version>]",
                s
            ));
        };

        let ecosystem = ecosystem_str.parse::<Ecosystem>()?;

        // Parse package@version or just package
        let (package, version) = if let Some((pkg, ver)) = rest.rsplit_once('@') {
            // Handle scoped packages like @org/pkg@version
            // Check if the @ is part of a scope (starts with @) or a version separator
            if pkg.is_empty() {
                // This means we have something like "@org/pkg" with no version
                // The rsplit_once('@') incorrectly split at the scope
                (rest.to_string(), None)
            } else {
                (pkg.to_string(), Some(ver.to_string()))
            }
        } else {
            (rest.to_string(), None)
        };

        if package.is_empty() {
            return Err("Package name cannot be empty".to_string());
        }

        // Normalize package name to lowercase
        let package = package.to_lowercase();

        // Normalize version: strip 'v' prefix if present
        let version = version.map(|v| v.strip_prefix('v').unwrap_or(&v).to_string());

        Ok(DepSpec {
            ecosystem,
            package,
            version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_dep() {
        let spec: DepSpec = "python:requests".parse().unwrap();
        assert_eq!(spec.ecosystem, Ecosystem::Python);
        assert_eq!(spec.package, "requests");
        assert_eq!(spec.version, None);
    }

    #[test]
    fn test_parse_dep_with_version() {
        let spec: DepSpec = "python:requests@2.31.0".parse().unwrap();
        assert_eq!(spec.ecosystem, Ecosystem::Python);
        assert_eq!(spec.package, "requests");
        assert_eq!(spec.version, Some("2.31.0".to_string()));
    }

    #[test]
    fn test_parse_dep_with_v_prefix() {
        let spec: DepSpec = "python:requests@v2.31.0".parse().unwrap();
        assert_eq!(spec.version, Some("2.31.0".to_string()));
    }

    #[test]
    fn test_parse_scoped_package() {
        let spec: DepSpec = "node:@org/pkg".parse().unwrap();
        assert_eq!(spec.ecosystem, Ecosystem::Node);
        assert_eq!(spec.package, "@org/pkg");
        assert_eq!(spec.version, None);
    }

    #[test]
    fn test_parse_scoped_package_with_version() {
        let spec: DepSpec = "node:@org/pkg@4.17.21".parse().unwrap();
        assert_eq!(spec.ecosystem, Ecosystem::Node);
        assert_eq!(spec.package, "@org/pkg");
        assert_eq!(spec.version, Some("4.17.21".to_string()));
    }

    #[test]
    fn test_parse_go_module() {
        let spec: DepSpec = "go:github.com/org/repo/v2".parse().unwrap();
        assert_eq!(spec.ecosystem, Ecosystem::Go);
        assert_eq!(spec.package, "github.com/org/repo/v2");
        assert_eq!(spec.version, None);
    }

    #[test]
    fn test_parse_go_module_with_version() {
        let spec: DepSpec = "go:github.com/org/repo/v2@1.0.0".parse().unwrap();
        assert_eq!(spec.ecosystem, Ecosystem::Go);
        assert_eq!(spec.package, "github.com/org/repo/v2");
        assert_eq!(spec.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_normalize_package_to_lowercase() {
        let spec: DepSpec = "python:Requests".parse().unwrap();
        assert_eq!(spec.package, "requests");
    }

    #[test]
    fn test_ecosystem_aliases() {
        assert_eq!("npm".parse::<Ecosystem>().unwrap(), Ecosystem::Node);
        assert_eq!("nodejs".parse::<Ecosystem>().unwrap(), Ecosystem::Node);
        assert_eq!("golang".parse::<Ecosystem>().unwrap(), Ecosystem::Go);
        assert_eq!("cargo".parse::<Ecosystem>().unwrap(), Ecosystem::Rust);
        assert_eq!("gem".parse::<Ecosystem>().unwrap(), Ecosystem::Ruby);
    }

    #[test]
    fn test_invalid_format_no_colon() {
        let result = "pythonrequests".parse::<DepSpec>();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_ecosystem() {
        let result = "java:something".parse::<DepSpec>();
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_package() {
        let result = "python:".parse::<DepSpec>();
        assert!(result.is_err());
    }
}
