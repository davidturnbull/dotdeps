use crate::{api, paths};
use std::collections::{HashMap, HashSet};

#[allow(clippy::manual_flatten)]
pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut installed_on_request_only = false;
    let mut installed_as_dependency_only = false;

    for arg in args {
        match arg.as_str() {
            "-r" | "--installed-on-request" => installed_on_request_only = true,
            "-p" | "--installed-as-dependency" => installed_as_dependency_only = true,
            "-v" | "--verbose" => {}
            "-q" | "--quiet" => {}
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

    // Conflicting flags check
    if installed_on_request_only && installed_as_dependency_only {
        eprintln!(
            "Error: --installed-on-request and --installed-as-dependency are mutually exclusive"
        );
        std::process::exit(1);
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
    // Stores: (installed_as_dep, installed_on_req, tap_name, display_name)
    let mut formula_metadata: HashMap<String, (bool, bool, Option<String>, String)> =
        HashMap::new();

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

                        // Get tap name from source field
                        let tap_name = tab
                            .get("source")
                            .and_then(|s| s.get("tap"))
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string());

                        // Construct display name
                        // Only show tap prefix for non-core taps
                        let display_name = if let Some(tap) = &tap_name {
                            if tap == "homebrew/core" || tap == "homebrew/cask" {
                                formula_name.clone()
                            } else {
                                format!("{}/{}", tap, formula_name)
                            }
                        } else {
                            formula_name.clone()
                        };

                        formula_metadata.insert(
                            formula_name.clone(),
                            (installed_as_dep, installed_on_req, tap_name, display_name),
                        );
                        break;
                    }
                }
            }
        }

        // Default: assume manually installed if no Tab found
        if !formula_metadata.contains_key(formula_name) {
            formula_metadata.insert(
                formula_name.clone(),
                (false, true, None, formula_name.clone()),
            );
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

    // Find leaf formulae - those with no dependents
    let mut leaves: Vec<String> = Vec::new();
    for formula_name in &installed_formulae {
        // Check if this formula has any dependents
        if !dependents.contains_key(formula_name) {
            // Apply filters based on flags
            if let Some((installed_as_dep, installed_on_req, _tap, display_name)) =
                formula_metadata.get(formula_name)
            {
                if installed_on_request_only && !installed_on_req {
                    continue;
                }
                if installed_as_dependency_only && !installed_as_dep {
                    continue;
                }

                leaves.push(display_name.clone());
            }
        }
    }

    // Sort alphabetically
    leaves.sort();

    // Output
    for leaf in leaves {
        println!("{}", leaf);
    }

    Ok(())
}

fn print_help() {
    println!("Usage: brew leaves [--installed-on-request] [--installed-as-dependency]");
    println!();
    println!("List installed formulae that are not dependencies of another installed formula");
    println!("or cask.");
    println!();
    println!("  -r, --installed-on-request       Only list leaves that were manually");
    println!("                                   installed.");
    println!("  -p, --installed-as-dependency    Only list leaves that were installed as");
    println!("                                   dependencies.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}
