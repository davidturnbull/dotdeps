use crate::commands::{Command, CommandResult};
use crate::paths;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

pub struct AliasCommand;

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

/// Get list of reserved command names that cannot be aliased
fn reserved_commands() -> Vec<&'static str> {
    vec![
        "--cache",
        "--caskroom",
        "--cellar",
        "--config",
        "--env",
        "--prefix",
        "--repository",
        "--taps",
        "--version",
        "-S",
        "-h",
        "-v",
        "-?",
        "abv",
        "alias",
        "autoremove",
        "cat",
        "cleanup",
        "commands",
        "config",
        "deps",
        "desc",
        "doctor",
        "dr",
        "help",
        "home",
        "homepage",
        "info",
        "install",
        "instal",
        "leaves",
        "link",
        "list",
        "ln",
        "log",
        "ls",
        "options",
        "outdated",
        "pin",
        "post_install",
        "postinstall",
        "reinstall",
        "remove",
        "rm",
        "search",
        "tap",
        "unalias",
        "uninstall",
        "uninstal",
        "unlink",
        "unpin",
        "untap",
        "up",
        "update",
        "upgrade",
        "uses",
        // Developer commands
        "audit",
        "bottle",
        "bump",
        "bump-cask-pr",
        "bump-formula-pr",
        "bump-revision",
        "bump-unversioned-casks",
        "contributions",
        "create",
        "debugger",
        "determine-test-runners",
        "developer",
        "dispatch-build-bottle",
        "docs",
        "edit",
        "extract",
        "formula",
        "formula-analytics",
        "generate-analytics-api",
        "generate-cask-api",
        "generate-cask-ci-matrix",
        "generate-formula-api",
        "generate-man-completions",
        "install-bundler-gems",
        "irb",
        "lc",
        "lgtm",
        "linkage",
        "livecheck",
        "pr-automerge",
        "pr-publish",
        "pr-pull",
        "pr-upload",
        "prof",
        "release",
        "rubocop",
        "ruby",
        "rubydoc",
        "sh",
        "style",
        "tap-new",
        "tc",
        "test",
        "test-bot",
        "tests",
        "typecheck",
        "unbottled",
        "unpack",
        "update-license-data",
        "update-maintainers",
        "update-perl-resources",
        "update-python-resources",
        "update-sponsors",
        "update-test",
        "vendor-gems",
        "verify",
        "which-update",
        // Core commands
        "analytics",
        "bundle",
        "casks",
        "command",
        "command-not-found-init",
        "completions",
        "fetch",
        "formulae",
        "gist-logs",
        "migrate",
        "missing",
        "nodenv-sync",
        "pyenv-sync",
        "rbenv-sync",
        "readall",
        "services",
        "setup-ruby",
        "shellenv",
        "source",
        "tab",
        "tap-info",
        "update-if-needed",
        "update-report",
        "update-reset",
        "vendor-install",
        "version-install",
        "which-formula",
        // Special
        "mcp-server",
    ]
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
    paths::homebrew_prefix()
        .join("bin")
        .join(format!("brew-{}", name))
}

/// Parse an alias file and extract the command
fn parse_alias_file(path: &Path) -> Option<(String, String)> {
    let content = fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    // Line 2 should be "# alias: brew <name>"
    let name = if lines.len() > 1 {
        lines[1]
            .strip_prefix("# alias: brew ")
            .map(|s| s.trim().to_string())?
    } else {
        return None;
    };

    // Find the command line (skip comments and empty lines)
    let command_line = lines
        .iter()
        .skip(2)
        .find(|line| !line.starts_with("#") && !line.trim().is_empty())?
        .trim();

    let command = command_line.strip_suffix(" $*").unwrap_or(command_line);

    // Convert command format for display
    let display_cmd = if let Some(cmd) = command.strip_prefix("brew ") {
        cmd.to_string()
    } else {
        format!("!{}", command)
    };

    Some((name, display_cmd))
}

