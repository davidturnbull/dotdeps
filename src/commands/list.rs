use crate::commands::{Command, CommandResult};
use crate::paths;
use std::fs;
use std::path::PathBuf;

pub struct ListCommand;

impl Command for ListCommand {
    fn run(&self, args: &[String]) -> CommandResult {
        let mut formulae_only = false;
        let mut casks_only = false;
        let mut show_versions = false;
        let mut pinned_only = false;
        let mut full_name = false;
        let mut specific_items = Vec::new();

        for arg in args {
            match arg.as_str() {
                "--formula" | "--formulae" => formulae_only = true,
                "--cask" | "--casks" => casks_only = true,
                "--versions" => show_versions = true,
                "--pinned" => pinned_only = true,
                "--full-name" => full_name = true,
                _ if !arg.starts_with('-') => specific_items.push(arg.clone()),
                _ => {
                    // Ignore other flags for now
                }
            }
        }

        if !specific_items.is_empty() {
            if show_versions {
                // List versions of specific formulae/casks
                list_specific_versions(&specific_items, formulae_only, casks_only)?;
            } else {
                // List files in specific formula/cask
                list_specific_items(&specific_items, formulae_only, casks_only)?;
            }
        } else if casks_only {
            list_casks(show_versions)?;
        } else if formulae_only {
            list_formulae(show_versions, pinned_only, full_name)?;
        } else {
            // List both formulae and casks
            list_formulae(show_versions, pinned_only, full_name)?;
        }

        Ok(())
    }
}

fn list_formulae(show_versions: bool, pinned_only: bool, full_name: bool) -> CommandResult {
    let cellar = paths::homebrew_cellar();

    if !cellar.exists() {
        return Ok(());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&cellar)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        if pinned_only {
            // Check if formula is pinned
            let pinned_path = paths::homebrew_prefix()
                .join("var/homebrew/pinned")
                .join(&name);
            if !pinned_path.exists() {
                continue;
            }
        }

        if show_versions {
            // List all versions
            let formula_path = entry.path();
            if formula_path.is_dir() {
                let mut versions = Vec::new();
                for version_entry in fs::read_dir(&formula_path)? {
                    let version_entry = version_entry?;
                    let version = version_entry.file_name().to_string_lossy().to_string();
                    if !version.starts_with('.') {
                        versions.push(version);
                    }
                }

                if !versions.is_empty() {
                    versions.sort();
                    println!("{} {}", name, versions.join(" "));
                }
            }
        } else {
            entries.push(name);
        }
    }

    if !show_versions {
        entries.sort();
        for entry in entries {
            if full_name {
                // For now, just print the name (would need tap info for full name)
                println!("{}", entry);
            } else {
                println!("{}", entry);
            }
        }
    }

    Ok(())
}

fn list_casks(show_versions: bool) -> CommandResult {
    let caskroom = paths::homebrew_caskroom();

    if !caskroom.exists() {
        return Ok(());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&caskroom)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        if show_versions {
            // List all versions
            let cask_path = entry.path();
            if cask_path.is_dir() {
                let mut versions = Vec::new();
                for version_entry in fs::read_dir(&cask_path)? {
                    let version_entry = version_entry?;
                    let version = version_entry.file_name().to_string_lossy().to_string();
                    if !version.starts_with('.') {
                        versions.push(version);
                    }
                }

                if !versions.is_empty() {
                    versions.sort();
                    println!("{} {}", name, versions.join(" "));
                }
            }
        } else {
            entries.push(name);
        }
    }

    if !show_versions {
        entries.sort();
        for entry in entries {
            println!("{}", entry);
        }
    }

    Ok(())
}

fn list_specific_versions(
    items: &[String],
    formulae_only: bool,
    casks_only: bool,
) -> CommandResult {
    for item in items {
        let formula_path = paths::homebrew_cellar().join(item);
        let cask_path = paths::homebrew_caskroom().join(item);

        let is_formula = formula_path.exists();
        let is_cask = cask_path.exists();

        if !formulae_only && !casks_only {
            // Auto-detect
            if is_formula {
                print_versions(&formula_path, item)?;
            } else if is_cask {
                print_versions(&cask_path, item)?;
            } else {
                eprintln!(
                    "Error: No available formula or cask with the name \"{}\".",
                    item
                );
                std::process::exit(1);
            }
        } else if formulae_only {
            if is_formula {
                print_versions(&formula_path, item)?;
            } else {
                eprintln!("Error: No available formula with the name \"{}\".", item);
                std::process::exit(1);
            }
        } else if casks_only {
            if is_cask {
                print_versions(&cask_path, item)?;
            } else {
                eprintln!("Error: No available cask with the name \"{}\".", item);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn print_versions(path: &PathBuf, name: &str) -> CommandResult {
    let mut versions = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let version = entry.file_name().to_string_lossy().to_string();
        if !version.starts_with('.') && entry.path().is_dir() {
            versions.push(version);
        }
    }

    if !versions.is_empty() {
        versions.sort();
        println!("{} {}", name, versions.join(" "));
    }

    Ok(())
}

fn list_specific_items(items: &[String], formulae_only: bool, casks_only: bool) -> CommandResult {
    for item in items {
        let formula_path = paths::homebrew_cellar().join(item);
        let cask_path = paths::homebrew_caskroom().join(item);

        let is_formula = formula_path.exists();
        let is_cask = cask_path.exists();

        if !formulae_only && !casks_only {
            // Auto-detect
            if is_formula {
                list_formula_files(&formula_path)?;
            } else if is_cask {
                list_cask_artifacts(&cask_path)?;
            } else {
                eprintln!(
                    "Error: No available formula or cask with the name \"{}\".",
                    item
                );
                std::process::exit(1);
            }
        } else if formulae_only {
            if is_formula {
                list_formula_files(&formula_path)?;
            } else {
                eprintln!("Error: No available formula with the name \"{}\".", item);
                std::process::exit(1);
            }
        } else if casks_only {
            if is_cask {
                list_cask_artifacts(&cask_path)?;
            } else {
                eprintln!("Error: No available cask with the name \"{}\".", item);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn list_formula_files(formula_path: &PathBuf) -> CommandResult {
    // Find the latest version directory
    let mut versions = Vec::new();
    for entry in fs::read_dir(formula_path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with('.') && entry.path().is_dir() {
            versions.push((name, entry.path()));
        }
    }

    if versions.is_empty() {
        return Ok(());
    }

    // Sort and get the latest version
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    let version_path = &versions[0].1;

    // Use the system ls command to list files (matching brew's behavior)
    use std::process::Command;
    let output = Command::new("ls").arg(version_path).output()?;

    if !output.status.success() {
        return Err(format!(
            "ls command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    print!("{}", String::from_utf8_lossy(&output.stdout));

    Ok(())
}

fn list_cask_artifacts(cask_path: &PathBuf) -> CommandResult {
    // Find the latest version directory
    let mut versions = Vec::new();
    for entry in fs::read_dir(cask_path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with('.') && entry.path().is_dir() {
            versions.push((name, entry.path()));
        }
    }

    if versions.is_empty() {
        return Ok(());
    }

    // Sort and get the latest version
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    let version_path = &versions[0].1;

    // Use the system ls command to list artifacts (matching brew's behavior)
    use std::process::Command;
    let output = Command::new("ls").arg(version_path).output()?;

    if !output.status.success() {
        return Err(format!(
            "ls command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    print!("{}", String::from_utf8_lossy(&output.stdout));

    Ok(())
}
