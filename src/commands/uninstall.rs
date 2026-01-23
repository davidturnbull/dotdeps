use crate::api;
use crate::formula;
use crate::paths;
use std::collections::HashSet;
use std::fs;

pub struct UninstallCommand;

impl crate::commands::Command for UninstallCommand {
    fn run(&self, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        let mut formulae = Vec::new();
        let mut force = false;
        let mut ignore_dependencies = false;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-f" | "--force" => force = true,
                "--ignore-dependencies" => ignore_dependencies = true,
                "-h" | "--help" => {
                    print_help();
                    return Ok(());
                }
                arg if arg.starts_with('-') => {
                    eprintln!("Warning: Unknown flag: {}", arg);
                }
                arg => formulae.push(arg.to_string()),
            }
            i += 1;
        }

        if formulae.is_empty() {
            return Err("No formulae specified".into());
        }

        for formula_name in &formulae {
            uninstall_formula(formula_name, force, ignore_dependencies)?;
        }

        Ok(())
    }
}

fn uninstall_formula(
    formula_name: &str,
    force: bool,
    ignore_dependencies: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = formula::normalize_name(formula_name);

    // Check if formula is installed
    let opt_path = paths::homebrew_prefix().join("opt").join(name);
    if !opt_path.exists() && opt_path.symlink_metadata().is_err() {
        return Err(format!("Error: {} is not installed", name).into());
    }

    // Find the installed version
    let cellar_path = paths::homebrew_cellar().join(name);
    if !cellar_path.exists() {
        return Err(format!("Error: No such keg: {}", cellar_path.display()).into());
    }

    // Get all installed versions
    let mut versions = Vec::new();
    if let Ok(entries) = fs::read_dir(&cellar_path) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && let Some(version) = entry.file_name().to_str()
            {
                versions.push(version.to_string());
            }
        }
    }

    if versions.is_empty() {
        return Err(format!("Error: No versions found for {}", name).into());
    }

    // If force flag is set, uninstall all versions
    let versions_to_remove = if force {
        versions.clone()
    } else {
        // Only remove the currently linked version
        // Find which version is linked from opt
        if let Ok(target) = fs::read_link(&opt_path) {
            if let Some(version) = target.file_name().and_then(|n| n.to_str()) {
                vec![version.to_string()]
            } else {
                versions.clone()
            }
        } else {
            versions.clone()
        }
    };

    // Check for dependents unless --ignore-dependencies
    if !ignore_dependencies {
        let dependents = find_dependents(name)?;
        if !dependents.is_empty() {
            let keg_path = cellar_path.join(&versions_to_remove[0]);
            let dep_list = if dependents.len() == 1 {
                dependents[0].clone()
            } else {
                format!(
                    "{} and {}",
                    dependents[..dependents.len() - 1].join(", "),
                    dependents.last().unwrap()
                )
            };

            return Err(format!(
                "Error: Refusing to uninstall {}\nbecause it is required by {}, which {} currently installed.\nYou can override this and force removal with:\n  brew uninstall --ignore-dependencies {}",
                keg_path.display(),
                dep_list,
                if dependents.len() == 1 { "is" } else { "are" },
                name
            ).into());
        }
    }

    // Calculate total files for display
    let mut total_files = 0;
    let mut total_size = 0u64;
    for version in &versions_to_remove {
        let version_path = cellar_path.join(version);
        if let Ok((files, size)) = count_files_and_size(&version_path) {
            total_files += files;
            total_size += size;
        }
    }

    let size_kb = total_size / 1024;
    let size_str = if size_kb < 1024 {
        format!("{}KB", size_kb)
    } else {
        format!("{:.1}MB", size_kb as f64 / 1024.0)
    };

    // Print uninstalling message
    for version in &versions_to_remove {
        let version_path = cellar_path.join(version);
        println!(
            "Uninstalling {}... ({} files, {})",
            version_path.display(),
            total_files,
            size_str
        );
    }

    // Remove opt symlink
    if opt_path.exists() || opt_path.symlink_metadata().is_ok() {
        fs::remove_file(&opt_path).map_err(|e| format!("Failed to remove opt symlink: {}", e))?;
    }

    // Remove version directories from Cellar
    for version in &versions_to_remove {
        let version_path = cellar_path.join(version);
        if version_path.exists() {
            fs::remove_dir_all(&version_path)
                .map_err(|e| format!("Failed to remove {}: {}", version_path.display(), e))?;
        }
    }

    // Remove the formula directory from Cellar if empty
    if let Ok(entries) = fs::read_dir(&cellar_path)
        && entries.count() == 0
    {
        fs::remove_dir(&cellar_path)
            .map_err(|e| format!("Failed to remove cellar directory: {}", e))?;
    }

    Ok(())
}

/// Find all installed formulae that depend on the given formula
fn find_dependents(formula_name: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut dependents = Vec::new();

    // Get list of all installed formulae
    let opt_dir = paths::homebrew_prefix().join("opt");
    if !opt_dir.exists() {
        return Ok(dependents);
    }

    let entries = fs::read_dir(&opt_dir).map_err(|e| format!("Failed to read opt dir: {}", e))?;

    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            // Skip the formula itself
            if name == formula_name {
                continue;
            }

            // Check if this formula depends on the target formula
            if depends_on(name, formula_name)? {
                dependents.push(name.to_string());
            }
        }
    }

    dependents.sort();
    Ok(dependents)
}

/// Check if formula depends on target_formula
fn depends_on(
    formula_name: &str,
    target_formula: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Load formula info
    let info = match api::get_formula(formula_name) {
        Ok(info) => info,
        Err(_) => return Ok(false), // Formula not found in API
    };

    // Check runtime dependencies
    if info.dependencies.contains(&target_formula.to_string()) {
        return Ok(true);
    }

    // Recursively check dependencies
    let mut visited = HashSet::new();
    check_deps_recursive(&info.dependencies, target_formula, &mut visited)
}

fn check_deps_recursive(
    deps: &[String],
    target: &str,
    visited: &mut HashSet<String>,
) -> Result<bool, Box<dyn std::error::Error>> {
    for dep in deps {
        if dep == target {
            return Ok(true);
        }

        if visited.contains(dep) {
            continue;
        }
        visited.insert(dep.clone());

        // Load this dependency's info
        if let Ok(dep_info) = api::get_formula(dep)
            && check_deps_recursive(&dep_info.dependencies, target, visited)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Count files and total size in a directory recursively
fn count_files_and_size(path: &std::path::Path) -> Result<(usize, u64), std::io::Error> {
    let mut count = 0;
    let mut size = 0u64;

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if metadata.is_dir() {
                let (sub_count, sub_size) = count_files_and_size(&entry.path())?;
                count += sub_count;
                size += sub_size;
            } else {
                count += 1;
                size += metadata.len();
            }
        }
    }

    Ok((count, size))
}

fn print_help() {
    println!(
        "Usage: brew uninstall [options] formula

Uninstall a formula or cask.

  -f, --force                      Delete all installed versions of formula.
      --ignore-dependencies        Don't fail uninstall, even if formula is a
                                   dependency of any installed formulae.
  -h, --help                       Show this message."
    );
}
