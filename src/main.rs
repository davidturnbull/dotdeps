mod cache;
mod cli;
mod deps;

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
        // TODO: Clone from repository
        println!("Fetching {} {}...", spec.package, version);

        // For now, just create the cache directory as a placeholder
        // This will be replaced with actual git cloning in task-003
        std::fs::create_dir_all(&cache_path)?;
        std::fs::create_dir_all(cache_path.join(".git"))?;

        println!("  (placeholder - git cloning not yet implemented)");
    }

    // Create symlink in .deps/
    let link_path = deps::link(spec.ecosystem, &spec.package, version)?;
    println!("Created {}", link_path.display());

    Ok(())
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
