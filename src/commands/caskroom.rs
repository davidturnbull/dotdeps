use crate::commands::{Command, CommandResult};
use crate::paths;

pub struct Caskroom;

impl Command for Caskroom {
    fn run(&self, args: &[String]) -> CommandResult {
        // No cask arguments - just output the caskroom
        if args.is_empty() {
            println!("{}", paths::homebrew_caskroom().display());
            return Ok(());
        }

        // TODO: Handle cask arguments
        Err("Cask argument not yet implemented".into())
    }
}
