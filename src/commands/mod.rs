pub mod cache;
pub mod caskroom;
pub mod cellar;
pub mod help;
pub mod list_commands;
pub mod prefix;
pub mod repository;
pub mod taps;
pub mod version;

pub type CommandResult = Result<(), Box<dyn std::error::Error>>;

/// Trait for all brew commands.
pub trait Command {
    /// Run the command with the given arguments.
    fn run(&self, args: &[String]) -> CommandResult;
}