/// List all aliases
fn list_aliases() -> CommandResult {
    let dir = aliases_dir();

    if !dir.exists() {
        return Ok(());
    }

    let mut aliases = Vec::new();

    for entry in
        fs::read_dir(&dir).map_err(|e| format!("Failed to read aliases directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        // Skip directories and backup files
        if path.is_dir() || path.to_string_lossy().ends_with('~') {
            continue;
        }

        if let Some((name, command)) = parse_alias_file(&path) {
            // Ensure symlink exists
            let symlink = symlink_path(&name);
            if !symlink.exists() {
                let _ = unix_fs::symlink(&path, &symlink);
            }

            aliases.push((name, command));
        }
    }

    // Sort by name
    aliases.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, command) in aliases {
        println!("brew alias {}='{}'", name, command);
    }

    Ok(())
}

/// Show a specific alias
fn show_alias(name: &str) -> CommandResult {
    let path = script_path(name);

    if !path.exists() {
        // Homebrew silently exits with success when alias doesn't exist
        return Ok(());
    }

    if let Some((_, command)) = parse_alias_file(&path) {
        println!("brew alias {}='{}'", name, command);

        // Ensure symlink exists
        let symlink = symlink_path(name);
        if !symlink.exists() {
            let _ = unix_fs::symlink(&path, &symlink);
        }

        Ok(())
    } else {
        eprintln!("Error: Failed to parse alias: {}", name);
        Err("Failed to parse alias".into())
    }
}

/// Create a new alias
fn create_alias(name: &str, command: &str) -> CommandResult {
    // Check if name is reserved
    if reserved_commands().contains(&name) {
        eprintln!("Error: '{}' is a reserved command. Sorry.", name);
        return Err("Reserved command".into());
    }

    // Check if command already exists (not as an alias)
    let cmd_path = paths::homebrew_prefix()
        .join("bin")
        .join(format!("brew-{}", name));
    if cmd_path.exists() {
        let real_path = fs::read_link(&cmd_path).unwrap_or_else(|_| cmd_path.clone());
        if !real_path.starts_with(aliases_dir()) {
            eprintln!("Error: 'brew {}' already exists. Sorry.", name);
            return Err("Command already exists".into());
        }
    }

    let path = script_path(name);

    // Check if alias already exists
    if path.exists() {
        eprintln!("Error: alias 'brew {}' already exists!", name);
        return Err("Alias already exists".into());
    }

    // Create aliases directory if it doesn't exist
    let dir = aliases_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create aliases directory: {}", e))?;

    // Determine the actual command to write
    let actual_command = if command.starts_with('!') || command.starts_with('%') {
        command[1..].to_string()
    } else {
        format!("brew {}", command)
    };

    // Create the alias script
    let bash_path = std::process::Command::new("which")
        .arg("bash")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "/bin/bash\n".to_string())
        .trim()
        .to_string();

    let content = format!(
        "#! {}\n# alias: brew {}\n#:  * `{}` [args...]\n#:    `brew {}` is an alias for `{}`\n{} $*\n",
        bash_path, name, name, name, actual_command, actual_command
    );

    fs::write(&path, content).map_err(|e| format!("Failed to write alias file: {}", e))?;

    // Make script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .map_err(|e| format!("Failed to get file metadata: {}", e))?
            .permissions();
        perms.set_mode(0o744);
        fs::set_permissions(&path, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    // Create symlink in bin directory
    let symlink = symlink_path(name);
    if symlink.exists() {
        fs::remove_file(&symlink)
            .map_err(|e| format!("Failed to remove existing symlink: {}", e))?;
    }

    unix_fs::symlink(&path, &symlink).map_err(|e| format!("Failed to create symlink: {}", e))?;

    Ok(())
}

impl Command for AliasCommand {
    fn run(&self, args: &[String]) -> CommandResult {
        // Check for --edit flag
        let has_edit = args.iter().any(|a| a == "--edit");

        if has_edit {
            eprintln!("Error: --edit flag is not yet implemented");
            return Err("Not implemented".into());
        }

        // Filter out flags
        let non_flag_args: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

        if non_flag_args.is_empty() {
            // No arguments: list all aliases
            list_aliases()
        } else {
            let arg = non_flag_args[0];

            // Check if this is an assignment (alias=command)
            if let Some(eq_pos) = arg.find('=') {
                let name = &arg[..eq_pos];
                let command = &arg[eq_pos + 1..];
                create_alias(name, command)
            } else {
                // Show specific alias
                show_alias(arg)
            }
        }
    }
}
