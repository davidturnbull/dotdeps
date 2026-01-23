use crate::api;
use crate::paths;
use std::collections::HashSet;
use std::fs;

pub fn execute(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut hide: Vec<String> = Vec::new();
    let mut formula_names: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(value) = arg.strip_prefix("--hide=") {
            // Parse comma-separated list
            hide.extend(value.split(',').map(|s| s.trim().to_string()));
        } else if arg == "--hide" && i + 1 < args.len() {
            i += 1;
            hide.extend(args[i].split(',').map(|s| s.trim().to_string()));
        } else if !arg.starts_with('-') {
            formula_names.push(arg.clone());
        }
        i += 1;
    }

    // Check if HOMEBREW_CELLAR exists
    let cellar = paths::homebrew_cellar();
    if !cellar.exists() {
        return Ok(());
    }

    let hide_set: HashSet<_> = hide.iter().map(|s| s.as_str()).collect();

    // Get list of formulae to check
    let formulae_to_check = if formula_names.is_empty() {
        // Check all installed formulae
        get_installed_formulae()?
    } else {
        formula_names
    };

    // Load formula cache to get dependency information
    let formula_cache = api::load_formula_cache()?;

    let mut has_missing = false;
    let show_formula_name = formulae_to_check.len() > 1;

    for formula_name in &formulae_to_check {
        // Get formula info from API cache
        let formula_info = match formula_cache.get(formula_name) {
            Some(info) => info,
            None => continue, // Skip if formula not in cache
        };

        // Get dependencies - prefer runtime_dependencies from installed versions, fall back to dependencies field
        let mut dependencies: Vec<String> = Vec::new();

        // First try to get runtime_dependencies from installed versions
        if let Some(installed) = formula_info.installed.first()
            && !installed.runtime_dependencies.is_empty()
        {
            dependencies = installed
                .runtime_dependencies
                .iter()
                .map(|d| d.full_name.clone())
                .collect();
        }

        // Fall back to dependencies field if no runtime_dependencies
        if dependencies.is_empty() {
            dependencies = formula_info.dependencies.clone();
        }

        // Check which dependencies are missing
        let mut missing: Vec<String> = Vec::new();

        for dep in &dependencies {
            // Check if this dep is in the hide list
            if hide_set.contains(dep.as_str()) {
                missing.push(dep.clone());
                continue;
            }

            // Check if dependency is installed
            let dep_opt = paths::homebrew_prefix().join("opt").join(dep);
            if !dep_opt.exists() {
                missing.push(dep.clone());
            }
        }

        // Print missing dependencies
        if !missing.is_empty() {
            has_missing = true;
            if show_formula_name {
                print!("{}: ", formula_name);
            }
            println!("{}", missing.join(" "));
        }
    }

    if has_missing {
        std::process::exit(1);
    }

    Ok(())
}

/// Get list of all installed formulae names.
fn get_installed_formulae() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let cellar = paths::homebrew_cellar();
    let mut formulae = Vec::new();

    if let Ok(entries) = fs::read_dir(&cellar) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type()
                && file_type.is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                formulae.push(name.to_string());
            }
        }
    }

    formulae.sort();
    Ok(formulae)
}
