mod api;
mod commands;
mod deps;
mod download;
mod formula;
mod install;
mod paths;
mod settings;
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
        "--env" => commands::env::EnvCommand.run(args),
        "help" | "--help" | "-h" | "-?" => commands::help::Help.run(args),
        "alias" => commands::alias::AliasCommand.run(args),
        "analytics" => commands::analytics::execute(args),
        "audit" => commands::audit::execute(args),
        "command" => {
            commands::command::execute(args);
            Ok(())
        }
        "command-not-found-init" => commands::command_not_found_init::execute(args),
        "completions" => commands::completions::execute(args),
        "commands" => commands::list_commands::Commands.run(args),
        "config" => commands::config::Config.run(args),
        "list" => commands::list::ListCommand.run(args),
        "info" => commands::info::InfoCommand.run(args),
        "search" => commands::search::run(args),
        "shellenv" => commands::shellenv::run(args),
        "source" => commands::source::execute(args),
        "install" => commands::install::run(args).map_err(|e| e.into()),
        "deps" => commands::deps::run(args).map_err(|e| e.into()),
        "uninstall" => commands::uninstall::UninstallCommand.run(args),
        "link" => commands::link::LinkCommand::run(args).map_err(|_| "link command failed".into()),
        "unlink" => commands::unlink::UnlinkCommand.run(args),
        "outdated" => commands::outdated::run(args).map_err(|e| e.into()),
        "update" => commands::update::run(args),
        "update-if-needed" => commands::update_if_needed::run(args),
        "update-report" => commands::update_report::run(args),
        "update-reset" => commands::update_reset::run(args),
        "upgrade" => commands::upgrade::run(args).map_err(|e| e.into()),
        "pin" => commands::pin::run(args),
        "unpin" => commands::unpin::run(args),
        "postinstall" | "post_install" => commands::postinstall::run(args),
        "cleanup" => commands::cleanup::run(args),
        "doctor" => commands::doctor::DoctorCommand.run(args),
        "docs" => commands::docs::execute(args),
        "edit" => {
            commands::edit::execute(args);
            Ok(())
        }
        "fetch" => commands::fetch::execute(args),
        "readall" => commands::readall::execute(args),
        "reinstall" => commands::reinstall::run(args),
        "tab" => commands::tab_cmd::execute(args),
        "tap" => {
            let code = commands::tap::run(args);
            if code == std::process::ExitCode::SUCCESS {
                Ok(())
            } else {
                Err("tap command failed".into())
            }
        }
        "tap-info" => commands::tap_info::execute(args),
        "untap" => commands::untap::run(args).map_err(|e| e.into()),
        "unalias" => commands::unalias::UnaliasCommand.run(args),
        "uses" => commands::uses::run(args),
        "leaves" => commands::leaves::run(args),
        "autoremove" => commands::autoremove::run(args),
        "bottle" => commands::bottle::execute(args),
        "bump-cask-pr" => commands::bump_cask_pr::execute(args),
        "desc" => commands::desc::run(args),
        "casks" => commands::casks::execute(args),
        "formula" => {
            commands::formula::execute(args);
            Ok(())
        }
        "formulae" => commands::formulae::execute(args),
        "cat" => commands::cat::run(args).map_err(|_| "cat command failed".into()),
        "home" | "homepage" => commands::home::run(args).map_err(|_| "home command failed".into()),
        "log" => commands::log::LogCommand::new().run(args),
        "missing" => commands::missing::execute(args),
        "nodenv-sync" => commands::nodenv_sync::execute(args),
        "options" => commands::options::run(args).map_err(|e| e.into()),
        "pyenv-sync" => commands::pyenv_sync::execute(args),
        "rbenv-sync" => commands::rbenv_sync::execute(args),
        "test" => commands::test::execute(args),
        "which-formula" => {
            commands::which_formula::execute(args);
            Ok(())
        }
        _ => {
            eprintln!("Error: Unknown command: brew {cmd}");
            Err("Unknown command".into())
        }
    }
}
