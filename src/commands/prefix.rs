use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use crate::commands::{Command, CommandResult};
use crate::paths;

pub struct Prefix;

impl Command for Prefix {
    fn run(&self, args: &[String]) -> CommandResult {
        let has_unbrewed = args.iter().any(|a| a == "--unbrewed");
        let has_installed = args.iter().any(|a| a == "--installed");

        // Get formula names (non-flag arguments)
        let formula_args: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

        // Validate flag conflicts
        if has_unbrewed && has_installed {
            return Err("--unbrewed and --installed are mutually exclusive.".into());
        }

        if has_installed && formula_args.is_empty() {
            return Err("`--installed` requires a formula argument.".into());
        }

        if has_unbrewed {
            if !formula_args.is_empty() {
                return Err("`--unbrewed` does not take a formula argument.".into());
            }
            return list_unbrewed();
        }

        // No formula arguments - just output the prefix
        if formula_args.is_empty() {
            println!("{}", paths::homebrew_prefix().display());
            return Ok(());
        }

        // Handle formula arguments
        let prefix = paths::homebrew_prefix();
        let mut missing_formulae = Vec::new();

        for formula_name in &formula_args {
            // Validate formula exists
            if !formula_exists(formula_name) {
                return Err(
                    format!("No available formula with the name \"{}\".", formula_name).into(),
                );
            }

            let opt_prefix = prefix.join("opt").join(formula_name);

            if has_installed {
                // With --installed, only output if the formula is actually installed (opt link exists)
                if opt_prefix.exists() {
                    println!("{}", opt_prefix.display());
                } else {
                    missing_formulae.push(formula_name.as_str());
                }
            } else {
                // Without --installed, always output the would-be path
                println!("{}", opt_prefix.display());
            }
        }

        if has_installed && !missing_formulae.is_empty() {
            let names = missing_formulae.join(" ");
            return Err(format!("The following formulae are not installed:\n{}", names).into());
        }

        Ok(())
    }
}

/// Check if a formula exists by looking up the formula names cache.
fn formula_exists(name: &str) -> bool {
    // Check the API cache for formula names
    let cache_path = get_formula_names_cache_path();

    if let Some(cache_path) = cache_path
        && let Ok(contents) = fs::read_to_string(&cache_path)
    {
        return contents.lines().any(|line| line == name);
    }

    // Fallback: if cache doesn't exist, check if the formula is installed (opt symlink exists)
    let opt_path = paths::homebrew_prefix().join("opt").join(name);
    opt_path.exists()
}

/// Get the path to the formula names cache file.
fn get_formula_names_cache_path() -> Option<PathBuf> {
    let cache = paths::homebrew_cache();
    let path = cache.join("api/formula_names.txt");
    if path.exists() {
        return Some(path);
    }
    None
}

/// List files in Homebrew's prefix not installed by Homebrew.
fn list_unbrewed() -> CommandResult {
    let prefix = paths::homebrew_prefix();

    // Get all subdirectories of prefix
    let entries = match fs::read_dir(&prefix) {
        Ok(e) => e,
        Err(e) => return Err(format!("Failed to read prefix directory: {}", e).into()),
    };

    let mut dirs: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();

    // Exclude special directories
    let excluded_dirs = ["Library", "Cellar", "Caskroom", ".git", "etc", "var"];
    dirs.retain(|d| !excluded_dirs.contains(&d.as_str()));

    // Also exclude cache, logs, repository if they're under prefix
    let cache = paths::homebrew_cache();
    let repo = paths::homebrew_repository();
    if let Ok(rel) = cache.strip_prefix(&prefix)
        && let Some(first) = rel.components().next()
    {
        dirs.retain(|d| d != first.as_os_str().to_str().unwrap_or(""));
    }
    if let Ok(rel) = repo.strip_prefix(&prefix)
        && let Some(first) = rel.components().next()
    {
        dirs.retain(|d| d != first.as_os_str().to_str().unwrap_or(""));
    }

    if dirs.is_empty() {
        return Ok(());
    }

    // Sort directories
    dirs.sort();

    // Paths to exclude
    let exclude_files = [".DS_Store"];
    let exclude_paths = [
        "*/.keepme",
        ".github/*",
        "bin/brew",
        "completions/zsh/_brew",
        "docs/*",
        "lib/gdk-pixbuf-2.0/*",
        "lib/gio/*",
        "lib/node_modules/*",
        "lib/python[23].[0-9]/*",
        "lib/python3.[0-9][0-9]/*",
        "lib/pypy/*",
        "lib/pypy3/*",
        "lib/ruby/gems/[12].*",
        "lib/ruby/site_ruby/[12].*",
        "lib/ruby/vendor_ruby/[12].*",
        "manpages/brew.1",
        "share/pypy/*",
        "share/pypy3/*",
        "share/info/dir",
        "share/man/whatis",
        "share/mime/*",
        "texlive/*",
    ];

    // Build find command arguments
    let mut find_args: Vec<String> = dirs;

    find_args.push("-type".to_string());
    find_args.push("f".to_string());
    find_args.push("(".to_string());

    // Add file exclusions
    for file in &exclude_files {
        find_args.push("!".to_string());
        find_args.push("-name".to_string());
        find_args.push(file.to_string());
    }

    // Add path exclusions
    for path in &exclude_paths {
        find_args.push("!".to_string());
        find_args.push("-path".to_string());
        find_args.push(path.to_string());
    }

    find_args.push(")".to_string());

    // Run find from the prefix directory
    let output = ProcessCommand::new("find")
        .args(&find_args)
        .current_dir(&prefix)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            print!("{}", String::from_utf8_lossy(&output.stdout));
            Ok(())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("find command failed: {}", stderr).into())
        }
        Err(e) => Err(format!("Failed to run find: {}", e).into()),
    }
}
