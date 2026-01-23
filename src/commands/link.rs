use crate::install::Keg;
use crate::paths;
use std::fs;
use std::os::unix;
use std::path::Path;

pub struct LinkCommand;

impl LinkCommand {
    pub fn run(args: &[String]) -> Result<(), i32> {
        let mut dry_run = false;
        let mut overwrite = false;
        let mut force = false;
        let mut verbose = false;
        let mut formulae = Vec::new();

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--help" | "-h" => {
                    print_help();
                    return Ok(());
                }
                "--dry-run" | "-n" => dry_run = true,
                "--overwrite" => overwrite = true,
                "--force" | "-f" => force = true,
                "--verbose" | "-v" => verbose = true,
                arg if arg.starts_with('-') => {
                    eprintln!("Error: Unknown option: {}", arg);
                    return Err(1);
                }
                formula => formulae.push(formula.to_string()),
            }
            i += 1;
        }

        if formulae.is_empty() {
            eprintln!("Error: This command requires at least one formula argument.");
            print_help();
            return Err(1);
        }

        for formula in formulae {
            if let Err(e) = link_formula(&formula, dry_run, overwrite, force, verbose) {
                eprintln!("Error: {}", e);
                return Err(1);
            }
        }

        Ok(())
    }
}

fn link_formula(
    formula_name: &str,
    dry_run: bool,
    overwrite: bool,
    _force: bool,
    verbose: bool,
) -> Result<(), String> {
    let opt_path = paths::homebrew_prefix().join("opt").join(formula_name);

    // Check if formula is installed by checking opt symlink
    if !opt_path.exists() && !opt_path.symlink_metadata().is_ok() {
        return Err(format!("No such keg: {}", formula_name));
    }

    // Resolve the opt symlink to get the actual keg path
    let keg_path = fs::read_link(&opt_path)
        .map_err(|_| format!("Failed to read opt symlink for {}", formula_name))?;

    // Extract version from keg path
    let version = keg_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Failed to extract version from keg path")?
        .to_string();

    let keg = Keg::new(formula_name.to_string(), version);

    // Check if already linked
    if is_linked(&keg)? && !dry_run {
        eprintln!("Warning: Already linked: {}", keg.path.display());
        eprintln!("To relink, run:");
        eprintln!(
            "  brew unlink {} && brew link {}",
            formula_name, formula_name
        );
        return Ok(());
    }

    if dry_run {
        if overwrite {
            println!("Would remove:");
        } else {
            println!("Would link:");
        }
    } else {
        print!("Linking {}... ", keg.path.display());
        if verbose {
            println!();
        }
    }

    let count = link_keg(&keg, dry_run, overwrite, verbose)?;

    if !dry_run {
        println!("{} symlinks created.", count);
    }

    Ok(())
}

