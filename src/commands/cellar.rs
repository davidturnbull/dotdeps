use crate::commands::{Command, CommandResult};
use crate::paths;

pub struct Cellar;

impl Command for Cellar {
    fn run(&self, args: &[String]) -> CommandResult {
        // No formula arguments - just output the cellar
        if args.is_empty() {
            println!("{}", paths::homebrew_cellar().display());
            return Ok(());
        }

        // TODO: Handle formula arguments
        Err("Formula argument not yet implemented".into())
    }
}
