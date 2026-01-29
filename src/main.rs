mod cache;
mod cli;
mod config;
mod context;
mod deps;
mod git;
mod go;
mod init;
mod lockfile;
mod node;
mod output;
mod python;
mod ruby;
mod rust;
mod swift;
mod update;

use clap::Parser;
use cli::{Cli, Command};
use output::{
    AddResult, CleanResult, InitAction, InitOutput, ListEntry, ListResult, RemoveResult,
    SkipResult, UpdateCheckOutput, UpdateOutput,
};

fn main() {
    let cli = Cli::parse();
    let json_output = cli.json;
    let dry_run = cli.dry_run;

    // Check for updates periodically (skip for update command itself and JSON output)
    // Capture the message now to avoid blocking on network after command completes
    let is_update_cmd = matches!(cli.command, Some(Command::Update { .. }));
    let update_msg = if !is_update_cmd && !json_output {
        update::maybe_notify_update()
    } else {
        None
    };

    let result = match cli.command {
        Some(Command::Init {
            skip_gitignore,
            skip_instructions,
        }) => run_init_cmd(skip_gitignore, skip_instructions, json_output, dry_run),
        Some(Command::Add { spec }) => run_add(spec, json_output, dry_run),
        Some(Command::Remove { spec }) => run_remove(spec, json_output, dry_run),
        Some(Command::List) => run_list(json_output),
        Some(Command::Context) => run_context(json_output),
        Some(Command::Clean) => run_clean(json_output, dry_run),
        Some(Command::Update { check }) => run_update(check, json_output),
        None => {
            eprintln!("No command specified. Use --help for usage information.");
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Only show update notification after successful command
    if let Some(msg) = update_msg {
        eprintln!("\n{}", msg);
    }
}

fn run_init_cmd(
    skip_gitignore: bool,
    skip_instructions: bool,
    json_output: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use init::{ActionResult, InitConfig};

    let config = InitConfig {
        skip_gitignore,
        skip_instructions,
        dry_run,
    };

    let result = init::run_init(config)?;
    let prefix = if dry_run { "[dry-run] " } else { "" };

    if json_output {
        let mut actions = Vec::new();

        // .deps/ action
        match &result.deps_dir {
            ActionResult::Created(msg) => {
                actions.push(InitAction::new("create_deps_dir", "created").with_message(msg));
            }
            ActionResult::AlreadyExists(msg) => {
                actions.push(InitAction::new("create_deps_dir", "exists").with_message(msg));
            }
            ActionResult::Skipped => {}
        }

        // .gitignore action
        match &result.gitignore {
            ActionResult::Created(msg) => {
                actions.push(
                    InitAction::new("update_gitignore", "created")
                        .with_file(".gitignore")
                        .with_message(msg),
                );
            }
            ActionResult::AlreadyExists(msg) => {
                actions.push(
                    InitAction::new("update_gitignore", "exists")
                        .with_file(".gitignore")
                        .with_message(msg),
                );
            }
            ActionResult::Skipped => {
                actions.push(InitAction::new("update_gitignore", "skipped"));
            }
        }

        // instructions action
        match &result.instructions {
            ActionResult::Created(msg) => {
                let mut action = InitAction::new("add_instructions", "created").with_message(msg);
                if let Some(ref file) = result.instructions_file {
                    action = action.with_file(file);
                }
                actions.push(action);
            }
            ActionResult::AlreadyExists(msg) => {
                let mut action = InitAction::new("add_instructions", "exists").with_message(msg);
                if let Some(ref file) = result.instructions_file {
                    action = action.with_file(file);
                }
                actions.push(action);
            }
            ActionResult::Skipped => {
                actions.push(InitAction::new("add_instructions", "skipped"));
            }
        }

        let output = InitOutput {
            initialized: !result.already_initialized(),
            actions,
            dry_run,
        };
        output::print_json(&output);
    } else if result.already_initialized() {
        println!("\ndotdeps is already initialized. Nothing to do.");
    } else {
        println!("\nInitializing dotdeps...\n");

        // Step 1: .deps/ directory
        println!("[1/3] Creating .deps/ directory");
        match &result.deps_dir {
            ActionResult::Created(msg) => println!("{}      {}", prefix, msg),
            ActionResult::AlreadyExists(msg) => println!("      {}", msg),
            ActionResult::Skipped => {}
        }

        // Step 2: .gitignore
        if !skip_gitignore {
            println!("\n[2/3] Updating .gitignore");
            match &result.gitignore {
                ActionResult::Created(msg) => println!("{}      {}", prefix, msg),
                ActionResult::AlreadyExists(msg) => println!("      {}", msg),
                ActionResult::Skipped => {}
            }
        }

        // Step 3: instructions
        if !skip_instructions {
            println!("\n[3/3] Adding usage instructions");
            if let Some(ref file) = result.instructions_file {
                match &result.instructions {
                    ActionResult::AlreadyExists(_) => {
                        println!("      {} already has dotdeps instructions", file);
                    }
                    ActionResult::Created(msg) => {
                        // Check if msg indicates we added to an existing file vs created new
                        let was_existing = msg.starts_with("Added");
                        if was_existing {
                            println!("      Detected {}", file);
                        }
                        println!("{}      {}", prefix, msg);
                    }
                    ActionResult::Skipped => {}
                }
            }
        }

        println!("\nDone! dotdeps is ready to use.");
        println!("\nQuick start:");
        println!("  dotdeps add python:requests    # Fetch a dependency");
        println!("  dotdeps list                   # See what's fetched");
        println!("  dotdeps context                # Show LLM instructions");
    }

    Ok(())
}

fn run_add(
    spec: cli::DepSpec,
    json_output: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = config::Config::load()?;

    // Verify cache is writable (fail fast) - skip in dry-run mode
    if !dry_run {
        cache::ensure_writable()?;
    }

    // Resolve version: use explicit version, or look up from lockfile
    let version_info = match spec.version.as_deref() {
        Some(v) => cli::VersionInfo::Version(v.to_string()),
        None => lookup_version(spec.ecosystem, &spec.package)?,
    };

    // Handle different version types
    match &version_info {
        cli::VersionInfo::LocalPath { path } => {
            // Skip local path dependencies
            if json_output {
                output::print_json(&SkipResult::local_path(spec.ecosystem, &spec.package, path));
            } else {
                let prefix = if dry_run { "[dry-run] " } else { "" };
                println!(
                    "{}Skipping local dependency {} (path: {})",
                    prefix, spec.package, path
                );
            }
            return Ok(());
        }
        cli::VersionInfo::Git { url, commit } => {
            // Git dependency - clone from URL, use commit as version
            run_add_git_dep(
                spec.ecosystem,
                &spec.package,
                url,
                commit,
                &config,
                json_output,
                dry_run,
            )?;
        }
        cli::VersionInfo::Version(version) => {
            // Regular version - use registry detection
            run_add_registry_dep(
                spec.ecosystem,
                &spec.package,
                version,
                &config,
                json_output,
                dry_run,
            )?;
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
    json_output: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // For git deps, use the commit hash as the version
    // Truncate long commit hashes for display/cache path
    let version = if commit.len() > 12 {
        &commit[..12]
    } else {
        commit
    };

    let prefix = if dry_run { "[dry-run] " } else { "" };
    let cache_path = cache::package_dir(ecosystem, package, version)?;

    // Check if already cached
    let (cached, cloned_ref) = if cache::exists(ecosystem, package, version)? {
        if !json_output {
            println!("{}Using cached {} {} (git)", prefix, package, version);
        }
        (true, None)
    } else {
        if !json_output {
            println!("{}Fetching {} {} (git)...", prefix, package, version);
        }

        if dry_run {
            // In dry-run mode, skip actual cloning
            if !json_output {
                println!("{}  cloned at {}", prefix, commit);
            }
            (false, Some(commit.to_string()))
        } else {
            // Clone at specific commit
            let result = git::clone_at_commit(url, commit, &cache_path)?;
            if !json_output {
                println!("  cloned at {}", result.cloned_ref);
            }

            // Run cache eviction if over limit
            run_cache_eviction(config, &cache_path, json_output)?;

            (false, Some(result.cloned_ref))
        }
    };

    // Calculate link path (but don't create in dry-run mode)
    let link_path = deps::link_path(ecosystem, package);

    if !dry_run {
        // Create symlink in .deps/
        deps::link(ecosystem, package, version)?;
    }

    if json_output {
        let mut result = AddResult::new(
            ecosystem,
            package,
            version,
            &link_path.display().to_string(),
            cached,
        );
        if let Some(ref cloned) = cloned_ref {
            result = result.with_cloned_ref(cloned);
        }
        if dry_run {
            result = result.with_dry_run();
        }
        output::print_json(&result);
    } else {
        println!("{}Created {}", prefix, link_path.display());
    }

    Ok(())
}

/// Add a regular registry dependency (version string)
fn run_add_registry_dep(
    ecosystem: cli::Ecosystem,
    package: &str,
    version: &str,
    config: &config::Config,
    json_output: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = if dry_run { "[dry-run] " } else { "" };
    let cache_path = cache::package_dir(ecosystem, package, version)?;

    // Check if already cached
    let (cached, cloned_ref, warning) = if cache::exists(ecosystem, package, version)? {
        if !json_output {
            println!("{}Using cached {} {}", prefix, package, version);
        }
        (true, None, None)
    } else {
        // Detect repository URL (check config override first)
        let repo_url = detect_repo_url(ecosystem, package, config)?;

        if !json_output {
            println!("{}Fetching {} {}...", prefix, package, version);
        }

        if dry_run {
            // In dry-run mode, skip actual cloning
            if !json_output {
                println!("{}  cloned at {}", prefix, version);
            }
            (false, None, None)
        } else {
            // Clone the repository
            let result = git::clone(&repo_url, version, package, &cache_path)?;

            let warning = if result.used_default_branch {
                let msg = format!(
                    "No tag found for version {}, cloned {}",
                    version, result.cloned_ref
                );
                if !json_output {
                    eprintln!("Warning: {}", msg);
                }
                Some(msg)
            } else {
                if !json_output {
                    println!("  cloned at {}", result.cloned_ref);
                }
                None
            };

            // Run cache eviction if over limit
            run_cache_eviction(config, &cache_path, json_output)?;

            (false, Some(result.cloned_ref), warning)
        }
    };

    // Calculate link path (but don't create in dry-run mode)
    let link_path = deps::link_path(ecosystem, package);

    if !dry_run {
        // Create symlink in .deps/
        deps::link(ecosystem, package, version)?;
    }

    if json_output {
        let mut result = AddResult::new(
            ecosystem,
            package,
            version,
            &link_path.display().to_string(),
            cached,
        );
        if let Some(ref cloned) = cloned_ref {
            result = result.with_cloned_ref(cloned);
        }
        if let Some(ref warn) = warning {
            result = result.with_warning(warn);
        }
        if dry_run {
            result = result.with_dry_run();
        }
        output::print_json(&result);
    } else {
        println!("{}Created {}", prefix, link_path.display());
    }

    Ok(())
}

/// Run cache eviction if cache exceeds configured limit
///
/// `new_entry` is the path to the newly added cache entry, which will be
/// excluded from eviction. If the new entry alone exceeds the cache limit,
/// returns an error.
fn run_cache_eviction(
    config: &config::Config,
    new_entry: &std::path::PathBuf,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let limit = config.cache_limit_bytes();
    if limit == 0 {
        // No limit configured (0 means unlimited)
        return Ok(());
    }

    // Check if the new entry alone exceeds the limit
    let entry_size = cache::entry_size(new_entry);
    if entry_size > limit {
        return Err(cache::CacheError::CacheTooSmall {
            limit_bytes: limit,
            entry_bytes: entry_size,
        }
        .into());
    }

    let evicted = cache::evict_to_limit(limit, Some(new_entry))?;

    if !evicted.is_empty() && !json_output {
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

fn run_remove(
    spec: cli::DepSpec,
    json_output: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = if dry_run { "[dry-run] " } else { "" };

    let removed = if dry_run {
        // Check if it would be removed
        deps::package_path(spec.ecosystem, &spec.package).exists()
    } else {
        deps::remove(spec.ecosystem, &spec.package)?
    };

    if json_output {
        let mut result = RemoveResult::new(spec.ecosystem, &spec.package, removed);
        if dry_run {
            result = result.with_dry_run();
        }
        output::print_json(&result);
    } else if removed {
        println!("{}Removed {}:{}", prefix, spec.ecosystem, spec.package);
    } else {
        println!("{}:{} not found", spec.ecosystem, spec.package);
    }
    Ok(())
}

fn run_list(json_output: bool) -> Result<(), Box<dyn std::error::Error>> {
    let entries = deps::list()?;

    if json_output {
        let list_entries: Vec<ListEntry> = entries
            .iter()
            .map(|e| ListEntry::new(e.ecosystem, &e.package, &e.version, e.is_broken))
            .collect();
        output::print_json(&ListResult {
            dependencies: list_entries,
        });
        return Ok(());
    }

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

fn run_clean(json_output: bool, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = if dry_run { "[dry-run] " } else { "" };

    let cleaned = if dry_run {
        // Check if it would be cleaned
        deps::deps_dir().exists()
    } else {
        deps::clean()?
    };

    if json_output {
        let mut result = CleanResult {
            cleaned,
            dry_run: false,
        };
        if dry_run {
            result.dry_run = true;
        }
        output::print_json(&result);
    } else if cleaned {
        println!("{}Removed .deps/", prefix);
    } else {
        println!(".deps/ not present");
    }
    Ok(())
}

fn run_context(json_output: bool) -> Result<(), Box<dyn std::error::Error>> {
    let context = context::render_context()?;
    if json_output {
        output::print_json(&output::ContextResult { context });
    } else if let Some(output) = context {
        print!("{}", output);
    }
    Ok(())
}

fn run_update(check_only: bool, json_output: bool) -> Result<(), Box<dyn std::error::Error>> {
    if check_only {
        let result = update::check_for_update()?;

        if json_output {
            output::print_json(&UpdateCheckOutput::new(
                &result.current_version,
                &result.latest_version,
                result.update_available,
            ));
        } else if result.update_available {
            println!(
                "Update available: {} -> {}",
                result.current_version, result.latest_version
            );
            println!("Run 'dotdeps update' to install.");
        } else {
            println!("dotdeps {} is the latest version.", result.current_version);
        }
    } else {
        let current_version = env!("CARGO_PKG_VERSION");

        if !json_output {
            println!("Checking for updates...");
        }

        let status = update::run_update(!json_output)?;

        match status {
            self_update::Status::UpToDate(v) => {
                if json_output {
                    output::print_json(&UpdateOutput::up_to_date(&v));
                } else {
                    println!("dotdeps {} is already up to date.", v);
                }
            }
            self_update::Status::Updated(v) => {
                if json_output {
                    output::print_json(&UpdateOutput::updated(current_version, &v));
                } else {
                    println!("Updated dotdeps to version {}.", v);
                }
            }
        }
    }

    Ok(())
}