/// Check if a keg is already linked
fn is_linked(keg: &Keg) -> Result<bool, String> {
    // Check if common directories have symlinks pointing to this keg
    let prefix = paths::homebrew_prefix();
    let bin_dir = prefix.join("bin");

    if !bin_dir.exists() {
        return Ok(false);
    }

    // Check if any symlinks in bin point to this keg
    if let Ok(entries) = fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            if let Ok(target) = fs::read_link(entry.path()) {
                if target.starts_with(&keg.path) {
                    return Ok(true);
                }
                // Also check relative paths
                let abs_target = bin_dir.join(&target);
                if let Ok(canonical) = abs_target.canonicalize() {
                    if canonical.starts_with(&keg.path) {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

/// Link all files from a keg to HOMEBREW_PREFIX
fn link_keg(keg: &Keg, dry_run: bool, overwrite: bool, verbose: bool) -> Result<usize, String> {
    let mut count = 0;

    // Link standard directories
    count += link_dir(&keg.path, "bin", dry_run, overwrite, verbose)?;
    count += link_dir(&keg.path, "sbin", dry_run, overwrite, verbose)?;
    count += link_dir(&keg.path, "lib", dry_run, overwrite, verbose)?;
    count += link_dir(&keg.path, "include", dry_run, overwrite, verbose)?;
    count += link_dir(&keg.path, "share", dry_run, overwrite, verbose)?;
    count += link_dir(&keg.path, "etc", dry_run, overwrite, verbose)?;
    count += link_dir(&keg.path, "Frameworks", dry_run, overwrite, verbose)?;

    Ok(count)
}

/// Link all files from a source directory to HOMEBREW_PREFIX
fn link_dir(
    keg_path: &Path,
    dir_name: &str,
    dry_run: bool,
    overwrite: bool,
    verbose: bool,
) -> Result<usize, String> {
    let src_dir = keg_path.join(dir_name);
    if !src_dir.exists() {
        return Ok(0);
    }

    let prefix = paths::homebrew_prefix();
    let mut count = 0;

    // Walk the directory tree
    for entry in walkdir::WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let src_path = entry.path();

        // Skip the root directory itself
        if src_path == src_dir {
            continue;
        }

        // Calculate relative path from keg
        let rel_path = src_path
            .strip_prefix(keg_path)
            .map_err(|e| format!("Failed to calculate relative path: {}", e))?;

        // Calculate destination path
        let dst_path = prefix.join(rel_path);

        // Handle files and symlinks
        if src_path.is_file() || src_path.is_symlink() {
            // Skip certain files
            if should_skip_file(src_path) {
                continue;
            }

            count += link_file(src_path, &dst_path, dry_run, overwrite, verbose)?;
        } else if src_path.is_dir() {
            // Create directories if they don't exist
            if !dst_path.exists() && !dry_run {
                fs::create_dir_all(&dst_path).map_err(|e| {
                    format!("Failed to create directory {}: {}", dst_path.display(), e)
                })?;
            }
        }
    }

    Ok(count)
}

/// Check if a file should be skipped during linking
fn should_skip_file(path: &Path) -> bool {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Skip .DS_Store files
    if filename == ".DS_Store" {
        return true;
    }

    // Skip INSTALL_RECEIPT.json
    if filename == "INSTALL_RECEIPT.json" {
        return true;
    }

    // Skip .brew directory
    if path.components().any(|c| c.as_os_str() == ".brew") {
        return true;
    }

    // Skip Python cached files in site-packages
    if let Some(ext) = path.extension() {
        if (ext == "pyc" || ext == "pyo") && path.to_string_lossy().contains("/site-packages/") {
            return true;
        }
    }

    false
}

/// Link a single file
fn link_file(
    src: &Path,
    dst: &Path,
    dry_run: bool,
    overwrite: bool,
    verbose: bool,
) -> Result<usize, String> {
    // Check if destination already exists and points to the same source
    if dst.exists() || dst.symlink_metadata().is_ok() {
        if let Ok(target) = fs::read_link(dst) {
            // Check if it already points to the right place
            let target_abs = dst.parent().unwrap().join(&target);
            if let Ok(canonical_target) = target_abs.canonicalize() {
                if let Ok(canonical_src) = src.canonicalize() {
                    if canonical_target == canonical_src {
                        if verbose && !dry_run {
                            println!("Skipping; link already exists: {}", dst.display());
                        }
                        return Ok(0);
                    }
                }
            }
        }

        if dry_run {
            if overwrite {
                if dst.is_symlink() {
                    if let Ok(target) = fs::read_link(dst) {
                        println!("{} -> {}", dst.display(), target.display());
                    }
                } else {
                    println!("{}", dst.display());
                }
            } else {
                println!("{}", dst.display());
            }
            return Ok(1);
        }

        if !overwrite {
            return Err(format!(
                "Could not symlink {}\nTarget {} already exists.",
                src.display(),
                dst.display()
            ));
        }

        // Remove existing file/symlink
        if dst.is_dir() && !dst.is_symlink() {
            return Err(format!(
                "Could not symlink {}\nTarget {} is a directory.",
                src.display(),
                dst.display()
            ));
        }

        fs::remove_file(dst).map_err(|e| format!("Failed to remove {}: {}", dst.display(), e))?;
    }

    if dry_run {
        println!("{}", dst.display());
        return Ok(1);
    }

    // Create parent directory if needed
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    // Create relative symlink
    make_relative_symlink(src, dst)?;

    if verbose {
        println!("{} -> {}", dst.display(), src.display());
    }

    Ok(1)
}

/// Create a relative symlink from dst to src
fn make_relative_symlink(src: &Path, dst: &Path) -> Result<(), String> {
    // Calculate relative path from dst to src
    let relative = pathdiff::diff_paths(src, dst.parent().unwrap()).ok_or_else(|| {
        format!(
            "Failed to calculate relative path from {} to {}",
            dst.display(),
            src.display()
        )
    })?;

    unix::fs::symlink(&relative, dst).map_err(|e| {
        format!(
            "Failed to create symlink {} -> {}: {}",
            dst.display(),
            relative.display(),
            e
        )
    })?;

    Ok(())
}

fn print_help() {
    println!(
        "Usage: brew link, ln [options] installed_formula [...]

Symlink all of formula's installed files into Homebrew's prefix. This is done
automatically when you install formulae but can be useful for manual
installations.

      --overwrite                  Delete files that already exist in the prefix
                                   while linking.
  -n, --dry-run                    List files which would be linked or deleted
                                   by brew link --overwrite without actually
                                   linking or deleting any files.
  -f, --force                      Allow keg-only formulae to be linked.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."
    );
}
