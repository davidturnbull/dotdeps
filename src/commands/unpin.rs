use crate::formula;
use crate::paths;
use std::fs;

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

    let pinned_dir = paths::homebrew_prefix().join("var/homebrew/pinned");

    let mut not_pinned = Vec::new();

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

        // Pin marker path
        let pin_marker = pinned_dir.join(normalized);

        // Check if pinned
        if !pin_marker.exists() {
            not_pinned.push(normalized);
            continue;
        }

        if verbose {
            println!("Unpinning {}", normalized);
        }

        // Remove the pin marker
        fs::remove_file(&pin_marker)?;
    }

    // Show warnings for not pinned
    for formula in not_pinned {
        if !quiet {
            println!("Warning: {} not pinned", formula);
        }
    }

    Ok(())
}
