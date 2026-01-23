pub mod cache;
pub mod caskroom;
pub mod cellar;
pub mod config;
pub mod help;
pub mod info;
pub mod list;
pub mod list_commands;
pub mod prefix;
pub mod repository;
pub mod search;
pub mod taps;
pub mod version;

pub type CommandResult = Result<(), Box<dyn std::error::Error>>;

/// Trait for all brew commands.
pub trait Command {
    /// Run the command with the given arguments.
    fn run(&self, args: &[String]) -> CommandResult;
}
