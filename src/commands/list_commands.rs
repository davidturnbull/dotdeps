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
    "alias",
    "analytics",
    "audit",
    "autoremove",
    "bottle",
    "bump",
    "bump-cask-pr",
    "bump-formula-pr",
    "bump-revision",
    "bump-unversioned-casks",
    "bundle",
    "casks",
    "cat",
    "cleanup",
    "command",
    "command-not-found-init",
    "commands",
    "completions",
    "config",
    "contributions",
    "create",
    "debugger",
    "deps",
    "desc",
    "determine-test-runners",
    "developer",
    "dispatch-build-bottle",
    "docs",
    "doctor",
    "edit",
    "extract",
    "fetch",
    "formula",
    "formula-analytics",
    "formulae",
    "generate-analytics-api",
    "generate-cask-api",
    "generate-cask-ci-matrix",
    "generate-formula-api",
    "generate-man-completions",
    "gist-logs",
    "help",
    "home",
    "info",
    "install",
    "install-bundler-gems",
    "irb",
    "leaves",
    "lgtm",
    "link",
    "linkage",
    "list",
    "livecheck",
    "log",
    "mcp-server",
    "migrate",
    "missing",
    "nodenv-sync",
    "options",
    "outdated",
    "pin",
    "postinstall",
    "pr-automerge",
    "pr-publish",
    "pr-pull",
    "pr-upload",
    "prof",
    "pyenv-sync",
    "rbenv-sync",
    "readall",
    "reinstall",
    "release",
    "rubocop",
    "ruby",
    "rubydoc",
    "search",
    "services",
    "setup-ruby",
    "sh",
    "shellenv",
    "source",
    "style",
    "tab",
    "tap",
    "tap-info",
    "tap-new",
    "test",
    "test-bot",
    "tests",
    "typecheck",
    "unalias",
    "unbottled",
    "uninstall",
    "unlink",
    "unpack",
    "unpin",
    "untap",
    "update",
    "update-if-needed",
    "update-license-data",
    "update-maintainers",
    "update-perl-resources",
    "update-python-resources",
    "update-report",
    "update-reset",
    "update-sponsors",
    "update-test",
    "upgrade",
    "uses",
    "vendor-gems",
    "vendor-install",
    "verify",
    "version-install",
    "which-formula",
    "which-update",
];

/// Command aliases (only shown with --include-aliases)
const COMMAND_ALIASES: &[&str] = &[
    "--repo",   // -> --repository
    "-S",       // -> search
    "-v",       // -> --version
    "abv",      // -> info
    "dr",       // -> doctor
    "homepage", // -> home
    "ln",       // -> link
    "ls",       // -> list
    "remove",   // -> uninstall
    "rm",       // -> uninstall
    "up",       // -> update
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

        Ok(())
    }
}
