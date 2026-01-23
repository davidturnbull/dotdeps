use crate::{api, paths};
use std::collections::{HashMap, HashSet};

#[allow(clippy::manual_flatten)]
pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut dry_run = false;
    let mut quiet = false;

    for arg in args {
        match arg.as_str() {
            "-n" | "--dry-run" => dry_run = true,
            "-v" | "--verbose" => {}
            "-q" | "--quiet" => quiet = true,
            "-d" | "--debug" => {}
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => {
                eprintln!("Error: Unknown option: {}", arg);
                std::process::exit(1);
            }
        }
    }

    // Get canonical formula names from Cellar directory
    let cellar_path = paths::homebrew_cellar();
    if !cellar_path.exists() {
        return Ok(());
    }

    let mut installed_formulae: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&cellar_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir()
            && let Some(name) = path.file_name()
        {
            installed_formulae.push(name.to_string_lossy().to_string());
        }
    }

    if installed_formulae.is_empty() {
        return Ok(());
    }

    // Read Tab metadata for each formula
    // Stores: (installed_as_dep, installed_on_req)
    let mut formula_metadata: HashMap<String, (bool, bool)> = HashMap::new();

    for formula_name in &installed_formulae {
        let cellar_formula_path = cellar_path.join(formula_name);
        if let Ok(entries) = std::fs::read_dir(&cellar_formula_path) {
            for entry in entries.flatten() {
                let version_path = entry.path();
                if version_path.is_dir() {
                    let tab_file = version_path.join("INSTALL_RECEIPT.json");
                    if tab_file.exists()
                        && let Ok(content) = std::fs::read_to_string(&tab_file)
                        && let Ok(tab) = serde_json::from_str::<serde_json::Value>(&content)
                    {
                        let installed_as_dep = tab
                            .get("installed_as_dependency")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let installed_on_req = tab
                            .get("installed_on_request")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);

                        formula_metadata
                            .insert(formula_name.clone(), (installed_as_dep, installed_on_req));
                        break;
                    }
                }
            }
        }

        // Default: assume manually installed if no Tab found
        if !formula_metadata.contains_key(formula_name) {
            formula_metadata.insert(formula_name.clone(), (false, true));
        }
    }

    // Build dependency graph - which formulae depend on which others
    // dependents[canonical_formula_name] = set of canonical formula names that depend on it
    let mut dependents: HashMap<String, HashSet<String>> = HashMap::new();

    // Track formula dependencies
    for formula_name in &installed_formulae {
        // Get dependencies for this formula
        if let Ok(formula_info) = api::get_formula(formula_name) {
            // Track runtime dependencies only
            for dep in &formula_info.dependencies {
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .insert(formula_name.clone());
            }
        }
    }

    // Also track cask formula dependencies
    let caskroom_path = paths::homebrew_prefix().join("Caskroom");
    if caskroom_path.exists()
        && let Ok(cask_entries) = std::fs::read_dir(&caskroom_path)
    {
        for cask_entry in cask_entries {
            if let Ok(cask_entry) = cask_entry {
                let cask_path = cask_entry.path();
                if cask_path.is_dir() {
                    // Look for .metadata/*/Casks/*.json files
                    let metadata_path = cask_path.join(".metadata");
                    if metadata_path.exists()
                        && let Ok(version_entries) = std::fs::read_dir(&metadata_path)
                    {
                        for version_entry in version_entries {
                            if let Ok(version_entry) = version_entry {
                                let version_path = version_entry.path();
                                if version_path.is_dir() {
                                    // Go one more level deep for timestamp directories
                                    if let Ok(timestamp_entries) = std::fs::read_dir(&version_path)
                                    {
                                        for timestamp_entry in timestamp_entries {
                                            if let Ok(timestamp_entry) = timestamp_entry {
                                                let timestamp_path = timestamp_entry.path();
                                                if timestamp_path.is_dir() {
                                                    let casks_path = timestamp_path.join("Casks");
                                                    if casks_path.exists()
                                                        && let Ok(cask_json_entries) =
                                                            std::fs::read_dir(&casks_path)
                                                    {
                                                        for cask_json_entry in cask_json_entries {
                                                            if let Ok(cask_json_entry) =
                                                                cask_json_entry
                                                            {
                                                                let cask_json_path =
                                                                    cask_json_entry.path();
                                                                if cask_json_path
                                                                    .extension()
                                                                    .map(|e| e == "json")
                                                                    .unwrap_or(false)
                                                                    && let Ok(content) =
                                                                        std::fs::read_to_string(
                                                                            &cask_json_path,
                                                                        )
                                                                    && let Ok(cask_json) =
                                                                        serde_json::from_str::<
                                                                            serde_json::Value,
                                                                        >(
                                                                            &content
                                                                        )
                                                                {
                                                                    // Get depends_on.formula array
                                                                    if let Some(depends_on) =
                                                                        cask_json.get("depends_on")
                                                                        && let Some(formula_deps) =
                                                                            depends_on
                                                                                .get("formula")
                                                                        && let Some(formula_array) =
                                                                            formula_deps.as_array()
                                                                    {
                                                                        for dep in formula_array {
                                                                            if let Some(dep_name) =
                                                                                dep.as_str()
                                                                            {
                                                                                // Mark this formula as having a dependent (the cask)
                                                                                dependents.entry(dep_name.to_string())
                                                                                                                .or_default()
                                                                                                                .insert("__cask__".to_string());
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Find formulae that can be removed:
    // - Installed as dependency (not manually)
    // - No longer have dependents (are leaves)
    let mut removable: Vec<String> = Vec::new();
    for formula_name in &installed_formulae {
        // Check metadata
        if let Some((installed_as_dep, _installed_on_req)) = formula_metadata.get(formula_name) {
            // Must be installed as dependency
            if *installed_as_dep {
                // Must have no dependents (is a leaf)
                if !dependents.contains_key(formula_name) {
                    removable.push(formula_name.clone());
                }
            }
        }
    }

    // Sort alphabetically
    removable.sort();

    // If nothing to remove
    if removable.is_empty() {
        // Brew outputs nothing when there's nothing to remove
        return Ok(());
    }

    // Show what will be removed
    let formula_word = if removable.len() == 1 {
        "formula"
    } else {
        "formulae"
    };

    if dry_run {
        if !quiet {
            println!(
                "==> Would autoremove {} unneeded {}:",
                removable.len(),
                formula_word
            );
        }
        for formula in &removable {
            println!("{}", formula);
        }
        return Ok(());
    }

    // Show what will be removed (unless quiet)
    if !quiet {
        println!(
            "==> Autoremoving {} unneeded {}:",
            removable.len(),
            formula_word
        );
        for formula in &removable {
            println!("{}", formula);
        }
    }

    // Remove each formula
    for formula_name in &removable {
        // Get the opt symlink target to find the version directory
        let opt_path = paths::homebrew_prefix().join("opt").join(formula_name);
        let keg_path = if opt_path.exists() {
            std::fs::canonicalize(&opt_path).unwrap_or_else(|_| cellar_path.join(formula_name))
        } else {
            cellar_path.join(formula_name)
        };

        // Remove the keg directory
        if keg_path.exists() {
            std::fs::remove_dir_all(&keg_path)?;
        }

        // Remove the opt symlink
        if opt_path.exists() {
            std::fs::remove_file(&opt_path)?;
        }

        // Remove symlinks from bin, lib, etc.
        let prefix = paths::homebrew_prefix();
        let dirs_to_check = [
            "bin",
            "sbin",
            "lib",
            "include",
            "share",
            "etc",
            "Frameworks",
        ];

        for dir_name in &dirs_to_check {
            let dir_path = prefix.join(dir_name);
            if !dir_path.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(&dir_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_symlink() {
                        // Check if this symlink points to our keg
                        if let Ok(target) = std::fs::read_link(&path)
                            && let Ok(target_path) = std::fs::canonicalize(dir_path.join(&target))
                            && target_path.starts_with(&keg_path)
                        {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!("Usage: brew autoremove [--dry-run]");
    println!();
    println!("Uninstall formulae that were only installed as a dependency of another formula");
    println!("and are now no longer needed.");
    println!();
    println!("  -n, --dry-run                    List what would be uninstalled, but do not");
    println!("                                   actually uninstall anything.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}
