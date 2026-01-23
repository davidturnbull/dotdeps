//! Implementation of the `deps` command.
//!
//! Shows dependencies for formulae.

use std::collections::HashSet;

use crate::api;
use crate::paths;

/// Options for showing dependency trees.
struct TreeOptions {
    include_build: bool,
    include_optional: bool,
    show_installed: bool,
    max_depth: usize,
}

pub fn run(args: &[String]) -> Result<(), String> {
    // Parse flags
    let mut show_tree = false;
    let mut include_build = false;
    let mut include_optional = false;
    let mut show_installed = false;
    let mut formulae = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--tree" => show_tree = true,
            "--include-build" => include_build = true,
            "--include-optional" => include_optional = true,
            "--installed" => show_installed = true,
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg));
            }
            arg => formulae.push(arg.to_string()),
        }
        i += 1;
    }

    if formulae.is_empty() {
        return Err("This command requires a formula argument".to_string());
    }

    // Show warning unless --installed is passed
    if !show_installed {
        if show_tree {
            eprintln!(
                "Warning: `brew deps` is not the actual runtime dependencies because --tree was passed!"
            );
        } else if include_build {
            eprintln!(
                "Warning: `brew deps` is not the actual runtime dependencies because --include-build was passed!"
            );
        } else {
            eprintln!(
                "Warning: `brew deps` is not the actual runtime dependencies because `--installed` was not passed!"
            );
        }
        eprintln!("This means dependencies may differ from a formula's declared dependencies.");
        eprintln!("Hide these hints with `HOMEBREW_NO_ENV_HINTS=1` (see `man brew`).");
    }

    if show_tree {
        // Tree view
        let opts = TreeOptions {
            include_build,
            include_optional,
            show_installed,
            max_depth: 100,
        };
        for formula in &formulae {
            println!("{}", formula);
            show_dependencies_tree(formula, &opts)?;
        }
    } else {
        // Flat list view
        let mut all_deps = HashSet::new();
        for formula in &formulae {
            collect_dependencies(
                formula,
                &mut all_deps,
                include_build,
                include_optional,
                show_installed,
            )?;
        }

        // Sort and print
        let mut deps: Vec<String> = all_deps.into_iter().collect();
        deps.sort();
        for dep in deps {
            println!("{}", dep);
        }
    }

    Ok(())
}

/// Collect all dependencies recursively.
fn collect_dependencies(
    formula_name: &str,
    all_deps: &mut HashSet<String>,
    include_build: bool,
    include_optional: bool,
    show_installed: bool,
) -> Result<(), String> {
    let info = api::get_formula(formula_name)
        .map_err(|e| format!("Formula not found: {}: {}", formula_name, e))?;

    // Get dependencies
    let mut deps = info.dependencies.clone();
    if include_build {
        deps.extend(info.build_dependencies.clone());
    }
    if include_optional {
        deps.extend(info.optional_dependencies.clone());
    }

    // Filter by installed if needed
    if show_installed {
        deps.retain(|d| is_installed(d));
    }

    // Recursively collect
    for dep in deps {
        if all_deps.insert(dep.clone()) {
            collect_dependencies(
                &dep,
                all_deps,
                include_build,
                include_optional,
                show_installed,
            )?;
        }
    }

    Ok(())
}

/// Show dependencies tree for a formula.
fn show_dependencies_tree(formula_name: &str, opts: &TreeOptions) -> Result<(), String> {
    // Load formula
    let info = api::get_formula(formula_name)
        .map_err(|e| format!("Formula not found: {}: {}", formula_name, e))?;

    // Get dependencies
    let mut deps = info.dependencies.clone();
    if opts.include_build {
        deps.extend(info.build_dependencies.clone());
    }
    if opts.include_optional {
        deps.extend(info.optional_dependencies.clone());
    }

    // Filter by installed if needed
    if opts.show_installed {
        deps.retain(|d| is_installed(d));
    }

    if deps.is_empty() {
        return Ok(());
    }

    // Show each dependency with tree formatting (with depth limit to prevent infinite recursion)
    for (i, dep) in deps.iter().enumerate() {
        let is_last = i == deps.len() - 1;
        show_tree_recursive(dep, "", is_last, opts, 0)?;
    }

    Ok(())
}

/// Show dependency tree recursively.
fn show_tree_recursive(
    formula_name: &str,
    prefix: &str,
    is_last: bool,
    opts: &TreeOptions,
    depth: usize,
) -> Result<(), String> {
    // Prevent infinite recursion
    if depth >= opts.max_depth {
        return Ok(());
    }
    // Print current node
    let branch = if is_last { "└── " } else { "├── " };

    println!("{}{}{}", prefix, branch, formula_name);

    // Load formula
    let info = api::get_formula(formula_name)
        .map_err(|e| format!("Formula not found: {}: {}", formula_name, e))?;

    // Get dependencies
    let mut deps = info.dependencies.clone();
    if opts.include_build {
        deps.extend(info.build_dependencies.clone());
    }
    if opts.include_optional {
        deps.extend(info.optional_dependencies.clone());
    }

    // Filter by installed if needed
    if opts.show_installed {
        deps.retain(|d| is_installed(d));
    }

    if deps.is_empty() {
        return Ok(());
    }

    // Calculate new prefix for children
    let child_prefix = if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
    };

    // Recursively show children
    for (i, dep) in deps.iter().enumerate() {
        let is_last_child = i == deps.len() - 1;
        show_tree_recursive(dep, &child_prefix, is_last_child, opts, depth + 1)?;
    }

    Ok(())
}

/// Check if a formula is installed by checking opt symlink.
fn is_installed(formula_name: &str) -> bool {
    let opt_path = paths::homebrew_prefix().join("opt").join(formula_name);
    opt_path.exists()
}
