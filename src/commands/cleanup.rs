use crate::paths;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut dry_run = false;
    let mut prune_days: Option<u64> = None;
    let mut specific_formulae = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-n" | "--dry-run" => dry_run = true,
            arg if arg.starts_with("--prune=") => {
                let value = arg.strip_prefix("--prune=").unwrap();
                if value == "all" {
                    prune_days = Some(0);
                } else if let Ok(days) = value.parse::<u64>() {
                    prune_days = Some(days);
                } else {
                    return Err(format!("Invalid value for --prune: {}", value).into());
                }
            }
            "--prune-prefix" => {
                // Not implemented yet
                eprintln!("Warning: --prune-prefix not yet implemented");
                return Ok(());
            }
            "-s" | "--scrub" => {
                // Not implemented yet
                eprintln!("Warning: --scrub not yet implemented");
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            arg if !arg.starts_with("-") => {
                specific_formulae.push(arg.to_string());
            }
            _ => {}
        }
        i += 1;
    }

    let default_prune_days = 120; // Default from HOMEBREW_CLEANUP_MAX_AGE_DAYS
    let days = prune_days.unwrap_or(default_prune_days);

    let mut total_freed = 0u64;

    if specific_formulae.is_empty() {
        // Clean all installed formulae
        total_freed += cleanup_all_formulae(dry_run)?;
        total_freed += cleanup_cache(dry_run, days)?;
    } else {
        // Clean specific formulae
        for formula_name in specific_formulae {
            total_freed += cleanup_formula(&formula_name, dry_run)?;
        }
    }

    // Show summary
    if total_freed > 0 {
        let size_str = format_size(total_freed);
        if dry_run {
            println!(
                "==> This operation would free approximately {} of disk space.",
                size_str
            );
        } else {
            println!(
                "==> This operation has freed approximately {} of disk space.",
                size_str
            );
        }
    }

    Ok(())
}

fn cleanup_all_formulae(dry_run: bool) -> Result<u64, Box<dyn std::error::Error>> {
    let cellar = paths::homebrew_cellar();
    if !cellar.exists() {
        return Ok(0);
    }

    let mut total_freed = 0u64;

    // Iterate through all formulae in the Cellar
    for entry in fs::read_dir(&cellar)? {
        let entry = entry?;
        let formula_name = entry.file_name().to_string_lossy().to_string();

        // Skip if not a directory
        if !entry.path().is_dir() {
            continue;
        }

        total_freed += cleanup_old_versions(&formula_name, dry_run)?;
    }

    Ok(total_freed)
}

fn cleanup_formula(formula_name: &str, dry_run: bool) -> Result<u64, Box<dyn std::error::Error>> {
    // Normalize formula name (remove tap prefix)
    let name = crate::formula::normalize_name(formula_name);

    // Check if formula exists
    if !crate::formula::exists(name) {
        return Err(format!("No available formula with the name \"{}\".", name).into());
    }

    cleanup_old_versions(name, dry_run)
}

fn cleanup_old_versions(
    formula_name: &str,
    dry_run: bool,
) -> Result<u64, Box<dyn std::error::Error>> {
    let cellar = paths::homebrew_cellar();
    let formula_dir = cellar.join(formula_name);

    if !formula_dir.exists() {
        return Ok(0);
    }

    // Find all installed versions
    let mut versions = Vec::new();
    for entry in fs::read_dir(&formula_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            versions.push(entry.path());
        }
    }

    // If only one version, nothing to clean
    if versions.len() <= 1 {
        return Ok(0);
    }

    // Find the currently linked version by checking opt symlink
    let opt_path = paths::homebrew_prefix().join("opt").join(formula_name);
    let linked_version = if opt_path.exists() && opt_path.is_symlink() {
        fs::read_link(&opt_path).ok()
    } else {
        None
    };

    let mut total_freed = 0u64;

    // Remove all versions except the linked one
    for version_path in versions {
        // Skip if this is the linked version
        if let Some(ref linked) = linked_version
            && version_path == *linked
        {
            continue;
        }

        // Calculate size
        let size = calculate_dir_size(&version_path)?;
        total_freed += size;

        if dry_run {
            println!(
                "Would remove: {} ({})",
                version_path.display(),
                format_size(size)
            );
        } else {
            fs::remove_dir_all(&version_path)?;
        }
    }

    Ok(total_freed)
}

fn cleanup_cache(dry_run: bool, days: u64) -> Result<u64, Box<dyn std::error::Error>> {
    let cache = paths::homebrew_cache();
    if !cache.exists() {
        return Ok(0);
    }

    let downloads_dir = cache.join("downloads");
    if !downloads_dir.exists() {
        return Ok(0);
    }

    let mut total_freed = 0u64;
    let cutoff_time = if days == 0 {
        SystemTime::now() // Remove everything
    } else {
        SystemTime::now() - std::time::Duration::from_secs(days * 24 * 60 * 60)
    };

    for entry in fs::read_dir(&downloads_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Check modification time
        if let Ok(metadata) = fs::metadata(&path)
            && let Ok(modified) = metadata.modified()
            && modified < cutoff_time
        {
            let size = metadata.len();
            total_freed += size;

            if dry_run {
                println!("Would remove: {} ({})", path.display(), format_size(size));
            } else {
                fs::remove_file(&path)?;
            }
        }
    }

    // Also check Cask cache
    let cask_cache = cache.join("Cask");
    if cask_cache.exists() {
        for entry in fs::read_dir(&cask_cache)? {
            let entry = entry?;
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Check modification time
            if let Ok(metadata) = fs::metadata(&path)
                && let Ok(modified) = metadata.modified()
                && modified < cutoff_time
            {
                let size = metadata.len();
                total_freed += size;

                if dry_run {
                    println!("Would remove: {} ({})", path.display(), format_size(size));
                } else {
                    fs::remove_file(&path)?;
                }
            }
        }
    }

    Ok(total_freed)
}

fn calculate_dir_size(path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let mut total = 0u64;

    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            total += entry.metadata()?.len();
        }
    }

    Ok(total)
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

fn print_help() {
    println!("Usage: brew cleanup [options] [formula|cask ...]");
    println!();
    println!("Remove stale lock files and outdated downloads for all formulae and casks,");
    println!("and remove old versions of installed formulae. If arguments are specified,");
    println!("only do this for the given formulae and casks. Removes all downloads more");
    println!("than 120 days old. This can be adjusted with $HOMEBREW_CLEANUP_MAX_AGE_DAYS.");
    println!();
    println!("      --prune                      Remove all cache files older than specified");
    println!("                                   days. If you want to remove everything, use");
    println!("                                   --prune=all.");
    println!("  -n, --dry-run                    Show what would be removed, but do not");
    println!("                                   actually remove anything.");
    println!("  -s, --scrub                      Scrub the cache, including downloads for even");
    println!("                                   the latest versions. Note that downloads for");
    println!("                                   any installed formulae or casks will still");
    println!("                                   not be deleted. If you want to delete those");
    println!("                                   too: rm -rf \"$(brew --cache)\"");
    println!("      --prune-prefix               Only prune the symlinks and directories from");
    println!("                                   the prefix and remove no other files.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}
