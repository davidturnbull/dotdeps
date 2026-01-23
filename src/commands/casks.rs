use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::paths;

pub fn execute(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let cask_names = get_all_casks()?;

    for name in cask_names {
        println!("{}", name);
    }

    Ok(())
}

fn get_all_casks() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut cask_set = HashSet::new();

    // First, try to read from API cache if not disabled
    let no_install_from_api = std::env::var("HOMEBREW_NO_INSTALL_FROM_API").is_ok();

    if !no_install_from_api {
        let cache_dir = paths::homebrew_cache();
        let api_cask_file = cache_dir.join("api/cask_names.txt");

        if api_cask_file.exists()
            && let Ok(contents) = fs::read_to_string(&api_cask_file)
        {
            for line in contents.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    cask_set.insert(line.to_string());
                }
            }
        }
    }

    // Scan filesystem for casks in taps
    let repository = paths::homebrew_repository();
    let taps_dir = repository.join("Library/Taps");

    if taps_dir.exists() {
        scan_casks_from_filesystem(&taps_dir, &mut cask_set)?;
    }

    // Convert to sorted vector
    let mut cask_names: Vec<String> = cask_set.into_iter().collect();
    cask_names.sort();

    Ok(cask_names)
}

fn scan_casks_from_filesystem(
    taps_dir: &PathBuf,
    cask_set: &mut HashSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Walk through taps directory looking for */Casks/*.rb files
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

            // Skip lib directories that are not in Formula or Casks
            if file_name == "lib" {
                let path_str = path.to_string_lossy();
                if !path_str.contains("/Formula/") && !path_str.contains("/Casks/") {
                    return false;
                }
            }

            true
        })
        .flatten()
    {
        let path = entry.path();

        // Only process .rb files in Casks directories
        if path.is_file()
            && path.extension().and_then(|e| e.to_str()) == Some("rb")
            && path.to_string_lossy().contains("/Casks/")
        {
            extract_cask_name(path, cask_set);
        }
    }

    Ok(())
}

fn extract_cask_name(path: &std::path::Path, cask_set: &mut HashSet<String>) {
    let path_str = path.to_string_lossy();

    // Extract tap name and cask name
    // Path format: .../Taps/homebrew/homebrew-cask/Casks/firefox.rb
    // or: .../Taps/user/homebrew-tap/Casks/formula.rb

    if let Some(taps_pos) = path_str.find("/Taps/") {
        let after_taps = &path_str[taps_pos + 6..];
        let parts: Vec<&str> = after_taps.split('/').collect();

        if parts.len() >= 4 {
            let user = parts[0];
            let tap_repo = parts[1];

            // Extract tap name: remove "homebrew-" or "linuxbrew-" prefix
            let tap_name = if let Some(stripped) = tap_repo.strip_prefix("homebrew-") {
                stripped
            } else if let Some(stripped) = tap_repo.strip_prefix("linuxbrew-") {
                stripped
            } else {
                tap_repo
            };

            // Find the cask file name
            if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                // For homebrew/cask tap, only add short name
                if user == "homebrew" && tap_name == "cask" {
                    cask_set.insert(file_stem.to_string());
                } else {
                    // For other taps, add both full name and short name
                    let full_name = format!("{}/{}/{}", user, tap_name, file_stem);
                    cask_set.insert(full_name);
                    cask_set.insert(file_stem.to_string());
                }
            }
        }
    }
}
