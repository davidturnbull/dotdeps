use crate::api;
use crate::paths;
use std::collections::{HashMap, HashSet};

struct DependencyCheckOptions {
    include_build: bool,
    include_optional: bool,
    include_test: bool,
    skip_recommended: bool,
    recursive: bool,
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut formulae: Vec<String> = Vec::new();
    let mut installed = false;
    let mut recursive = false;
    let mut include_build = false;
    let mut include_optional = false;
    let mut include_test = false;
    let mut skip_recommended = false;
    let mut eval_all = false;
    let mut _formula_only = false;
    let mut _cask_only = false;

    for arg in args {
        if arg == "--installed" {
            installed = true;
        } else if arg == "--recursive" {
            recursive = true;
        } else if arg == "--include-build" {
            include_build = true;
        } else if arg == "--include-optional" {
            include_optional = true;
        } else if arg == "--include-test" {
            include_test = true;
        } else if arg == "--skip-recommended" {
            skip_recommended = true;
        } else if arg == "--eval-all" {
            eval_all = true;
        } else if arg == "--formula" || arg == "--formulae" {
            _formula_only = true;
        } else if arg == "--cask" || arg == "--casks" {
            _cask_only = true;
        } else if arg == "--help" || arg == "-h" {
            println!("{}", include_str!("../../help/uses.txt"));
            return Ok(());
        } else if !arg.starts_with('-') {
            formulae.push(arg.clone());
        }
    }

    if formulae.is_empty() {
        eprintln!("Error: This command requires a formula argument");
        std::process::exit(1);
    }

    // Check if --installed or --eval-all is set or HOMEBREW_EVAL_ALL env var
    if !installed && !eval_all && std::env::var("HOMEBREW_EVAL_ALL").is_err() {
        eprintln!(
            "Error: Invalid usage: `brew uses` needs `--installed` or `--eval-all` passed or `HOMEBREW_EVAL_ALL=1` set!"
        );
        std::process::exit(1);
    }

    // Get list of formulae to check
    let check_formulae: Vec<String> = if installed {
        // Only check installed formulae
        list_installed_formulae()?
    } else {
        // Check all formulae
        load_all_formula_names()?
    };

    let options = DependencyCheckOptions {
        include_build,
        include_optional,
        include_test,
        skip_recommended,
        // Recursive behavior:
        // - Default (no flags): recursive (all levels)
        // - With --recursive: recursive (explicitly requested)
        // - With --include-build/optional/test: non-recursive (direct only)
        // This matches brew's behavior where --include-build implies direct deps only
        recursive: recursive || (!include_build && !include_optional && !include_test),
    };

    // Build reverse dependency map once upfront (formula -> dependents)
    let reverse_deps = build_reverse_dependency_map(&check_formulae, &options)?;

    // Find dependents for each requested formula
    let mut results: Option<HashSet<String>> = None;

    for formula_name in &formulae {
        let dependents = find_dependents_from_map(formula_name, &reverse_deps, &options);

        results = match results {
            None => Some(dependents),
            Some(prev) => {
                // Intersection: only keep formulae that depend on ALL requested formulae
                Some(prev.intersection(&dependents).cloned().collect())
            }
        };
    }

    // Print results
    if let Some(mut deps) = results {
        let mut deps_vec: Vec<String> = deps.drain().collect();
        deps_vec.sort();
        for dep in deps_vec {
            println!("{}", dep);
        }
    }

    Ok(())
}

fn list_installed_formulae() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let opt_dir = paths::homebrew_prefix().join("opt");
    let mut formulae = Vec::new();

    if opt_dir.exists() {
        for entry in std::fs::read_dir(&opt_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_symlink()
                && let Some(name) = path.file_name()
            {
                formulae.push(name.to_string_lossy().to_string());
            }
        }
    }

    Ok(formulae)
}

fn load_all_formula_names() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let cache_dir = paths::homebrew_cache().join("api");
    let formula_names_path = cache_dir.join("formula_names.txt");

    if !formula_names_path.exists() {
        return Err("Formula names cache not found. Run `brew update` first.".into());
    }

    let content = std::fs::read_to_string(&formula_names_path)?;
    let names: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    Ok(names)
}

/// Build a reverse dependency map: formula -> direct dependents
fn build_reverse_dependency_map(
    check_formulae: &[String],
    options: &DependencyCheckOptions,
) -> Result<HashMap<String, HashSet<String>>, Box<dyn std::error::Error>> {
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();

    for formula_name in check_formulae {
        // Load formula info once
        let formula = match api::get_formula(formula_name) {
            Ok(f) => f,
            Err(_) => continue, // Skip formulae we can't load
        };

        // Process runtime dependencies
        for dep in &formula.dependencies {
            map.entry(dep.clone())
                .or_default()
                .insert(formula_name.clone());
        }

        // Process build dependencies
        if options.include_build {
            for dep in &formula.build_dependencies {
                map.entry(dep.clone())
                    .or_default()
                    .insert(formula_name.clone());
            }
        }

        // Process test dependencies
        if options.include_test {
            for dep in &formula.test_dependencies {
                map.entry(dep.clone())
                    .or_default()
                    .insert(formula_name.clone());
            }
        }

        // Process optional dependencies
        if options.include_optional {
            for dep in &formula.optional_dependencies {
                map.entry(dep.clone())
                    .or_default()
                    .insert(formula_name.clone());
            }
        }

        // Process recommended dependencies (unless skipped)
        if !options.skip_recommended {
            for dep in &formula.recommended_dependencies {
                map.entry(dep.clone())
                    .or_default()
                    .insert(formula_name.clone());
            }
        }
    }

    Ok(map)
}

/// Find all dependents of a formula using the reverse dependency map
fn find_dependents_from_map(
    target: &str,
    reverse_deps: &HashMap<String, HashSet<String>>,
    options: &DependencyCheckOptions,
) -> HashSet<String> {
    if !options.recursive {
        // Non-recursive: just return direct dependents
        reverse_deps
            .get(target)
            .cloned()
            .unwrap_or_else(HashSet::new)
    } else {
        // Recursive: find all transitive dependents
        let mut all_dependents = HashSet::new();
        let mut to_visit = Vec::new();

        // Start with direct dependents
        if let Some(direct_deps) = reverse_deps.get(target) {
            for dep in direct_deps {
                to_visit.push(dep.clone());
            }
        }

        // BFS to find all transitive dependents
        while let Some(current) = to_visit.pop() {
            if all_dependents.contains(&current) {
                continue; // Already visited
            }

            all_dependents.insert(current.clone());

            // Add dependents of current to visit list
            if let Some(next_deps) = reverse_deps.get(&current) {
                for dep in next_deps {
                    if !all_dependents.contains(dep) {
                        to_visit.push(dep.clone());
                    }
                }
            }
        }

        all_dependents
    }
}
