mod api;
mod commands;
mod deps;
mod download;
mod formula;
mod install;
mod paths;
mod system;
mod tap;

use std::env;
use std::process::ExitCode;

use commands::{Command, CommandResult};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> CommandResult {
    // Handle empty args - show help
    if args.is_empty() {
        return commands::help::Help.run(&[]);
    }

    let cmd = &args[0];
    let cmd_args = &args[1..];

    // Resolve command aliases
    let resolved_cmd = resolve_alias(cmd);

    // Dispatch to command
    dispatch(&resolved_cmd, cmd_args)
}

/// Resolve command aliases to their canonical names.
/// Matches Homebrew's HOMEBREW_INTERNAL_COMMAND_ALIASES.
fn resolve_alias(cmd: &str) -> String {
    match cmd {
        "ls" => "list".to_string(),
        "homepage" => "home".to_string(),
        "-S" => "search".to_string(),
        "up" => "update".to_string(),
        "ln" => "link".to_string(),
        "instal" => "install".to_string(),
        "uninstal" => "uninstall".to_string(),
        "post_install" => "postinstall".to_string(),
        "rm" | "remove" => "uninstall".to_string(),
        "abv" => "info".to_string(),
        "dr" => "doctor".to_string(),
        "--repo" => "--repository".to_string(),
        "environment" => "--env".to_string(),
        "--config" => "config".to_string(),
        "-v" => "--version".to_string(),
        "lc" => "livecheck".to_string(),
        "tc" => "typecheck".to_string(),
        other => other.to_string(),
    }
}

fn dispatch(cmd: &str, args: &[String]) -> CommandResult {
    match cmd {
        "--version" => commands::version::Version.run(args),
        "--prefix" => commands::prefix::Prefix.run(args),
        "--cellar" => commands::cellar::Cellar.run(args),
        "--cache" => commands::cache::Cache.run(args),
        "--repository" => commands::repository::Repository.run(args),
        "--caskroom" => commands::caskroom::Caskroom.run(args),
        "--taps" => commands::taps::Taps.run(args),
        "help" | "--help" | "-h" | "-?" => commands::help::Help.run(args),
        "commands" => commands::list_commands::Commands.run(args),
        "config" => commands::config::Config.run(args),
        "list" => commands::list::ListCommand.run(args),
        "info" => commands::info::InfoCommand.run(args),
        "search" => commands::search::run(args),
        "install" => commands::install::run(args).map_err(|e| e.into()),
        _ => {
            eprintln!("Error: Unknown command: brew {cmd}");
            Err("Unknown command".into())
        }
    }
}
