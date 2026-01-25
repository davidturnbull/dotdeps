mod cache;
mod cli;
mod deps;
mod git;

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
    // Verify cache is writable (fail fast)
    cache::ensure_writable()?;

    let version = spec.version.as_deref().unwrap_or_else(|| {
        // TODO: Look up version from lockfile
        eprintln!(
            "Error: No version specified. Specify version explicitly: dotdeps add {}@<version>",
            spec
        );
        std::process::exit(1);
    });

    let cache_path = cache::package_dir(spec.ecosystem, &spec.package, version)?;

    // Check if already cached
    if cache::exists(spec.ecosystem, &spec.package, version)? {
        println!("Using cached {} {}", spec.package, version);
    } else {
        // Detect repository URL
        let repo_url = detect_repo_url(spec.ecosystem, &spec.package)?;

        println!("Fetching {} {}...", spec.package, version);

        // Clone the repository
        let result = git::clone(&repo_url, version, &cache_path)?;

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
    let link_path = deps::link(spec.ecosystem, &spec.package, version)?;
    println!("Created {}", link_path.display());

    Ok(())
}

/// Detect the repository URL for a package
///
/// For now, this only handles Go modules (where the package path is the repo)
/// and returns an error for other ecosystems until task-004 implements registry lookups.
fn detect_repo_url(
    ecosystem: cli::Ecosystem,
    package: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    match ecosystem {
        cli::Ecosystem::Go => {
            // Go modules: package path is typically the repo
            // e.g., github.com/gin-gonic/gin -> https://github.com/gin-gonic/gin
            if package.starts_with("github.com/") {
                // Strip any version suffix like /v2
                let repo_path = package
                    .strip_suffix("/v2")
                    .or_else(|| package.strip_suffix("/v3"))
                    .or_else(|| package.strip_suffix("/v4"))
                    .or_else(|| package.strip_suffix("/v5"))
                    .unwrap_or(package);
                Ok(format!("https://{}.git", repo_path))
            } else if package.starts_with("golang.org/x/") {
                // golang.org/x/* -> go.googlesource.com/[name]
                let name = package.strip_prefix("golang.org/x/").unwrap();
                Ok(format!("https://go.googlesource.com/{}.git", name))
            } else {
                Err(format!(
                    "Repository URL not found for go:{}. Add override to ~/.config/dotdeps/config.json",
                    package
                ).into())
            }
        }
        _ => {
            // Other ecosystems need registry lookup (task-004)
            Err(format!(
                "Repository detection for {} not yet implemented. Specify version explicitly or add override to ~/.config/dotdeps/config.json",
                ecosystem
            ).into())
        }
    }
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
