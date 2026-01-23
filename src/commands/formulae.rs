use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::paths;

pub fn execute(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let formula_names = get_all_formulae()?;

    for name in formula_names {
        println!("{}", name);
    }

    Ok(())
}

fn get_all_formulae() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut formula_set = HashSet::new();

    // First, try to read from API cache if not disabled
    let no_install_from_api = std::env::var("HOMEBREW_NO_INSTALL_FROM_API").is_ok();

    if !no_install_from_api {
        let cache_dir = paths::homebrew_cache();
        let api_formula_file = cache_dir.join("api/formula_names.txt");

        if api_formula_file.exists()
            && let Ok(contents) = fs::read_to_string(&api_formula_file)
        {
            for line in contents.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    formula_set.insert(line.to_string());
                }
            }
        }
    }

    // Scan filesystem for formulae in taps
    let repository = paths::homebrew_repository();
    let taps_dir = repository.join("Library/Taps");

    if taps_dir.exists() {
        scan_formulae_from_filesystem(&taps_dir, &mut formula_set)?;
    }

    // Convert to sorted vector
    let mut formula_names: Vec<String> = formula_set.into_iter().collect();
    formula_names.sort();

    Ok(formula_names)
}

fn scan_formulae_from_filesystem(
    taps_dir: &PathBuf,
    formula_set: &mut HashSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Walk through taps directory looking for *.rb files (excluding certain directories)
    for entry in WalkDir::new(taps_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let path = e.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip .git directories, cmd, .github, spec, vendor directories
            if file_name == ".git"
                || file_name == "cmd"
                || file_name == ".github"
                || file_name == "spec"
                || file_name == "vendor"
            {
                return false;
            }

            // Skip Casks directories entirely
            if file_name == "Casks" {
                return false;
            }

            // Skip lib directories that are not in Formula
            if file_name == "lib" {
                let path_str = path.to_string_lossy();
                if !path_str.contains("/Formula/") {
                    return false;
                }
            }

            true
        })
        .flatten()
    {
        let path = entry.path();

        // Only process .rb files (excluding those in Casks directories)
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("rb") {
            let path_str = path.to_string_lossy();
            // Skip anything in Casks directories
            if !path_str.contains("/Casks/") {
                extract_formula_name(path, formula_set);
            }
        }
    }

    Ok(())
}

fn extract_formula_name(path: &std::path::Path, formula_set: &mut HashSet<String>) {
    let path_str = path.to_string_lossy();

    // Mimic Homebrew's sed transformations:
    // 1. Remove .rb extension
    // 2. Transform .../Taps/user/(home|linux)brew-tap/... to user/tap/...
    // 3. Remove /Formula/ and any subdirectories within it

    if let Some(taps_pos) = path_str.find("/Taps/") {
        let after_taps = &path_str[taps_pos + 6..];

        // Remove .rb extension
        let without_rb = after_taps.strip_suffix(".rb").unwrap_or(after_taps);

        // Transform user/homebrew-tap/... to user/tap/...
        let transformed = without_rb
            .replace("/homebrew-", "/")
            .replace("/linuxbrew-", "/");

        // Remove /Formula/ and any subdirectories within Formula
        // Pattern: /Formula/(.+/)? means /Formula/ followed by optional subdirectories
        let final_path = if let Some(formula_pos) = transformed.find("/Formula/") {
            let before_formula = &transformed[..formula_pos];
            let after_formula = &transformed[formula_pos + 9..]; // Skip "/Formula/"

            // Find the file name (last component after removing any subdirectories)
            let file_name = after_formula
                .split('/')
                .next_back()
                .unwrap_or(after_formula);

            format!("{}/{}", before_formula, file_name)
        } else {
            transformed
        };

        // Extract components
        let parts: Vec<&str> = final_path.split('/').collect();

        if parts.len() >= 3 {
            let user = parts[0];
            let tap_name = parts[1];

            // Get the formula name (everything after user/tap/)
            let formula_path = parts[2..].join("/");

            // For homebrew/core tap, only add short name
            if user == "homebrew" && tap_name == "core" {
                formula_set.insert(formula_path);
            } else {
                // For other taps, add full name
                let full_name = format!("{}/{}/{}", user, tap_name, formula_path);
                formula_set.insert(full_name);

                // Add short name (3rd field - index 2)
                // This matches Homebrew's: cut -d "/" -f 3
                formula_set.insert(parts[2].to_string());
            }
        }
    }
}
