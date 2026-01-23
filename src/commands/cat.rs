use crate::paths;
use crate::tap::Tap;
use std::fs;
use std::path::PathBuf;

pub fn run(args: &[String]) -> Result<(), i32> {
    let mut formula_mode = false;
    let mut cask_mode = false;
    let mut names = Vec::new();

    for arg in args {
        match arg.as_str() {
            "--formula" | "--formulae" => formula_mode = true,
            "--cask" | "--casks" => cask_mode = true,
            "-d" | "--debug" => { /* ignored for now */ }
            "-q" | "--quiet" => { /* ignored for now */ }
            "-v" | "--verbose" => { /* ignored for now */ }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => names.push(arg.clone()),
        }
    }

    if names.is_empty() {
        eprintln!("This command requires a formula or cask argument.");
        return Err(1);
    }

    let mut first = true;
    for name in names {
        if !first {
            println!(); // Blank line between formulae
        }
        first = false;

        cat_formula(&name, formula_mode, cask_mode)?;
    }

    Ok(())
}

fn cat_formula(name: &str, formula_mode: bool, cask_mode: bool) -> Result<(), i32> {
    // Try to find the formula/cask file
    let path = if cask_mode {
        find_cask_file(name)
    } else if formula_mode {
        find_formula_file(name)
    } else {
        // Try formula first, then cask
        find_formula_file(name).or_else(|| find_cask_file(name))
    };

    match path {
        Some(path) => {
            // Read and print the file
            match fs::read_to_string(&path) {
                Ok(content) => {
                    print!("{}", content);
                    Ok(())
                }
                Err(e) => {
                    eprintln!("Error: Failed to read {}: {}", path.display(), e);
                    Err(1)
                }
            }
        }
        None => {
            // Formula/cask not found on disk
            let prefix = paths::homebrew_prefix();
            eprintln!(
                "Error: {}/{}'s source doesn't exist on disk.",
                prefix.display(),
                name
            );
            eprintln!("The name may be wrong, or the tap hasn't been tapped. Instead try:");
            eprintln!("  brew info --github {}", name);
            Err(1)
        }
    }
}

fn find_formula_file(name: &str) -> Option<PathBuf> {
    // Parse the name to extract tap and formula name
    let (tap_name, formula_name) = if name.contains('/') {
        // Format: tap/formula or user/repo/formula
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() == 2 {
            // user/formula -> user/homebrew-/formula (assumes homebrew- prefix)
            (
                Some(format!("{}/{}", parts[0], parts[1])),
                parts[1].to_string(),
            )
        } else if parts.len() == 3 {
            // user/repo/formula
            (
                Some(format!("{}/{}", parts[0], parts[1])),
                parts[2].to_string(),
            )
        } else {
            (None, name.to_string())
        }
    } else {
        (None, name.to_string())
    };

    // If tap is specified, search that tap
    if let Some(tap) = tap_name {
        if let Some(tap_obj) = Tap::parse(&tap) {
            let tap_path = tap_obj.path();
            let formula_path = tap_path
                .join("Formula")
                .join(format!("{}.rb", formula_name));
            if formula_path.exists() {
                return Some(formula_path);
            }

            // Try subdirectory structure (e.g., Formula/a/axe.rb)
            let first_char = formula_name.chars().next().unwrap_or('a');
            let subdir_path = tap_path
                .join("Formula")
                .join(first_char.to_string())
                .join(format!("{}.rb", formula_name));
            if subdir_path.exists() {
                return Some(subdir_path);
            }
        }
    } else {
        // Search all taps
        let taps_dir = paths::homebrew_prefix().join("Library/Taps");
        if let Ok(entries) = fs::read_dir(taps_dir) {
            for entry in entries.flatten() {
                if let Ok(user_entries) = fs::read_dir(entry.path()) {
                    for tap_entry in user_entries.flatten() {
                        let formula_path = tap_entry
                            .path()
                            .join("Formula")
                            .join(format!("{}.rb", formula_name));
                        if formula_path.exists() {
                            return Some(formula_path);
                        }

                        // Try subdirectory structure
                        let first_char = formula_name.chars().next().unwrap_or('a');
                        let subdir_path = tap_entry
                            .path()
                            .join("Formula")
                            .join(first_char.to_string())
                            .join(format!("{}.rb", formula_name));
                        if subdir_path.exists() {
                            return Some(subdir_path);
                        }
                    }
                }
            }
        }
    }

    None
}

fn find_cask_file(name: &str) -> Option<PathBuf> {
    // Parse the name to extract tap and cask name
    let (tap_name, cask_name) = if name.contains('/') {
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() == 2 {
            (
                Some(format!("{}/{}", parts[0], parts[1])),
                parts[1].to_string(),
            )
        } else if parts.len() == 3 {
            (
                Some(format!("{}/{}", parts[0], parts[1])),
                parts[2].to_string(),
            )
        } else {
            (None, name.to_string())
        }
    } else {
        (None, name.to_string())
    };

    // If tap is specified, search that tap
    if let Some(tap) = tap_name {
        if let Some(tap_obj) = Tap::parse(&tap) {
            let tap_path = tap_obj.path();
            let cask_path = tap_path.join("Casks").join(format!("{}.rb", cask_name));
            if cask_path.exists() {
                return Some(cask_path);
            }

            // Try subdirectory structure
            let first_char = cask_name.chars().next().unwrap_or('a');
            let subdir_path = tap_path
                .join("Casks")
                .join(first_char.to_string())
                .join(format!("{}.rb", cask_name));
            if subdir_path.exists() {
                return Some(subdir_path);
            }
        }
    } else {
        // Search all taps
        let taps_dir = paths::homebrew_prefix().join("Library/Taps");
        if let Ok(entries) = fs::read_dir(taps_dir) {
            for entry in entries.flatten() {
                if let Ok(user_entries) = fs::read_dir(entry.path()) {
                    for tap_entry in user_entries.flatten() {
                        let cask_path = tap_entry
                            .path()
                            .join("Casks")
                            .join(format!("{}.rb", cask_name));
                        if cask_path.exists() {
                            return Some(cask_path);
                        }

                        // Try subdirectory structure
                        let first_char = cask_name.chars().next().unwrap_or('a');
                        let subdir_path = tap_entry
                            .path()
                            .join("Casks")
                            .join(first_char.to_string())
                            .join(format!("{}.rb", cask_name));
                        if subdir_path.exists() {
                            return Some(subdir_path);
                        }
                    }
                }
            }
        }
    }

    None
}

fn print_help() {
    println!("Usage: brew cat [--formula] [--cask] formula|cask [...]");
    println!();
    println!("Display the source of a formula or cask.");
    println!();
    println!("      --formula, --formulae        Treat all named arguments as formulae.");
    println!("      --cask, --casks              Treat all named arguments as casks.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}
