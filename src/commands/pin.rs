use crate::formula;
use crate::paths;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Parse flags
    let mut quiet = false;
    let mut verbose = false;
    let mut formulae = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-q" | "--quiet" => quiet = true,
            "-v" | "--verbose" => verbose = true,
            "-d" | "--debug" => {
                // Debug flag parsed but not used
            }
            _ if arg.starts_with('-') => {
                eprintln!("Error: Unknown option: {}", arg);
                std::process::exit(1);
            }
            _ => formulae.push(arg.clone()),
        }
    }

    if formulae.is_empty() {
        eprintln!("Error: This command requires a formula argument");
        std::process::exit(1);
    }

    // Ensure pinned directory exists
    let pinned_dir = paths::homebrew_prefix().join("var/homebrew/pinned");
    fs::create_dir_all(&pinned_dir)?;

    let mut already_pinned = Vec::new();

    for formula_name in &formulae {
        // Normalize formula name
        let normalized = formula::normalize_name(formula_name);

        // Check if formula exists
        if !formula::exists(normalized) {
            eprintln!(
                "Error: No available formula with the name \"{}\".",
                normalized
            );
            std::process::exit(1);
        }

        // Check if formula is installed
        let opt_path = paths::homebrew_prefix().join("opt").join(normalized);

        if !opt_path.exists() {
            eprintln!("Error: {} is not installed", normalized);
            std::process::exit(1);
        }

        // Resolve opt symlink to get installed version
        let installed_path = match fs::read_link(&opt_path) {
            Ok(path) => path,
            Err(_) => {
                eprintln!("Error: {} is not properly linked", normalized);
                std::process::exit(1);
            }
        };

        // Pin marker path
        let pin_marker = pinned_dir.join(normalized);

        // Check if already pinned
        if pin_marker.exists() {
            already_pinned.push(normalized);
            continue;
        }

        // Create symlink to cellar version
        // The symlink should be relative: ../../../Cellar/formula/version
        let relative_target = PathBuf::from("../../../Cellar")
            .join(normalized)
            .join(installed_path.file_name().unwrap());

        if verbose {
            println!("Pinning {} to {}", normalized, installed_path.display());
        }

        symlink(&relative_target, &pin_marker)?;
    }

    // Show warnings for already pinned
    for formula in already_pinned {
        if !quiet {
            println!("Warning: {} already pinned", formula);
        }
    }

    Ok(())
}
