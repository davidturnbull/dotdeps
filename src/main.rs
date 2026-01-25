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
    let version = match spec.version.as_deref() {
        Some(v) => v.to_string(),
        None => lookup_version(spec.ecosystem, &spec.package)?,
    };

    let cache_path = cache::package_dir(spec.ecosystem, &spec.package, &version)?;

    // Check if already cached
    if cache::exists(spec.ecosystem, &spec.package, &version)? {
        println!("Using cached {} {}", spec.package, version);
    } else {
        // Detect repository URL (check config override first)
        let repo_url = detect_repo_url(spec.ecosystem, &spec.package, &config)?;

        println!("Fetching {} {}...", spec.package, version);

        // Clone the repository
        let result = git::clone(&repo_url, &version, &cache_path)?;

        if result.used_default_branch {
            eprintln!(
                "Warning: No tag found for version {}, cloned {}",
                version, result.cloned_ref
            );
        } else {
            println!("  cloned at {}", result.cloned_ref);
        }
    }

    // Create symlink in .deps/
    let link_path = deps::link(spec.ecosystem, &spec.package, &version)?;
    println!("Created {}", link_path.display());

    Ok(())
}

/// Look up package version from ecosystem-specific lockfile
fn lookup_version(
    ecosystem: cli::Ecosystem,
    package: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    match ecosystem {
        cli::Ecosystem::Python => python::find_version(package).map_err(|e| e.into()),
        cli::Ecosystem::Node => node::find_version(package).map_err(|e| e.into()),
        cli::Ecosystem::Go => go::find_version(package).map_err(|e| e.into()),
        cli::Ecosystem::Rust => rust::find_version(package).map_err(|e| e.into()),
        cli::Ecosystem::Ruby => ruby::find_version(package).map_err(|e| e.into()),
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
