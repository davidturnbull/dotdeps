use crate::commands::{Command, CommandResult};
use crate::install::Keg;
use crate::paths;
use std::fs;
use walkdir::WalkDir;

pub struct UnlinkCommand;

impl Command for UnlinkCommand {
    fn run(&self, args: &[String]) -> CommandResult {
        let mut dry_run = false;
        let mut verbose = false;
        let mut formulae = Vec::new();

        for arg in args {
            match arg.as_str() {
                "-n" | "--dry-run" => dry_run = true,
                "-v" | "--verbose" => verbose = true,
                "-q" | "--quiet" => { /* ignore */ }
                "-d" | "--debug" => { /* ignore */ }
                "-h" | "--help" => {
                    println!("{}", include_str!("../../help/unlink.txt"));
                    return Ok(());
                }
                _ if arg.starts_with('-') => {
                    return Err(format!("Unknown option: {}", arg).into());
                }
                _ => formulae.push(arg.clone()),
            }
        }

        if formulae.is_empty() {
            return Err("This command requires a formula argument"
                .to_string()
                .into());
        }

        for formula_name in formulae {
            if let Err(e) = unlink_formula(&formula_name, dry_run, verbose) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }

        Ok(())
    }
}

fn unlink_formula(formula_name: &str, dry_run: bool, verbose: bool) -> Result<(), String> {
    let opt_path = paths::homebrew_prefix().join("opt").join(formula_name);

    // Check if formula is installed by checking opt symlink
    if !opt_path.exists() && opt_path.symlink_metadata().is_err() {
        return Err(format!(
            "No such keg: {}",
            paths::homebrew_cellar().join(formula_name).display()
        ));
    }

    // Resolve the opt symlink to get the keg path
    let keg_path = match fs::read_link(&opt_path) {
        Ok(target) => {
            // Handle relative paths
            if target.is_relative() {
                opt_path.parent().unwrap().join(target)
            } else {
                target
            }
        }
        Err(_) => {
            return Err(format!(
                "No such keg: {}",
                paths::homebrew_cellar().join(formula_name).display()
            ));
        }
    };

    // Extract name and version from the keg path
    // Keg path format: /opt/homebrew/Cellar/formula_name/version
    let version = keg_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid keg path")?
        .to_string();

    let keg = Keg {
        name: formula_name.to_string(),
        version,
        path: keg_path
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize path: {}", e))?,
    };

    if dry_run {
        // Show what would be removed
        let links = find_links(&keg)?;
        if links.is_empty() {
            println!("Already unlinked: {}", keg.path.display());
            return Ok(());
        }
        println!("Would remove:");
        for link in links {
            println!("{}", link.display());
        }
    } else {
        // Actually unlink
        let count = remove_links(&keg, verbose)?;
        println!(
            "Unlinking {}... {} symlink{} removed.",
            keg.path.display(),
            count,
            if count == 1 { "" } else { "s" }
        );
    }

    Ok(())
}

/// Find all symlinks that point to files in the keg
fn find_links(keg: &Keg) -> Result<Vec<std::path::PathBuf>, String> {
    let prefix = paths::homebrew_prefix();
    let mut links = Vec::new();

    // Standard directories to check for symlinks
    let link_dirs = [
        "bin",
        "sbin",
        "lib",
        "include",
        "share",
        "etc",
        "Frameworks",
    ];

    for dir_name in &link_dirs {
        let link_dir = prefix.join(dir_name);
        if !link_dir.exists() {
            continue;
        }

        // Walk the directory looking for symlinks that point to the keg
        for entry in WalkDir::new(&link_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if let Ok(metadata) = path.symlink_metadata()
                && metadata.is_symlink()
                && let Ok(target) = fs::read_link(path)
            {
                // Resolve relative symlinks
                let target_abs = if target.is_relative() {
                    path.parent().unwrap().join(&target)
                } else {
                    target
                };

                // Check if the target is in the keg
                if let Ok(canonical_target) = target_abs.canonicalize()
                    && canonical_target.starts_with(&keg.path)
                {
                    links.push(path.to_path_buf());
                }
            }
        }
    }

    Ok(links)
}

/// Remove all symlinks that point to files in the keg
fn remove_links(keg: &Keg, verbose: bool) -> Result<usize, String> {
    let links = find_links(keg)?;
    let mut count = 0;

    for link in links {
        if verbose {
            println!("Removing: {}", link.display());
        }
        fs::remove_file(&link)
            .map_err(|e| format!("Failed to remove {}: {}", link.display(), e))?;
        count += 1;
    }

    Ok(count)
}
