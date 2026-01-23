pub mod autoremove;
pub mod cache;
pub mod caskroom;
pub mod cat;
pub mod cellar;
pub mod cleanup;
pub mod config;
pub mod deps;
pub mod desc;
pub mod doctor;
pub mod env;
pub mod help;
pub mod home;
pub mod info;
pub mod install;
pub mod leaves;
pub mod link;
pub mod list;
pub mod list_commands;
pub mod log;
pub mod options;
pub mod outdated;
pub mod pin;
pub mod prefix;
pub mod reinstall;
pub mod repository;
pub mod search;
pub mod tap;
pub mod taps;
pub mod uninstall;
pub mod unlink;
pub mod unpin;
pub mod untap;
pub mod update;
pub mod upgrade;
pub mod uses;
pub mod version;

pub type CommandResult = Result<(), Box<dyn std::error::Error>>;

/// Trait for all brew commands.
pub trait Command {
    /// Run the command with the given arguments.
    fn run(&self, args: &[String]) -> CommandResult;
}
