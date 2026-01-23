use crate::paths;
use std::path::PathBuf;
use std::process;

const INTERNAL_COMMAND_ALIASES: &[(&str, &str)] = &[
    ("ls", "list"),
    ("homepage", "home"),
    ("-S", "search"),
    ("up", "update"),
    ("ln", "link"),
    ("instal", "install"),
    ("uninstal", "uninstall"),
    ("post_install", "postinstall"),
    ("rm", "uninstall"),
    ("remove", "uninstall"),
    ("abv", "info"),
    ("dr", "doctor"),
    ("--repo", "--repository"),
    ("environment", "--env"),
    ("--config", "config"),
    ("-v", "--version"),
    ("lc", "livecheck"),
    ("tc", "typecheck"),
];

pub fn execute(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: brew command command [...]");
        eprintln!();
        eprintln!("Display the path to the file being used when invoking brew cmd.");
        eprintln!();
        eprintln!("  -d, --debug                      Display any debugging information.");
        eprintln!("  -q, --quiet                      Make some output more quiet.");
        eprintln!("  -v, --verbose                    Make some output more verbose.");
        eprintln!("  -h, --help                       Show this message.");
        eprintln!();
        eprintln!("Error: Invalid usage: This command requires at least 1 command argument.");
        process::exit(1);
    }

    for cmd in args {
        match find_command_path(cmd) {
            Some(path) => println!("{}", path.display()),
            None => {
                eprintln!("Error: Unknown command: brew {}", cmd);
                process::exit(1);
            }
        }
    }
}

fn find_command_path(cmd: &str) -> Option<PathBuf> {
    // First check if it's an alias
    let is_alias = INTERNAL_COMMAND_ALIASES
        .iter()
        .any(|(alias, _)| *alias == cmd);

    let internal_cmd = INTERNAL_COMMAND_ALIASES
        .iter()
        .find(|(alias, _)| *alias == cmd)
        .map(|(_, target)| *target)
        .unwrap_or(cmd);

    // Check internal commands (cmd/*.rb, cmd/*.sh)
    // For aliases, prefer .rb (documentation), for direct commands prefer .sh (implementation)
    if let Some(path) = find_internal_cmd_path(internal_cmd, is_alias) {
        return Some(path);
    }

    // Check internal dev commands (dev-cmd/*.rb, dev-cmd/*.sh)
    if let Some(path) = find_internal_dev_cmd_path(internal_cmd, is_alias) {
        return Some(path);
    }

    // Check external commands in tap cmd directories
    if let Some(path) = find_external_cmd_path(cmd) {
        return Some(path);
    }

    None
}

fn find_internal_cmd_path(cmd: &str, is_alias: bool) -> Option<PathBuf> {
    let homebrew_library = paths::homebrew_repository().join("Library/Homebrew");
    let cmd_path = homebrew_library.join("cmd");

    // For aliases, prefer .rb (documentation)
    // For direct commands, prefer .sh (implementation)
    let extensions = if is_alias {
        &["rb", "sh"]
    } else {
        &["sh", "rb"]
    };

    for ext in extensions {
        let path = cmd_path.join(format!("{}.{}", cmd, ext));
        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn find_internal_dev_cmd_path(cmd: &str, is_alias: bool) -> Option<PathBuf> {
    let homebrew_library = paths::homebrew_repository().join("Library/Homebrew");
    let dev_cmd_path = homebrew_library.join("dev-cmd");

    // For aliases, prefer .rb (documentation)
    // For direct commands, prefer .sh (implementation)
    let extensions = if is_alias {
        &["rb", "sh"]
    } else {
        &["sh", "rb"]
    };

    for ext in extensions {
        let path = dev_cmd_path.join(format!("{}.{}", cmd, ext));
        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn find_external_cmd_path(cmd: &str) -> Option<PathBuf> {
    // Check tap cmd directories for external commands
    let tap_directory = paths::homebrew_repository().join("Library/Taps");

    if !tap_directory.exists() {
        return None;
    }

    // Look for tap cmd directories (e.g., user/tap/cmd/)
    if let Ok(entries) = std::fs::read_dir(&tap_directory) {
        for entry in entries.flatten() {
            let user_path = entry.path();
            if !user_path.is_dir() {
                continue;
            }

            if let Ok(tap_entries) = std::fs::read_dir(&user_path) {
                for tap_entry in tap_entries.flatten() {
                    let tap_path = tap_entry.path();
                    if !tap_path.is_dir() {
                        continue;
                    }

                    let cmd_dir = tap_path.join("cmd");
                    if !cmd_dir.exists() {
                        continue;
                    }

                    // Look for the command file in this tap's cmd directory
                    // Try: cmd.rb, brew-cmd.rb, cmd (executable)
                    for pattern in &[
                        format!("{}.rb", cmd),
                        format!("brew-{}.rb", cmd),
                        cmd.to_string(),
                    ] {
                        let cmd_file = cmd_dir.join(pattern);
                        if cmd_file.exists() {
                            return Some(cmd_file);
                        }
                    }
                }
            }
        }
    }

    // Also check PATH for brew-* commands
    if let Ok(path_var) = std::env::var("PATH") {
        for path_dir in path_var.split(':') {
            let brew_cmd = PathBuf::from(path_dir).join(format!("brew-{}", cmd));
            if brew_cmd.exists() {
                return Some(brew_cmd);
            }
        }
    }

    None
}
