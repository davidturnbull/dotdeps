use crate::commands::{Command, CommandResult};
use std::fs;
use std::path::PathBuf;

pub struct UnaliasCommand;

/// Get the path to the aliases directory.
/// Follows Homebrew's logic: ~/.config/brew-aliases or ~/.brew-aliases
fn aliases_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable not set");

    // Check ~/.config/brew-aliases first
    let config_path = PathBuf::from(&home).join(".config/brew-aliases");
    if config_path.exists() {
        return config_path;
    }

    // Check ~/.brew-aliases
    let legacy_path = PathBuf::from(&home).join(".brew-aliases");
    if legacy_path.exists() {
        return legacy_path;
    }

    // Default to ~/.brew-aliases if neither exists
    legacy_path
}

/// Sanitize alias name for filename (replace non-word chars with underscore)
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Get the script path for an alias
fn script_path(name: &str) -> PathBuf {
    aliases_dir().join(sanitize_name(name))
}

/// Get the symlink path for an alias in HOMEBREW_PREFIX/bin
fn symlink_path(name: &str) -> PathBuf {
    crate::paths::homebrew_prefix()
        .join("bin")
        .join(format!("brew-{}", name))
}

/// Remove an alias
fn remove_alias(name: &str) {
    let path = script_path(name);

    if !path.exists() {
        eprintln!("Error: 'brew {}' is not aliased to anything.", name);
        std::process::exit(1);
    }

    // Remove the alias script file
    if let Err(e) = fs::remove_file(&path) {
        eprintln!("Error: Failed to remove alias file: {}", e);
        std::process::exit(1);
    }

    // Remove the symlink
    let symlink = symlink_path(name);
    if symlink.exists()
        && let Err(e) = fs::remove_file(&symlink)
    {
        eprintln!("Error: Failed to remove symlink: {}", e);
        std::process::exit(1);
    }
}

impl Command for UnaliasCommand {
    fn run(&self, args: &[String]) -> CommandResult {
        // Filter out flags (--debug, --quiet, --verbose, --help)
        let non_flag_args: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

        if non_flag_args.is_empty() {
            // Show usage message
            eprintln!("Usage: brew unalias alias [...]");
            eprintln!();
            eprintln!("Remove aliases.");
            eprintln!();
            eprintln!("  -d, --debug                      Display any debugging information.");
            eprintln!("  -q, --quiet                      Make some output more quiet.");
            eprintln!("  -v, --verbose                    Make some output more verbose.");
            eprintln!("  -h, --help                       Show this message.");
            eprintln!();
            eprintln!("Error: Invalid usage: This command requires at least 1 alias argument.");
            std::process::exit(1);
        }

        // Remove specified aliases (stop at first error)
        for alias in non_flag_args {
            remove_alias(alias);
        }

        Ok(())
    }
}
