mod cache;
mod cli;
mod config;
mod deps;
mod git;
mod go;
mod node;
mod python;
mod ruby;
mod rust;
mod swift;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    // Handle --clean flag (mutually exclusive with subcommands)
    if cli.clean {
        if cli.command.is_some() {
            eprintln!("Error: --clean cannot be used with a subcommand");
            std::process::exit(1);
        }
        if let Err(e) = run_clean() {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    let result = match cli.command {
        Some(Command::Add { spec }) => run_add(spec),
        Some(Command::Remove { spec }) => run_remove(spec),
        Some(Command::List) => run_list(),
        None => {
            eprintln!("No command specified. Use --help for usage information.");
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_add(spec: cli::DepSpec) -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = config::Config::load()?;

    // Verify cache is writable (fail fast)
    cache::ensure_writable()?;

    // Resolve version: use explicit version, or look up from lockfile
    let version_info = match spec.version.as_deref() {
        Some(v) => cli::VersionInfo::Version(v.to_string()),
        None => lookup_version(spec.ecosystem, &spec.package)?,
    };

    // Handle different version types
    match &version_info {
        cli::VersionInfo::LocalPath { path } => {
            // Skip local path dependencies silently
            println!(
                "Skipping local dependency {} (path: {})",
                spec.package, path
            );
            return Ok(());
        }
        cli::VersionInfo::Git { url, commit } => {
            // Git dependency - clone from URL, use commit as version
            run_add_git_dep(spec.ecosystem, &spec.package, url, commit, &config)?;
        }
        cli::VersionInfo::Version(version) => {
            // Regular version - use registry detection
            run_add_registry_dep(spec.ecosystem, &spec.package, version, &config)?;
        }
    }

    Ok(())
}

/// Add a git dependency (URL + commit hash)
fn run_add_git_dep(
    ecosystem: cli::Ecosystem,
    package: &str,
    url: &str,
    commit: &str,
    config: &config::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    // For git deps, use the commit hash as the version
    // Truncate long commit hashes for display/cache path
    let version = if commit.len() > 12 {
        &commit[..12]
    } else {
        commit
    };

    let cache_path = cache::package_dir(ecosystem, package, version)?;

    // Check if already cached
    if cache::exists(ecosystem, package, version)? {
        println!("Using cached {} {} (git)", package, version);
    } else {
        println!("Fetching {} {} (git)...", package, version);

        // Clone at specific commit
        let result = git::clone_at_commit(url, commit, &cache_path)?;
        println!("  cloned at {}", result.cloned_ref);

        // Run cache eviction if over limit
        run_cache_eviction(config)?;
    }

    // Create symlink in .deps/
    let link_path = deps::link(ecosystem, package, version)?;
    println!("Created {}", link_path.display());

    Ok(())
}

/// Add a regular registry dependency (version string)
fn run_add_registry_dep(
    ecosystem: cli::Ecosystem,
    package: &str,
    version: &str,
    config: &config::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let cache_path = cache::package_dir(ecosystem, package, version)?;

    // Check if already cached
    if cache::exists(ecosystem, package, version)? {
        println!("Using cached {} {}", package, version);
    } else {
        // Detect repository URL (check config override first)
        let repo_url = detect_repo_url(ecosystem, package, config)?;

        println!("Fetching {} {}...", package, version);

        // Clone the repository
        let result = git::clone(&repo_url, version, package, &cache_path)?;

        if result.used_default_branch {
            eprintln!(
                "Warning: No tag found for version {}, cloned {}",
                version, result.cloned_ref
            );
        } else {
            println!("  cloned at {}", result.cloned_ref);
        }

        // Run cache eviction if over limit
        run_cache_eviction(config)?;
    }

    // Create symlink in .deps/
    let link_path = deps::link(ecosystem, package, version)?;
    println!("Created {}", link_path.display());

    Ok(())
}

/// Run cache eviction if cache exceeds configured limit
fn run_cache_eviction(config: &config::Config) -> Result<(), Box<dyn std::error::Error>> {
    let limit = config.cache_limit_bytes();
    if limit == 0 {
        // No limit configured (0 means unlimited)
        return Ok(());
    }

    let evicted = cache::evict_to_limit(limit)?;

    if !evicted.is_empty() {
        eprintln!(
            "Cache eviction: removed {} old entries to stay under {}GB limit",
            evicted.len(),
            config.cache_limit_gb
        );
    }

    Ok(())
}

/// Look up package version from ecosystem-specific lockfile
fn lookup_version(
    ecosystem: cli::Ecosystem,
    package: &str,
) -> Result<cli::VersionInfo, Box<dyn std::error::Error>> {
    match ecosystem {
        cli::Ecosystem::Python => python::find_version(package).map_err(|e| e.into()),
        cli::Ecosystem::Node => node::find_version(package).map_err(|e| e.into()),
        cli::Ecosystem::Go => go::find_version(package).map_err(|e| e.into()),
        cli::Ecosystem::Rust => rust::find_version(package).map_err(|e| e.into()),
        cli::Ecosystem::Ruby => ruby::find_version(package).map_err(|e| e.into()),
        cli::Ecosystem::Swift => swift::find_version(package).map_err(|e| e.into()),
    }
}

/// Detect the repository URL for a package
///
/// Checks config override first, then falls back to ecosystem-specific detection.
fn detect_repo_url(
    ecosystem: cli::Ecosystem,
    package: &str,
    config: &config::Config,
) -> Result<String, Box<dyn std::error::Error>> {
    // Check for config override first
    if let Some(repo_url) = config.repo_override(ecosystem, package) {
        return Ok(repo_url.to_string());
    }

    // Fall back to ecosystem-specific detection
    match ecosystem {
        cli::Ecosystem::Python => python::detect_repo_url(package).map_err(|e| e.into()),
        cli::Ecosystem::Node => node::detect_repo_url(package).map_err(|e| e.into()),
        cli::Ecosystem::Go => detect_go_repo_url(package),
        cli::Ecosystem::Rust => rust::detect_repo_url(package).map_err(|e| e.into()),
        cli::Ecosystem::Ruby => ruby::detect_repo_url(package).map_err(|e| e.into()),
        cli::Ecosystem::Swift => swift::detect_repo_url(package).map_err(|e| e.into()),
    }
}

/// Detect repository URL for Go modules
///
/// Go modules have the repo URL embedded in their module path:
/// - github.com/org/repo -> https://github.com/org/repo.git
/// - golang.org/x/name -> https://go.googlesource.com/name.git
fn detect_go_repo_url(package: &str) -> Result<String, Box<dyn std::error::Error>> {
    if package.starts_with("github.com/") {
        // Strip any version suffix like /v2, /v3, etc.
        let repo_path = strip_go_version_suffix(package);
        Ok(format!("https://{}.git", repo_path))
    } else if package.starts_with("golang.org/x/") {
        let name = package.strip_prefix("golang.org/x/").unwrap();
        // Also strip version suffix from golang.org packages
        let name = strip_go_version_suffix(name);
        Ok(format!("https://go.googlesource.com/{}.git", name))
    } else {
        Err(format!(
            "Repository URL not found for go:{}. Add override to ~/.config/dotdeps/config.json",
            package
        )
        .into())
    }
}

/// Strip Go module version suffix (/v2, /v3, etc.)
fn strip_go_version_suffix(path: &str) -> &str {
    // Check for /vN suffix where N is a digit
    if let Some(idx) = path.rfind("/v") {
        let suffix = &path[idx + 2..];
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            return &path[..idx];
        }
    }
    path
}

fn run_remove(spec: cli::DepSpec) -> Result<(), Box<dyn std::error::Error>> {
    deps::remove(spec.ecosystem, &spec.package)?;
    println!("Removed {}:{}", spec.ecosystem, spec.package);
    Ok(())
}

fn run_list() -> Result<(), Box<dyn std::error::Error>> {
    let entries = deps::list()?;

    if entries.is_empty() {
        println!("No dependencies in .deps/");
        return Ok(());
    }

    for entry in entries {
        let status = if entry.is_broken {
            " (broken - cache evicted)"
        } else {
            ""
        };
        println!(
            "{}:{}@{}{}",
            entry.ecosystem, entry.package, entry.version, status
        );
    }

    Ok(())
}

fn run_clean() -> Result<(), Box<dyn std::error::Error>> {
    deps::clean()?;
    println!("Removed .deps/");
    Ok(())
}
