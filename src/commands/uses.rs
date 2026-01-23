use crate::api;
use crate::paths;
use std::collections::HashSet;

struct DependencyCheckOptions {
    include_build: bool,
    include_optional: bool,
    include_test: bool,
    skip_recommended: bool,
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut formulae: Vec<String> = Vec::new();
    let mut installed = false;
    let mut _recursive = false;
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
            _recursive = true;
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

    // Find dependents for each requested formula
    let mut results: Option<HashSet<String>> = None;
    let options = DependencyCheckOptions {
        include_build,
        include_optional,
        include_test,
        skip_recommended,
    };

    for formula_name in &formulae {
        let dependents = find_dependents(formula_name, &check_formulae, &options)?;

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

fn find_dependents(
    target: &str,
    check_formulae: &[String],
    options: &DependencyCheckOptions,
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let mut dependents = HashSet::new();

    for formula_name in check_formulae {
        if depends_on(
            formula_name,
            target,
            options,
            &mut HashSet::new(),
            0, // depth
        )? {
            dependents.insert(formula_name.clone());
        }
    }

    Ok(dependents)
}

fn depends_on(
    formula_name: &str,
    target: &str,
    options: &DependencyCheckOptions,
    visited: &mut HashSet<String>,
    depth: u32,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Depth limit to prevent excessive recursion (set low for performance)
    // Only check direct dependencies (depth 1) for now
    if depth > 1 {
        return Ok(false);
    }

    // Prevent infinite loops
    if visited.contains(formula_name) {
        return Ok(false);
    }
    visited.insert(formula_name.to_string());

    // Load formula info
    let formula = match api::get_formula(formula_name) {
        Ok(f) => f,
        Err(_) => return Ok(false),
    };

    // Check direct dependencies
    for dep in &formula.dependencies {
        if dep == target {
            return Ok(true);
        }
        // Check transitive dependencies
        if depends_on(dep, target, options, visited, depth + 1)? {
            return Ok(true);
        }
    }

    // Check build dependencies
    if options.include_build {
        for dep in &formula.build_dependencies {
            if dep == target {
                return Ok(true);
            }
            // Always check transitive dependencies
            if depends_on(dep, target, options, visited, depth + 1)? {
                return Ok(true);
            }
        }
    }

    // Check test dependencies
    if options.include_test {
        for dep in &formula.test_dependencies {
            if dep == target {
                return Ok(true);
            }
            // Always check transitive dependencies
            if depends_on(dep, target, options, visited, depth + 1)? {
                return Ok(true);
            }
        }
    }

    // Check optional dependencies
    if options.include_optional {
        for dep in &formula.optional_dependencies {
            if dep == target {
                return Ok(true);
            }
            // Always check transitive dependencies
            if depends_on(dep, target, options, visited, depth + 1)? {
                return Ok(true);
            }
        }
    }

    // Check recommended dependencies (unless skipped)
    if !options.skip_recommended {
        for dep in &formula.recommended_dependencies {
            if dep == target {
                return Ok(true);
            }
            // Always check transitive dependencies
            if depends_on(dep, target, options, visited, depth + 1)? {
                return Ok(true);
            }
        }
    }

    Ok(false)
}
