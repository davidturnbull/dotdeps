use crate::paths;
use std::fs;
use std::path::PathBuf;

pub fn execute(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Parse flags
    let mut mark_installed = false;
    let mut mark_not_installed = false;
    let mut formula_only = false;
    let mut cask_only = false;
    let mut names = Vec::new();

    for arg in args {
        match arg.as_str() {
            "--installed-on-request" => mark_installed = true,
            "--no-installed-on-request" => mark_not_installed = true,
            "--formula" | "--formulae" => formula_only = true,
            "--cask" | "--casks" => cask_only = true,
            "-d" | "--debug" | "-q" | "--quiet" | "-v" | "--verbose" => {
                // These flags are accepted but not used
            }
            s if s.starts_with('-') => {
                eprintln!("Error: Unknown option: {}", s);
                std::process::exit(1);
            }
            _ => names.push(arg.clone()),
        }
    }

    // Validate usage
    if names.is_empty() || (!mark_installed && !mark_not_installed) {
        print_usage();
        if names.is_empty() {
            std::process::exit(1);
        } else {
            eprintln!("Error: Invalid usage: No marking option specified.");
            std::process::exit(1);
        }
    }

    // Process each formula/cask
    for name in &names {
        if let Err(e) = process_tab(name, mark_installed, formula_only, cask_only) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn process_tab(
    name: &str,
    mark_installed: bool,
    formula_only: bool,
    cask_only: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // First try to find as formula (unless cask-only)
    if !cask_only && let Some(receipt_path) = find_formula_receipt(name) {
        update_receipt(&receipt_path, mark_installed, name)?;
        return Ok(());
    }

    // Then try to find as cask (unless formula-only)
    if !formula_only && let Some(receipt_path) = find_cask_receipt(name) {
        update_receipt(&receipt_path, mark_installed, name)?;
        return Ok(());
    }

    // Not found
    Err(format!("No available formula with the name \"{}\".", name).into())
}

fn find_formula_receipt(name: &str) -> Option<PathBuf> {
    let cellar = paths::homebrew_cellar();
    let formula_dir = cellar.join(name);

    if !formula_dir.exists() {
        return None;
    }

    // Find the installed version directory
    if let Ok(entries) = fs::read_dir(&formula_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let receipt = path.join("INSTALL_RECEIPT.json");
                if receipt.exists() {
                    return Some(receipt);
                }
            }
        }
    }

    None
}

fn find_cask_receipt(name: &str) -> Option<PathBuf> {
    let caskroom = paths::homebrew_prefix().join("Caskroom").join(name);

    if !caskroom.exists() {
        return None;
    }

    // Find the installed version directory
    if let Ok(entries) = fs::read_dir(&caskroom) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let receipt = path.join(".metadata").join("INSTALL_RECEIPT.json");
                if receipt.exists() {
                    return Some(receipt);
                }
            }
        }
    }

    None
}

fn update_receipt(
    receipt_path: &PathBuf,
    mark_installed: bool,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read the receipt
    let content = fs::read_to_string(receipt_path)?;
    let mut receipt: serde_json::Value = serde_json::from_str(&content)?;

    // Get current value
    let current_value = receipt
        .get("installed_on_request")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Check if already in desired state
    if current_value == mark_installed {
        if mark_installed {
            println!("==> {} is already marked as installed on request.", name);
        } else {
            println!(
                "==> {} is already marked as not installed on request.",
                name
            );
        }
        return Ok(());
    }

    // Update the value
    if let Some(obj) = receipt.as_object_mut() {
        obj.insert(
            "installed_on_request".to_string(),
            serde_json::Value::Bool(mark_installed),
        );
    }

    // Write back
    let updated = serde_json::to_string_pretty(&receipt)?;
    fs::write(receipt_path, updated)?;

    if mark_installed {
        println!("==> {} is now marked as installed on request.", name);
    } else {
        println!("==> {} is now marked as not installed on request.", name);
    }

    Ok(())
}

fn print_usage() {
    println!("Usage: brew tab [options] installed_formula|installed_cask [...]");
    println!();
    println!("Edit tab information for installed formulae or casks.");
    println!();
    println!("This can be useful when you want to control whether an installed formula should");
    println!("be removed by brew autoremove. To prevent removal, mark the formula as");
    println!("installed on request; to allow removal, mark the formula as not installed on");
    println!("request.");
    println!();
    println!("      --installed-on-request       Mark installed_formula or installed_cask");
    println!("                                   as installed on request.");
    println!("      --no-installed-on-request    Mark installed_formula or installed_cask");
    println!("                                   as not installed on request.");
    println!("      --formula, --formulae        Only mark formulae.");
    println!("      --cask, --casks              Only mark casks.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
    println!();
}
