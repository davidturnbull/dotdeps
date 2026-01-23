use crate::commands::{Command, CommandResult};
use crate::paths;
use std::fs;
use std::path::PathBuf;
use std::process::Command as StdCommand;

pub struct DoctorCommand;

impl Command for DoctorCommand {
    fn run(&self, args: &[String]) -> CommandResult {
        let mut list_checks = false;
        let mut verbose = false;

        // Parse flags
        for arg in args {
            match arg.as_str() {
                "--list-checks" => list_checks = true,
                "-v" | "--verbose" => verbose = true,
                "-h" | "--help" => {
                    print_help();
                    return Ok(());
                }
                _ => {}
            }
        }

        if list_checks {
            list_all_checks();
            return Ok(());
        }

        // Show intro message
        println!("Please note that these warnings are just used to help the Homebrew maintainers");
        println!("with debugging if you file an issue. If everything you use Homebrew for is");
        println!("working fine: please don't worry or file an issue; just ignore this. Thanks!");

        let mut issues_found = false;

        // Run diagnostic checks
        issues_found |= check_for_unlinked_but_not_keg_only(verbose);
        issues_found |= check_user_path(verbose);
        issues_found |= check_for_broken_symlinks(verbose);
        issues_found |= check_for_git(verbose);

        if !issues_found {
            println!("\nYour system is ready to brew.");
        }

        // Exit with non-zero status if issues found (matching brew behavior)
        if issues_found {
            std::process::exit(1);
        }

        Ok(())
    }
}

fn print_help() {
    println!("Usage: brew doctor, dr [--list-checks] [--audit-debug] [diagnostic_check ...]");
    println!();
    println!("Check your system for potential problems. Will exit with a non-zero status if");
    println!("any potential problems are found.");
    println!();
    println!("Please note that these warnings are just used to help the Homebrew maintainers");
    println!("with debugging if you file an issue. If everything you use Homebrew for is");
    println!("working fine: please don't worry or file an issue; just ignore this.");
    println!();
    println!("      --list-checks                List all audit methods, which can be run");
    println!("                                   individually if provided as arguments.");
    println!("  -D, --audit-debug                Enable debugging and profiling of audit");
    println!("                                   methods.");
    println!("  -d, --debug                      Display any debugging information.");
    println!("  -q, --quiet                      Make some output more quiet.");
    println!("  -v, --verbose                    Make some output more verbose.");
    println!("  -h, --help                       Show this message.");
}

fn list_all_checks() {
    // List the checks we've implemented
    println!("check_for_broken_symlinks");
    println!("check_for_git");
    println!("check_for_unlinked_but_not_keg_only");
    println!("check_user_path");
}

