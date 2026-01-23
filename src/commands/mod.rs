pub mod alias;
pub mod analytics;
pub mod autoremove;
pub mod cache;
pub mod caskroom;
pub mod casks;
pub mod cat;
pub mod cellar;
pub mod cleanup;
pub mod command;
pub mod command_not_found_init;
pub mod completions;
pub mod config;
pub mod deps;
pub mod desc;
pub mod docs;
pub mod doctor;
pub mod env;
pub mod formula;
pub mod formulae;
pub mod help;
pub mod home;
pub mod info;
pub mod install;
pub mod leaves;
pub mod link;
pub mod list;
pub mod list_commands;
pub mod log;
pub mod missing;
pub mod nodenv_sync;
pub mod options;
pub mod outdated;
pub mod pin;
pub mod prefix;
pub mod pyenv_sync;
pub mod rbenv_sync;
pub mod reinstall;
pub mod repository;
pub mod search;
pub mod shellenv;
pub mod source;
pub mod tab_cmd;
pub mod tap;
pub mod tap_info;
pub mod taps;
pub mod unalias;
pub mod uninstall;
pub mod unlink;
pub mod unpin;
pub mod untap;
pub mod update;
pub mod update_reset;
pub mod upgrade;
pub mod uses;
pub mod version;
pub mod which_formula;

pub type CommandResult = Result<(), Box<dyn std::error::Error>>;

/// Trait for all brew commands.
pub trait Command {
    /// Run the command with the given arguments.
    fn run(&self, args: &[String]) -> CommandResult;
}
