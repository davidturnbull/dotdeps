use crate::commands::{Command, CommandResult};
use crate::paths;

pub struct Cache;

impl Command for Cache {
    fn run(&self, args: &[String]) -> CommandResult {
        // No formula/cask arguments - just output the cache
        if args.is_empty() || args.iter().all(|a| a.starts_with('-')) {
            println!("{}", paths::homebrew_cache().display());
            return Ok(());
        }

        // TODO: Handle formula/cask arguments and --os/--arch flags
        Err("Formula/cask argument not yet implemented".into())
    }
}
