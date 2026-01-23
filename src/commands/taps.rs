use crate::commands::{Command, CommandResult};
use crate::paths;

pub struct Taps;

impl Command for Taps {
    fn run(&self, _args: &[String]) -> CommandResult {
        println!("{}", paths::homebrew_taps().display());
        Ok(())
    }
}