/// Check for unlinked kegs that are not keg-only
fn check_for_unlinked_but_not_keg_only(_verbose: bool) -> bool {
    let prefix = paths::homebrew_prefix();
    let cellar = paths::homebrew_cellar();
    let opt_dir = prefix.join("opt");

    let mut unlinked = Vec::new();

    // Read cellar directory
    if let Ok(entries) = fs::read_dir(&cellar) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type()
                && file_type.is_dir()
            {
                let formula_name = entry.file_name();
                let opt_link = opt_dir.join(&formula_name);

                // If there's a cellar directory but no opt link, it's unlinked
                if !opt_link.exists() {
                    unlinked.push(formula_name.to_string_lossy().to_string());
                } else if let Ok(metadata) = fs::symlink_metadata(&opt_link)
                    && metadata.is_symlink()
                    && let Ok(target) = fs::read_link(&opt_link)
                {
                    // Check if the symlink is broken
                    let absolute_target = if target.is_absolute() {
                        target
                    } else {
                        opt_link.parent().unwrap().join(target)
                    };
                    if !absolute_target.exists() {
                        unlinked.push(formula_name.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    if !unlinked.is_empty() {
        println!();
        println!("Warning: You have unlinked kegs in your Cellar.");
        println!(
            "Leaving kegs unlinked can lead to build-trouble and cause formulae that depend on"
        );
        println!("those kegs to fail to run properly once built. Run `brew link` on these:");
        for formula in &unlinked {
            println!("  {}", formula);
        }
        return true;
    }

    false
}

/// Check PATH ordering
fn check_user_path(_verbose: bool) -> bool {
    let prefix = paths::homebrew_prefix();
    let homebrew_bin = prefix.join("bin");

    // Get PATH
    let path = match std::env::var("PATH") {
        Ok(p) => p,
        Err(_) => return false,
    };

    let paths: Vec<&str> = path.split(':').collect();

    // Find positions of /usr/bin and homebrew/bin
    let usr_bin_pos = paths.iter().position(|&p| p == "/usr/bin");
    let homebrew_pos = paths
        .iter()
        .position(|&p| p == homebrew_bin.to_str().unwrap());

    if let (Some(usr_pos), Some(brew_pos)) = (usr_bin_pos, homebrew_pos)
        && usr_pos < brew_pos
    {
        println!();
        println!(
            "Warning: /usr/bin occurs before {} in your PATH.",
            homebrew_bin.display()
        );
        println!("This means that system-provided programs will be used instead of those");
        println!("provided by Homebrew. Consider setting your PATH so that");
        println!(
            "{} occurs before /usr/bin. Here is a one-liner:",
            homebrew_bin.display()
        );

        // Detect shell
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let rc_file = if shell.contains("zsh") {
            "~/.zshrc"
        } else if shell.contains("bash") {
            "~/.bashrc"
        } else {
            "~/.profile"
        };

        println!(
            "  echo 'export PATH=\"{}:$PATH\"' >> {}",
            homebrew_bin.display(),
            rc_file
        );

        // Find conflicting tools
        let mut conflicts = Vec::new();
        if let Ok(entries) = fs::read_dir(&homebrew_bin) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let usr_bin_path = PathBuf::from("/usr/bin").join(&name);
                if usr_bin_path.exists() {
                    conflicts.push(name.to_string_lossy().to_string());
                }
            }
        }

        if !conflicts.is_empty() {
            println!();
            println!("The following tools exist at both paths:");
            conflicts.sort();
            for tool in conflicts.iter().take(10) {
                println!("  {}", tool);
            }
            if conflicts.len() > 10 {
                println!("  ... and {} more", conflicts.len() - 10);
            }
        }

        return true;
    }

    false
}

/// Check for broken symlinks in prefix
fn check_for_broken_symlinks(_verbose: bool) -> bool {
    let prefix = paths::homebrew_prefix();
    let mut broken_links = Vec::new();

    // Check standard directories for broken symlinks
    let check_dirs = vec!["bin", "sbin", "lib", "include", "share", "etc", "opt"];

    for dir_name in check_dirs {
        let dir = prefix.join(dir_name);
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(metadata) = fs::symlink_metadata(&path)
                    && metadata.is_symlink()
                {
                    // Check if symlink is broken
                    if let Ok(target) = fs::read_link(&path) {
                        let absolute_target = if target.is_absolute() {
                            target
                        } else {
                            path.parent().unwrap().join(target)
                        };

                        if !absolute_target.exists() {
                            broken_links.push(path.display().to_string());
                        }
                    }
                }
            }
        }
    }

    if !broken_links.is_empty() {
        println!();
        println!("Warning: You have broken symlinks in your Homebrew installation.");
        println!(
            "These can cause problems when building software. Run `brew cleanup` to remove them."
        );
        println!();
        println!("Broken symlinks:");
        for link in broken_links.iter().take(10) {
            println!("  {}", link);
        }
        if broken_links.len() > 10 {
            println!("  ... and {} more", broken_links.len() - 10);
        }
        return true;
    }

    false
}

/// Check for git installation
fn check_for_git(_verbose: bool) -> bool {
    let output = StdCommand::new("which").arg("git").output();

    match output {
        Ok(output) if output.status.success() => false,
        _ => {
            println!();
            println!("Warning: Git is not installed.");
            println!("Homebrew requires Git to update itself and install formulae from source.");
            println!("Please install Git using your system's package manager.");
            true
        }
    }
}
