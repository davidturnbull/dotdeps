use crate::paths;
use std::fs;
use std::path::Path;
use std::process;

fn print_help() {
    println!("Usage: brew reinstall [options] formula|cask [...]");
    println!();
    println!("Uninstall and then reinstall a formula or cask using the same options it was");
    println!("originally installed with, plus any appended options specific to a formula.");
    println!();
    println!("Options:");
    println!("  -f, --force              Install without checking for previously installed");
    println!("                           keg-only or non-migrated versions");
    println!("  -v, --verbose            Print the verification and post-install steps");
    println!("  -q, --quiet              Make some output more quiet");
    println!(
        "  -s, --build-from-source  Compile formula from source even if a bottle is available"
    );
    println!("  -h, --help               Show this message");
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut verbose = false;
    let mut quiet = false;
    let mut force = false;
    let mut build_from_source = false;
    let mut formula_names = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-v" | "--verbose" => verbose = true,
            "-q" | "--quiet" => quiet = true,
            "-f" | "--force" => force = true,
            "-s" | "--build-from-source" => build_from_source = true,
            "--formula" | "--formulae" => {} // Just treat as formulae
            _ => {
                if !arg.starts_with('-') {
                    formula_names.push(arg.clone());
                }
            }
        }
    }

    if formula_names.is_empty() {
        eprintln!("Error: No formulae specified");
        eprintln!();
        eprintln!("Usage: brew reinstall [options] formula [...]");
        process::exit(1);
    }

    let prefix = paths::homebrew_prefix();

    // Process each formula
    for formula_name in &formula_names {
        // Check if installed
        let opt_path = prefix.join("opt").join(formula_name);
        if !opt_path.exists() {
            eprintln!("Error: {} is not installed", formula_name);
            process::exit(1);
        }

        if !quiet {
            println!("==> Reinstalling {}", formula_name);
        }

        // Uninstall (without removing dependencies)
        uninstall_formula(formula_name, verbose)?;

        // Install
        let mut install_args = vec![formula_name.clone()];
        if verbose {
            install_args.push("--verbose".to_string());
        }
        if force {
            install_args.push("--force".to_string());
        }
        if build_from_source {
            install_args.push("--build-from-source".to_string());
        }

        if verbose {
            println!("==> Installing {}", formula_name);
        }

        crate::commands::install::run(&install_args)
            .map_err(|e| format!("Failed to install {}: {}", formula_name, e))?;

        // Cleanup
        if !quiet {
            println!("==> Running `brew cleanup {}`...", formula_name);
        }
        cleanup_formula(formula_name, verbose)?;
    }

    Ok(())
}

fn uninstall_formula(formula_name: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = paths::homebrew_prefix();
    let cellar = paths::homebrew_cellar();
    let formula_dir = cellar.join(formula_name);

    if !formula_dir.exists() {
        return Ok(());
    }

    // Get linked version from opt symlink
    let opt_path = prefix.join("opt").join(formula_name);
    let linked_version = if let Ok(target) = fs::read_link(&opt_path) {
        target
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    } else {
        None
    };

    if let Some(version) = linked_version {
        let keg_path = formula_dir.join(&version);
        if keg_path.exists() {
            // Remove symlinks
            if verbose {
                println!("Unlinking {}...", formula_name);
            }
            remove_symlinks(&prefix, &keg_path, verbose);

            // Remove keg directory
            if verbose {
                println!("Removing {}...", keg_path.display());
            }
            fs::remove_dir_all(&keg_path)?;
        }
    }

    // Remove opt symlink
    if opt_path.exists() {
        fs::remove_file(&opt_path)?;
    }

    Ok(())
}

fn remove_symlinks(prefix: &Path, keg_path: &Path, verbose: bool) {
    let dirs = [
        "bin",
        "sbin",
        "lib",
        "include",
        "share",
        "etc",
        "Frameworks",
    ];
    let keg_path_canonical = match fs::canonicalize(keg_path) {
        Ok(p) => p,
        Err(_) => return,
    };

    for dir in &dirs {
        let prefix_dir = prefix.join(dir);
        if !prefix_dir.exists() {
            continue;
        }

        let walker = walkdir::WalkDir::new(&prefix_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok());

        for entry in walker {
            if !entry.file_type().is_symlink() {
                continue;
            }

            let link_path = entry.path();
            if let Ok(target) = fs::read_link(link_path) {
                let target_path = if target.is_absolute() {
                    target
                } else {
                    link_path.parent().unwrap().join(&target)
                };

                if let Ok(canonical_target) = fs::canonicalize(&target_path)
                    && canonical_target.starts_with(&keg_path_canonical)
                {
                    if verbose {
                        println!("Removing symlink: {}", link_path.display());
                    }
                    let _ = fs::remove_file(link_path);
                }
            }
        }
    }
}

fn cleanup_formula(formula_name: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let cellar = paths::homebrew_cellar();
    let formula_dir = cellar.join(formula_name);

    if !formula_dir.exists() {
        return Ok(());
    }

    // Find linked version
    let prefix = paths::homebrew_prefix();
    let opt_path = prefix.join("opt").join(formula_name);

    let linked_version = if let Ok(target) = fs::read_link(&opt_path) {
        target
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    } else {
        None
    };

    // Remove old versions
    let entries = fs::read_dir(&formula_dir)?;
    for entry in entries {
        let entry = entry?;
        let version = entry.file_name().to_string_lossy().to_string();

        // Skip linked version
        if Some(&version) == linked_version.as_ref() {
            continue;
        }

        let old_path = entry.path();
        if verbose {
            println!("Removing: {}", old_path.display());
        }

        fs::remove_dir_all(&old_path)?;
    }

    Ok(())
}
