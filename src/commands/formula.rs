use crate::paths;
use std::path::PathBuf;
use std::process;

pub fn execute(args: &[String]) {
    // Require at least one argument
    if args.is_empty() {
        eprintln!("Usage: brew formula formula [...]");
        eprintln!();
        eprintln!("Display the path where formula is located.");
        eprintln!();
        eprintln!("  -d, --debug                      Display any debugging information.");
        eprintln!("  -q, --quiet                      Make some output more quiet.");
        eprintln!("  -v, --verbose                    Make some output more verbose.");
        eprintln!("  -h, --help                       Show this message.");
        eprintln!();
        eprintln!("Error: Invalid usage: This command requires at least 1 formula argument.");
        process::exit(1);
    }

    let mut formula_paths = Vec::new();
    let mut found_casks = false;

    for formula_name in args {
        // Try to find formula path
        if let Some(path) = find_formula_path(formula_name) {
            formula_paths.push(path);
        } else {
            // Check if it's a cask instead
            if is_cask(formula_name) {
                found_casks = true;
            }
        }
    }

    // If no formula paths but found casks, show error
    if formula_paths.is_empty() && found_casks {
        eprintln!("Error: Found casks but did not find formulae!");
        process::exit(1);
    }

    // Print each formula path
    for path in formula_paths {
        println!("{}", path.display());
    }
}

fn find_formula_path(formula_name: &str) -> Option<PathBuf> {
    // First, extract the base formula name (without tap prefix)
    let base_name = formula_name.split('/').next_back().unwrap_or(formula_name);

    // Try to find the tap path for this formula
    // We need to check all taps to find where this formula is defined
    let taps_dir = paths::homebrew_repository().join("Library/Taps");

    if let Ok(entries) = std::fs::read_dir(&taps_dir) {
        for entry in entries.flatten() {
            if let Ok(user_entries) = std::fs::read_dir(entry.path()) {
                for user_entry in user_entries.flatten() {
                    // Check in Formula/ directory
                    let formula_dir = user_entry.path().join("Formula");
                    let formula_path = formula_dir.join(format!("{}.rb", base_name));
                    if formula_path.exists() {
                        return Some(formula_path);
                    }

                    // Also check root directory (some taps put formulae in root)
                    let root_formula_path = user_entry.path().join(format!("{}.rb", base_name));
                    if root_formula_path.exists() {
                        return Some(root_formula_path);
                    }
                }
            }
        }
    }

    // If not found in taps, check opt/ (for core formulae when homebrew/core is not installed)
    let opt_path = paths::homebrew_prefix()
        .join("opt")
        .join(base_name)
        .join(".brew")
        .join(format!("{}.rb", base_name));

    if opt_path.exists() {
        return Some(opt_path);
    }

    None
}

fn is_cask(name: &str) -> bool {
    // Check if cask exists in opt/
    let cask_path = paths::homebrew_prefix().join("Caskroom").join(name);

    if cask_path.exists() {
        return true;
    }

    // Check in tap cask directories
    let taps_dir = paths::homebrew_repository().join("Library/Taps");
    if let Ok(entries) = std::fs::read_dir(&taps_dir) {
        for entry in entries.flatten() {
            if let Ok(user_entries) = std::fs::read_dir(entry.path()) {
                for user_entry in user_entries.flatten() {
                    let casks_dir = user_entry.path().join("Casks");
                    let cask_file = casks_dir.join(format!("{}.rb", name));
                    if cask_file.exists() {
                        return true;
                    }
                }
            }
        }
    }

    false
}
