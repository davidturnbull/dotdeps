//! Outdated command implementation.

use crate::api;
use crate::paths;
use std::fs;

pub fn run(args: &[String]) -> Result<(), String> {
    let mut quiet = false;
    let mut verbose = false;
    let mut json_mode = false;
    let mut json_version = "v1"; // Default to v1 for backwards compatibility
    let mut specific_formulae = Vec::new();

    // Parse flags
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--quiet" | "-q" => quiet = true,
            "--verbose" | "-v" => verbose = true,
            "--json" => {
                json_mode = true;
                // Check if next arg is a version
                if i + 1 < args.len() && (args[i + 1] == "v1" || args[i + 1] == "v2") {
                    json_version = &args[i + 1];
                    i += 1;
                }
            }
            "--formula" | "--formulae" => {
                // TODO: Filter to formulae only when cask support is added
            }
            "--cask" | "--casks" => {
                // TODO: Filter to casks only when cask support is added
            }
            "--fetch-HEAD" | "-g" | "--greedy" | "--greedy-latest" | "--greedy-auto-updates" => {
                // Parse but ignore for now
            }
            arg if !arg.starts_with('-') => {
                specific_formulae.push(arg.to_string());
            }
            _ => {}
        }
        i += 1;
    }

    // Load all formulae from API cache
    let formulae = api::load_all_formulae()?;

    // Find outdated formulae
    let mut outdated_formulae = Vec::new();

    for formula in formulae {
        // If specific formulae requested, filter
        if !specific_formulae.is_empty()
            && !specific_formulae.contains(&formula.name)
            && !specific_formulae.contains(&formula.full_name)
        {
            continue;
        }

        // Skip if not installed (check opt symlink exists)
        let opt_path = paths::homebrew_prefix().join("opt").join(&formula.name);
        if !opt_path.exists() {
            continue;
        }

        // Get installed versions from Cellar directory
        let cellar_path = paths::homebrew_cellar().join(&formula.name);
        let mut installed_versions = Vec::new();

        if let Ok(entries) = fs::read_dir(&cellar_path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_dir()
                    && let Some(version) = entry.file_name().to_str()
                {
                    installed_versions.push(version.to_string());
                }
            }
        }

        if installed_versions.is_empty() {
            continue;
        }

        // Get current version from API (stable + revision)
        let current_version = match &formula.versions.stable {
            Some(v) => {
                if formula.revision > 0 {
                    format!("{}_{}", v, formula.revision)
                } else {
                    v.clone()
                }
            }
            None => continue,
        };

        // Check if any installed version is outdated
        let mut is_outdated = false;

        for installed in &installed_versions {
            if installed != &current_version {
                is_outdated = true;
                break;
            }
        }

        // If all installed versions match current, not outdated
        if !is_outdated {
            continue;
        }

        outdated_formulae.push((
            formula.name.clone(),
            installed_versions.clone(),
            current_version.clone(),
            formula.pinned,
        ));
    }

    // TODO: Add cask support when cask info is available in API cache

    // Output
    if json_mode {
        output_json(&outdated_formulae, json_version)?;
    } else if quiet {
        for (name, _, _, _) in &outdated_formulae {
            println!("{}", name);
        }
    } else if verbose {
        for (name, installed_versions, current_version, pinned) in &outdated_formulae {
            let installed_str = installed_versions.join(", ");
            let pinned_marker = if *pinned { " (pinned)" } else { "" };
            println!(
                "{} ({}) < {}{}",
                name, installed_str, current_version, pinned_marker
            );
        }
    } else {
        // Default: just show names in interactive mode
        for (name, _, _, _) in &outdated_formulae {
            println!("{}", name);
        }
    }

    Ok(())
}

fn output_json(
    outdated_formulae: &[(String, Vec<String>, String, bool)],
    version: &str,
) -> Result<(), String> {
    if version == "v2" || version == "v1" {
        // v2 format (and v1 for now)
        let formulae_json: Vec<serde_json::Value> = outdated_formulae
            .iter()
            .map(|(name, installed, current, pinned)| {
                serde_json::json!({
                    "name": name,
                    "installed_versions": installed,
                    "current_version": current,
                    "pinned": pinned,
                    "pinned_version": if *pinned { Some(installed.last()) } else { None }
                })
            })
            .collect();

        let output = serde_json::json!({
            "formulae": formulae_json,
            "casks": []
        });

        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    }

    Ok(())
}
