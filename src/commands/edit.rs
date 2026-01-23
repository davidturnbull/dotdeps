use crate::paths;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

pub fn execute(args: &[String]) {
    // Parse arguments
    let mut print_path = false;
    let mut is_cask = false;
    let mut is_formula = false;
    let mut targets = Vec::new();

    for arg in args {
        match arg.as_str() {
            "--print-path" => print_path = true,
            "--cask" | "--casks" => is_cask = true,
            "--formula" | "--formulae" => is_formula = true,
            "-d" | "--debug" | "-q" | "--quiet" | "-v" | "--verbose" => {
                // Accept but ignore
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            _ => targets.push(arg.clone()),
        }
    }

    // If no targets, open the Homebrew repository
    if targets.is_empty() {
        let homebrew_repo = paths::homebrew_repository();
        if print_path {
            println!("{}", homebrew_repo.display());
        } else {
            open_in_editor(&[homebrew_repo]);
        }
        return;
    }

    // Resolve paths for all targets
    let mut paths_to_edit = Vec::new();
    for target in &targets {
        match resolve_path(target, is_cask, is_formula) {
            Ok(path) => paths_to_edit.push(path),
            Err(err) => {
                print_help();
                eprintln!("\nError: Invalid usage: {err}");
                exit(1);
            }
        }
    }

    if print_path {
        for path in paths_to_edit {
            println!("{}", path.display());
        }
    } else {
        open_in_editor(&paths_to_edit);
    }
}

fn resolve_path(name: &str, force_cask: bool, force_formula: bool) -> Result<PathBuf, String> {
    // Check if it's a tap name (user/tap format)
    if name.contains('/') && name.matches('/').count() == 1 {
        return resolve_tap_path(name);
    }

    // Otherwise it's a formula or cask name
    if force_cask {
        resolve_cask_path(name)
    } else if force_formula {
        resolve_formula_path(name)
    } else {
        // Try formula first, then cask
        // If neither exists, return the formula error (which is the default)
        match resolve_formula_path(name) {
            Ok(path) => Ok(path),
            Err(formula_err) => {
                // Try cask
                match resolve_cask_path(name) {
                    Ok(path) => Ok(path),
                    Err(_) => Err(formula_err), // Return formula error as default
                }
            }
        }
    }
}

fn resolve_tap_path(tap_name: &str) -> Result<PathBuf, String> {
    let parts: Vec<&str> = tap_name.split('/').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid tap name: {tap_name}"));
    }

    let user = parts[0];
    let repo = parts[1];

    // Add homebrew- prefix if not present
    let full_repo = if repo.starts_with("homebrew-") {
        repo.to_string()
    } else {
        format!("homebrew-{repo}")
    };

    let tap_path = paths::homebrew_repository()
        .join("Library/Taps")
        .join(user)
        .join(&full_repo);

    if !tap_path.exists() {
        // Special handling for homebrew/core and homebrew/cask
        if user == "homebrew" && (repo == "core" || repo == "cask") {
            return Err(format!(
                "No available tap {}/{}.\nRun brew tap --force {}/{} to tap {}/{}!",
                user, repo, user, repo, user, repo
            ));
        }
        return Err(format!("Tap {tap_name} does not exist"));
    }

    Ok(tap_path)
}

