use crate::commands::{Command, CommandResult};

pub struct Commands;

/// List of built-in commands that are implemented.
const BUILTIN_COMMANDS: &[&str] = &[
    "--cache",
    "--caskroom",
    "--cellar",
    "--env",
    "--prefix",
    "--repository",
    "--taps",
    "--version",
    "commands",
    "config",
    "deps",
    "help",
    "info",
    "install",
    "link",
    "list",
    "search",
    "uninstall",
    "unlink",
];

/// Command aliases (only shown with --include-aliases)
const COMMAND_ALIASES: &[&str] = &[
    "--repo", // -> --repository
    "-S",     // -> search
    "-v",     // -> --version
    "abv",    // -> info
    "ln",     // -> link
    "ls",     // -> list
    "remove", // -> uninstall
    "rm",     // -> uninstall
];

impl Command for Commands {
    fn run(&self, args: &[String]) -> CommandResult {
        let quiet = args.iter().any(|a| a == "-q" || a == "--quiet");
        let include_aliases = args.iter().any(|a| a == "--include-aliases");

        // --include-aliases requires --quiet
        if include_aliases && !quiet {
            eprintln!(
                "Error: Invalid usage: `--include-aliases` cannot be passed without `--quiet`."
            );
            std::process::exit(1);
        }

        if !quiet {
            println!("==> Built-in commands");
        }

        // Collect all commands (and optionally aliases), then sort
        let mut all_commands: Vec<&str> = BUILTIN_COMMANDS.to_vec();
        if include_aliases {
            all_commands.extend(COMMAND_ALIASES);
        }
        all_commands.sort();

        for cmd in all_commands {
            println!("{cmd}");
        }

        // TODO: Add developer commands section
        // TODO: Add external commands section

        Ok(())
    }
}
