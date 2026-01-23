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
    "help",
];

impl Command for Commands {
    fn run(&self, args: &[String]) -> CommandResult {
        let quiet = args.iter().any(|a| a == "-q" || a == "--quiet");

        if !quiet {
            println!("==> Built-in commands");
        }

        for cmd in BUILTIN_COMMANDS {
            println!("{cmd}");
        }

        // TODO: Add developer commands section
        // TODO: Add external commands section

        Ok(())
    }
}