fn resolve_formula_path(name: &str) -> Result<PathBuf, String> {
    // First check if it's in a tap (user/tap/formula format)
    if name.matches('/').count() == 2 {
        let parts: Vec<&str> = name.split('/').collect();
        let user = parts[0];
        let repo = parts[1];
        let formula = parts[2];

        let full_repo = if repo.starts_with("homebrew-") {
            repo.to_string()
        } else {
            format!("homebrew-{repo}")
        };

        // Try Formula subdirectory first
        let mut formula_path = paths::homebrew_repository()
            .join("Library/Taps")
            .join(user)
            .join(&full_repo)
            .join("Formula")
            .join(format!("{formula}.rb"));

        if !formula_path.exists() {
            // Try root directory
            formula_path = paths::homebrew_repository()
                .join("Library/Taps")
                .join(user)
                .join(&full_repo)
                .join(format!("{formula}.rb"));
        }

        if formula_path.exists() {
            return Ok(formula_path);
        }
    }

    // Try opt path (for API-installed formulae)
    let opt_path = paths::homebrew_prefix()
        .join("opt")
        .join(name)
        .join(".brew")
        .join(format!("{name}.rb"));

    if opt_path.exists() {
        return Ok(opt_path);
    }

    // Try scanning all taps for this formula
    let taps_dir = paths::homebrew_repository().join("Library/Taps");
    if let Ok(entries) = std::fs::read_dir(&taps_dir) {
        for entry in entries.flatten() {
            if let Ok(user_entries) = std::fs::read_dir(entry.path()) {
                for tap_entry in user_entries.flatten() {
                    // Try Formula subdirectory
                    let formula_path = tap_entry.path().join("Formula").join(format!("{name}.rb"));
                    if formula_path.exists() {
                        return Ok(formula_path);
                    }

                    // Try root directory
                    let formula_path = tap_entry.path().join(format!("{name}.rb"));
                    if formula_path.exists() {
                        return Ok(formula_path);
                    }
                }
            }
        }
    }

    Err(format!(
        "{name} doesn't exist on disk.\nRun brew create --set-name {name} $URL to create a new formula!"
    ))
}

fn resolve_cask_path(name: &str) -> Result<PathBuf, String> {
    // First check if it's in a tap (user/tap/cask format)
    if name.matches('/').count() == 2 {
        let parts: Vec<&str> = name.split('/').collect();
        let user = parts[0];
        let repo = parts[1];
        let cask = parts[2];

        let full_repo = if repo.starts_with("homebrew-") {
            repo.to_string()
        } else {
            format!("homebrew-{repo}")
        };

        let cask_path = paths::homebrew_repository()
            .join("Library/Taps")
            .join(user)
            .join(&full_repo)
            .join("Casks")
            .join(format!("{cask}.rb"));

        if cask_path.exists() {
            return Ok(cask_path);
        }
    }

    // Try scanning all taps for this cask
    let taps_dir = paths::homebrew_repository().join("Library/Taps");
    if let Ok(entries) = std::fs::read_dir(&taps_dir) {
        for entry in entries.flatten() {
            if let Ok(user_entries) = std::fs::read_dir(entry.path()) {
                for tap_entry in user_entries.flatten() {
                    let cask_path = tap_entry.path().join("Casks").join(format!("{name}.rb"));
                    if cask_path.exists() {
                        return Ok(cask_path);
                    }
                }
            }
        }
    }

    Err(format!(
        "{name} doesn't exist on disk.\nRun brew create --cask --set-name {name} $URL to create a new cask!"
    ))
}

fn open_in_editor(paths: &[PathBuf]) {
    // Get editor from environment
    let editor = std::env::var("HOMEBREW_EDITOR")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vim".to_string());

    let path_strs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();

    let status = Command::new(&editor).args(path_strs).status();

    match status {
        Ok(status) if status.success() => {}
        Ok(_) => {
            eprintln!("Editor exited with error");
            exit(1);
        }
        Err(e) => {
            eprintln!("Failed to launch editor: {e}");
            exit(1);
        }
    }
}

fn print_help() {
    println!("Usage: brew edit [options] [formula|cask|tap ...]");
    println!();
    println!("Open a formula, cask or tap in the editor set by $EDITOR or");
    println!("$HOMEBREW_EDITOR, or open the Homebrew repository for editing if no argument");
    println!("is provided.");
    println!();
    println!("      --formula, --formulae        Treat all named arguments as formulae.");
    println!("      --cask, --casks              Treat all named arguments as casks.");
    println!("      --print-path                 Print the file path to be edited, without");
    println!("                                   opening an editor.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}
